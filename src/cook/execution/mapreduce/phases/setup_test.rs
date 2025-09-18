//! Unit tests for the setup phase executor

use super::*;
use crate::cook::execution::mapreduce::SetupPhase;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::WorkflowStep;
use crate::subprocess::SubprocessManager;
use std::path::PathBuf;
use std::sync::Arc;

fn create_test_environment() -> ExecutionEnvironment {
    ExecutionEnvironment {
        working_dir: PathBuf::from("/tmp/test"),
        project_dir: PathBuf::from("/tmp/test"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    }
}

fn create_test_setup_phase() -> SetupPhase {
    SetupPhase {
        commands: vec![
            WorkflowStep {
                command: "shell: echo 'Setting up environment'".to_string(),
                on_failure: None,
                on_success: None,
                timeout: None,
                commit_required: false,
                capture_output: Default::default(),
            },
            WorkflowStep {
                command: "shell: echo 'Preparing data'".to_string(),
                on_failure: None,
                on_success: None,
                timeout: None,
                commit_required: false,
                capture_output: Default::default(),
            },
        ],
    }
}

#[tokio::test]
async fn test_setup_phase_executor_creation() {
    let setup_phase = create_test_setup_phase();
    let executor = SetupPhaseExecutor::new(setup_phase.clone());
    assert_eq!(executor.setup_phase.commands.len(), 2);
}

#[tokio::test]
async fn test_phase_type() {
    let setup_phase = create_test_setup_phase();
    let executor = SetupPhaseExecutor::new(setup_phase);
    assert_eq!(executor.phase_type(), PhaseType::Setup);
}

#[tokio::test]
async fn test_can_skip_with_empty_commands() {
    let mut setup_phase = create_test_setup_phase();
    setup_phase.commands.clear();
    let executor = SetupPhaseExecutor::new(setup_phase);

    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    // Should skip when no setup commands
    assert!(executor.can_skip(&context));
}

#[tokio::test]
async fn test_can_skip_with_commands() {
    let setup_phase = create_test_setup_phase();
    let executor = SetupPhaseExecutor::new(setup_phase);

    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    // Should not skip when setup commands exist
    assert!(!executor.can_skip(&context));
}

#[tokio::test]
async fn test_validate_context_with_empty_commands() {
    let mut setup_phase = create_test_setup_phase();
    setup_phase.commands.clear();
    let executor = SetupPhaseExecutor::new(setup_phase);

    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    let result = executor.validate_context(&context);
    assert!(result.is_err());

    if let Err(PhaseError::ValidationError { message }) = result {
        assert!(message.contains("No setup commands"));
    } else {
        panic!("Expected ValidationError");
    }
}

#[tokio::test]
async fn test_validate_context_success() {
    let setup_phase = create_test_setup_phase();
    let executor = SetupPhaseExecutor::new(setup_phase);

    let context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    let result = executor.validate_context(&context);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_setup_interpolation_context() {
    let setup_phase = create_test_setup_phase();
    let executor = SetupPhaseExecutor::new(setup_phase);

    let mut context = PhaseContext::new(
        create_test_environment(),
        Arc::new(SubprocessManager::production()),
    );

    // Add some variables to context
    context.variables.insert("env_var".to_string(), "env_value".to_string());
    context.variables.insert("project".to_string(), "test_project".to_string());

    let interp_context = executor.create_setup_interpolation_context(&context);

    assert_eq!(
        interp_context.variables.get("env_var").unwrap(),
        "env_value"
    );
    assert_eq!(
        interp_context.variables.get("project").unwrap(),
        "test_project"
    );
    assert_eq!(
        interp_context.environment.session_id,
        "test-session"
    );
}

#[test]
fn test_setup_phase_serialization() {
    let setup_phase = create_test_setup_phase();

    // Serialize
    let json = serde_json::to_string(&setup_phase).unwrap();
    assert!(json.contains("Setting up environment"));
    assert!(json.contains("Preparing data"));

    // Deserialize
    let deserialized: SetupPhase = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.commands.len(), 2);
}

#[test]
fn test_setup_phase_with_complex_commands() {
    let setup_phase = SetupPhase {
        commands: vec![
            WorkflowStep {
                command: "shell: mkdir -p /tmp/test/data".to_string(),
                on_failure: None,
                on_success: None,
                timeout: Some(30),
                commit_required: false,
                capture_output: Default::default(),
            },
            WorkflowStep {
                command: "claude: /analyze-project".to_string(),
                on_failure: None,
                on_success: None,
                timeout: Some(120),
                commit_required: true,
                capture_output: Default::default(),
            },
        ],
    };

    let executor = SetupPhaseExecutor::new(setup_phase.clone());
    assert_eq!(executor.setup_phase.commands.len(), 2);
    assert_eq!(executor.setup_phase.commands[0].timeout, Some(30));
    assert_eq!(executor.setup_phase.commands[1].timeout, Some(120));
    assert!(executor.setup_phase.commands[1].commit_required);
}

#[test]
fn test_phase_error_variants() {
    // Test ExecutionFailed variant
    let exec_error = PhaseError::ExecutionFailed {
        message: "Execution failed".to_string(),
    };
    assert!(format!("{}", exec_error).contains("Execution failed"));

    // Test ValidationError variant
    let val_error = PhaseError::ValidationError {
        message: "Validation failed".to_string(),
    };
    assert!(format!("{}", val_error).contains("Validation failed"));

    // Test TransitionError variant
    let trans_error = PhaseError::TransitionError {
        message: "Transition error".to_string(),
    };
    assert!(format!("{}", trans_error).contains("Transition error"));

    // Test Timeout variant
    let timeout_error = PhaseError::Timeout {
        message: "Operation timed out".to_string(),
    };
    assert!(format!("{}", timeout_error).contains("timed out"));
}