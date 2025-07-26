use crate::config::workflow::WorkflowConfig;
use crate::improve::git_ops::get_last_commit_message;
use crate::improve::retry::{check_claude_cli, execute_with_retry, format_subprocess_error};
use anyhow::{anyhow, Context as _, Result};
use tokio::process::Command;

/// Execute a configurable workflow
pub struct WorkflowExecutor {
    config: WorkflowConfig,
    verbose: bool,
}

impl WorkflowExecutor {
    pub fn new(config: WorkflowConfig, verbose: bool) -> Self {
        Self { config, verbose }
    }

    /// Execute a single iteration of the workflow
    pub async fn execute_iteration(&mut self, iteration: u32, focus: Option<&str>) -> Result<bool> {
        if self.verbose {
            println!(
                "🔄 Workflow iteration {}/{}...",
                iteration, self.config.max_iterations
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
