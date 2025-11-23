//! Effect-based orchestration for composable I/O operations
//!
//! This module uses Stillwater's Effect pattern to separate pure business logic
//! from I/O operations, enabling testable workflow orchestration.
//!
//! This is a foundational implementation demonstrating the pattern. Full integration
//! with existing orchestrator logic will be done incrementally.

use crate::config::WorkflowConfig;
use crate::cook::orchestrator::environment::OrchestratorEnv;
use crate::cook::orchestrator::pure;
use std::path::PathBuf;
use stillwater::Effect;

/// Effect type for orchestrator operations
pub type OrchEffect<T> = Effect<T, anyhow::Error, OrchestratorEnv>;

/// Workflow session state
#[derive(Debug, Clone)]
pub struct WorkflowSession {
    /// Session ID
    pub session_id: String,
    /// Worktree path (if using worktree)
    pub worktree_path: Option<PathBuf>,
    /// Workflow configuration
    pub config: WorkflowConfig,
}

/// Step execution context
#[derive(Debug, Clone)]
pub struct StepContext {
    /// Session ID
    pub session_id: String,
    /// Step index in workflow
    pub step_index: usize,
    /// Working directory
    pub working_dir: PathBuf,
    /// Previous step results
    pub previous_results: Vec<StepResult>,
}

/// Result from executing a workflow step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step index
    pub step_index: usize,
    /// Whether step succeeded
    pub success: bool,
    /// Output from step
    pub output: String,
    /// Error message if failed
    pub error: Option<String>,
}

/// Complete workflow execution result
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    /// Session ID
    pub session_id: String,
    /// Results from all steps
    pub step_results: Vec<StepResult>,
    /// Overall success
    pub success: bool,
}

/// Validate workflow configuration (Effect wrapping pure validation)
///
/// This demonstrates how to lift pure validations into Effects.
pub fn validate_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowConfig> {
    use stillwater::Validation;

    match pure::validate_workflow(&config) {
        Validation::Success(_) => Effect::pure(config),
        Validation::Failure(errors) => {
            let error_msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Effect::fail(anyhow::anyhow!("Workflow validation failed: {}", error_msg))
        }
    }
}

/// Setup workflow environment (Effect)
///
/// Creates session and prepares execution environment.
/// This is a simplified implementation demonstrating Effect composition.
pub fn setup_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowSession> {
    // First validate
    validate_workflow(config.clone()).and_then_auto(|validated_config| {
        // Then setup session
        Effect::from_async(move |env: &OrchestratorEnv| {
            let session_manager = env.session_manager.clone();
            async move {
                // Generate session ID
                let session_id = uuid::Uuid::new_v4().to_string();

                // Start session
                session_manager
                    .start_session(&session_id)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to start session: {}", e))?;

                // For now, we don't create worktree here - that's handled elsewhere
                // This is a simplified implementation focusing on the Effect pattern

                Ok::<WorkflowSession, anyhow::Error>(WorkflowSession {
                    session_id,
                    worktree_path: None,
                    config: validated_config,
                })
            }
        })
    })
}

/// Execute complete workflow (placeholder)
///
/// This demonstrates the Effect composition pattern.
/// Full workflow execution will be integrated incrementally.
pub fn execute_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowResult> {
    setup_workflow(config).and_then_auto(|session| {
        let result = WorkflowResult {
            session_id: session.session_id,
            step_results: vec![],
            success: true,
        };
        OrchEffect::pure(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::WorkflowCommand;

    // Test fixtures
    fn simple_workflow_config() -> WorkflowConfig {
        WorkflowConfig {
            name: Some("test".to_string()),
            commands: vec![WorkflowCommand::Simple("echo test".to_string())],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    fn empty_workflow_config() -> WorkflowConfig {
        WorkflowConfig {
            name: Some("empty".to_string()),
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    #[test]
    fn test_workflow_session_creation() {
        let config = simple_workflow_config();
        let session = WorkflowSession {
            session_id: "test-123".to_string(),
            worktree_path: Some(PathBuf::from("/tmp/test")),
            config,
        };

        assert_eq!(session.session_id, "test-123");
        assert!(session.worktree_path.is_some());
    }

    #[test]
    fn test_workflow_session_without_worktree() {
        let config = simple_workflow_config();
        let session = WorkflowSession {
            session_id: "test-456".to_string(),
            worktree_path: None,
            config,
        };

        assert_eq!(session.session_id, "test-456");
        assert!(session.worktree_path.is_none());
    }

    #[test]
    fn test_step_context_creation() {
        let context = StepContext {
            session_id: "test-123".to_string(),
            step_index: 0,
            working_dir: PathBuf::from("/tmp"),
            previous_results: vec![],
        };

        assert_eq!(context.session_id, "test-123");
        assert_eq!(context.step_index, 0);
        assert!(context.previous_results.is_empty());
    }

    #[test]
    fn test_step_context_with_previous_results() {
        let prev_result = StepResult {
            step_index: 0,
            success: true,
            output: "previous output".to_string(),
            error: None,
        };

        let context = StepContext {
            session_id: "test-123".to_string(),
            step_index: 1,
            working_dir: PathBuf::from("/tmp"),
            previous_results: vec![prev_result],
        };

        assert_eq!(context.step_index, 1);
        assert_eq!(context.previous_results.len(), 1);
    }

    #[test]
    fn test_step_result_success() {
        let result = StepResult {
            step_index: 0,
            success: true,
            output: "test output".to_string(),
            error: None,
        };

        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.output, "test output");
    }

    #[test]
    fn test_step_result_failure() {
        let result = StepResult {
            step_index: 0,
            success: false,
            output: String::new(),
            error: Some("test error".to_string()),
        };

        assert!(!result.success);
        assert!(result.error.is_some());
        assert_eq!(result.error.unwrap(), "test error");
    }

    #[test]
    fn test_workflow_result_success() {
        let result = WorkflowResult {
            session_id: "test-789".to_string(),
            step_results: vec![StepResult {
                step_index: 0,
                success: true,
                output: "done".to_string(),
                error: None,
            }],
            success: true,
        };

        assert!(result.success);
        assert_eq!(result.step_results.len(), 1);
    }

    #[test]
    fn test_workflow_result_failure() {
        let result = WorkflowResult {
            session_id: "test-999".to_string(),
            step_results: vec![
                StepResult {
                    step_index: 0,
                    success: true,
                    output: "step 1 ok".to_string(),
                    error: None,
                },
                StepResult {
                    step_index: 1,
                    success: false,
                    output: "".to_string(),
                    error: Some("step 2 failed".to_string()),
                },
            ],
            success: false,
        };

        assert!(!result.success);
        assert_eq!(result.step_results.len(), 2);
        assert!(!result.step_results[1].success);
    }

    #[test]
    fn test_validate_workflow_pure_success() {
        let config = simple_workflow_config();
        let result = pure::validate_workflow(&config);
        use stillwater::Validation;
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_workflow_pure_failure() {
        let config = empty_workflow_config();
        let result = pure::validate_workflow(&config);
        use stillwater::Validation;
        assert!(matches!(result, Validation::Failure(_)));
    }

    #[test]
    fn test_validate_workflow_effect_pure_transform() {
        let config = simple_workflow_config();
        // The validate_workflow effect wraps pure validation
        // We test the pure part here; Effect execution requires runtime
        let pure_result = pure::validate_workflow(&config);
        use stillwater::Validation;
        assert!(matches!(pure_result, Validation::Success(_)));
    }

    // NOTE: Integration tests with full mock environment deferred
    // Current tests focus on pure logic and data structures
    // Full Effect execution tests require complete mock implementations
    // of all traits (SessionManager, CommandExecutor, ClaudeExecutor, etc.)
    //
    // For now, Effect-based code is tested through:
    // 1. Pure function tests (above)
    // 2. Data structure tests (above)
    // 3. Integration with actual orchestrator in production code
}
