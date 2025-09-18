//! Unit tests for the reduce phase executor

use super::*;
use crate::cook::execution::mapreduce::{AgentResult, AgentStatus, ReducePhase};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::WorkflowStep;
use crate::subprocess::SubprocessManager;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn create_test_environment() -> ExecutionEnvironment {
    ExecutionEnvironment {
        working_dir: PathBuf::from("/tmp/test"),
        project_dir: PathBuf::from("/tmp/test"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    }
}

fn create_test_reduce_phase() -> ReducePhase {
    ReducePhase {
        commands: vec![
            WorkflowStep {
                command: "shell: echo 'Processing results'".to_string(),
                on_failure: None,
                on_success: None,
                timeout: None,
                commit_required: false,
                capture_output: Default::default(),
            },
        ],
    }
}

fn create_test_agent_results() -> Vec<AgentResult> {
    vec![
        AgentResult::success(
            "item-1".to_string(),
            Some("output1".to_string()),
            Duration::from_secs(1),
        ),
        AgentResult::success(
            "item-2".to_string(),
            Some("output2".to_string()),
            Duration::from_secs(2),
        ),
        AgentResult::failed(
            "item-3".to_string(),
            "error".to_string(),
            Duration::from_secs(1),
        ),
    ]
}

#[tokio::test]
async fn test_reduce_phase_executor_creation() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase.clone());
    assert_eq!(executor.reduce_phase.commands.len(), 1);
}

#[tokio::test]
async fn test_phase_type() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);
    assert_eq!(executor.phase_type(), PhaseType::Reduce);
}

#[tokio::test]
async fn test_can_skip_with_no_map_results() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    // Should skip when no map results
    assert!(executor.can_skip(&context));
}

#[tokio::test]
async fn test_can_skip_with_empty_commands() {
    let mut reduce_phase = create_test_reduce_phase();
    reduce_phase.commands.clear();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );
    context.map_results = Some(create_test_agent_results());

    // Should skip when no commands to execute
    assert!(executor.can_skip(&context));
}

#[tokio::test]
async fn test_can_skip_with_map_results_and_commands() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );
    context.map_results = Some(create_test_agent_results());

    // Should not skip when both map results and commands exist
    assert!(!executor.can_skip(&context));
}

#[tokio::test]
async fn test_validate_context_without_map_results() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    let result = executor.validate_context(&context);
    assert!(result.is_err());

    if let Err(PhaseError::ValidationError { message }) = result {
        assert!(message.contains("No map results available"));
    } else {
        panic!("Expected ValidationError");
    }
}

#[tokio::test]
async fn test_validate_context_with_empty_commands() {
    let mut reduce_phase = create_test_reduce_phase();
    reduce_phase.commands.clear();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );
    context.map_results = Some(create_test_agent_results());

    let result = executor.validate_context(&context);
    assert!(result.is_err());

    if let Err(PhaseError::ValidationError { message }) = result {
        assert!(message.contains("No reduce commands"));
    } else {
        panic!("Expected ValidationError");
    }
}

#[tokio::test]
async fn test_validate_context_success() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );
    context.map_results = Some(create_test_agent_results());

    let result = executor.validate_context(&context);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_prepare_reduce_context() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    let map_results = create_test_agent_results();
    executor.prepare_reduce_context(&map_results, &mut context).await.unwrap();

    // Check that variables were set
    assert!(context.variables.contains_key("successful"));
    assert!(context.variables.contains_key("failed"));
    assert!(context.variables.contains_key("total"));
    assert_eq!(context.variables.get("successful").unwrap(), "2");
    assert_eq!(context.variables.get("failed").unwrap(), "1");
    assert_eq!(context.variables.get("total").unwrap(), "3");
}

#[tokio::test]
async fn test_prepare_reduce_context_with_all_successful() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    let map_results = vec![
        AgentResult::success(
            "item-1".to_string(),
            Some("output1".to_string()),
            Duration::from_secs(1),
        ),
        AgentResult::success(
            "item-2".to_string(),
            Some("output2".to_string()),
            Duration::from_secs(2),
        ),
    ];

    executor.prepare_reduce_context(&map_results, &mut context).await.unwrap();

    assert_eq!(context.variables.get("successful").unwrap(), "2");
    assert_eq!(context.variables.get("failed").unwrap(), "0");
    assert_eq!(context.variables.get("total").unwrap(), "2");
}

#[tokio::test]
async fn test_build_reduce_context_variables() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let map_results = create_test_agent_results();
    let variables = executor.build_reduce_context_variables(&map_results);

    assert_eq!(variables.get("map.successful").unwrap(), "2");
    assert_eq!(variables.get("map.failed").unwrap(), "1");
    assert_eq!(variables.get("map.total").unwrap(), "3");

    // Check that results are stored
    let map_results_value = variables.get("map.results").unwrap();
    assert!(map_results_value.contains("item-1"));
    assert!(map_results_value.contains("item-2"));
    assert!(map_results_value.contains("item-3"));
}

#[tokio::test]
async fn test_create_reduce_interpolation_context() {
    let reduce_phase = create_test_reduce_phase();
    let executor = ReducePhaseExecutor::new(reduce_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    context.variables.insert("test_var".to_string(), "test_value".to_string());
    context.variables.insert("map.successful".to_string(), "5".to_string());

    let interp_context = executor.create_reduce_interpolation_context(&context);

    assert_eq!(
        interp_context.variables.get("test_var").unwrap(),
        "test_value"
    );
    assert_eq!(
        interp_context.variables.get("map.successful").unwrap(),
        "5"
    );
}