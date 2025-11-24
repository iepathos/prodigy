//! Command execution module for workflow executor
//!
//! This module provides command execution orchestration for the WorkflowExecutor,
//! delegating to effects modules for actual I/O operations and pure modules for
//! transformations.
//!
//! ## Architecture (Post-174f Refactor)
//!
//! - **Pure Transformations**: Command building and output parsing via `pure/` module
//! - **Effect-based I/O**: Claude, shell, handler execution via `effects/` module
//! - **Orchestration Only**: This module focuses on dispatch and coordination
//!
//! ## Design Principles
//!
//! 1. **Delegation**: All I/O delegated to effect modules (174d)
//! 2. **Pure Core**: Transformations use pure functions (174b)
//! 3. **Thin Orchestration**: This module is ~300 LOC of coordination code

use crate::commands::{AttributeValue, ExecutionContext};
use crate::cook::error::ResultExt;
use crate::cook::execution::{ClaudeExecutor, ExecutionResult};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::effects::environment::{DefaultShellRunner, ShellRunner};
use crate::cook::workflow::on_failure::OnFailureConfig;
use crate::cook::workflow::pure::build_command;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::{CommandType, StepResult, WorkflowContext, WorkflowExecutor, WorkflowStep};

// ============================================================================
// Standalone Command Execution Functions (Thin Wrappers)
// ============================================================================

/// Execute a Claude CLI command
pub async fn execute_claude_command(
    claude_executor: &Arc<dyn ClaudeExecutor>,
    command: &str,
    working_dir: &Path,
    env_vars: HashMap<String, String>,
) -> Result<StepResult> {
    let result = claude_executor
        .execute_claude_command(command, working_dir, env_vars)
        .await
        .with_context(|| format!("Claude command failed: '{}'", command))
        .map_err(|e| anyhow::Error::msg(e.to_string()))?;

    Ok(convert_execution_result(result))
}

/// Convert ExecutionResult to StepResult
fn convert_execution_result(result: ExecutionResult) -> StepResult {
    let json_log_location = result.json_log_location().map(|s| s.to_string());
    StepResult {
        success: result.success,
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
        json_log_location,
    }
}

/// Execute a shell command with optional timeout
///
/// Delegates to `DefaultShellRunner` from the effects module (spec 174d).
pub async fn execute_shell_command(
    command: &str,
    working_dir: &Path,
    env_vars: HashMap<String, String>,
    timeout: Option<u64>,
) -> Result<StepResult> {
    tracing::info!("Executing shell: {}", command);
    let runner = DefaultShellRunner::new();
    let output = runner.run(command, working_dir, env_vars, timeout).await?;
    Ok(convert_runner_output_to_step_result(output))
}

/// Convert RunnerOutput from effects module to StepResult
fn convert_runner_output_to_step_result(
    output: crate::cook::workflow::effects::environment::RunnerOutput,
) -> StepResult {
    StepResult {
        success: output.success,
        exit_code: output.exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
        json_log_location: output.json_log_location,
    }
}

/// Execute a goal-seek command
pub async fn execute_goal_seek_command(
    config: crate::cook::goal_seek::GoalSeekConfig,
) -> Result<StepResult> {
    use crate::cook::goal_seek::{
        shell_executor::ShellCommandExecutor, GoalSeekEngine, GoalSeekResult,
    };

    let mut engine = GoalSeekEngine::new(Box::new(ShellCommandExecutor::new()));
    let result = engine.seek(config.clone()).await?;

    match result {
        GoalSeekResult::Success {
            attempts,
            final_score,
            ..
        } => Ok(StepResult {
            success: true,
            stdout: format!(
                "Goal '{}' achieved in {} attempts ({}%)",
                config.goal, attempts, final_score
            ),
            stderr: String::new(),
            exit_code: Some(0),
            json_log_location: None,
        }),
        GoalSeekResult::MaxAttemptsReached {
            attempts,
            best_score,
            ..
        } => {
            if config.fail_on_incomplete.unwrap_or(false) {
                Err(anyhow!(
                    "Goal '{}' not achieved after {} attempts (best: {}%)",
                    config.goal,
                    attempts,
                    best_score
                ))
            } else {
                Ok(StepResult {
                    success: false,
                    stdout: format!(
                        "Goal '{}' not achieved after {} attempts (best: {}%)",
                        config.goal, attempts, best_score
                    ),
                    stderr: String::new(),
                    exit_code: Some(1),
                    json_log_location: None,
                })
            }
        }
        GoalSeekResult::Timeout {
            attempts,
            best_score,
            elapsed,
        } => Err(anyhow!(
            "Goal '{}' timed out after {} attempts ({:?}). Best: {}%",
            config.goal,
            attempts,
            elapsed,
            best_score
        )),
        GoalSeekResult::Converged {
            attempts,
            final_score,
            reason,
        } => {
            let success = final_score >= config.threshold;
            if !success && config.fail_on_incomplete.unwrap_or(false) {
                Err(anyhow!(
                    "Goal '{}' converged but didn't reach threshold ({}%). Reason: {}",
                    config.goal,
                    final_score,
                    reason
                ))
            } else {
                Ok(StepResult {
                    success,
                    stdout: format!(
                        "Goal '{}' converged after {} attempts ({}%). Reason: {}",
                        config.goal, attempts, final_score, reason
                    ),
                    stderr: String::new(),
                    exit_code: Some(if success { 0 } else { 1 }),
                    json_log_location: None,
                })
            }
        }
        GoalSeekResult::Failed { attempts, error } => Err(anyhow!(
            "Goal '{}' failed after {} attempts: {}",
            config.goal,
            attempts,
            error
        )),
    }
}

/// Execute a foreach command
pub async fn execute_foreach_command(
    config: crate::config::command::ForeachConfig,
) -> Result<StepResult> {
    let result = crate::cook::execution::foreach::execute_foreach(&config).await?;
    Ok(StepResult {
        success: result.failed_items == 0,
        stdout: format!(
            "Foreach completed: {} total, {} successful, {} failed",
            result.total_items, result.successful_items, result.failed_items
        ),
        stderr: if result.failed_items > 0 {
            format!("{} items failed", result.failed_items)
        } else {
            String::new()
        },
        exit_code: Some(if result.failed_items == 0 { 0 } else { 1 }),
        json_log_location: None,
    })
}

/// Execute a write_file command
pub async fn execute_write_file_command(
    config: &crate::config::command::WriteFileConfig,
    working_dir: &Path,
) -> Result<StepResult> {
    use crate::config::command::WriteFileFormat;
    use crate::cook::error::ResultExt;
    use std::fs;

    if config.path.contains("..") {
        return Err(anyhow!(
            "Invalid path: parent directory traversal not allowed"
        ));
    }

    let file_path = working_dir.join(&config.path);
    if config.create_dirs {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create dirs for {}", file_path.display()))
                .map_err(|e| anyhow::Error::msg(e.to_string()))?;
        }
    }

    let content = match config.format {
        WriteFileFormat::Text => config.content.clone(),
        WriteFileFormat::Json => serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(&config.content)
                .map_err(|e| anyhow!("Invalid JSON: {}", e))?,
        )?,
        WriteFileFormat::Yaml => serde_yaml::to_string(
            &serde_yaml::from_str::<serde_yaml::Value>(&config.content)
                .map_err(|e| anyhow!("Invalid YAML: {}", e))?,
        )?,
    };

    fs::write(&file_path, &content)
        .with_context(|| format!("Failed to write {}", file_path.display()))
        .map_err(|e| anyhow::Error::msg(e.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode =
            u32::from_str_radix(&config.mode, 8).map_err(|e| anyhow!("Invalid mode: {}", e))?;
        fs::set_permissions(&file_path, fs::Permissions::from_mode(mode))?;
    }

    Ok(StepResult {
        success: true,
        exit_code: Some(0),
        stdout: format!("Wrote {} bytes to {}", content.len(), config.path),
        stderr: String::new(),
        json_log_location: None,
    })
}

/// Format command description for logging
pub fn format_command_description(command_type: &CommandType) -> String {
    match command_type {
        CommandType::Claude(cmd) | CommandType::Legacy(cmd) => format!("claude: {}", cmd),
        CommandType::Shell(cmd) => format!("shell: {}", cmd),
        CommandType::Test(cmd) => format!("test: {}", cmd.command),
        CommandType::Handler { handler_name, .. } => format!("handler: {}", handler_name),
        CommandType::GoalSeek(cfg) => format!("goal_seek: {}", cfg.goal),
        CommandType::Foreach(cfg) => format!("foreach: {:?}", cfg.input),
        CommandType::WriteFile(cfg) => format!("write_file: {}", cfg.path),
    }
}

// ============================================================================
// WorkflowExecutor Command Methods
// ============================================================================

impl WorkflowExecutor {
    /// Main command dispatcher
    pub(super) async fn execute_command_by_type(
        &mut self,
        command_type: &CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        if self.dry_run {
            return self.handle_dry_run_mode(command_type, step, env, ctx).await;
        }

        if let Some(timeout) = step.timeout {
            env_vars.insert("PRODIGY_COMMAND_TIMEOUT".to_string(), timeout.to_string());
        }

        self.dispatch_command(command_type.clone(), step, env, ctx, env_vars)
            .await
    }

    async fn dispatch_command(
        &mut self,
        command_type: CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        match command_type {
            CommandType::Claude(cmd) | CommandType::Legacy(cmd) => {
                let (interpolated, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_claude_command(&interpolated, env, env_vars)
                    .await
            }
            CommandType::Shell(cmd) => {
                let (interpolated, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_shell_for_step(&interpolated, step, env, ctx, env_vars)
                    .await
            }
            CommandType::Test(test_cmd) => {
                self.execute_test_command(test_cmd, env, ctx, env_vars, None, None)
                    .await
            }
            CommandType::Handler {
                handler_name,
                attributes,
            } => {
                self.execute_handler_command(handler_name, attributes, env, ctx, env_vars)
                    .await
            }
            CommandType::GoalSeek(config) => execute_goal_seek_command(config).await,
            CommandType::Foreach(config) => execute_foreach_command(config).await,
            CommandType::WriteFile(mut config) => {
                let (path, p_res) = ctx.interpolate_with_tracking(&config.path);
                let (content, c_res) = ctx.interpolate_with_tracking(&config.content);
                self.log_variable_resolutions(&p_res);
                self.log_variable_resolutions(&c_res);
                config.path = path;
                config.content = content;
                execute_write_file_command(&config, &env.working_dir).await
            }
        }
    }

    pub(crate) async fn execute_claude_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        execute_claude_command(&self.claude_executor, command, &env.working_dir, env_vars).await
    }

    pub(crate) async fn execute_shell_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> Result<StepResult> {
        execute_shell_command(command, &env.working_dir, env_vars, timeout).await
    }

    async fn execute_shell_for_step(
        &self,
        cmd: &str,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        if let Some(test_cmd) = &step.test {
            if test_cmd.on_failure.is_some() {
                return self
                    .execute_shell_with_retry(
                        cmd,
                        test_cmd.on_failure.as_ref(),
                        env,
                        ctx,
                        env_vars,
                        step.timeout,
                    )
                    .await;
            }
        }
        self.execute_shell_command(cmd, env, env_vars, step.timeout)
            .await
    }

    /// Execute handler command using pure interpolation from effects/handler.rs pattern
    async fn execute_handler_command(
        &self,
        handler_name: String,
        mut attributes: HashMap<String, AttributeValue>,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        let registry = self
            .command_registry
            .as_ref()
            .ok_or_else(|| anyhow!("Command registry not initialized"))?;
        let mut exec_context = ExecutionContext::new(env.working_dir.to_path_buf());
        exec_context.add_env_vars(env_vars);

        if let Some(session_id) = ctx.variables.get("SESSION_ID") {
            exec_context = exec_context.with_session_id(session_id.clone());
        }

        // Pure: interpolate attribute values using build_command from pure/ module
        for (_, value) in attributes.iter_mut() {
            if let AttributeValue::String(s) = value {
                *s = build_command(s, &ctx.variables);
            }
        }

        let result = registry
            .execute(&handler_name, &exec_context, attributes)
            .await;
        Ok(StepResult {
            success: result.is_success(),
            exit_code: result.exit_code,
            stdout: result.stdout.unwrap_or_else(|| {
                result
                    .data
                    .as_ref()
                    .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
                    .unwrap_or_default()
            }),
            stderr: result
                .stderr
                .unwrap_or_else(|| result.error.unwrap_or_default()),
            json_log_location: None,
        })
    }

    async fn handle_dry_run_mode(
        &mut self,
        command_type: &CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        let desc = format_command_description(command_type);
        println!("[DRY RUN] Would execute: {}", desc);
        self.dry_run_commands.push(desc.clone());

        if let Some(on_failure) = &step.on_failure {
            let handler_desc = match on_failure {
                OnFailureConfig::SingleCommand(cmd) => format!("on_failure: {}", cmd),
                OnFailureConfig::MultipleCommands(cmds) => {
                    format!("on_failure: {} commands", cmds.len())
                }
                _ => "on_failure: configured".to_string(),
            };
            self.dry_run_potential_handlers.push(handler_desc);
        }

        if step.commit_required {
            println!(
                "[DRY RUN] commit_required - assuming commit created by: {}",
                desc
            );
            self.assumed_commits.push(desc.clone());
        }

        if let Some(validation_config) = &step.validate {
            self.handle_validation(validation_config, env, ctx).await?;
        }

        if let Some(step_validation) = &step.step_validate {
            if !step.skip_validation {
                self.handle_step_validation(step_validation, env, ctx, step)
                    .await?;
            }
        }

        Ok(StepResult {
            success: true,
            stdout: format!("[dry-run] {}", desc),
            stderr: String::new(),
            exit_code: Some(0),
            json_log_location: None,
        })
    }

    pub(crate) fn handle_test_mode_execution(
        &self,
        step: &WorkflowStep,
        command_type: &CommandType,
    ) -> Result<StepResult> {
        let desc = format_command_description(command_type);
        println!("[TEST MODE] Would execute: {}", desc);

        let simulate_no_changes = matches!(command_type, CommandType::Claude(cmd) | CommandType::Legacy(cmd) if self.is_test_mode_no_changes_command(cmd));

        if simulate_no_changes {
            println!("[TEST MODE] Simulating no changes");
            if step.commit_required
                && std::env::var("PRODIGY_NO_COMMIT_VALIDATION").unwrap_or_default() != "true"
            {
                return Err(anyhow!(
                    "No changes were committed by {}",
                    self.get_step_display_name(step)
                ));
            }
            return Ok(StepResult {
                success: true,
                exit_code: Some(0),
                stdout: "[TEST MODE] No changes made".to_string(),
                stderr: String::new(),
                json_log_location: None,
            });
        }

        Ok(StepResult {
            success: true,
            exit_code: Some(0),
            stdout: "[TEST MODE] Command executed successfully".to_string(),
            stderr: String::new(),
            json_log_location: None,
        })
    }

    pub(super) fn json_to_attribute_value(&self, value: serde_json::Value) -> AttributeValue {
        Self::json_to_attribute_value_static(value)
    }

    pub(super) fn json_to_attribute_value_static(value: serde_json::Value) -> AttributeValue {
        match value {
            serde_json::Value::String(s) => AttributeValue::String(s),
            serde_json::Value::Number(n) => AttributeValue::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::Bool(b) => AttributeValue::Boolean(b),
            serde_json::Value::Array(arr) => AttributeValue::Array(
                arr.into_iter()
                    .map(Self::json_to_attribute_value_static)
                    .collect(),
            ),
            serde_json::Value::Object(obj) => AttributeValue::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, Self::json_to_attribute_value_static(v)))
                    .collect(),
            ),
            serde_json::Value::Null => AttributeValue::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_execution_result_success() {
        let exec_result = ExecutionResult {
            success: true,
            exit_code: Some(0),
            stdout: "output".to_string(),
            stderr: String::new(),
            metadata: HashMap::new(),
        };
        let step_result = convert_execution_result(exec_result);
        assert!(step_result.success);
        assert_eq!(step_result.exit_code, Some(0));
    }

    #[test]
    fn test_format_command_description() {
        assert_eq!(
            format_command_description(&CommandType::Claude("test".to_string())),
            "claude: test"
        );
        assert_eq!(
            format_command_description(&CommandType::Shell("ls".to_string())),
            "shell: ls"
        );
    }

    #[tokio::test]
    async fn test_execute_shell_command_success() {
        let result = execute_shell_command(
            "echo 'test'",
            std::path::Path::new("/tmp"),
            HashMap::new(),
            None,
        )
        .await
        .unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("test"));
    }

    #[tokio::test]
    async fn test_execute_shell_command_timeout() {
        let result = execute_shell_command(
            "sleep 10",
            std::path::Path::new("/tmp"),
            HashMap::new(),
            Some(1),
        )
        .await
        .unwrap();
        assert!(!result.success);
        assert!(result.stderr.contains("timed out"));
    }
}
