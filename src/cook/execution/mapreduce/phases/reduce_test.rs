//! Unit tests for the reduce phase executor

use super::*;
use crate::cook::execution::mapreduce::{AgentResult, ReducePhase};
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
        commands: vec![WorkflowStep {
            shell: Some("echo 'Processing results'".to_string()),
            ..Default::default()
        }],
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
    let _executor = ReducePhaseExecutor::new(reduce_phase.clone());
    // Executor created successfully - private fields cannot be accessed directly
    assert_eq!(reduce_phase.commands.len(), 1);
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

// Test removed: prepare_reduce_context is a private method
// This functionality is tested through the public execute() method

// Test removed: prepare_reduce_context is a private method
// This functionality is tested through the public execute() method

// Test removed: build_reduce_context_variables is a private method
// This functionality is tested through the public execute() method

// Test removed: create_reduce_interpolation_context is a private method
// This functionality is tested through the public execute() method
