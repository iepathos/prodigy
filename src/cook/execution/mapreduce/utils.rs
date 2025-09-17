//! Pure utility functions for MapReduce operations
//!
//! This module contains pure functions extracted from the main MapReduce executor
//! to improve testability, maintainability, and functional programming practices.

use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::mapreduce::{AgentResult, AgentStatus};
use serde_json::json;
use std::collections::HashMap;

/// Summary statistics for map results
#[derive(Debug, Clone, PartialEq)]
pub struct MapResultSummary {
    pub successful: usize,
    pub failed: usize,
    pub total: usize,
}

/// Agent status enumeration for classification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentEventType {
    Completed,
    Failed,
    TimedOut,
    Retrying,
    InProgress,
}

// ============================================================================
// Result Aggregation Functions
// ============================================================================

/// Calculate summary statistics from map results (pure function)
///
/// # Arguments
/// * `map_results` - Collection of agent results to summarize
///
/// # Returns
/// Summary containing counts of successful, failed, and total agents
pub fn calculate_map_result_summary(map_results: &[AgentResult]) -> MapResultSummary {
    let successful = map_results
        .iter()
        .filter(|r| matches!(r.status, AgentStatus::Success))
        .count();

    let failed = map_results
        .iter()
        .filter(|r| matches!(r.status, AgentStatus::Failed(_) | AgentStatus::Timeout))
        .count();

    MapResultSummary {
        successful,
        failed,
        total: map_results.len(),
    }
}

/// Build InterpolationContext with map results (pure function)
///
/// # Arguments
/// * `map_results` - Collection of agent results
/// * `summary` - Pre-calculated summary statistics
///
/// # Returns
/// InterpolationContext populated with map results and statistics
pub fn build_map_results_interpolation_context(
    map_results: &[AgentResult],
    summary: &MapResultSummary,
) -> Result<InterpolationContext, serde_json::Error> {
    let mut context = InterpolationContext::new();

    // Add summary statistics
    context.set(
        "map",
        json!({
            "successful": summary.successful,
            "failed": summary.failed,
            "total": summary.total
        }),
    );

    // Add complete results as JSON value
    let results_value = serde_json::to_value(map_results)?;
    context.set("map.results", results_value);

    // Add individual result access
    for (index, result) in map_results.iter().enumerate() {
        let result_value = serde_json::to_value(result)?;
        context.set(format!("results[{}]", index), result_value);
    }

    Ok(context)
}

// ============================================================================
// Variable Transformation Functions
// ============================================================================

/// Build AgentContext variables for shell commands (pure function)
///
/// # Arguments
/// * `map_results` - Collection of agent results
/// * `summary` - Pre-calculated summary statistics
///
/// # Returns
/// HashMap of variable names to string values for shell command substitution
pub fn build_agent_context_variables(
    map_results: &[AgentResult],
    summary: &MapResultSummary,
) -> Result<HashMap<String, String>, serde_json::Error> {
    let mut variables = HashMap::new();

    // Add summary statistics as strings for shell command substitution
    variables.insert("map.successful".to_string(), summary.successful.to_string());
    variables.insert("map.failed".to_string(), summary.failed.to_string());
    variables.insert("map.total".to_string(), summary.total.to_string());

    // Add complete results as JSON string for complex access patterns
    let results_json = serde_json::to_string(map_results)?;
    variables.insert("map.results_json".to_string(), results_json.clone());
    variables.insert("map.results".to_string(), results_json);

    // Add individual result summaries for easier access in shell commands
    for (index, result) in map_results.iter().enumerate() {
        add_individual_result_variables(&mut variables, index, result);
    }

    Ok(variables)
}

/// Add variables for a single agent result (pure function)
///
/// # Arguments
/// * `variables` - HashMap to populate with result variables
/// * `index` - Index of the result in the collection
/// * `result` - Individual agent result to process
pub fn add_individual_result_variables(
    variables: &mut HashMap<String, String>,
    index: usize,
    result: &AgentResult,
) {
    // Add basic result info
    variables.insert(format!("result.{}.item_id", index), result.item_id.clone());

    let status_string = match &result.status {
        AgentStatus::Success => "success".to_string(),
        AgentStatus::Failed(err) => format!("failed: {}", err),
        AgentStatus::Timeout => "timeout".to_string(),
        AgentStatus::Pending => "pending".to_string(),
        AgentStatus::Running => "running".to_string(),
        AgentStatus::Retrying(attempt) => format!("retrying: {}", attempt),
    };
    variables.insert(format!("result.{}.status", index), status_string);

    // Add output if available (truncated for safety)
    if let Some(ref output) = result.output {
        let truncated_output = truncate_output(output, 500);
        variables.insert(format!("result.{}.output", index), truncated_output);
    }

    // Add commit count
    variables.insert(
        format!("result.{}.commits", index),
        result.commits.len().to_string(),
    );
}

// ============================================================================
// ID Generation Functions
// ============================================================================

/// Generate agent ID from index and item ID (pure function)
///
/// # Arguments
/// * `agent_index` - Numeric index of the agent
/// * `item_id` - Unique identifier for the work item
///
/// # Returns
/// Formatted agent ID string
pub fn generate_agent_id(agent_index: usize, item_id: &str) -> String {
    format!("agent-{}-{}", agent_index, item_id)
}

/// Generate branch name for an agent (pure function)
///
/// # Arguments
/// * `session_id` - Session identifier for the MapReduce job
/// * `item_id` - Unique identifier for the work item
///
/// # Returns
/// Formatted git branch name
pub fn generate_agent_branch_name(session_id: &str, item_id: &str) -> String {
    format!("prodigy-agent-{}-{}", session_id, item_id)
}

// ============================================================================
// Error Classification Functions
// ============================================================================

/// Classify agent status for event logging (pure function)
///
/// # Arguments
/// * `status` - Agent status to classify
///
/// # Returns
/// Corresponding event type for the status
pub fn classify_agent_status(status: &AgentStatus) -> AgentEventType {
    match status {
        AgentStatus::Success => AgentEventType::Completed,
        AgentStatus::Failed(_) => AgentEventType::Failed,
        AgentStatus::Timeout => AgentEventType::TimedOut,
        AgentStatus::Retrying(_) => AgentEventType::Retrying,
        _ => AgentEventType::InProgress,
    }
}

// ============================================================================
// Text Processing Functions
// ============================================================================

/// Truncate output to safe length (pure function)
///
/// # Arguments
/// * `output` - Text to potentially truncate
/// * `max_length` - Maximum allowed length
///
/// # Returns
/// Truncated string with indicator if truncation occurred
pub fn truncate_output(output: &str, max_length: usize) -> String {
    if output.len() > max_length {
        format!("{}...[truncated]", &output[..max_length])
    } else {
        output.to_string()
    }
}

/// Truncate command for display (pure function)
///
/// # Arguments
/// * `cmd` - Command string to truncate
/// * `max_len` - Maximum display length
///
/// # Returns
/// Truncated command suitable for display
pub fn truncate_command(cmd: &str, max_len: usize) -> String {
    if cmd.len() <= max_len {
        cmd.to_string()
    } else {
        format!("{}...", &cmd[..max_len - 3])
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Helper function to create test AgentResult
    fn create_test_agent_result(
        item_id: &str,
        status: AgentStatus,
        output: Option<String>,
        commits: Vec<String>,
    ) -> AgentResult {
        AgentResult {
            item_id: item_id.to_string(),
            status,
            output,
            commits,
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        }
    }

    #[test]
    fn test_calculate_map_result_summary_mixed_results() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Success,
                Some("success output".to_string()),
                vec!["commit1".to_string()],
            ),
            create_test_agent_result(
                "item2",
                AgentStatus::Failed("error".to_string()),
                Some("error output".to_string()),
                vec![],
            ),
            create_test_agent_result(
                "item3",
                AgentStatus::Success,
                Some("success output 2".to_string()),
                vec!["commit2".to_string(), "commit3".to_string()],
            ),
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.total, 3);
    }

    #[test]
    fn test_calculate_map_result_summary_all_successful() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Success,
                Some("success".to_string()),
                vec!["commit1".to_string()],
            ),
            create_test_agent_result(
                "item2",
                AgentStatus::Success,
                Some("success".to_string()),
                vec!["commit2".to_string()],
            ),
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.total, 2);
    }

    #[test]
    fn test_calculate_map_result_summary_all_failed() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Failed("error1".to_string()),
                None,
                vec![],
            ),
            create_test_agent_result("item2", AgentStatus::Timeout, None, vec![]),
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 0);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.total, 2);
    }

    #[test]
    fn test_calculate_map_result_summary_empty() {
        let map_results: Vec<AgentResult> = vec![];
        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.total, 0);
    }

    #[test]
    fn test_generate_agent_id() {
        assert_eq!(generate_agent_id(0, "abc-123"), "agent-0-abc-123");
        assert_eq!(generate_agent_id(42, "item"), "agent-42-item");
    }

    #[test]
    fn test_generate_agent_branch_name() {
        assert_eq!(
            generate_agent_branch_name("session-123", "abc"),
            "prodigy-agent-session-123-abc"
        );
    }

    #[test]
    fn test_classify_agent_status() {
        assert_eq!(
            classify_agent_status(&AgentStatus::Success),
            AgentEventType::Completed
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Failed("error".to_string())),
            AgentEventType::Failed
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Timeout),
            AgentEventType::TimedOut
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Retrying(1)),
            AgentEventType::Retrying
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Pending),
            AgentEventType::InProgress
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Running),
            AgentEventType::InProgress
        );
    }

    #[test]
    fn test_truncate_output() {
        assert_eq!(truncate_output("short", 10), "short");
        assert_eq!(
            truncate_output("this is a very long output", 10),
            "this is a ...[truncated]"
        );
    }

    #[test]
    fn test_truncate_command() {
        assert_eq!(truncate_command("ls", 10), "ls");
        assert_eq!(
            truncate_command("very long command with many arguments", 15),
            "very long co..."
        );
    }

    #[test]
    fn test_build_agent_context_variables() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Success,
                Some("output1".to_string()),
                vec!["commit1".to_string()],
            ),
            create_test_agent_result(
                "item2",
                AgentStatus::Failed("error".to_string()),
                None,
                vec![],
            ),
        ];

        let summary = calculate_map_result_summary(&map_results);
        let variables = build_agent_context_variables(&map_results, &summary).unwrap();

        assert_eq!(variables.get("map.successful").unwrap(), "1");
        assert_eq!(variables.get("map.failed").unwrap(), "1");
        assert_eq!(variables.get("map.total").unwrap(), "2");
        assert_eq!(variables.get("result.0.item_id").unwrap(), "item1");
        assert_eq!(variables.get("result.0.status").unwrap(), "success");
        assert_eq!(variables.get("result.0.output").unwrap(), "output1");
        assert_eq!(variables.get("result.1.item_id").unwrap(), "item2");
        assert_eq!(variables.get("result.1.status").unwrap(), "failed: error");
        assert_eq!(variables.get("result.0.commits").unwrap(), "1");
        assert_eq!(variables.get("result.1.commits").unwrap(), "0");
    }

    #[test]
    fn test_add_individual_result_variables() {
        let mut variables = HashMap::new();
        let result = create_test_agent_result(
            "test-item",
            AgentStatus::Success,
            Some("test output".to_string()),
            vec!["commit1".to_string(), "commit2".to_string()],
        );

        add_individual_result_variables(&mut variables, 0, &result);

        assert_eq!(variables.get("result.0.item_id").unwrap(), "test-item");
        assert_eq!(variables.get("result.0.status").unwrap(), "success");
        assert_eq!(variables.get("result.0.output").unwrap(), "test output");
        assert_eq!(variables.get("result.0.commits").unwrap(), "2");
    }

    #[test]
    fn test_build_map_results_interpolation_context() {
        let map_results = vec![create_test_agent_result(
            "item1",
            AgentStatus::Success,
            Some("output".to_string()),
            vec![],
        )];

        let summary = calculate_map_result_summary(&map_results);
        let context = build_map_results_interpolation_context(&map_results, &summary).unwrap();

        // Test summary in context
        let map_value = context.variables.get("map").unwrap();
        assert_eq!(map_value.get("successful").unwrap(), 1);
        assert_eq!(map_value.get("failed").unwrap(), 0);
        assert_eq!(map_value.get("total").unwrap(), 1);

        // Test results array
        let results_value = context.variables.get("map.results").unwrap();
        assert!(results_value.is_array());
        assert_eq!(results_value.as_array().unwrap().len(), 1);
    }
}
