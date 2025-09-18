//! Unit tests for the map phase executor

use super::*;
use crate::cook::execution::mapreduce::{MapPhase, MapReduceConfig};
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
        commit_required: false,
        fail_on_error: false,
    }
}

#[tokio::test]
async fn test_map_phase_executor_creation() {
    let map_phase = create_test_map_phase();
    let executor = MapPhaseExecutor::new(map_phase.clone());
    assert_eq!(executor.map_phase.config.input, "test_items.json");
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

#[tokio::test]
async fn test_apply_filters_with_no_filter() {
    let map_phase = create_test_map_phase();
    let executor = MapPhaseExecutor::new(map_phase);

    let items = vec![
        serde_json::json!({"id": 1, "name": "item1"}),
        serde_json::json!({"id": 2, "name": "item2"}),
        serde_json::json!({"id": 3, "name": "item3"}),
    ];

    let filtered = executor.apply_filters(items.clone());
    assert_eq!(filtered.len(), 3);
    assert_eq!(filtered, items);
}

#[tokio::test]
async fn test_apply_limits_with_max_items() {
    let mut map_phase = create_test_map_phase();
    map_phase.config.max_items = Some(2);

    let executor = MapPhaseExecutor::new(map_phase);

    let items = vec![
        serde_json::json!({"id": 1}),
        serde_json::json!({"id": 2}),
        serde_json::json!({"id": 3}),
        serde_json::json!({"id": 4}),
    ];

    let limited = executor.apply_limits(items);
    assert_eq!(limited.len(), 2);
}

#[tokio::test]
async fn test_apply_limits_with_offset() {
    let mut map_phase = create_test_map_phase();
    map_phase.config.offset = Some(2);

    let executor = MapPhaseExecutor::new(map_phase);

    let items = vec![
        serde_json::json!({"id": 1}),
        serde_json::json!({"id": 2}),
        serde_json::json!({"id": 3}),
        serde_json::json!({"id": 4}),
    ];

    let limited = executor.apply_limits(items);
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0]["id"], 3);
}

#[tokio::test]
async fn test_parse_work_items_from_file() {
    // Create a temp directory and file
    let temp_dir = TempDir::new().unwrap();
    let work_items_path = temp_dir.path().join("items.json");

    // Write test items
    let items = serde_json::json!([
        {"id": 1, "name": "test1"},
        {"id": 2, "name": "test2"}
    ]);
    std::fs::write(&work_items_path, items.to_string()).unwrap();

    // Create executor with file input
    let mut map_phase = create_test_map_phase();
    map_phase.config.input = "items.json".to_string();

    let executor = MapPhaseExecutor::new(map_phase);

    // Create context with temp dir as working dir
    let mut env = create_test_environment();
    env.working_dir = temp_dir.path().to_path_buf();

    let context = PhaseContext::new(
        env,
        Arc::new(SubprocessManager::production()),
    );

    // Parse items
    let result = executor.parse_work_items(&context).await;
    assert!(result.is_ok());

    let parsed_items = result.unwrap();
    assert_eq!(parsed_items.len(), 2);
    assert_eq!(parsed_items[0]["id"], 1);
    assert_eq!(parsed_items[1]["id"], 2);
}

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

#[tokio::test]
async fn test_aggregate_results() {
    let map_phase = create_test_map_phase();
    let executor = MapPhaseExecutor::new(map_phase);

    let results = vec![
        AgentResult::success(
            "item-1".to_string(),
            Some("output1".to_string()),
            std::time::Duration::from_secs(1),
        ),
        AgentResult::failed(
            "item-2".to_string(),
            "error".to_string(),
            std::time::Duration::from_secs(2),
        ),
        AgentResult::success(
            "item-3".to_string(),
            Some("output3".to_string()),
            std::time::Duration::from_secs(1),
        ),
    ];

    let (successful, failed) = executor.aggregate_results(&results);
    assert_eq!(successful, 2);
    assert_eq!(failed, 1);
}