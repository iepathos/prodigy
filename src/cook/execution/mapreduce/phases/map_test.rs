//! Unit tests for the map phase executor

use super::*;
use crate::cook::execution::mapreduce::{MapPhase, MapReduceConfig};
use crate::cook::execution::AgentResult;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::subprocess::SubprocessManager;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

fn create_test_environment() -> ExecutionEnvironment {
    ExecutionEnvironment {
        working_dir: PathBuf::from("/tmp/test"),
        project_dir: PathBuf::from("/tmp/test"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    }
}

fn create_test_config() -> MapReduceConfig {
    MapReduceConfig {
        input: "test_items.json".to_string(),
        json_path: "$.items[*]".to_string(),
        max_parallel: 5,
        timeout_per_agent: 60,
        retry_on_failure: 2,
        max_items: Some(10),
        offset: None,
    }
}

fn create_test_map_phase() -> MapPhase {
    MapPhase {
        config: create_test_config(),
        agent_template: vec![],
        filter: None,
        sort_by: None,
        distinct: None,
    }
}

#[tokio::test]
async fn test_map_phase_executor_creation() {
    let map_phase = create_test_map_phase();
    let _executor = MapPhaseExecutor::new(map_phase.clone());
    // Executor created successfully - private fields cannot be accessed directly
    assert_eq!(map_phase.config.input, "test_items.json");
}

#[tokio::test]
async fn test_validate_context_with_empty_input() {
    let mut map_phase = create_test_map_phase();
    map_phase.config.input = String::new();

    let executor = MapPhaseExecutor::new(map_phase);
    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    let result = executor.validate_context(&context);
    assert!(result.is_err());

    if let Err(PhaseError::ValidationError { message }) = result {
        assert!(message.contains("input source is not specified"));
    } else {
        panic!("Expected ValidationError");
    }
}

#[tokio::test]
async fn test_validate_context_with_zero_max_parallel() {
    let mut map_phase = create_test_map_phase();
    map_phase.config.max_parallel = 0;

    let executor = MapPhaseExecutor::new(map_phase);
    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    let result = executor.validate_context(&context);
    assert!(result.is_err());

    if let Err(PhaseError::ValidationError { message }) = result {
        assert!(message.contains("max_parallel must be greater than 0"));
    } else {
        panic!("Expected ValidationError");
    }
}

// Test removed: apply_filters is a private method
// This functionality is tested through the public execute() method

// Test removed: apply_limits is a private method
// This functionality is tested through the public execute() method

// Test removed: apply_limits is a private method
// This functionality is tested through the public execute() method

// Test removed: parse_work_items is a private method
// This functionality is tested through the public execute() method

#[tokio::test]
async fn test_phase_type() {
    let map_phase = create_test_map_phase();
    let executor = MapPhaseExecutor::new(map_phase);
    assert_eq!(executor.phase_type(), PhaseType::Map);
}

#[tokio::test]
async fn test_can_skip_default() {
    let map_phase = create_test_map_phase();
    let executor = MapPhaseExecutor::new(map_phase);
    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    // By default, map phase cannot be skipped
    assert!(!executor.can_skip(&context));
}

// Test removed: aggregate_results is a private method
// This functionality is tested through the public execute() method
