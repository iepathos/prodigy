//! Command execution module for MapReduce agents
//!
//! This module handles executing different types of commands (Claude, shell, write_file)
//! within agent worktrees with variable interpolation support.

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::{StepResult, WorkflowStep};
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

/// Executor for workflow commands in agent worktrees
#[derive(Clone)]
pub struct CommandExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    subprocess: Arc<SubprocessManager>,
}

impl CommandExecutor {
    /// Create a new command executor
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        subprocess: Arc<SubprocessManager>,
    ) -> Self {
        Self {
            claude_executor,
            subprocess,
        }
    }

    /// Get a displayable name for a workflow step
    pub fn get_step_display_name(step: &WorkflowStep) -> String {
        if let Some(claude_cmd) = &step.claude {
            format!("claude: {}", claude_cmd)
        } else if let Some(shell_cmd) = &step.shell {
            // Truncate long shell commands for readability
            if shell_cmd.len() > 60 {
                format!("shell: {}...", &shell_cmd[..57])
            } else {
                format!("shell: {}", shell_cmd)
            }
        } else if let Some(write_file) = &step.write_file {
            format!("write_file: {}", write_file.path)
        } else {
            "unknown step".to_string()
        }
    }

    /// Execute a setup step (no variable interpolation)
    pub async fn execute_setup_step(
        &self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
    ) -> MapReduceResult<StepResult> {
        if let Some(shell_cmd) = &step.shell {
            info!("Executing shell command: {}", shell_cmd);
            info!("Working directory: {}", env.working_dir.display());

            let command = ProcessCommandBuilder::new("sh")
                .args(["-c", shell_cmd])
                .current_dir(&env.working_dir)
                .envs(env_vars)
                .build();

            let output = self.subprocess.runner().run(command).await.map_err(|e| {
                MapReduceError::ProcessingError(format!("Shell command failed: {}", e))
            })?;

            let exit_code = match output.status {
                crate::subprocess::runner::ExitStatus::Success => 0,
                crate::subprocess::runner::ExitStatus::Error(code) => code,
                crate::subprocess::runner::ExitStatus::Timeout => -1,
                crate::subprocess::runner::ExitStatus::Signal(sig) => -sig,
            };

            Ok(StepResult {
                success: exit_code == 0,
                exit_code: Some(exit_code),
                stdout: output.stdout,
                stderr: output.stderr,
                json_log_location: None,
            })
        } else if let Some(claude_cmd) = &step.claude {
            info!("Executing Claude command: {}", claude_cmd);

            let result = self
                .claude_executor
                .execute_claude_command(claude_cmd, &env.working_dir, env_vars)
                .await?;

            let json_log_location = result.json_log_location().map(|s| s.to_string());

            Ok(StepResult {
                success: result.success,
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                json_log_location,
            })
        } else if let Some(write_file_cfg) = &step.write_file {
            info!("Executing write_file command: {}", write_file_cfg.path);

            let result =
                crate::cook::workflow::execute_write_file_command(write_file_cfg, &env.working_dir)
                    .await
                    .map_err(|e| {
                        MapReduceError::ProcessingError(format!("Write file command failed: {}", e))
                    })?;

            Ok(StepResult {
                success: result.success,
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                json_log_location: result.json_log_location,
            })
        } else {
            Err(MapReduceError::InvalidConfiguration {
                reason: "Step must have either 'claude', 'shell', or 'write_file' command"
                    .to_string(),
                field: "step".to_string(),
                value: format!("{:?}", step),
            })
        }
    }

    /// Execute a step in an agent's worktree with variable interpolation
    ///
    /// # Arguments
    ///
    /// * `variables` - Limited scalar variables for environment variable export.
    ///   Excludes large data like `map.results` to prevent E2BIG errors.
    /// * `full_context` - Optional full interpolation context including large
    ///   variables. Used for write_file commands to enable `${map.results}`.
    ///   If None, falls back to building context from `variables` HashMap.
    ///
    /// # Variable Context Strategy
    ///
    /// - **Shell/Claude commands**: Use `variables` HashMap → converted to env vars
    /// - **write_file commands**: Use `full_context` if provided for interpolation
    /// - **Fallback**: If no `full_context`, build from `variables` (map phase)
    pub async fn execute_step_in_worktree(
        &self,
        worktree_path: &Path,
        step: &WorkflowStep,
        variables: &HashMap<String, String>,
        full_context: Option<&InterpolationContext>,
    ) -> MapReduceResult<StepResult> {
        let mut engine = InterpolationEngine::default();

        // Build interpolation context with priority fallback
        let interp_context = if let Some(full_ctx) = full_context {
            full_ctx.clone()
        } else {
            Self::build_context_from_variables(variables)
        };

        // Execute based on step type
        if let Some(claude_cmd) = &step.claude {
            self.execute_claude_in_worktree(claude_cmd, worktree_path, &mut engine, &interp_context)
                .await
        } else if let Some(shell_cmd) = &step.shell {
            self.execute_shell_in_worktree(
                shell_cmd,
                worktree_path,
                variables,
                &mut engine,
                &interp_context,
            )
            .await
        } else if let Some(write_file_cfg) = &step.write_file {
            self.execute_write_file_in_worktree(
                write_file_cfg,
                worktree_path,
                &mut engine,
                &interp_context,
            )
            .await
        } else {
            Err(MapReduceError::InvalidConfiguration {
                reason: "Step must have either 'claude', 'shell', or 'write_file' command"
                    .to_string(),
                field: "step".to_string(),
                value: format!("{:?}", step),
            })
        }
    }

    /// Build interpolation context from variables HashMap
    fn build_context_from_variables(variables: &HashMap<String, String>) -> InterpolationContext {
        let mut ctx = InterpolationContext::new();
        let mut item_obj = serde_json::Map::new();
        let mut other_vars = serde_json::Map::new();

        for (key, value) in variables {
            if let Some(item_field) = key.strip_prefix("item.") {
                item_obj.insert(
                    item_field.to_string(),
                    serde_json::Value::String(value.clone()),
                );
            } else {
                other_vars.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }

        if !item_obj.is_empty() {
            ctx.set("item", serde_json::Value::Object(item_obj));
        }

        for (key, value) in other_vars {
            ctx.set(key, value);
        }

        ctx
    }

    /// Execute Claude command in worktree
    async fn execute_claude_in_worktree(
        &self,
        claude_cmd: &str,
        worktree_path: &Path,
        engine: &mut InterpolationEngine,
        context: &InterpolationContext,
    ) -> MapReduceResult<StepResult> {
        let interpolated_cmd = engine.interpolate(claude_cmd, context).map_err(|e| {
            MapReduceError::ProcessingError(format!("Variable interpolation failed: {}", e))
        })?;

        info!("Executing Claude command in worktree: {}", interpolated_cmd);

        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "false".to_string());

        let result = self
            .claude_executor
            .execute_claude_command(&interpolated_cmd, worktree_path, env_vars)
            .await
            .map_err(|e| {
                MapReduceError::ProcessingError(format!("Failed to execute Claude command: {}", e))
            })?;

        let json_log_location = result.json_log_location().map(|s| s.to_string());

        Ok(StepResult {
            success: result.success,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
            json_log_location,
        })
    }

    /// Execute shell command in worktree
    async fn execute_shell_in_worktree(
        &self,
        shell_cmd: &str,
        worktree_path: &Path,
        variables: &HashMap<String, String>,
        engine: &mut InterpolationEngine,
        context: &InterpolationContext,
    ) -> MapReduceResult<StepResult> {
        debug!("Interpolating shell command: {}", shell_cmd);
        debug!("Context variables: {:?}", context);

        let interpolated_cmd = engine.interpolate(shell_cmd, context).map_err(|e| {
            MapReduceError::ProcessingError(format!("Variable interpolation failed: {}", e))
        })?;

        info!("Executing shell command in worktree: {}", interpolated_cmd);

        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", &interpolated_cmd])
            .current_dir(worktree_path)
            .envs(variables.clone())
            .build();

        let output = self.subprocess.runner().run(command).await.map_err(|e| {
            MapReduceError::ProcessingError(format!("Failed to execute shell command: {}", e))
        })?;

        let exit_code = match output.status {
            crate::subprocess::runner::ExitStatus::Success => 0,
            crate::subprocess::runner::ExitStatus::Error(code) => code,
            crate::subprocess::runner::ExitStatus::Timeout => -1,
            crate::subprocess::runner::ExitStatus::Signal(sig) => -sig,
        };

        Ok(StepResult {
            success: exit_code == 0,
            exit_code: Some(exit_code),
            stdout: output.stdout,
            stderr: output.stderr,
            json_log_location: None,
        })
    }

    /// Execute write_file command in worktree
    async fn execute_write_file_in_worktree(
        &self,
        write_file_cfg: &crate::config::command::WriteFileConfig,
        worktree_path: &Path,
        engine: &mut InterpolationEngine,
        context: &InterpolationContext,
    ) -> MapReduceResult<StepResult> {
        let interpolated_path = engine
            .interpolate(&write_file_cfg.path, context)
            .map_err(|e| {
                MapReduceError::ProcessingError(format!(
                    "Variable interpolation failed for path: {}",
                    e
                ))
            })?;

        let interpolated_content = engine
            .interpolate(&write_file_cfg.content, context)
            .map_err(|e| {
                MapReduceError::ProcessingError(format!(
                    "Variable interpolation failed for content: {}",
                    e
                ))
            })?;

        let interpolated_cfg = crate::config::command::WriteFileConfig {
            path: interpolated_path,
            content: interpolated_content,
            format: write_file_cfg.format.clone(),
            create_dirs: write_file_cfg.create_dirs,
            mode: write_file_cfg.mode.clone(),
        };

        info!(
            "Executing write_file command in worktree: {}",
            interpolated_cfg.path
        );

        let result =
            crate::cook::workflow::execute_write_file_command(&interpolated_cfg, worktree_path)
                .await
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!(
                        "Failed to execute write_file command: {}",
                        e
                    ))
                })?;

        Ok(StepResult {
            success: result.success,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
            json_log_location: result.json_log_location,
        })
    }
}
