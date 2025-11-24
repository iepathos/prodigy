//! Effect-based orchestration for composable I/O operations
//!
//! This module uses Stillwater's Effect pattern to separate pure business logic
//! from I/O operations, enabling testable workflow orchestration.
//!
//! # Architecture
//!
//! Following the spec 174e pattern, effects are composed in three stages:
//! 1. `setup_environment_effect` - Session creation and worktree setup
//! 2. `execute_plan_effect` - Execute the plan based on mode
//! 3. `finalize_session_effect` - Cleanup and session finalization
//!
//! # Example
//!
//! ```ignore
//! use prodigy::core::orchestration::plan_execution;
//! use prodigy::cook::orchestrator::effects::*;
//!
//! let plan = plan_execution(&config);
//!
//! let effect = setup_environment_effect(&plan)
//!     .and_then(|env| execute_plan_effect(&plan, env))
//!     .and_then(|result| finalize_session_effect(result));
//!
//! effect.run_async(&deps).await
//! ```

use crate::config::WorkflowConfig;
use crate::cook::orchestrator::environment::OrchestratorEnv;
use crate::cook::orchestrator::pure;
use crate::core::orchestration::{ExecutionMode, ExecutionPlan, Phase};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
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

/// Execution environment created during setup
#[derive(Debug, Clone)]
pub struct ExecutionEnvironment {
    /// Working directory (may be worktree)
    pub working_dir: Arc<PathBuf>,
    /// Original project directory
    pub project_dir: Arc<PathBuf>,
    /// Worktree name if using worktree
    pub worktree_name: Option<Arc<str>>,
    /// Session ID
    pub session_id: Arc<str>,
    /// Variables for interpolation
    pub variables: HashMap<String, String>,
}

/// Setup environment effect (I/O)
///
/// Creates session and worktree based on the execution plan.
///
/// # Arguments
///
/// * `plan` - The execution plan from pure planning
/// * `project_path` - Project root path
/// * `dry_run` - Whether this is a dry run
///
/// # Returns
///
/// An Effect that creates the execution environment
pub fn setup_environment_effect(
    plan: ExecutionPlan,
    project_path: Arc<PathBuf>,
    dry_run: bool,
) -> OrchEffect<ExecutionEnvironment> {
    Effect::from_async(move |env: &OrchestratorEnv| {
        let plan = plan.clone();
        let project_path = project_path.clone();
        let session_manager = env.session_manager.clone();

        async move {
            // Generate session ID
            let session_id = crate::unified_session::SessionId::new().to_string();
            let session_id_arc: Arc<str> = Arc::from(session_id.as_str());

            // Start session
            session_manager
                .start_session(&session_id)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to start session: {}", e))?;

            // Create worktree if needed (based on pure plan)
            let (working_dir, worktree_name) = if plan.requires_worktrees() && !dry_run {
                // Worktree creation delegated to external manager
                // For now, we use project path - full implementation
                // would use WorktreeManager here
                (project_path.clone(), None)
            } else {
                (project_path.clone(), None)
            };

            Ok(ExecutionEnvironment {
                working_dir,
                project_dir: project_path,
                worktree_name,
                session_id: session_id_arc,
                variables: HashMap::new(),
            })
        }
    })
}

/// Execute plan effect (I/O)
///
/// Executes the plan based on detected mode.
///
/// # Arguments
///
/// * `plan` - The execution plan
/// * `env` - The execution environment from setup
///
/// # Returns
///
/// An Effect that executes the plan
pub fn execute_plan_effect(
    plan: ExecutionPlan,
    exec_env: ExecutionEnvironment,
) -> OrchEffect<WorkflowResult> {
    Effect::from_async(move |_env: &OrchestratorEnv| {
        let plan = plan.clone();
        let exec_env = exec_env.clone();

        async move {
            // Dispatch based on execution mode (determined by pure planning)
            let step_results = match plan.mode {
                ExecutionMode::MapReduce => {
                    // MapReduce execution would be delegated to mapreduce executor
                    vec![]
                }
                ExecutionMode::Standard | ExecutionMode::Iterative => {
                    // Standard/Iterative execution would execute phases
                    execute_phases(&plan.phases, &exec_env).await?
                }
                ExecutionMode::DryRun => {
                    // Dry run - no actual execution
                    vec![StepResult {
                        step_index: 0,
                        success: true,
                        output: "Dry run completed".to_string(),
                        error: None,
                    }]
                }
            };

            let success = step_results.iter().all(|r| r.success);

            Ok(WorkflowResult {
                session_id: exec_env.session_id.to_string(),
                step_results,
                success,
            })
        }
    })
}

/// Helper: Execute phases sequentially
async fn execute_phases(
    phases: &[Phase],
    _exec_env: &ExecutionEnvironment,
) -> anyhow::Result<Vec<StepResult>> {
    let mut results = Vec::new();

    for (idx, phase) in phases.iter().enumerate() {
        // Each phase would execute its commands
        // For now, return placeholder results
        let result = StepResult {
            step_index: idx,
            success: true,
            output: format!("Phase {} completed", phase),
            error: None,
        };
        results.push(result);
    }

    Ok(results)
}

/// Finalize session effect (I/O)
///
/// Cleans up session and worktree after execution.
///
/// # Arguments
///
/// * `result` - The execution result
/// * `exec_env` - The execution environment
///
/// # Returns
///
/// An Effect that finalizes the session
pub fn finalize_session_effect(
    result: WorkflowResult,
    exec_env: ExecutionEnvironment,
) -> OrchEffect<WorkflowResult> {
    Effect::from_async(move |env: &OrchestratorEnv| {
        let result = result.clone();
        let exec_env = exec_env.clone();
        let session_manager = env.session_manager.clone();

        async move {
            // Complete session
            let _ = session_manager.complete_session().await;

            // Cleanup worktree if present
            if let Some(_worktree) = exec_env.worktree_name {
                // Worktree cleanup would be delegated to WorktreeManager
                // Full implementation would prompt for merge or cleanup
            }

            Ok(result)
        }
    })
}

/// Compose all effects into complete workflow execution
///
/// This is the main entry point for effect-based workflow execution.
/// It composes setup, execution, and finalization into a single effect.
///
/// # Arguments
///
/// * `plan` - Pure execution plan
/// * `project_path` - Project root path
/// * `dry_run` - Whether this is a dry run
///
/// # Returns
///
/// A composed effect that executes the complete workflow
pub fn run_workflow_effect(
    plan: ExecutionPlan,
    project_path: Arc<PathBuf>,
    dry_run: bool,
) -> OrchEffect<WorkflowResult> {
    let plan_for_exec = plan.clone();
    let _plan_for_finalize = plan.clone(); // Reserved for future phase integration

    setup_environment_effect(plan, project_path, dry_run).and_then_auto(move |exec_env| {
        let env_for_finalize = exec_env.clone();
        execute_plan_effect(plan_for_exec.clone(), exec_env)
            .and_then_auto(move |result| finalize_session_effect(result, env_for_finalize))
    })
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
