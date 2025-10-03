//! Command execution module for workflow executor
//!
//! This module contains I/O-bound command execution logic extracted from WorkflowExecutor.
//! It provides clean separation between pure logic (in pure.rs) and I/O operations.
//!
//! ## Architecture
//!
//! - **Claude Commands**: Delegated to ClaudeExecutor trait
//! - **Shell Commands**: Direct tokio::process execution
//! - **Test Commands**: Retry logic with validation
//! - **Handler Commands**: Registry-based modular handlers
//! - **Goal-Seek Commands**: Delegated to goal_seek module (already well-separated)
//! - **Foreach Commands**: Delegated to foreach module (already well-separated)
//!
//! ## Design Principles
//!
//! 1. **I/O at Boundaries**: All async I/O operations contained here
//! 2. **Clear Interfaces**: Simple request/response data structures
//! 3. **Minimal Dependencies**: Only essential dependencies on WorkflowExecutor state
//! 4. **Testability**: Execution logic can be tested with mocks

use crate::cook::execution::{ClaudeExecutor, ExecutionResult};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::StepResult;

// ============================================================================
// Claude Command Execution
// ============================================================================

/// Execute a Claude CLI command
///
/// This is a thin wrapper around ClaudeExecutor that converts the result
/// to our internal StepResult format.
pub async fn execute_claude_command(
    claude_executor: &Arc<dyn ClaudeExecutor>,
    command: &str,
    working_dir: &Path,
    env_vars: HashMap<String, String>,
) -> Result<StepResult> {
    let result = claude_executor
        .execute_claude_command(command, working_dir, env_vars)
        .await
        .with_context(|| {
            format!(
                "Claude command execution failed for command: '{}' in directory: {}",
                command,
                working_dir.display()
            )
        })?;

    Ok(convert_execution_result(result))
}

/// Convert ExecutionResult to StepResult
fn convert_execution_result(result: ExecutionResult) -> StepResult {
    StepResult {
        success: result.success,
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
    }
}

// ============================================================================
// Shell Command Execution
// ============================================================================

/// Execute a shell command with optional timeout
///
/// Uses tokio::process for async shell execution. Commands are run via
/// `sh -c` on Unix-like systems.
pub async fn execute_shell_command(
    command: &str,
    working_dir: &Path,
    env_vars: HashMap<String, String>,
    timeout: Option<u64>,
) -> Result<StepResult> {
    use tokio::process::Command;
    use tokio::time::{timeout as tokio_timeout, Duration};

    // Log shell command execution details
    tracing::info!("Executing shell command: {}", command);
    tracing::info!("Working directory: {}", working_dir.display());
    if !env_vars.is_empty() {
        tracing::debug!("  With {} environment variables set", env_vars.len());
    }

    // Create command (Unix-like systems only)
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command]);

    // Set working directory
    cmd.current_dir(working_dir);

    // Set environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    // Execute with optional timeout
    let output = if let Some(timeout_secs) = timeout {
        let duration = Duration::from_secs(timeout_secs);
        match tokio_timeout(duration, cmd.output()).await {
            Ok(result) => result?,
            Err(_) => {
                return Ok(StepResult {
                    success: false,
                    exit_code: Some(-1),
                    stdout: String::new(),
                    stderr: format!("Command timed out after {timeout_secs} seconds"),
                });
            }
        }
    } else {
        cmd.output().await?
    };

    Ok(StepResult {
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

// ============================================================================
// Goal-Seek Command Execution
// ============================================================================

/// Execute a goal-seek command (delegates to goal_seek module)
///
/// Goal-seeking is already well-separated into its own module with
/// GoalSeekEngine. This function provides the bridge between workflow
/// execution and the goal-seek subsystem.
pub async fn execute_goal_seek_command(
    goal_seek_config: crate::cook::goal_seek::GoalSeekConfig,
) -> Result<StepResult> {
    use crate::cook::goal_seek::{shell_executor::ShellCommandExecutor, GoalSeekEngine};

    // Create shell command executor for goal-seeking
    let executor = Box::new(ShellCommandExecutor::new());

    // Create goal-seek engine
    let mut engine = GoalSeekEngine::new(executor);

    // Execute goal-seeking
    let result = engine.seek(goal_seek_config.clone()).await?;

    // Convert result to StepResult
    match result {
        crate::cook::goal_seek::GoalSeekResult::Success {
            attempts,
            final_score,
            execution_time: _,
        } => Ok(StepResult {
            success: true,
            stdout: format!(
                "Goal '{}' achieved in {} attempts with score {}%",
                goal_seek_config.goal, attempts, final_score
            ),
            stderr: String::new(),
            exit_code: Some(0),
        }),
        crate::cook::goal_seek::GoalSeekResult::MaxAttemptsReached {
            attempts,
            best_score,
            last_output: _,
        } => {
            if goal_seek_config.fail_on_incomplete.unwrap_or(false) {
                Err(anyhow::anyhow!(
                    "Goal '{}' not achieved after {} attempts. Best score: {}%",
                    goal_seek_config.goal,
                    attempts,
                    best_score
                ))
            } else {
                Ok(StepResult {
                    success: false,
                    stdout: format!(
                        "Goal '{}' not achieved after {} attempts. Best score: {}%",
                        goal_seek_config.goal, attempts, best_score
                    ),
                    stderr: String::new(),
                    exit_code: Some(1),
                })
            }
        }
        crate::cook::goal_seek::GoalSeekResult::Timeout {
            attempts,
            best_score,
            elapsed,
        } => Err(anyhow::anyhow!(
            "Goal '{}' timed out after {} attempts and {:?}. Best score: {}%",
            goal_seek_config.goal,
            attempts,
            elapsed,
            best_score
        )),
        crate::cook::goal_seek::GoalSeekResult::Converged {
            attempts,
            final_score,
            reason,
        } => {
            if goal_seek_config.fail_on_incomplete.unwrap_or(false)
                && final_score < goal_seek_config.threshold
            {
                Err(anyhow::anyhow!(
                    "Goal '{}' converged after {} attempts but didn't reach threshold. Score: {}%, Reason: {}",
                    goal_seek_config.goal, attempts, final_score, reason
                ))
            } else {
                Ok(StepResult {
                    success: final_score >= goal_seek_config.threshold,
                    stdout: format!(
                        "Goal '{}' converged after {} attempts. Score: {}%, Reason: {}",
                        goal_seek_config.goal, attempts, final_score, reason
                    ),
                    stderr: String::new(),
                    exit_code: Some(if final_score >= goal_seek_config.threshold {
                        0
                    } else {
                        1
                    }),
                })
            }
        }
        crate::cook::goal_seek::GoalSeekResult::Failed { attempts, error } => Err(anyhow::anyhow!(
            "Goal '{}' failed after {} attempts: {}",
            goal_seek_config.goal,
            attempts,
            error
        )),
    }
}

// ============================================================================
// Foreach Command Execution
// ============================================================================

/// Execute a foreach command (delegates to foreach module)
///
/// Foreach execution is already well-separated into its own module.
/// This function provides the bridge between workflow execution and
/// the foreach subsystem for parallel iteration.
pub async fn execute_foreach_command(
    foreach_config: crate::config::command::ForeachConfig,
) -> Result<StepResult> {
    use crate::cook::execution::foreach::execute_foreach;

    let result = execute_foreach(&foreach_config).await?;

    // Return aggregated results
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
    })
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Build command description for logging/dry-run
#[allow(dead_code)] // Will be used in future refactoring phases
pub fn format_command_description(command_type: &super::CommandType) -> String {
    match command_type {
        super::CommandType::Claude(cmd) | super::CommandType::Legacy(cmd) => {
            format!("claude: {}", cmd)
        }
        super::CommandType::Shell(cmd) => format!("shell: {}", cmd),
        super::CommandType::Test(cmd) => format!("test: {}", cmd.command),
        super::CommandType::Handler { handler_name, .. } => {
            format!("handler: {}", handler_name)
        }
        super::CommandType::GoalSeek(cfg) => format!("goal_seek: {}", cfg.goal),
        super::CommandType::Foreach(cfg) => format!("foreach: {:?}", cfg.input),
    }
}

// ============================================================================
// Tests
// ============================================================================

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
        };

        let step_result = convert_execution_result(exec_result);
        assert!(step_result.success);
        assert_eq!(step_result.exit_code, Some(0));
        assert_eq!(step_result.stdout, "output");
    }

    #[test]
    fn test_convert_execution_result_failure() {
        let exec_result = ExecutionResult {
            success: false,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "error".to_string(),
        };

        let step_result = convert_execution_result(exec_result);
        assert!(!step_result.success);
        assert_eq!(step_result.exit_code, Some(1));
        assert_eq!(step_result.stderr, "error");
    }

    #[test]
    fn test_format_command_description_claude() {
        let cmd = super::super::CommandType::Claude("test command".to_string());
        assert_eq!(format_command_description(&cmd), "claude: test command");
    }

    #[test]
    fn test_format_command_description_shell() {
        let cmd = super::super::CommandType::Shell("ls -la".to_string());
        assert_eq!(format_command_description(&cmd), "shell: ls -la");
    }

    #[tokio::test]
    async fn test_execute_shell_command_success() {
        let result = execute_shell_command(
            "echo 'test'",
            std::path::Path::new("/tmp"),
            HashMap::new(),
            None,
        )
        .await;

        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(step_result.success);
        assert!(step_result.stdout.contains("test"));
    }

    #[tokio::test]
    async fn test_execute_shell_command_failure() {
        let result =
            execute_shell_command("exit 1", std::path::Path::new("/tmp"), HashMap::new(), None)
                .await;

        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(!step_result.success);
        assert_eq!(step_result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_execute_shell_command_timeout() {
        let result = execute_shell_command(
            "sleep 10",
            std::path::Path::new("/tmp"),
            HashMap::new(),
            Some(1), // 1 second timeout
        )
        .await;

        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(!step_result.success);
        assert!(step_result.stderr.contains("timed out"));
    }
}
