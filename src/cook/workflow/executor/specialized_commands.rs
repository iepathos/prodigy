//! Specialized command execution functions
//!
//! This module contains execution logic for specialized command types:
//! - GoalSeek: Iterative goal achievement with validation
//! - Foreach: Parallel/sequential iteration over collections
//! - WriteFile: File writing with format support
//!
//! These commands were extracted from commands.rs to reduce LOC and improve
//! separation of concerns (spec 174f refactor).

use super::StepResult;
use anyhow::{anyhow, Result};
use std::path::Path;

// ============================================================================
// Goal Seek Command
// ============================================================================

/// Execute a goal-seek command
///
/// Goal-seek iteratively attempts to achieve a goal using Claude commands
/// until success, max attempts, or timeout.
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

// ============================================================================
// Foreach Command
// ============================================================================

/// Execute a foreach command
///
/// Iterates over a collection and executes nested commands for each item.
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

// ============================================================================
// Write File Command
// ============================================================================

/// Execute a write_file command
///
/// Writes content to a file with optional format validation and directory creation.
pub async fn execute_write_file_command(
    config: &crate::config::command::WriteFileConfig,
    working_dir: &Path,
) -> Result<StepResult> {
    use crate::config::command::WriteFileFormat;
    use crate::cook::error::ResultExt;
    use std::fs;

    // Security: Prevent directory traversal
    if config.path.contains("..") {
        return Err(anyhow!(
            "Invalid path: parent directory traversal not allowed"
        ));
    }

    let file_path = working_dir.join(&config.path);

    // Create parent directories if requested
    if config.create_dirs {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create dirs for {}", file_path.display()))
                .map_err(|e| anyhow::Error::msg(e.to_string()))?;
        }
    }

    // Format content based on specified format
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

    // Write content to file
    fs::write(&file_path, &content)
        .with_context(|| format!("Failed to write {}", file_path.display()))
        .map_err(|e| anyhow::Error::msg(e.to_string()))?;

    // Set file permissions on Unix
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_execute_write_file_text() {
        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "test.txt".to_string(),
            content: "hello world".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path())
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("11 bytes"));

        let written = std::fs::read_to_string(temp_dir.path().join("test.txt")).unwrap();
        assert_eq!(written, "hello world");
    }

    #[tokio::test]
    async fn test_execute_write_file_json() {
        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "test.json".to_string(),
            content: r#"{"key":"value"}"#.to_string(),
            format: crate::config::command::WriteFileFormat::Json,
            mode: "644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path())
            .await
            .unwrap();

        assert!(result.success);

        let written = std::fs::read_to_string(temp_dir.path().join("test.json")).unwrap();
        assert!(written.contains("key"));
        assert!(written.contains("value"));
    }

    #[tokio::test]
    async fn test_execute_write_file_creates_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "nested/dir/test.txt".to_string(),
            content: "content".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "644".to_string(),
            create_dirs: true,
        };

        let result = execute_write_file_command(&config, temp_dir.path())
            .await
            .unwrap();

        assert!(result.success);
        assert!(temp_dir.path().join("nested/dir/test.txt").exists());
    }

    #[tokio::test]
    async fn test_execute_write_file_rejects_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "../escape.txt".to_string(),
            content: "malicious".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("traversal"));
    }

    #[tokio::test]
    async fn test_execute_write_file_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "test.json".to_string(),
            content: "not valid json".to_string(),
            format: crate::config::command::WriteFileFormat::Json,
            mode: "644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }
}
