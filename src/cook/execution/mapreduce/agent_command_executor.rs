//! Agent command execution module
//!
//! Handles the execution of commands within agent contexts, including
//! retry logic, progress tracking, and error handling.

use super::{AgentContext, AgentResult, AgentStatus, MapReduceError, MapReduceResult};
use crate::commands::CommandRegistry;
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::variables::VariableContext;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::WorkflowStep;
use crate::subprocess::SubprocessManager;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Result from executing a workflow step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub success: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub commits: Option<Vec<String>>,
    pub files_changed: Option<Vec<String>>,
}

/// Executor for agent commands
pub struct AgentCommandExecutor {
    /// Command executor
    command_executor: Arc<dyn ClaudeExecutor>,
    /// Subprocess manager
    subprocess_manager: Arc<SubprocessManager>,
    /// Session manager (unused for now)
    _session_manager: Arc<dyn SessionManager>,
    /// Command registry (unused for now)
    _command_registry: Arc<CommandRegistry>,
}

impl AgentCommandExecutor {
    /// Create a new agent command executor
    pub fn new(
        command_executor: Arc<dyn ClaudeExecutor>,
        subprocess_manager: Arc<SubprocessManager>,
        session_manager: Arc<dyn SessionManager>,
        command_registry: Arc<CommandRegistry>,
    ) -> Self {
        Self {
            command_executor,
            subprocess_manager,
            _session_manager: session_manager,
            _command_registry: command_registry,
        }
    }

    /// Execute agent commands with retry info
    pub async fn execute_agent_commands_with_retry_info(
        &self,
        agent_context: &AgentContext,
        commands: &[WorkflowStep],
        env: &ExecutionEnvironment,
        job_id: &str,
        retry_attempt: u32,
        max_retries: u32,
    ) -> MapReduceResult<AgentResult> {
        let start_time = Instant::now();
        let agent_id = &agent_context.item_id;

        info!(
            "Agent {} executing {} commands (attempt {}/{})",
            agent_id,
            commands.len(),
            retry_attempt + 1,
            max_retries + 1
        );

        // Initialize result
        let mut result = AgentResult {
            item_id: agent_context.item_id.clone(),
            status: AgentStatus::Running,
            output: None,
            commits: Vec::new(),
            files_modified: Vec::new(),
            duration: Duration::from_secs(0),
            error: None,
            worktree_path: Some(env.working_dir.to_path_buf()),
            branch_name: Some(agent_context.worktree_name.clone()),
            worktree_session_id: None,
        };

        // Execute commands
        match self
            .execute_all_steps(commands, agent_context, env, job_id)
            .await
        {
            Ok(step_results) => {
                // Collect commits and files modified
                for step_result in step_results {
                    if let Some(commits) = step_result.commits {
                        result.commits.extend(commits);
                    }
                    if let Some(files) = step_result.files_changed {
                        result.files_modified.extend(files);
                    }
                }

                result.status = AgentStatus::Success;
                result.duration = start_time.elapsed();

                info!(
                    "Agent {} completed successfully in {:?}",
                    agent_id, result.duration
                );
            }
            Err(e) => {
                let error_msg = format!("Agent {} failed: {}", agent_id, e);
                error!("{}", error_msg);

                result.status = AgentStatus::Failed(error_msg.clone());
                result.error = Some(error_msg);
                result.duration = start_time.elapsed();

                // Check if we should retry
                if retry_attempt < max_retries {
                    warn!(
                        "Agent {} will retry (attempt {}/{})",
                        agent_id,
                        retry_attempt + 2,
                        max_retries + 1
                    );
                    result.status = AgentStatus::Retrying(retry_attempt + 1);
                }
            }
        }

        Ok(result)
    }

    /// Execute agent commands without retry info
    pub async fn execute_agent_commands(
        &self,
        agent_context: &AgentContext,
        commands: &[WorkflowStep],
        env: &ExecutionEnvironment,
        job_id: &str,
    ) -> MapReduceResult<AgentResult> {
        self.execute_agent_commands_with_retry_info(agent_context, commands, env, job_id, 0, 0)
            .await
    }

    /// Execute all workflow steps
    async fn execute_all_steps(
        &self,
        steps: &[WorkflowStep],
        agent_context: &AgentContext,
        env: &ExecutionEnvironment,
        job_id: &str,
    ) -> MapReduceResult<Vec<StepResult>> {
        let mut results = Vec::new();
        let mut variable_context = VariableContext::new();

        // Initialize with agent variables
        for (key, value) in &agent_context.variables {
            variable_context.set_global(
                key.clone(),
                crate::cook::execution::variables::Variable::Static(serde_json::Value::String(
                    value.clone(),
                )),
            );
        }

        for (index, step) in steps.iter().enumerate() {
            debug!(
                "Agent {} executing step {}/{}",
                agent_context.item_id,
                index + 1,
                steps.len()
            );

            // Create interpolation context - convert variables to HashMap
            let mut vars_map = HashMap::new();
            // Add item variable for interpolation
            vars_map.insert("item".to_string(), serde_json::json!({}));

            let interp_context = InterpolationContext {
                variables: vars_map,
                parent: None,
            };

            // Execute the step
            let step_result = self
                .execute_single_step(step, &interp_context, env, job_id)
                .await?;

            // Capture output if configured
            if let Some(output) = &step_result.stdout {
                variable_context.set_global(
                    "output",
                    crate::cook::execution::variables::Variable::Static(serde_json::Value::String(
                        output.clone(),
                    )),
                );
            }

            results.push(step_result);
        }

        Ok(results)
    }

    /// Execute a single workflow step
    async fn execute_single_step(
        &self,
        step: &WorkflowStep,
        context: &InterpolationContext,
        env: &ExecutionEnvironment,
        _job_id: &str,
    ) -> MapReduceResult<StepResult> {
        // Interpolate the command
        let mut interpolation_engine = InterpolationEngine::new(false);
        let command_str = step.command.as_deref().unwrap_or("");
        let interpolated_command = interpolation_engine
            .interpolate(command_str, context)
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to interpolate command: {}", e),
                source: None,
            })?;

        // Execute based on command type
        let result = if interpolated_command.starts_with("claude:") {
            self.execute_claude_command(&interpolated_command, env)
                .await?
        } else if interpolated_command.starts_with("shell:") {
            self.execute_shell_command(&interpolated_command, env)
                .await?
        } else {
            return Err(MapReduceError::General {
                message: format!("Unknown command type: {}", interpolated_command),
                source: None,
            });
        };

        Ok(result)
    }

    /// Execute a Claude command
    async fn execute_claude_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<StepResult> {
        let claude_cmd = command
            .strip_prefix("claude:")
            .ok_or_else(|| MapReduceError::General {
                message: format!("Invalid Claude command format: {}", command),
                source: None,
            })?
            .trim();

        // Execute via command executor
        let result = self
            .command_executor
            .execute_claude_command(claude_cmd, &env.working_dir, HashMap::new())
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Claude command failed: {}", e),
                source: None,
            })?;

        Ok(StepResult {
            success: result.success,
            stdout: Some(result.stdout),
            stderr: if result.stderr.is_empty() {
                None
            } else {
                Some(result.stderr)
            },
            commits: None,
            files_changed: None,
        })
    }

    /// Execute a shell command
    async fn execute_shell_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<StepResult> {
        let shell_cmd = command
            .strip_prefix("shell:")
            .ok_or_else(|| MapReduceError::General {
                message: format!("Invalid shell command format: {}", command),
                source: None,
            })?
            .trim();

        // Execute via subprocess manager
        use crate::subprocess::ProcessCommandBuilder;
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", shell_cmd])
            .current_dir(&env.working_dir)
            .build();

        let result = self
            .subprocess_manager
            .runner()
            .run(command)
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Shell command failed: {}", e),
                source: None,
            })?;

        Ok(StepResult {
            success: result.status.success(),
            stdout: Some(result.stdout),
            stderr: if result.status.success() {
                None
            } else {
                Some(result.stderr)
            },
            commits: None,
            files_changed: None,
        })
    }
}
