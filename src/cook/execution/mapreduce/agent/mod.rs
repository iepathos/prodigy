//! Agent module for MapReduce parallel execution
//!
//! This module provides the core functionality for managing agent lifecycle,
//! execution, and result aggregation in the MapReduce framework.

pub mod execution;
pub mod lifecycle;
pub mod lifecycle_with_cleanup;
pub mod results;
pub mod types;

// Re-export core types for convenience
pub use types::{
    AgentConfig, AgentHandle, AgentOperation, AgentResult, AgentState, AgentStateStatus,
    AgentStatus,
};

// Re-export execution functionality
pub use execution::{AgentExecutor, EnhancedProgressExecutor, ExecutionStrategy, StandardExecutor};

// Re-export lifecycle management
pub use lifecycle::{AgentLifecycleManager, DefaultLifecycleManager};
pub use lifecycle_with_cleanup::CleanupAwareLifecycleManager;

// Re-export result aggregation
pub use results::{AgentResultAggregator, AggregatedResults, DefaultResultAggregator};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod basic_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_agent_result_creation() {
        let result = AgentResult::success(
            "item-1".to_string(),
            Some("output".to_string()),
            Duration::from_secs(5),
        );

        assert!(result.is_success());
        assert!(!result.is_failure());
        assert_eq!(result.item_id, "item-1");
        assert_eq!(result.output, Some("output".to_string()));
    }

    #[test]
    fn test_agent_result_failure() {
        let result = AgentResult::failed(
            "item-2".to_string(),
            "error occurred".to_string(),
            Duration::from_secs(2),
        );

        assert!(!result.is_success());
        assert!(result.is_failure());
        assert_eq!(result.item_id, "item-2");
        assert_eq!(result.error, Some("error occurred".to_string()));
    }

    #[test]
    fn test_aggregated_results() {
        let results = vec![
            AgentResult::success("item-1".to_string(), None, Duration::from_secs(5)),
            AgentResult::failed(
                "item-2".to_string(),
                "error".to_string(),
                Duration::from_secs(3),
            ),
            AgentResult::success("item-3".to_string(), None, Duration::from_secs(4)),
        ];

        let aggregated = AggregatedResults::from_results(results);

        assert_eq!(aggregated.success_count, 2);
        assert_eq!(aggregated.failure_count, 1);
        assert_eq!(aggregated.total, 3);
        assert_eq!(aggregated.successful.len(), 2);
        assert_eq!(aggregated.failed.len(), 1);
    }

    #[test]
    fn test_agent_config_creation() {
        let config = AgentConfig::new(
            "agent-1".to_string(),
            "item-1".to_string(),
            "branch-1".to_string(),
            3,
            Duration::from_secs(60),
            0,
            10,
        );

        assert_eq!(config.id, "agent-1");
        assert_eq!(config.item_id, "item-1");
        assert_eq!(config.branch_name, "branch-1");
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.agent_index, 0);
        assert_eq!(config.total_items, 10);
    }

    #[test]
    fn test_agent_state_transitions() {
        let mut state = AgentState::default();

        assert_eq!(state.status, AgentStateStatus::Idle);

        state.set_operation("Starting work".to_string());
        assert_eq!(state.current_operation, Some("Starting work".to_string()));

        state.mark_retrying(2);
        assert_eq!(state.status, AgentStateStatus::Retrying(2));
        assert_eq!(state.retry_count, 2);

        state.mark_completed();
        assert_eq!(state.status, AgentStateStatus::Completed);
        assert_eq!(state.current_operation, None);
    }

    #[test]
    fn test_agent_operation_display() {
        let op = AgentOperation::Claude("command".to_string());
        assert_eq!(op.display(), "Claude: command");

        let op = AgentOperation::Retrying("task".to_string(), 3);
        assert_eq!(op.display(), "Retrying (3): task");

        let op = AgentOperation::Complete;
        assert_eq!(op.display(), "Complete");
    }

    #[test]
    fn test_aggregated_results_summary() {
        let results = vec![
            AgentResult::success("item-1".to_string(), None, Duration::from_secs(5)),
            AgentResult::success("item-2".to_string(), None, Duration::from_secs(3)),
        ];

        let aggregated = AggregatedResults::from_results(results);
        let summary = aggregated.summary();

        assert!(summary.contains("2/2 succeeded"));
        assert!(summary.contains("0 failed"));
    }
}
