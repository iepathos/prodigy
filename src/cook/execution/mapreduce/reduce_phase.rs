//! Reduce phase execution functionality
//!
//! This module handles the execution of the reduce phase in MapReduce workflows,
//! including result aggregation and final processing.

use crate::cook::execution::errors::MapReduceResult;
use crate::cook::execution::mapreduce::agent::AgentResult;
use crate::cook::execution::mapreduce::{AgentContext, ReducePhase};
use crate::cook::workflow::StepResult;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};

/// Configuration for reduce phase execution
pub struct ReducePhaseConfig {
    pub job_id: String,
    pub parent_worktree_path: PathBuf,
    pub parent_worktree_name: String,
}

/// Result from reduce phase execution
pub struct ReducePhaseResult {
    pub success: bool,
    pub output: String,
    pub variables: HashMap<String, String>,
}

/// Execute reduce phase with aggregated map results
pub async fn execute<F>(
    reduce_phase: &ReducePhase,
    map_results: &[AgentResult],
    config: ReducePhaseConfig,
    step_executor: F,
) -> MapReduceResult<ReducePhaseResult>
where
    F: Fn(
            &crate::cook::workflow::WorkflowStep,
            &mut AgentContext,
        ) -> futures::future::BoxFuture<'static, MapReduceResult<StepResult>>
        + Send
        + Sync,
{
    info!(
        "Executing reduce phase with {} map results",
        map_results.len()
    );

    // Create reduce context with aggregated variables
    let mut reduce_context = create_reduce_context(map_results, &config);

    // Execute each reduce step
    let mut all_success = true;
    let mut final_output = String::new();

    for step in &reduce_phase.commands {
        let result = step_executor(step, &mut reduce_context).await?;

        if !result.success {
            warn!("Reduce step failed: {}", result.stderr);
            all_success = false;

            // Check if we should continue on failure
            if !should_continue_on_failure(reduce_phase) {
                return Ok(ReducePhaseResult {
                    success: false,
                    output: result.stderr,
                    variables: reduce_context.variables.clone(),
                });
            }
        }

        // Capture output from successful steps
        if result.success && !result.stdout.is_empty() {
            final_output.push_str(&result.stdout);
            final_output.push('\n');
        }
    }

    Ok(ReducePhaseResult {
        success: all_success,
        output: final_output,
        variables: reduce_context.variables,
    })
}

/// Create reduce phase context with aggregated map results
pub fn create_reduce_context(
    map_results: &[AgentResult],
    config: &ReducePhaseConfig,
) -> AgentContext {
    let mut context = AgentContext::new(
        "reduce".to_string(),
        config.parent_worktree_path.clone(),
        config.parent_worktree_name.clone(),
        crate::cook::orchestrator::ExecutionEnvironment {
            working_dir: config.parent_worktree_path.clone().into(),
            project_dir: config.parent_worktree_path.clone().into(),
            worktree_name: Some(config.parent_worktree_name.clone().into()),
            session_id: format!("reduce-session-{}", config.job_id).into(),
        },
    );

    // Add aggregate statistics
    add_aggregate_statistics(&mut context, map_results);

    // Add individual results
    add_individual_results(&mut context, map_results);

    // Add serialized results for complex processing
    add_serialized_results(&mut context, map_results);

    context
}

/// Add aggregate statistics to context
fn add_aggregate_statistics(context: &mut AgentContext, results: &[AgentResult]) {
    let total = results.len();
    let successful = results.iter().filter(|r| r.is_success()).count();
    let failed = total - successful;

    context
        .variables
        .insert("map.total".to_string(), total.to_string());
    context
        .variables
        .insert("map.successful".to_string(), successful.to_string());
    context
        .variables
        .insert("map.failed".to_string(), failed.to_string());

    // Calculate success rate
    let success_rate = if total > 0 {
        (successful as f64 / total as f64 * 100.0) as i32
    } else {
        0
    };
    context
        .variables
        .insert("map.success_rate".to_string(), success_rate.to_string());
}

/// Add individual result variables to context
fn add_individual_results(context: &mut AgentContext, results: &[AgentResult]) {
    for (index, result) in results.iter().enumerate() {
        let prefix = format!("map.results.{}", index);

        // Add result status
        context.variables.insert(
            format!("{}.success", prefix),
            result.is_success().to_string(),
        );

        // Add result output if available
        if let Some(output) = &result.output {
            let truncated = truncate_output(output, 1000);
            context
                .variables
                .insert(format!("{}.output", prefix), truncated);
        }

        // Add agent ID (use item_id as we don't have agent_id field)
        context
            .variables
            .insert(format!("{}.agent_id", prefix), result.item_id.clone());

        // Add item ID
        context
            .variables
            .insert(format!("{}.item_id", prefix), result.item_id.clone());
    }
}

/// Add serialized results for complex processing
fn add_serialized_results(context: &mut AgentContext, results: &[AgentResult]) {
    // Create JSON representation of results
    let results_json: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "agent_id": r.item_id.clone(),
                "item_id": r.item_id,
                "success": r.is_success(),
                "output": r.output,
                "error": r.error,
            })
        })
        .collect();

    // Store as JSON string
    if let Ok(json_str) = serde_json::to_string(&results_json) {
        context
            .variables
            .insert("map.results".to_string(), json_str);
    }

    // Store successful outputs as array
    let successful_outputs: Vec<String> = results
        .iter()
        .filter(|r| r.is_success())
        .filter_map(|r| r.output.clone())
        .collect();

    if let Ok(outputs_json) = serde_json::to_string(&successful_outputs) {
        context
            .variables
            .insert("map.outputs".to_string(), outputs_json);
    }
}

/// Truncate output to specified length
fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        output.to_string()
    } else {
        format!("{}...[truncated]", &output[..max_len])
    }
}

/// Check if reduce should continue on failure
fn should_continue_on_failure(_reduce_phase: &ReducePhase) -> bool {
    // Could be configured via reduce phase settings
    // For now, always stop on first failure
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_statistics() {
        let results = vec![
            create_test_result("agent1", true),
            create_test_result("agent2", false),
            create_test_result("agent3", true),
        ];

        let config = ReducePhaseConfig {
            job_id: "test-job".to_string(),
            parent_worktree_path: PathBuf::from("/test"),
            parent_worktree_name: "test-worktree".to_string(),
        };

        let context = create_reduce_context(&results, &config);

        assert_eq!(context.variables.get("map.total"), Some(&"3".to_string()));
        assert_eq!(
            context.variables.get("map.successful"),
            Some(&"2".to_string())
        );
        assert_eq!(context.variables.get("map.failed"), Some(&"1".to_string()));
        assert_eq!(
            context.variables.get("map.success_rate"),
            Some(&"66".to_string())
        );
    }

    fn create_test_result(agent_id: &str, success: bool) -> AgentResult {
        use crate::cook::execution::mapreduce::agent::AgentStatus;

        AgentResult {
            item_id: format!("item-{}", agent_id),
            status: if success {
                AgentStatus::Success
            } else {
                AgentStatus::Failed("Test failure".to_string())
            },
            output: Some(format!("Output from {}", agent_id)),
            commits: vec![],
            files_modified: vec![],
            duration: std::time::Duration::from_secs(1),
            error: if !success {
                Some("Test error".to_string())
            } else {
                None
            },
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
            cleanup_status: None,
        }
    }
}
