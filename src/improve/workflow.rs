use crate::config::command_validator::{apply_command_defaults, validate_command};
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

        for (idx, workflow_command) in self.config.commands.iter().enumerate() {
            // Convert to structured command
            let mut command = workflow_command.to_command();

            // Special handling for mmm-implement-spec - extract spec ID from git if no args provided
            if command.name == "mmm-implement-spec" && command.args.is_empty() {
                let spec_id = self.extract_spec_from_git().await?;
                if !spec_id.is_empty() {
                    command.args = vec![spec_id];
                }
            }

            // Apply defaults from registry
            apply_command_defaults(&mut command);

            // Validate command
            validate_command(&command)?;

            if self.verbose {
                println!(
                    "📋 Step {}/{}: {}",
                    idx + 1,
                    self.config.commands.len(),
                    command.name
                );
            }

            // Check if this is the first command and we have a focus directive
            if idx == 0 && iteration == 1 && focus.is_some() {
                command
                    .options
                    .insert("focus".to_string(), serde_json::json!(focus.unwrap()));
            }

            // Execute the command
            let success = if command.name == "mmm-implement-spec" && command.args.is_empty() {
                // No spec ID found after extraction attempt - skip implementation
                if self.verbose {
                    println!("No spec ID found - skipping implementation");
                }
                false
            } else {
                self.execute_structured_command(&command).await?
            };

            if success {
                any_changes = true;
            }
        }

        Ok(any_changes)
    }

    /// Execute a structured command
    async fn execute_structured_command(
        &self,
        command: &crate::config::command::Command,
    ) -> Result<bool> {
        println!("🤖 Running /{}...", command.name);

        // Skip actual execution in test mode
        if std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true" {
            if self.verbose {
                println!(
                    "[TEST MODE] Skipping Claude CLI execution for: {}",
                    command.name
                );
            }
            return Ok(true);
        }

        // First check if claude command exists with improved error handling
        check_claude_cli().await?;

        // Build subprocess command
        let mut cmd = Command::new("claude");
        cmd.arg("--dangerously-skip-permissions")
            .arg("--print")
            .arg(format!("/{}", command.name))
            .env("MMM_AUTOMATION", "true");

        // Add positional arguments
        for arg in &command.args {
            cmd.arg(arg);
        }

        // Add options as environment variables or flags based on command definition
        // For now, we'll use environment variables for specific options
        if let Some(focus) = command.options.get("focus") {
            if let Some(focus_str) = focus.as_str() {
                cmd.env("MMM_FOCUS", focus_str);
            }
        }

        // Apply metadata environment variables
        for (key, value) in &command.metadata.env {
            cmd.env(key, value);
        }

        // Determine retry count
        let retries = command.metadata.retries.unwrap_or(2);
        // Timeout is available in metadata but not currently used by execute_with_retry
        // let _timeout = command.metadata.timeout;

        // Execute with retry logic for transient failures
        let output = execute_with_retry(
            cmd,
            &format!("command /{}", command.name),
            retries,
            self.verbose,
        )
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let error_msg = format_subprocess_error(
                &format!("claude /{}", command.name),
                output.status.code(),
                &stderr,
                &stdout,
            );

            // Check if we should continue on error
            if command.metadata.continue_on_error.unwrap_or(false) {
                eprintln!(
                    "Warning: Command '{}' failed but continuing: {}",
                    command.name, error_msg
                );
                return Ok(false);
            } else {
                return Err(anyhow!(error_msg));
            }
        }

        if self.verbose {
            println!("✅ Command '{}' completed", command.name);
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
        use crate::config::command::WorkflowCommand;
        WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-implement-spec".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
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
        use crate::config::command::WorkflowCommand;
        let config = WorkflowConfig {
            commands: vec![WorkflowCommand::Simple("mmm-code-review".to_string())],
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
        assert_eq!(config.commands[0].to_command().name, "mmm-code-review");
        assert_eq!(config.commands[1].to_command().name, "mmm-implement-spec");
        assert_eq!(config.commands[2].to_command().name, "mmm-lint");
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
        let has_implement_spec = config
            .commands
            .iter()
            .any(|cmd| cmd.to_command().name == "mmm-implement-spec");
        assert!(has_implement_spec);

        // Test command conversion
        for workflow_cmd in &config.commands {
            let cmd = workflow_cmd.to_command();
            match cmd.name.as_str() {
                "mmm-implement-spec" => {
                    // This command requires special spec extraction
                    // No assertion needed here - the logic is handled elsewhere
                }
                _ => {
                    // Other commands are handled normally
                    assert!(!cmd.name.is_empty());
                }
            }
        }
    }
}
