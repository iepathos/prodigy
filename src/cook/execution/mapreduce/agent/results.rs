//! Agent result handling and aggregation
//!
//! This module provides functionality for collecting, aggregating, and
//! transforming agent execution results within the MapReduce framework.

use super::types::{AgentResult, AgentStatus};
use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::variables::{Variable, VariableContext};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::WorkflowStep;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

/// Aggregated results from map phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResults {
    /// Successfully processed items
    pub successful: Vec<AgentResult>,
    /// Failed items
    pub failed: Vec<AgentResult>,
    /// Total number of items processed
    pub total: usize,
    /// Number of successful items
    pub success_count: usize,
    /// Number of failed items
    pub failure_count: usize,
    /// Average execution time
    pub average_duration_secs: f64,
    /// Total execution time
    pub total_duration_secs: f64,
}

impl AggregatedResults {
    /// Create new aggregated results from a list of agent results
    pub fn from_results(results: Vec<AgentResult>) -> Self {
        // Partition results and calculate total duration in one pass
        let (successful, failed, total_duration) = results.into_iter().fold(
            (Vec::new(), Vec::new(), 0.0),
            |(mut succ, mut fail, dur), result| {
                let new_duration = dur + result.duration.as_secs_f64();
                if result.is_success() {
                    succ.push(result);
                } else {
                    fail.push(result);
                }
                (succ, fail, new_duration)
            },
        );

        let total = successful.len() + failed.len();
        let average_duration = if total > 0 {
            total_duration / total as f64
        } else {
            0.0
        };

        Self {
            success_count: successful.len(),
            failure_count: failed.len(),
            successful,
            failed,
            total,
            average_duration_secs: average_duration,
            total_duration_secs: total_duration,
        }
    }

    /// Convert to JSON value for interpolation
    pub fn to_json_value(&self) -> Value {
        json!({
            "successful": self.successful,
            "failed": self.failed,
            "total": self.total,
            "success_count": self.success_count,
            "failure_count": self.failure_count,
            "average_duration_secs": self.average_duration_secs,
            "total_duration_secs": self.total_duration_secs,
        })
    }

    /// Get a summary string
    pub fn summary(&self) -> String {
        format!(
            "{}/{} succeeded, {} failed (avg: {:.2}s, total: {:.2}s)",
            self.success_count,
            self.total,
            self.failure_count,
            self.average_duration_secs,
            self.total_duration_secs
        )
    }
}

/// Trait for aggregating and transforming agent results
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait AgentResultAggregator: Send + Sync {
    /// Aggregate multiple agent results into summary statistics
    fn aggregate(&self, results: Vec<AgentResult>) -> AggregatedResults;

    /// Convert aggregated results to an interpolation context
    fn to_interpolation_context(&self, results: &AggregatedResults) -> InterpolationContext;

    /// Convert aggregated results to a variable context
    async fn to_variable_context(&self, results: &AggregatedResults) -> VariableContext;

    /// Finalize a single agent result with commit tracking and cleanup
    async fn finalize_agent_result(
        &self,
        item_id: &str,
        worktree_path: &Path,
        worktree_name: &str,
        branch_name: &str,
        worktree_session_id: String,
        env: &ExecutionEnvironment,
        template_steps: &[WorkflowStep],
        execution_error: Option<String>,
        total_output: String,
        start_time: Instant,
    ) -> Result<AgentResult, Box<dyn std::error::Error>>;

    /// Create result for a failed agent
    fn create_failure_result(
        &self,
        item_id: String,
        error: String,
        duration: std::time::Duration,
    ) -> AgentResult;

    /// Create result for a successful agent
    fn create_success_result(
        &self,
        item_id: String,
        output: Option<String>,
        duration: std::time::Duration,
    ) -> AgentResult;
}

/// Default implementation of result aggregator
pub struct DefaultResultAggregator;

impl DefaultResultAggregator {
    /// Create a new result aggregator
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultResultAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentResultAggregator for DefaultResultAggregator {
    fn aggregate(&self, results: Vec<AgentResult>) -> AggregatedResults {
        AggregatedResults::from_results(results)
    }

    fn to_interpolation_context(&self, results: &AggregatedResults) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        // Add summary statistics
        context.set("map.successful", json!(results.success_count));
        context.set("map.failed", json!(results.failure_count));
        context.set("map.total", json!(results.total));

        // Add the full results as a structured JSON value
        if let Ok(results_value) = serde_json::to_value(results.to_json_value()) {
            context.set("map.results", results_value);
        }

        // Add individual successful results using iterator
        results
            .successful
            .iter()
            .enumerate()
            .for_each(|(i, result)| {
                context.set(
                    format!("map.successful.{}.item_id", i),
                    json!(result.item_id),
                );
                if let Some(output) = &result.output {
                    context.set(format!("map.successful.{}.output", i), json!(output));
                }
            });

        // Add individual failed results using iterator
        results.failed.iter().enumerate().for_each(|(i, result)| {
            context.set(format!("map.failed.{}.item_id", i), json!(result.item_id));
            if let Some(error) = &result.error {
                context.set(format!("map.failed.{}.error", i), json!(error));
            }
        });

        context
    }

    async fn to_variable_context(&self, results: &AggregatedResults) -> VariableContext {
        let mut context = VariableContext::new();

        // Add summary statistics
        context.set_global(
            "map.successful",
            Variable::Static(json!(results.success_count)),
        );
        context.set_global("map.failed", Variable::Static(json!(results.failure_count)));
        context.set_global("map.total", Variable::Static(json!(results.total)));

        // Add the full results as a structured JSON value
        if let Ok(results_value) = serde_json::to_value(results.to_json_value()) {
            context.set_global("map.results", Variable::Static(results_value));
        }

        // Add individual results for easier access
        let results_array: Vec<Value> = results
            .successful
            .iter()
            .chain(results.failed.iter())
            .map(|result| {
                if let Ok(result_json) = serde_json::to_value(result) {
                    result_json
                } else {
                    json!(format!("{:?}", result))
                }
            })
            .collect();

        context.set_global("map.results_array", Variable::Static(json!(results_array)));

        context
    }

    async fn finalize_agent_result(
        &self,
        item_id: &str,
        worktree_path: &Path,
        _worktree_name: &str,
        branch_name: &str,
        worktree_session_id: String,
        _env: &ExecutionEnvironment,
        _template_steps: &[WorkflowStep],
        execution_error: Option<String>,
        total_output: String,
        start_time: Instant,
    ) -> Result<AgentResult, Box<dyn std::error::Error>> {
        // Get commits and modified files from the worktree
        let commits = get_worktree_commits(worktree_path).await?;
        let files_modified = get_modified_files(worktree_path).await?;

        // Determine status
        let status = execution_error
            .clone()
            .map(AgentStatus::Failed)
            .unwrap_or(AgentStatus::Success);

        Ok(AgentResult {
            item_id: item_id.to_string(),
            status,
            output: Some(total_output),
            commits,
            files_modified,
            duration: start_time.elapsed(),
            error: execution_error,
            worktree_path: Some(worktree_path.to_path_buf()),
            branch_name: Some(branch_name.to_string()),
            worktree_session_id: Some(worktree_session_id),
        })
    }

    fn create_failure_result(
        &self,
        item_id: String,
        error: String,
        duration: std::time::Duration,
    ) -> AgentResult {
        AgentResult::failed(item_id, error, duration)
    }

    fn create_success_result(
        &self,
        item_id: String,
        output: Option<String>,
        duration: std::time::Duration,
    ) -> AgentResult {
        AgentResult::success(item_id, output, duration)
    }
}

// Helper functions for git operations
async fn get_worktree_commits(
    worktree_path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use crate::cook::execution::mapreduce::resources::git_operations::{
        GitOperationsConfig, GitOperationsService, GitResultExt,
    };

    let mut service = GitOperationsService::new(GitOperationsConfig::default());
    match service
        .get_worktree_commits(worktree_path, None, None)
        .await
    {
        Ok(commits) => Ok(commits.to_string_list()),
        Err(e) => {
            // Log the error but return empty list to avoid breaking workflow
            tracing::warn!("Failed to get worktree commits: {}", e);
            Ok(vec![])
        }
    }
}

async fn get_modified_files(
    worktree_path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use crate::cook::execution::mapreduce::resources::git_operations::{
        GitOperationsConfig, GitOperationsService, GitResultExt,
    };

    let mut service = GitOperationsService::new(GitOperationsConfig::default());
    match service
        .get_worktree_modified_files(worktree_path, None)
        .await
    {
        Ok(files) => Ok(files.to_string_list()),
        Err(e) => {
            // Log the error but return empty list to avoid breaking workflow
            tracing::warn!("Failed to get modified files: {}", e);
            Ok(vec![])
        }
    }
}
