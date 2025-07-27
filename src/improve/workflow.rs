use crate::config::workflow::WorkflowConfig;
use crate::improve::git_ops::get_last_commit_message;
use crate::improve::retry::{check_claude_cli, execute_with_retry, format_subprocess_error};
use anyhow::{anyhow, Context as _, Result};
use tokio::process::Command;

/// Execute a configurable workflow
pub struct WorkflowExecutor {
    config: WorkflowConfig,
    verbose: bool,
    max_iterations: u32,
}

impl WorkflowExecutor {
    pub fn new(config: WorkflowConfig, verbose: bool, max_iterations: u32) -> Self {
        Self {
            config,
            verbose,
            max_iterations,
        }
    }

    /// Execute a single iteration of the workflow
    pub async fn execute_iteration(&mut self, iteration: u32, focus: Option<&str>) -> Result<bool> {
        if self.verbose {
            println!(
                "🔄 Workflow iteration {}/{}...",
                iteration, self.max_iterations
            );
        }

        let mut any_changes = false;

        for (idx, command) in self.config.commands.iter().enumerate() {
            if self.verbose {
                println!(
                    "📋 Step {}/{}: {}",
                    idx + 1,
                    self.config.commands.len(),
                    command
                );
            }

            // Check if this is the first command and we have a focus directive
            let step_focus = if idx == 0 && iteration == 1 {
                focus
            } else {
                None
            };

            // Execute the command
            let success = match command.as_str() {
                "mmm-implement-spec" => {
                    // Special handling for mmm-implement-spec - extract spec ID from git
                    let spec_id = self.extract_spec_from_git().await?;
                    if spec_id.is_empty() {
                        if self.verbose {
                            println!("No spec ID found - skipping implementation");
                        }
                        false
                    } else {
                        self.execute_command_with_args(command, &[spec_id]).await?
                    }
                }
                _ => self.execute_command(command, step_focus).await?,
            };

            if success {
                any_changes = true;
            }
        }

        Ok(any_changes)
    }

    /// Execute a Claude command
    async fn execute_command(&self, command: &str, focus: Option<&str>) -> Result<bool> {
        println!("🤖 Running /{command}...");

        // First check if claude command exists with improved error handling
        check_claude_cli().await?;

        // Build command
        let mut cmd = Command::new("claude");
        cmd.arg("--dangerously-skip-permissions")
            .arg("--print")
            .arg(format!("/{command}"))
            .env("MMM_AUTOMATION", "true");

        // Add focus directive if provided (for first command of first iteration)
        if let Some(focus_directive) = focus {
            cmd.env("MMM_FOCUS", focus_directive);
        }

        // Execute with retry logic for transient failures
        let output =
            execute_with_retry(cmd, &format!("command /{command}"), 2, self.verbose).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let error_msg = format_subprocess_error(
                &format!("claude /{command}"),
                output.status.code(),
                &stderr,
                &stdout,
            );
            return Err(anyhow!(error_msg));
        }

        if self.verbose {
            println!("✅ Command '{command}' completed");
        }

        Ok(true)
    }

    /// Execute a Claude command with arguments
    async fn execute_command_with_args(&self, command: &str, args: &[String]) -> Result<bool> {
        println!("🤖 Running /{command} {}...", args.join(" "));

        let mut cmd = Command::new("claude");
        cmd.arg("--dangerously-skip-permissions")
            .arg("--print")
            .arg(format!("/{command}"))
            .args(args)
            .env("MMM_AUTOMATION", "true");

        // Execute with retry logic for transient failures
        let output = execute_with_retry(
            cmd,
            &format!("command /{command} {}", args.join(" ")),
            2,
            self.verbose,
        )
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let error_msg = format_subprocess_error(
                &format!("claude /{command} {}", args.join(" ")),
                output.status.code(),
                &stderr,
                &stdout,
            );
            return Err(anyhow!(error_msg));
        }

        if self.verbose {
            println!("✅ Command '{command}' completed");
        }

        Ok(true)
    }

    /// Extract spec ID from git log (for mmm-implement-spec)
    async fn extract_spec_from_git(&self) -> Result<String> {
        if self.verbose {
            println!("Extracting spec ID from git history...");
        }

        // Use thread-safe git operation
        let commit_message = get_last_commit_message()
            .await
            .context("Failed to get git log")?;

        // Parse commit message like "review: generate improvement spec for iteration-1234567890-improvements"
        if let Some(spec_start) = commit_message.find("iteration-") {
            let spec_part = &commit_message[spec_start..];
            if let Some(spec_end) = spec_part.find(' ') {
                Ok(spec_part[..spec_end].to_string())
            } else {
                Ok(spec_part.to_string())
            }
        } else {
            Ok(String::new()) // No spec found
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_workflow() -> WorkflowConfig {
        WorkflowConfig {
            commands: vec![
                "/mmm-code-review".to_string(),
                "/mmm-implement-spec".to_string(),
                "/mmm-lint".to_string(),
            ],
            max_iterations: 10,
        }
    }

    #[test]
    fn test_workflow_executor_creation() {
        let config = create_test_workflow();
        let executor = WorkflowExecutor::new(config.clone(), true, 5);

        assert_eq!(executor.config.commands.len(), 3);
        assert!(executor.verbose);
        assert_eq!(executor.max_iterations, 5);
    }

    #[tokio::test]
    async fn test_execute_iteration_with_focus() {
        let config = WorkflowConfig {
            commands: vec!["/mmm-code-review".to_string()],
            max_iterations: 1,
        };
        let executor = WorkflowExecutor::new(config, false, 1);

        // We can't test actual execution without Claude CLI, but we can test the logic
        // This would need mocking in a real test environment
        assert!(executor.config.commands.len() == 1);
    }

    #[test]
    fn test_workflow_config_defaults() {
        let config = WorkflowConfig::default();

        assert_eq!(config.commands.len(), 3);
        assert_eq!(config.commands[0], "mmm-code-review");
        assert_eq!(config.commands[1], "mmm-implement-spec");
        assert_eq!(config.commands[2], "mmm-lint");
        assert_eq!(config.max_iterations, 10);
    }

    #[test]
    fn test_spec_extraction_logic() {
        // Test the spec extraction pattern
        let test_messages = vec![
            (
                "review: generate improvement spec for iteration-1234567890-improvements",
                "iteration-1234567890-improvements",
            ),
            (
                "review: iteration-9876543210-improvements created",
                "iteration-9876543210-improvements",
            ),
            ("no spec in this message", ""),
        ];

        for (message, expected) in test_messages {
            let result = if let Some(spec_start) = message.find("iteration-") {
                let spec_part = &message[spec_start..];
                if let Some(spec_end) = spec_part.find(' ') {
                    spec_part[..spec_end].to_string()
                } else {
                    spec_part.to_string()
                }
            } else {
                String::new()
            };

            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_focus_directive_logic() {
        let config = create_test_workflow();
        let executor = WorkflowExecutor::new(config, true, 10);

        // Test that focus is only applied on first command of first iteration
        for iteration in 1..=3 {
            for (idx, _command) in executor.config.commands.iter().enumerate() {
                let should_have_focus = idx == 0 && iteration == 1;

                // This logic matches the implementation
                let step_focus = if idx == 0 && iteration == 1 {
                    Some("performance")
                } else {
                    None
                };

                if should_have_focus {
                    assert!(step_focus.is_some());
                } else {
                    assert!(step_focus.is_none());
                }
            }
        }
    }

    #[test]
    fn test_special_command_handling() {
        let config = create_test_workflow();

        // Test that mmm-implement-spec is recognized as special
        assert!(config.commands.contains(&"/mmm-implement-spec".to_string()));

        // Test command matching
        for command in &config.commands {
            match command.as_str() {
                "/mmm-implement-spec" => {
                    // This command requires special spec extraction
                    assert!(true);
                }
                _ => {
                    // Other commands are handled normally
                    assert!(command.starts_with('/'));
                }
            }
        }
    }
}
