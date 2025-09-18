//! Agent command execution module
//!
//! Handles the execution of commands within agent contexts, including
//! retry logic, progress tracking, and error handling.

use super::{AgentContext, AgentResult, AgentStatus, MapReduceError, MapReduceResult};
use crate::commands::CommandRegistry;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::execution::errors::ErrorContext;
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::progress::{AgentState as ProgressAgentState, UpdateType};
use crate::cook::execution::variables::VariableContext;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{ErrorPolicyExecutor, StepResult, WorkflowErrorPolicy, WorkflowStep};
use crate::subprocess::SubprocessManager;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Executor for agent commands
pub struct AgentCommandExecutor {
    /// Command executor
    command_executor: Arc<ClaudeExecutor>,
    /// Subprocess manager
    subprocess_manager: Arc<SubprocessManager>,
    /// Session manager
    session_manager: Arc<dyn SessionManager>,
    /// Command registry
    command_registry: Arc<CommandRegistry>,
}

impl AgentCommandExecutor {
    /// Create a new agent command executor
    pub fn new(
        command_executor: Arc<ClaudeExecutor>,
        subprocess_manager: Arc<SubprocessManager>,
        session_manager: Arc<dyn SessionManager>,
        command_registry: Arc<CommandRegistry>,
    ) -> Self {
        Self {
            command_executor,
            subprocess_manager,
            session_manager,
            command_registry,
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
        let agent_id = &agent_context.agent_id;

        info!(
            "Agent {} executing {} commands (attempt {}/{})",
            agent_id,
            commands.len(),
            retry_attempt + 1,
            max_retries + 1
        );

        // Initialize result
        let mut result = AgentResult {
            item_id: agent_context.work_item_id.clone(),
            status: AgentStatus::Running,
            output: None,
            commits: Vec::new(),
            files_modified: Vec::new(),
            duration: Duration::from_secs(0),
            error: None,
            worktree_path: Some(env.working_dir.clone()),
            branch_name: agent_context.branch_name.clone(),
            worktree_session_id: agent_context.worktree_session_id.clone(),
        };

        // Execute commands
        match self.execute_all_steps(commands, agent_context, env, job_id).await {
            Ok(step_results) => {
                // Collect commits and files modified
                for step_result in step_results {
                    if let Some(commits) = step_result.commits {
                        result.commits.extend(commits);
                    }
                    if let Some(files) = step_result.files_modified {
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
        self.execute_agent_commands_with_retry_info(
            agent_context,
            commands,
            env,
            job_id,
            0,
            0,
        )
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
            variable_context.set(key.clone(), value.clone());
        }

        for (index, step) in steps.iter().enumerate() {
            debug!(
                "Agent {} executing step {}/{}",
                agent_context.agent_id,
                index + 1,
                steps.len()
            );

            // Create interpolation context
            let interp_context = InterpolationContext {
                variables: variable_context.clone(),
                work_item: Some(agent_context.work_item.clone()),
                environment: env.clone(),
            };

            // Execute the step
            let step_result = self
                .execute_single_step(step, &interp_context, env, job_id)
                .await?;

            // Capture output if configured
            if let Some(output) = &step_result.output {
                variable_context.set("output".to_string(), output.clone());
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
        let interpolation_engine = InterpolationEngine::new();
        let interpolated_command = interpolation_engine
            .interpolate(&step.command, context)
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to interpolate command: {}", e),
                source: None,
            })?;

        // Execute based on command type
        let result = if interpolated_command.starts_with("claude:") {
            self.execute_claude_command(&interpolated_command, env).await?
        } else if interpolated_command.starts_with("shell:") {
            self.execute_shell_command(&interpolated_command, env).await?
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
        let claude_cmd = command.strip_prefix("claude:").unwrap().trim();

        // Execute via command executor
        let result = self
            .command_executor
            .execute_claude_command(claude_cmd, &env.working_dir)
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Claude command failed: {}", e),
                source: None,
            })?;

        Ok(StepResult {
            success: result.success,
            output: result.output,
            error: result.error,
            commits: result.commits,
            files_modified: result.files_changed,
        })
    }

    /// Execute a shell command
    async fn execute_shell_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<StepResult> {
        let shell_cmd = command.strip_prefix("shell:").unwrap().trim();

        // Execute via subprocess manager
        let git = self.subprocess_manager.git();
        let result = git
            .run_in_dir(&env.working_dir, &["sh", "-c", shell_cmd])
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Shell command failed: {}", e),
                source: None,
            })?;

        Ok(StepResult {
            success: result.success,
            output: Some(result.stdout),
            error: if result.success {
                None
            } else {
                Some(result.stderr)
            },
            commits: None,
            files_modified: None,
        })
    }
}