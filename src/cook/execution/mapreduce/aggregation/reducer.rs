//! Result reduction logic for MapReduce operations
//!
//! This module contains pure functions for reducing and combining
//! agent execution results following functional programming patterns.

use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::mapreduce::{AgentResult, AgentStatus};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Type alias for complex reduction function to reduce type complexity
pub type ReductionFunction = Arc<dyn Fn(&[AgentResult]) -> Value + Send + Sync>;

/// Strategy for reducing results
#[derive(Clone)]
pub enum ReductionStrategy {
    /// Concatenate all outputs
    Concatenate,
    /// Merge JSON objects
    MergeJson,
    /// Keep only successful results
    FilterSuccess,
    /// Group by status
    GroupByStatus,
    /// Custom reduction function
    Custom(ReductionFunction),
}

impl std::fmt::Debug for ReductionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Concatenate => write!(f, "Concatenate"),
            Self::MergeJson => write!(f, "MergeJson"),
            Self::FilterSuccess => write!(f, "FilterSuccess"),
            Self::GroupByStatus => write!(f, "GroupByStatus"),
            Self::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

/// Result reducer for combining agent results
pub struct ResultReducer {
    strategy: ReductionStrategy,
}

impl ResultReducer {
    /// Create a new reducer with the specified strategy
    pub fn new(strategy: ReductionStrategy) -> Self {
        Self { strategy }
    }

    /// Reduce results according to the configured strategy
    pub fn reduce(&self, results: &[AgentResult]) -> Value {
        match &self.strategy {
            ReductionStrategy::Concatenate => self.concatenate_results(results),
            ReductionStrategy::MergeJson => self.merge_json_results(results),
            ReductionStrategy::FilterSuccess => self.filter_successful_results(results),
            ReductionStrategy::GroupByStatus => self.group_by_status(results),
            ReductionStrategy::Custom(f) => f(results),
        }
    }

    /// Concatenate all output strings
    fn concatenate_results(&self, results: &[AgentResult]) -> Value {
        let output = results
            .iter()
            .filter_map(|r| r.output.as_ref())
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        json!({
            "output": output,
            "count": results.len()
        })
    }

    /// Merge JSON outputs into a single object
    fn merge_json_results(&self, results: &[AgentResult]) -> Value {
        let mut merged = json!({});

        for result in results {
            if let Some(output) = &result.output {
                if let (Ok(Value::Object(obj)), Value::Object(ref mut target)) =
                    (serde_json::from_str::<Value>(output), &mut merged)
                {
                    target.extend(obj);
                }
            }
        }

        merged
    }

    /// Filter and return only successful results
    fn filter_successful_results(&self, results: &[AgentResult]) -> Value {
        let successful: Vec<&AgentResult> = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .collect();

        json!({
            "successful": successful,
            "count": successful.len()
        })
    }

    /// Group results by their status
    fn group_by_status(&self, results: &[AgentResult]) -> Value {
        let mut groups: HashMap<String, Vec<&AgentResult>> = HashMap::new();

        for result in results {
            let status_key = match &result.status {
                AgentStatus::Success => "success",
                AgentStatus::Failed(_) => "failed",
                AgentStatus::Timeout => "timeout",
                AgentStatus::Pending => "pending",
                AgentStatus::Running => "running",
                AgentStatus::Retrying(_) => "retrying",
            };

            groups
                .entry(status_key.to_string())
                .or_default()
                .push(result);
        }

        json!(groups)
    }

    /// Build interpolation context for reduce phase
    pub fn build_reduce_context(
        results: &[AgentResult],
        summary: &super::AggregationSummary,
    ) -> Result<InterpolationContext, serde_json::Error> {
        let mut context = InterpolationContext::new();

        // Add summary statistics
        context.set(
            "map",
            json!({
                "successful": summary.successful,
                "failed": summary.failed,
                "total": summary.total,
                "avg_duration": summary.avg_duration_secs,
                "total_duration": summary.total_duration_secs
            }),
        );

        // Add complete results
        let results_value = serde_json::to_value(results)?;
        context.set("map.results", results_value);

        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_result(id: &str, status: AgentStatus, output: Option<String>) -> AgentResult {
        AgentResult {
            item_id: id.to_string(),
            status,
            output,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
        }
    }

    #[test]
    fn test_concatenate_strategy() {
        let results = vec![
            create_test_result("1", AgentStatus::Success, Some("output1".to_string())),
            create_test_result("2", AgentStatus::Success, Some("output2".to_string())),
        ];

        let reducer = ResultReducer::new(ReductionStrategy::Concatenate);
        let reduced = reducer.reduce(&results);

        assert_eq!(reduced["output"], "output1\noutput2");
        assert_eq!(reduced["count"], 2);
    }

    #[test]
    fn test_filter_success_strategy() {
        let results = vec![
            create_test_result("1", AgentStatus::Success, None),
            create_test_result("2", AgentStatus::Failed("error".to_string()), None),
            create_test_result("3", AgentStatus::Success, None),
        ];

        let reducer = ResultReducer::new(ReductionStrategy::FilterSuccess);
        let reduced = reducer.reduce(&results);

        assert_eq!(reduced["count"], 2);
    }
}
