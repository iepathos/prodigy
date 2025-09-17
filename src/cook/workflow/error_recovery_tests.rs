//! Tests for error recovery during workflow resume

#[cfg(test)]
mod tests {
    use super::super::checkpoint::{
        ExecutionState, WorkflowCheckpoint, WorkflowStatus, CHECKPOINT_VERSION,
    };
    use super::super::error_recovery::*;
    use super::super::executor::WorkflowContext;
    use super::super::on_failure::{HandlerCommand, OnFailureConfig};
    use crate::cook::workflow::HandlerStrategy;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::time::Duration;

    #[tokio::test]
    async fn test_error_recovery_state_default() {
        let state = ErrorRecoveryState::default();
        assert_eq!(state.recovery_attempts, 0);
        assert_eq!(state.max_recovery_attempts, 3);
        assert!(state.active_handlers.is_empty());
        assert!(state.error_context.is_empty());
        assert!(state.handler_execution_history.is_empty());
        assert!(state.retry_state.is_none());
        assert!(!state.correlation_id.is_empty()); // Should have a UUID
    }

    #[tokio::test]
    async fn test_resume_error_recovery_new() {
        let recovery = ResumeErrorRecovery::new();
        assert_eq!(recovery.recovery_state.recovery_attempts, 0);
        assert_eq!(recovery.recovery_state.max_recovery_attempts, 3);
    }

    #[tokio::test]
    async fn test_handle_corrupted_checkpoint_error() {
        let mut recovery = ResumeErrorRecovery::new();

        let checkpoint = create_test_checkpoint();
        let error = ResumeError::CorruptedCheckpoint("Invalid format".to_string());

        let result = recovery.handle_resume_error(&error, &checkpoint).await;
        assert!(result.is_ok());

        match result.unwrap() {
            RecoveryAction::PartialResume { from_step } => {
                assert_eq!(from_step, 0); // Should start from beginning if no completed steps
            }
            _ => panic!("Expected PartialResume action"),
        }
    }

    #[tokio::test]
    async fn test_handle_missing_dependency_error() {
        let mut recovery = ResumeErrorRecovery::new();

        let checkpoint = create_test_checkpoint();
        let error = ResumeError::MissingDependency("claude:test-command".to_string());

        let result = recovery.handle_resume_error(&error, &checkpoint).await;
        assert!(result.is_ok());

        match result.unwrap() {
            RecoveryAction::RequestIntervention { message } => {
                assert!(message.contains("Missing command dependency"));
                assert!(message.contains("claude:test-command"));
            }
            _ => panic!("Expected RequestIntervention action"),
        }
    }

    #[tokio::test]
    async fn test_handle_environment_mismatch_error() {
        let mut recovery = ResumeErrorRecovery::new();

        let checkpoint = create_test_checkpoint();
        let error = ResumeError::EnvironmentMismatch("PATH changed".to_string());

        let result = recovery.handle_resume_error(&error, &checkpoint).await;
        assert!(result.is_ok());

        match result.unwrap() {
            RecoveryAction::Continue => {
                // Should continue with current environment
            }
            _ => panic!("Expected Continue action"),
        }
    }

    #[tokio::test]
    async fn test_recovery_attempt_limit() {
        let mut recovery = ResumeErrorRecovery::new();
        recovery.recovery_state.recovery_attempts = 3; // Already at max

        let checkpoint = create_test_checkpoint();
        let error = ResumeError::Other(anyhow::anyhow!("Generic error"));

        let result = recovery.handle_resume_error(&error, &checkpoint).await;
        assert!(result.is_ok());

        match result.unwrap() {
            RecoveryAction::SafeAbort { cleanup_actions } => {
                assert!(cleanup_actions.is_empty());
            }
            _ => panic!("Expected SafeAbort action when recovery limit exceeded"),
        }
    }

    #[tokio::test]
    async fn test_on_failure_to_error_handler() {
        let on_failure = OnFailureConfig::SingleCommand("shell: echo 'recovery'".to_string());
        let handler = on_failure_to_error_handler(&on_failure, 0);

        assert!(handler.is_some());
        let handler = handler.unwrap();
        assert_eq!(handler.id, "step_0_handler");
        assert_eq!(handler.commands.len(), 1);
        assert_eq!(handler.scope, ErrorHandlerScope::Step);
    }

    #[tokio::test]
    async fn test_save_and_load_recovery_state() {
        let recovery_state = ErrorRecoveryState {
            active_handlers: vec![],
            error_context: HashMap::new(),
            handler_execution_history: vec![],
            retry_state: None,
            correlation_id: "test-123".to_string(),
            recovery_attempts: 1,
            max_recovery_attempts: 3,
        };

        let mut checkpoint = create_test_checkpoint();

        // Save recovery state
        save_recovery_state_to_checkpoint(&mut checkpoint, &recovery_state);
        assert!(checkpoint
            .variable_state
            .contains_key("__error_recovery_state"));

        // Load recovery state
        let loaded = load_recovery_state_from_checkpoint(&checkpoint);
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.correlation_id, "test-123");
        assert_eq!(loaded.recovery_attempts, 1);
        assert_eq!(loaded.max_recovery_attempts, 3);
    }

    #[tokio::test]
    async fn test_execute_error_handler_with_context() {
        let mut recovery = ResumeErrorRecovery::new();
        let mut workflow_context = WorkflowContext::default();

        let handler = ErrorHandler {
            id: "test_handler".to_string(),
            condition: None,
            commands: vec![HandlerCommand {
                shell: Some("echo 'handling error'".to_string()),
                claude: None,
                continue_on_error: false,
            }],
            retry_config: None,
            timeout: Some(Duration::from_secs(30)),
            scope: ErrorHandlerScope::Step,
            strategy: HandlerStrategy::Recovery,
        };

        let result = recovery
            .execute_error_handler_with_resume_context(
                &handler,
                "Test error message",
                &mut workflow_context,
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(recovery.recovery_state.handler_execution_history.len(), 1);

        let execution = &recovery.recovery_state.handler_execution_history[0];
        assert_eq!(execution.handler_id, "test_handler");
    }

    #[tokio::test]
    async fn test_restore_error_handlers_from_checkpoint() {
        let mut recovery = ResumeErrorRecovery::new();
        let checkpoint = create_test_checkpoint();

        let result = recovery.restore_error_handlers(&checkpoint).await;
        assert!(result.is_ok());

        let handlers = result.unwrap();
        assert!(handlers.is_empty()); // Empty checkpoint should have no handlers
    }

    // Helper function to create a test checkpoint
    fn create_test_checkpoint() -> WorkflowCheckpoint {
        WorkflowCheckpoint {
            workflow_id: "test-workflow".to_string(),
            execution_state: ExecutionState {
                current_step_index: 0,
                total_steps: 5,
                status: WorkflowStatus::Running,
                start_time: Utc::now(),
                last_checkpoint: Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: Vec::new(),
            variable_state: HashMap::new(),
            mapreduce_state: None,
            timestamp: Utc::now(),
            version: CHECKPOINT_VERSION,
            workflow_hash: "test-hash".to_string(),
            total_steps: 5,
            workflow_name: Some("Test Workflow".to_string()),
            workflow_path: None,
            error_recovery_state: None,
        }
    }
}
