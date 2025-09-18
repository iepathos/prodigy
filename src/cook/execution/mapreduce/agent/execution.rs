//! Agent execution logic
//!
//! This module contains the core execution logic for agents in the MapReduce framework.
//! It handles command execution, retry logic, progress tracking, and error handling.

use super::types::{AgentHandle, AgentResult, AgentStatus};
use crate::commands::{CommandRegistry, ExecutionContext as CommandExecutionContext};
use crate::cook::execution::dlq::DeadLetterQueue;
use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::progress::{AgentProgress, EnhancedProgressTracker};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::{StepResult, WorkflowStep};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use crate::commands::attributes::AttributeValue;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, warn};

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

    /// Execute agent commands
    async fn execute_commands(
        &self,
        handle: &AgentHandle,
        item: &Value,
        env: &ExecutionEnvironment,
        context: &ExecutionContext,
    ) -> ExecutionResult<(String, Vec<String>, Vec<String>)> {
        let mut total_output = String::new();
        let all_commits = Vec::new();
        let all_files = Vec::new();

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

            // Interpolate the step
            let interpolated_step = self
                .interpolate_workflow_step(step, &interp_context)
                .await?;

            // Execute the command
            let result = self
                .execute_single_command(&interpolated_step, handle.worktree_path(), env, context)
                .await?;

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
        }

        Ok((total_output, all_commits, all_files))
    }

    /// Build interpolation context for item
    fn build_interpolation_context(&self, item: &Value, item_id: &str) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        // Add item as the main variable
        context.variables.insert("item".to_string(), item.clone());
        context.variables.insert("item_id".to_string(), Value::String(item_id.to_string()));

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

    /// Execute a single command
    async fn execute_single_command(
        &self,
        step: &WorkflowStep,
        worktree_path: &Path,
        env: &ExecutionEnvironment,
        context: &ExecutionContext,
    ) -> ExecutionResult<StepResult> {
        // Create execution context for command
        let mut exec_context = CommandExecutionContext::new(worktree_path.to_path_buf());
        exec_context.env_vars = step.env.clone();

        // Execute based on type
        let result = if let Some(command) = &step.claude {
            let mut attributes = HashMap::new();
            attributes.insert("command".to_string(), AttributeValue::String(command.clone()));

            let cmd_result = context
                .command_registry
                .execute("claude", &exec_context, attributes)
                .await;

            if !cmd_result.success {
                return Err(ExecutionError::CommandFailed(cmd_result.stderr.unwrap_or_else(|| "Command failed".to_string())));
            }
            cmd_result
        } else if let Some(command) = &step.shell {
            let mut attributes = HashMap::new();
            attributes.insert("command".to_string(), AttributeValue::String(command.clone()));

            let cmd_result = context
                .command_registry
                .execute("shell", &exec_context, attributes)
                .await;

            if !cmd_result.success {
                return Err(ExecutionError::CommandFailed(cmd_result.stderr.unwrap_or_else(|| "Command failed".to_string())));
            }
            cmd_result
        } else {
            return Err(ExecutionError::CommandFailed(
                "No command specified in step".to_string(),
            ));
        };

        Ok(StepResult {
            success: result.exit_code == Some(0),
            stdout: result.stdout.unwrap_or_default(),
            stderr: result.stderr.unwrap_or_default(),
            exit_code: result.exit_code,
        })
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
        let start_time = Instant::now();

        // Update state to executing
        {
            let mut state = handle.state.write().await;
            state.status = super::types::AgentStateStatus::Executing;
        }

        // Execute commands
        let result = self.execute_commands(handle, item, env, &context).await;

        // Create agent result
        let agent_result = match result {
            Ok((output, commits, files)) => {
                // Update state to completed
                {
                    let mut state = handle.state.write().await;
                    state.mark_completed();
                }

                AgentResult {
                    item_id: handle.item_id().to_string(),
                    status: AgentStatus::Success,
                    output: Some(output),
                    commits,
                    files_modified: files,
                    duration: start_time.elapsed(),
                    error: None,
                    worktree_path: Some(handle.worktree_path().to_path_buf()),
                    branch_name: Some(handle.config.branch_name.clone()),
                    worktree_session_id: Some(handle.worktree_session.name.clone()),
                }
            }
            Err(e) => {
                // Update state to failed
                {
                    let mut state = handle.state.write().await;
                    state.mark_failed(e.to_string());
                }

                AgentResult {
                    item_id: handle.item_id().to_string(),
                    status: AgentStatus::Failed(e.to_string()),
                    output: None,
                    commits: Vec::new(),
                    files_modified: Vec::new(),
                    duration: start_time.elapsed(),
                    error: Some(e.to_string()),
                    worktree_path: Some(handle.worktree_path().to_path_buf()),
                    branch_name: Some(handle.config.branch_name.clone()),
                    worktree_session_id: Some(handle.worktree_session.name.clone()),
                }
            }
        };

        Ok(agent_result)
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
                .update_agent_state(&format!("agent-{}", context.agent_index),
                    crate::cook::execution::progress::AgentState::Running {
                        step: "Executing".to_string(),
                        progress: 0.0,
                    })
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
