//! Agent execution logic with state machine integration
//!
//! This module contains the core execution logic for agents in the MapReduce framework.
//! It handles command execution, retry logic, progress tracking, and error handling.
//!
//! The execution logic uses the pure state machine from state_machine.rs to manage
//! agent lifecycle transitions (Created → Running → Completed/Failed).

use super::commit_validator::{CommitValidationResult, CommitValidator};
use super::state_machine::{apply_transition, state_to_result};
use super::types::{AgentHandle, AgentLifecycleState, AgentResult, AgentTransition};
use crate::abstractions::git::GitOperations;
use crate::commands::attributes::AttributeValue;
use crate::commands::{CommandRegistry, ExecutionContext as CommandExecutionContext};
use crate::cook::execution::dlq::DeadLetterQueue;
use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::progress::{AgentProgress, EnhancedProgressTracker};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::{StepResult, WorkflowStep};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// Error type for execution operations
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Command execution failed: {0}")]
    CommandFailed(String),
    #[error("Timeout occurred after {0} seconds")]
    Timeout(u64),
    #[error("Interpolation failed: {0}")]
    InterpolationError(String),
    #[error("Worktree operation failed: {0}")]
    WorktreeError(String),
    #[error("Agent execution failed: {0}")]
    AgentError(String),
    #[error("Commit validation failed for agent {0}: {0}")]
    CommitValidationFailed(Box<CommitValidationError>),
}

/// Details for commit validation failures
#[derive(Debug, Clone, thiserror::Error)]
#[error("Command '{command}' (step {step_index}) did not create required commits. Branch still at {base_commit}. Worktree: {worktree_path}")]
pub struct CommitValidationError {
    pub agent_id: String,
    pub item_id: String,
    pub step_index: usize,
    pub command: String,
    pub base_commit: String,
    pub worktree_path: String,
}

/// Result type for execution operations
pub type ExecutionResult<T> = Result<T, ExecutionError>;

/// Execution strategy for agents
#[derive(Debug, Clone, Copy)]
pub enum ExecutionStrategy {
    /// Standard execution with basic progress tracking
    Standard,
    /// Enhanced execution with detailed progress tracking
    Enhanced,
}

/// Trait for executing agent commands
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute commands for an agent
    async fn execute(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        context: ExecutionContext,
    ) -> ExecutionResult<AgentResult>;

    /// Execute with retry support
    async fn execute_with_retry(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        context: ExecutionContext,
        max_retries: u32,
    ) -> ExecutionResult<AgentResult>;
}

/// Context for agent execution
#[derive(Clone)]
pub struct ExecutionContext {
    /// Agent index in the pool
    pub agent_index: usize,
    /// Progress tracker
    pub progress_tracker: Option<Arc<AgentProgress>>,
    /// Event logger
    pub event_logger: Option<Arc<crate::cook::execution::events::EventLogger>>,
    /// Dead letter queue
    pub dlq: Option<Arc<DeadLetterQueue>>,
    /// Current retry attempt
    pub attempt: u32,
    /// Previous error if retrying
    pub previous_error: Option<String>,
    /// Execution strategy
    pub strategy: ExecutionStrategy,
    /// Command registry
    pub command_registry: Arc<CommandRegistry>,
    /// Enhanced progress tracker (for enhanced strategy)
    pub enhanced_progress: Option<Arc<EnhancedProgressTracker>>,
    /// Git operations for commit validation
    pub git_operations: Arc<dyn GitOperations>,
}

/// Standard executor implementation
pub struct StandardExecutor {
    interpolation_engine: Arc<RwLock<crate::cook::execution::interpolation::InterpolationEngine>>,
}

impl StandardExecutor {
    /// Create a new standard executor
    pub fn new() -> Self {
        Self {
            interpolation_engine: Arc::new(RwLock::new(
                crate::cook::execution::interpolation::InterpolationEngine::new(false),
            )),
        }
    }

    /// Get display name for a workflow step
    fn get_step_display_name(step: &WorkflowStep) -> String {
        if let Some(name) = &step.name {
            name.clone()
        } else if let Some(claude) = &step.claude {
            format!("claude: {}", claude)
        } else if let Some(shell) = &step.shell {
            format!("shell: {}", shell)
        } else {
            "unknown command".to_string()
        }
    }

    /// Execute agent commands
    async fn execute_commands(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        context: &ExecutionContext,
    ) -> ExecutionResult<(String, Vec<String>, Vec<String>, Option<String>)> {
        let mut total_output = String::new();
        let mut all_commits = Vec::new();
        let all_files = Vec::new();
        let mut json_log_location: Option<String> = None;

        // Create commit validator
        let commit_validator = CommitValidator::new(Arc::clone(&context.git_operations));

        // Build interpolation context
        let interp_context = self.build_interpolation_context(item, &handle.config.item_id);

        // Execute each command
        for (idx, step) in handle.commands.iter().enumerate() {
            // Update state
            {
                let mut state = handle.state.write().await;
                state.update_progress(idx + 1, handle.commands.len());
                state.set_operation(format!(
                    "Executing command {}/{}",
                    idx + 1,
                    handle.commands.len()
                ));
            }

            // COMMIT VALIDATION: Capture HEAD before command execution
            let head_before = if step.commit_required {
                Some(
                    commit_validator
                        .get_head(handle.worktree_path())
                        .await
                        .map_err(|e| {
                            ExecutionError::AgentError(format!("Failed to get HEAD before: {}", e))
                        })?,
                )
            } else {
                None
            };

            // Interpolate the step
            let interpolated_step = self
                .interpolate_workflow_step(step, &interp_context)
                .await?;

            // Execute the command
            let (result, log_location) = self
                .execute_single_command(&interpolated_step, handle.worktree_path(), env, context)
                .await?;

            // Store the log location from the last Claude command
            if log_location.is_some() {
                json_log_location = log_location;
            }

            // Collect output
            total_output.push_str(&result.stdout);
            if !result.stderr.is_empty() {
                total_output.push_str("\n[STDERR]: ");
                total_output.push_str(&result.stderr);
            }

            // Check for failure
            if !result.success {
                return Err(ExecutionError::CommandFailed(format!(
                    "Command {} failed with exit code {}",
                    idx + 1,
                    result.exit_code.unwrap_or(-1)
                )));
            }

            // COMMIT VALIDATION: Check if commits were created
            if let Some(before_sha) = head_before {
                let head_after = commit_validator
                    .get_head(handle.worktree_path())
                    .await
                    .map_err(|e| {
                        ExecutionError::AgentError(format!("Failed to get HEAD after: {}", e))
                    })?;

                let validation_result = commit_validator
                    .verify_commits_created(handle.worktree_path(), &before_sha, &head_after)
                    .await
                    .map_err(|e| {
                        ExecutionError::AgentError(format!("Commit validation failed: {}", e))
                    })?;

                match validation_result {
                    CommitValidationResult::NoCommits => {
                        // NO COMMITS CREATED - FAIL VALIDATION
                        return Err(ExecutionError::CommitValidationFailed(Box::new(
                            CommitValidationError {
                                agent_id: handle.config.id.clone(),
                                item_id: handle.config.item_id.clone(),
                                step_index: idx,
                                command: Self::get_step_display_name(step),
                                base_commit: before_sha,
                                worktree_path: handle.worktree_path().to_string_lossy().to_string(),
                            },
                        )));
                    }
                    CommitValidationResult::Valid { commits } => {
                        // Commits were created - collect metadata
                        for commit in commits {
                            all_commits.push(commit.sha.clone());
                        }

                        debug!(
                            agent_id = %handle.config.id,
                            commits = ?all_commits,
                            "Commit validation passed"
                        );
                    }
                }
            }
        }

        Ok((total_output, all_commits, all_files, json_log_location))
    }

    /// Build interpolation context for item
    fn build_interpolation_context(&self, item: &Value, item_id: &str) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        // Add item as the main variable
        context.variables.insert("item".to_string(), item.clone());
        context
            .variables
            .insert("item_id".to_string(), Value::String(item_id.to_string()));

        // If item is an object, add individual fields
        if let Some(obj) = item.as_object() {
            for (key, value) in obj {
                let key_path = format!("item.{}", key);
                context.variables.insert(key_path, value.clone());
            }
        }

        context
    }

    /// Interpolate a workflow step
    async fn interpolate_workflow_step(
        &self,
        step: &WorkflowStep,
        context: &InterpolationContext,
    ) -> ExecutionResult<WorkflowStep> {
        let mut engine = self.interpolation_engine.write().await;
        let mut interpolated = step.clone();

        // Interpolate string fields
        if let Some(name) = &step.name {
            interpolated.name = Some(
                engine
                    .interpolate(name, context)
                    .map_err(|e| ExecutionError::InterpolationError(e.to_string()))?,
            );
        }

        if let Some(claude) = &step.claude {
            interpolated.claude = Some(
                engine
                    .interpolate(claude, context)
                    .map_err(|e| ExecutionError::InterpolationError(e.to_string()))?,
            );
        }

        if let Some(shell) = &step.shell {
            interpolated.shell = Some(
                engine
                    .interpolate(shell, context)
                    .map_err(|e| ExecutionError::InterpolationError(e.to_string()))?,
            );
        }

        Ok(interpolated)
    }

    /// Create initial agent state using state machine
    fn create_initial_state(agent_id: String, work_item: Value) -> AgentLifecycleState {
        AgentLifecycleState::Created {
            agent_id,
            work_item,
        }
    }

    /// Transition to running state using state machine
    fn transition_to_running(
        state: AgentLifecycleState,
        worktree_path: PathBuf,
    ) -> Result<AgentLifecycleState, ExecutionError> {
        let transition = AgentTransition::Start { worktree_path };
        apply_transition(state, transition)
            .map_err(|e| ExecutionError::AgentError(format!("State transition failed: {}", e)))
    }

    /// Transition to completed state using state machine
    fn transition_to_completed(
        state: AgentLifecycleState,
        output: Option<String>,
        commits: Vec<String>,
    ) -> Result<AgentLifecycleState, ExecutionError> {
        let transition = AgentTransition::Complete { output, commits };
        apply_transition(state, transition)
            .map_err(|e| ExecutionError::AgentError(format!("State transition failed: {}", e)))
    }

    /// Transition to failed state using state machine
    fn transition_to_failed(
        state: AgentLifecycleState,
        error: String,
        json_log_location: Option<String>,
    ) -> Result<AgentLifecycleState, ExecutionError> {
        let transition = AgentTransition::Fail {
            error,
            json_log_location,
        };
        apply_transition(state, transition)
            .map_err(|e| ExecutionError::AgentError(format!("State transition failed: {}", e)))
    }

    /// Execute a single command
    async fn execute_single_command(
        &self,
        step: &WorkflowStep,
        worktree_path: &Path,
        _env: &ExecutionEnvironment,
        context: &ExecutionContext,
    ) -> ExecutionResult<(StepResult, Option<String>)> {
        // Create execution context for command
        let mut exec_context = CommandExecutionContext::new(worktree_path.to_path_buf());
        exec_context.env_vars = step.env.clone();

        // Execute based on type
        let result = if let Some(command) = &step.claude {
            let mut attributes = HashMap::new();
            attributes.insert(
                "command".to_string(),
                AttributeValue::String(command.clone()),
            );

            let cmd_result = context
                .command_registry
                .execute("claude", &exec_context, attributes)
                .await;

            if !cmd_result.success {
                return Err(ExecutionError::CommandFailed(
                    cmd_result
                        .stderr
                        .unwrap_or_else(|| "Command failed".to_string()),
                ));
            }
            cmd_result
        } else if let Some(command) = &step.shell {
            let mut attributes = HashMap::new();
            attributes.insert(
                "command".to_string(),
                AttributeValue::String(command.clone()),
            );

            let cmd_result = context
                .command_registry
                .execute("shell", &exec_context, attributes)
                .await;

            if !cmd_result.success {
                return Err(ExecutionError::CommandFailed(
                    cmd_result
                        .stderr
                        .unwrap_or_else(|| "Command failed".to_string()),
                ));
            }
            cmd_result
        } else {
            return Err(ExecutionError::CommandFailed(
                "No command specified in step".to_string(),
            ));
        };

        // Extract json_log_location from command result
        let json_log_location = result.json_log_location.clone();

        Ok((
            StepResult {
                success: result.exit_code == Some(0),
                stdout: result.stdout.unwrap_or_default(),
                stderr: result.stderr.unwrap_or_default(),
                exit_code: result.exit_code,
                json_log_location: None,
            },
            json_log_location,
        ))
    }
}

impl Default for StandardExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentExecutor for StandardExecutor {
    async fn execute(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        context: ExecutionContext,
    ) -> ExecutionResult<AgentResult> {
        // Create initial lifecycle state using state machine
        let lifecycle_state =
            Self::create_initial_state(handle.item_id().to_string(), item.clone());

        // Transition to running state
        let lifecycle_state =
            Self::transition_to_running(lifecycle_state, handle.worktree_path().to_path_buf())?;

        // Update legacy state for backward compatibility
        {
            let mut state = handle.state.write().await;
            state.status = super::types::AgentStateStatus::Executing;
        }

        // Execute commands
        let result = self.execute_commands(handle, item, env, &context).await;

        // Transition to final state based on result
        let lifecycle_state = match result {
            Ok((output, commits, files, json_log_location)) => {
                // Transition to completed state
                let final_state = Self::transition_to_completed(
                    lifecycle_state,
                    Some(output.clone()),
                    commits.clone(),
                )?;

                // Update legacy state for backward compatibility
                {
                    let mut state = handle.state.write().await;
                    state.mark_completed();
                }

                // Convert state machine state to AgentResult
                let mut agent_result = state_to_result(&final_state).ok_or_else(|| {
                    ExecutionError::AgentError("State conversion failed".to_string())
                })?;

                // Add additional fields not in state machine
                agent_result.files_modified = files;
                agent_result.worktree_path = Some(handle.worktree_path().to_path_buf());
                agent_result.branch_name = Some(handle.config.branch_name.clone());
                agent_result.worktree_session_id = Some(handle.worktree_session.name.clone());
                agent_result.json_log_location = json_log_location;

                agent_result
            }
            Err(e) => {
                // Transition to failed state
                let final_state = Self::transition_to_failed(
                    lifecycle_state,
                    e.to_string(),
                    None, // json_log_location not available on error
                )?;

                // Update legacy state for backward compatibility
                {
                    let mut state = handle.state.write().await;
                    state.mark_failed(e.to_string());
                }

                // Convert state machine state to AgentResult
                let mut agent_result = state_to_result(&final_state).ok_or_else(|| {
                    ExecutionError::AgentError("State conversion failed".to_string())
                })?;

                // Add additional fields not in state machine
                agent_result.worktree_path = Some(handle.worktree_path().to_path_buf());
                agent_result.branch_name = Some(handle.config.branch_name.clone());
                agent_result.worktree_session_id = Some(handle.worktree_session.name.clone());

                agent_result
            }
        };

        Ok(lifecycle_state)
    }

    async fn execute_with_retry(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        mut context: ExecutionContext,
        max_retries: u32,
    ) -> ExecutionResult<AgentResult> {
        let mut attempt = 0;
        let mut last_error = None;

        loop {
            attempt += 1;
            context.attempt = attempt;
            context.previous_error = last_error.clone();

            // Update retry state
            if attempt > 1 {
                let mut state = handle.state.write().await;
                state.mark_retrying(attempt);
            }

            // Try execution
            match self.execute(handle, item, env, context.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) if attempt <= max_retries => {
                    last_error = Some(e.to_string());
                    warn!(
                        "Agent {} attempt {} failed: {}, retrying...",
                        handle.id(),
                        attempt,
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                Err(e) => {
                    error!(
                        "Agent {} failed after {} attempts: {}",
                        handle.id(),
                        attempt,
                        e
                    );
                    return Err(e);
                }
            }
        }
    }
}

/// Enhanced progress executor implementation
pub struct EnhancedProgressExecutor {
    standard_executor: StandardExecutor,
}

impl EnhancedProgressExecutor {
    /// Create a new enhanced executor
    pub fn new() -> Self {
        Self {
            standard_executor: StandardExecutor::new(),
        }
    }
}

impl Default for EnhancedProgressExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentExecutor for EnhancedProgressExecutor {
    async fn execute(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        context: ExecutionContext,
    ) -> ExecutionResult<AgentResult> {
        // Use enhanced progress tracking if available
        if let Some(progress) = &context.enhanced_progress {
            progress
                .update_agent_state(
                    &format!("agent-{}", context.agent_index),
                    crate::cook::execution::progress::AgentState::Running {
                        step: "Executing".to_string(),
                        progress: 0.0,
                    },
                )
                .await
                .ok();
        }

        // Delegate to standard executor with progress updates
        let result = self
            .standard_executor
            .execute(handle, item, env, context.clone())
            .await;

        // Update final status
        if let Some(progress) = &context.enhanced_progress {
            let state = if result.is_ok() {
                crate::cook::execution::progress::AgentState::Completed
            } else {
                crate::cook::execution::progress::AgentState::Failed {
                    error: "Execution failed".to_string(),
                }
            };
            progress
                .update_agent_state(&format!("agent-{}", context.agent_index), state)
                .await
                .ok();
        }

        result
    }

    async fn execute_with_retry(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        context: ExecutionContext,
        max_retries: u32,
    ) -> ExecutionResult<AgentResult> {
        self.standard_executor
            .execute_with_retry(handle, item, env, context, max_retries)
            .await
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_state_machine_integration_create_to_running() {
        // Test creating initial state and transitioning to running
        let agent_id = "test-agent-1".to_string();
        let work_item = json!({"id": 1, "name": "test"});

        // Create initial state
        let state = StandardExecutor::create_initial_state(agent_id.clone(), work_item.clone());

        assert!(matches!(state, AgentLifecycleState::Created { .. }));

        // Transition to running
        let worktree_path = PathBuf::from("/tmp/test-worktree");
        let running_state = StandardExecutor::transition_to_running(state, worktree_path.clone());

        assert!(running_state.is_ok());
        let running_state = running_state.unwrap();
        assert!(matches!(running_state, AgentLifecycleState::Running { .. }));
    }

    #[test]
    fn test_state_machine_integration_running_to_completed() {
        // Test transitioning from running to completed
        let agent_id = "test-agent-2".to_string();
        let work_item = json!({"id": 2});

        // Create and transition to running
        let state = StandardExecutor::create_initial_state(agent_id, work_item);
        let running_state =
            StandardExecutor::transition_to_running(state, PathBuf::from("/tmp/test")).unwrap();

        // Transition to completed
        let output = Some("Command executed successfully".to_string());
        let commits = vec!["abc123".to_string(), "def456".to_string()];
        let completed_state = StandardExecutor::transition_to_completed(
            running_state,
            output.clone(),
            commits.clone(),
        );

        assert!(completed_state.is_ok());
        let completed_state = completed_state.unwrap();
        assert!(matches!(
            completed_state,
            AgentLifecycleState::Completed { .. }
        ));

        // Convert to result and verify
        let result = state_to_result(&completed_state);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.is_success());
        assert_eq!(result.output, output);
        assert_eq!(result.commits, commits);
    }

    #[test]
    fn test_state_machine_integration_running_to_failed() {
        // Test transitioning from running to failed
        let agent_id = "test-agent-3".to_string();
        let work_item = json!({"id": 3});

        // Create and transition to running
        let state = StandardExecutor::create_initial_state(agent_id, work_item);
        let running_state =
            StandardExecutor::transition_to_running(state, PathBuf::from("/tmp/test")).unwrap();

        // Transition to failed
        let error_msg = "Command execution failed".to_string();
        let json_log = Some("/tmp/logs/session-123.json".to_string());
        let failed_state = StandardExecutor::transition_to_failed(
            running_state,
            error_msg.clone(),
            json_log.clone(),
        );

        assert!(failed_state.is_ok());
        let failed_state = failed_state.unwrap();
        assert!(matches!(failed_state, AgentLifecycleState::Failed { .. }));

        // Convert to result and verify
        let result = state_to_result(&failed_state);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(!result.is_success());
        assert_eq!(result.error, Some(error_msg));
        assert_eq!(result.json_log_location, json_log);
    }

    #[test]
    fn test_state_machine_integration_invalid_transition() {
        // Test that invalid transitions are rejected
        let agent_id = "test-agent-4".to_string();
        let work_item = json!({"id": 4});

        // Create initial state
        let state = StandardExecutor::create_initial_state(agent_id, work_item);

        // Try to transition directly from Created to Completed (invalid)
        let invalid_result =
            StandardExecutor::transition_to_completed(state, Some("output".to_string()), vec![]);

        assert!(invalid_result.is_err());
        assert!(matches!(
            invalid_result.unwrap_err(),
            ExecutionError::AgentError(_)
        ));
    }

    #[test]
    fn test_state_machine_integration_full_lifecycle_success() {
        // Test complete lifecycle: Created → Running → Completed
        let agent_id = "test-agent-5".to_string();
        let work_item = json!({"id": 5, "file": "test.rs"});

        // Create
        let state = StandardExecutor::create_initial_state(agent_id.clone(), work_item);
        assert!(matches!(state, AgentLifecycleState::Created { .. }));

        // Start
        let state =
            StandardExecutor::transition_to_running(state, PathBuf::from("/tmp/worktree-5"))
                .unwrap();
        assert!(matches!(state, AgentLifecycleState::Running { .. }));

        // Complete
        let state = StandardExecutor::transition_to_completed(
            state,
            Some("All tests passed".to_string()),
            vec!["commit-1".to_string()],
        )
        .unwrap();
        assert!(matches!(state, AgentLifecycleState::Completed { .. }));

        // Verify result
        let result = state_to_result(&state).unwrap();
        assert!(result.is_success());
        assert_eq!(result.item_id, agent_id);
    }

    #[test]
    fn test_state_machine_integration_full_lifecycle_failure() {
        // Test complete lifecycle: Created → Running → Failed
        let agent_id = "test-agent-6".to_string();
        let work_item = json!({"id": 6});

        // Create
        let state = StandardExecutor::create_initial_state(agent_id.clone(), work_item);

        // Start
        let state =
            StandardExecutor::transition_to_running(state, PathBuf::from("/tmp/worktree-6"))
                .unwrap();

        // Fail
        let state = StandardExecutor::transition_to_failed(
            state,
            "Test execution failed".to_string(),
            Some("/logs/test.json".to_string()),
        )
        .unwrap();
        assert!(matches!(state, AgentLifecycleState::Failed { .. }));

        // Verify result
        let result = state_to_result(&state).unwrap();
        assert!(!result.is_success());
        assert_eq!(result.item_id, agent_id);
        assert!(result.error.is_some());
    }
}
