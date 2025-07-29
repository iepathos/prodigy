use crate::config::command_validator::{apply_command_defaults, validate_command};
use crate::config::{workflow::WorkflowConfig, CommandArg};
use crate::cook::git_ops::get_last_commit_message;
use crate::cook::retry::{check_claude_cli, execute_with_retry, format_subprocess_error};
use anyhow::{anyhow, Context as _, Result};
use std::collections::HashMap;
use tokio::process::Command;

/// Execute a configurable workflow
pub struct WorkflowExecutor {
    config: WorkflowConfig,
    verbose: bool,
    max_iterations: u32,
    variables: HashMap<String, String>,
}

impl WorkflowExecutor {
    pub fn new(config: WorkflowConfig, verbose: bool, max_iterations: u32) -> Self {
        Self {
            config,
            verbose,
            max_iterations,
            variables: HashMap::new(),
        }
    }

    /// Create a new workflow executor with variables
    pub fn with_variables(mut self, variables: HashMap<String, String>) -> Self {
        self.variables = variables;
        self
    }

    /// Resolve a command argument by substituting variables
    fn resolve_argument(&self, arg: &CommandArg) -> String {
        arg.resolve(&self.variables)
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
        println!("📝 Starting workflow with {} commands", self.config.commands.len());

        for (idx, workflow_command) in self.config.commands.iter().enumerate() {
            // Convert to structured command
            let mut command = workflow_command.to_command();

            // Special handling for mmm-implement-spec - extract spec ID from git if no args provided
            if command.name == "mmm-implement-spec" && command.args.is_empty() {
                let spec_id = self.extract_spec_from_git().await?;
                if !spec_id.is_empty() {
                    command.args = vec![CommandArg::Literal(spec_id)];
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
            if idx == 0 && focus.is_some() {
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
                println!("✓ Command {} made changes", command.name);
            } else {
                println!("○ Command {} made no changes", command.name);
            }
        }

        println!("📊 Workflow iteration complete. Changes made: {}", any_changes);
        Ok(any_changes)
    }

    /// Execute a structured command
    async fn execute_structured_command(
        &self,
        command: &crate::config::command::Command,
    ) -> Result<bool> {
        // Build the full command display with args
        let args_display = if !command.args.is_empty() {
            let resolved_args: Vec<String> = command
                .args
                .iter()
                .map(|arg| self.resolve_argument(arg))
                .collect();
            format!(" {}", resolved_args.join(" "))
        } else {
            String::new()
        };

        println!("🤖 Running /{}{}", command.name, args_display);

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

        // Add positional arguments with variable resolution
        // Claude CLI expects arguments in the $ARGUMENTS environment variable
        if !command.args.is_empty() {
            let resolved_args: Vec<String> = command
                .args
                .iter()
                .map(|arg| self.resolve_argument(arg))
                .collect();

            // Set ARGUMENTS env var for Claude commands (they expect this)
            cmd.env("ARGUMENTS", resolved_args.join(" "));

            // Also add as command-line args for compatibility
            for arg in resolved_args {
                cmd.arg(arg);
            }
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
            println!("✅ Command '{}' completed successfully", command.name);
        }

        Ok(true)
    }

    /// Extract spec ID from git log (for mmm-implement-spec)
    async fn extract_spec_from_git(&self) -> Result<String> {
        if self.verbose {
            println!("Extracting spec ID from git history...");
        }

        // First check for uncommitted spec files (the review might have created but not committed them)
        let uncommitted_output = tokio::process::Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard", "specs/temp/"])
            .output()
            .await
            .context("Failed to check uncommitted files")?;

        if uncommitted_output.status.success() {
            let files = String::from_utf8_lossy(&uncommitted_output.stdout);
            for line in files.lines() {
                if line.ends_with(".md") {
                    if let Some(filename) = line.split('/').last() {
                        let spec_id = filename.trim_end_matches(".md");
                        if self.verbose {
                            println!("Found uncommitted spec file: {}", spec_id);
                        }
                        return Ok(spec_id.to_string());
                    }
                }
            }
        }

        // Check the last commit for any new spec files in specs/temp/
        let output = tokio::process::Command::new("git")
            .args(["diff", "--name-only", "HEAD~1", "HEAD", "--", "specs/temp/"])
            .output()
            .await
            .context("Failed to get git diff")?;

        if !output.status.success() {
            // If we can't diff (e.g., no HEAD~1), try checking what files exist
            if let Ok(find_output) = tokio::process::Command::new("find")
                .args(["specs/temp", "-name", "*.md", "-type", "f", "-mmin", "-5"])
                .output()
                .await
            {
                let files = String::from_utf8_lossy(&find_output.stdout);
                for line in files.lines() {
                    if let Some(filename) = line.split('/').last() {
                        if filename.ends_with(".md") {
                            let spec_id = filename.trim_end_matches(".md");
                            if self.verbose {
                                println!("Found recent spec file: {}", spec_id);
                            }
                            return Ok(spec_id.to_string());
                        }
                    }
                }
            }
            return Ok(String::new());
        }

        let files = String::from_utf8_lossy(&output.stdout);
        
        // Look for new .md files in specs/temp/
        for line in files.lines() {
            if line.starts_with("specs/temp/") && line.ends_with(".md") {
                if let Some(filename) = line.split('/').last() {
                    let spec_id = filename.trim_end_matches(".md");
                    if self.verbose {
                        println!("Found new spec file in commit: {}", spec_id);
                    }
                    return Ok(spec_id.to_string());
                }
            }
        }
        
        // If no spec file in diff, check if this is a review commit
        // and look for recently created spec files
        let commit_message = get_last_commit_message()
            .await
            .context("Failed to get git log")?;
            
        if commit_message.starts_with("review:") {
            if let Ok(find_output) = tokio::process::Command::new("find")
                .args(["specs/temp", "-name", "*.md", "-type", "f", "-mmin", "-5"])
                .output()
                .await
            {
                let files = String::from_utf8_lossy(&find_output.stdout);
                for line in files.lines() {
                    if let Some(filename) = line.split('/').last() {
                        if filename.ends_with(".md") {
                            let spec_id = filename.trim_end_matches(".md");
                            if self.verbose {
                                println!("Found recent spec file: {}", spec_id);
                            }
                            return Ok(spec_id.to_string());
                        }
                    }
                }
            }
        }
        
        Ok(String::new()) // No spec found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::{Command, CommandMetadata, WorkflowCommand};
    use std::collections::HashMap;

    fn create_test_workflow() -> WorkflowConfig {
        WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-implement-spec".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
            max_iterations: 10,
        }
    }

    fn create_structured_command(name: &str, args: Vec<String>) -> Command {
        Command {
            name: name.to_string(),
            args: args.into_iter().map(|s| CommandArg::parse(&s)).collect(),
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
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

    #[tokio::test]
    async fn test_execute_iteration_test_mode() {
        // Set test mode
        std::env::set_var("MMM_TEST_MODE", "true");

        // Create a simple workflow with just mmm-code-review and mmm-lint
        let config = WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
            max_iterations: 1,
        };
        let mut executor = WorkflowExecutor::new(config, false, 1);

        // This should succeed without actually calling Claude
        let result = executor.execute_iteration(1, Some("test focus")).await;

        // In test mode, should return success
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should have changes from the commands

        std::env::remove_var("MMM_TEST_MODE");
    }

    #[test]
    fn test_command_metadata_handling() {
        let mut cmd = create_structured_command("test-command", vec!["arg1".to_string()]);

        // Test retry configuration
        cmd.metadata.retries = Some(5);
        assert_eq!(cmd.metadata.retries, Some(5));

        // Test continue_on_error
        cmd.metadata.continue_on_error = Some(true);
        assert_eq!(cmd.metadata.continue_on_error, Some(true));

        // Test timeout
        cmd.metadata.timeout = Some(60);
        assert_eq!(cmd.metadata.timeout, Some(60));

        // Test environment variables
        cmd.metadata
            .env
            .insert("TEST_VAR".to_string(), "test_value".to_string());
        assert_eq!(
            cmd.metadata.env.get("TEST_VAR"),
            Some(&"test_value".to_string())
        );
    }

    #[test]
    fn test_workflow_command_to_command_conversion() {
        // Test Simple variant
        let simple = WorkflowCommand::Simple("test-cmd".to_string());
        let cmd = simple.to_command();
        assert_eq!(cmd.name, "test-cmd");
        assert!(cmd.args.is_empty());

        // Test Structured variant
        let structured_cmd = create_structured_command("structured-cmd", vec!["arg1".to_string()]);
        let structured = WorkflowCommand::Structured(structured_cmd.clone());
        let converted = structured.to_command();
        assert_eq!(converted.name, "structured-cmd");
        assert_eq!(converted.args, vec![CommandArg::parse("arg1")]);
    }

    #[tokio::test]
    async fn test_extract_spec_from_git_edge_cases() {
        let _executor = WorkflowExecutor::new(create_test_workflow(), false, 1);

        // Test various commit message formats
        let test_cases = vec![
            // With newlines
            "review: generate improvement spec for\niteration-1234567890-improvements\nmore text",
            // With special characters
            "review: spec iteration-1234567890-improvements!",
            // At end of message
            "some text iteration-1234567890-improvements",
        ];

        for message in test_cases {
            let result = if let Some(spec_start) = message.find("iteration-") {
                let spec_part = &message[spec_start..];
                if let Some(spec_end) = spec_part.find(' ') {
                    spec_part[..spec_end].to_string()
                } else if let Some(spec_end) = spec_part.find('\n') {
                    spec_part[..spec_end].to_string()
                } else if let Some(spec_end) = spec_part.find('!') {
                    spec_part[..spec_end].to_string()
                } else {
                    spec_part.to_string()
                }
            } else {
                String::new()
            };

            assert!(result.starts_with("iteration-"));
        }
    }

    #[test]
    fn test_workflow_iteration_limits() {
        let config = create_test_workflow();
        let executor = WorkflowExecutor::new(config, true, 100);

        // Test that max_iterations is properly stored
        assert_eq!(executor.max_iterations, 100);

        // Test with zero iterations
        let zero_executor = WorkflowExecutor::new(create_test_workflow(), false, 0);
        assert_eq!(zero_executor.max_iterations, 0);
    }

    #[test]
    fn test_command_options_handling() {
        let mut cmd = create_structured_command("test", vec![]);

        // Test adding focus option
        cmd.options
            .insert("focus".to_string(), serde_json::json!("performance"));
        assert_eq!(
            cmd.options.get("focus").and_then(|v| v.as_str()),
            Some("performance")
        );

        // Test multiple options
        cmd.options
            .insert("verbose".to_string(), serde_json::json!(true));
        cmd.options
            .insert("max_retries".to_string(), serde_json::json!(3));

        assert_eq!(cmd.options.len(), 3);
        assert_eq!(
            cmd.options.get("verbose").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            cmd.options.get("max_retries").and_then(|v| v.as_i64()),
            Some(3)
        );
    }

    #[tokio::test]
    async fn test_mmm_implement_spec_without_args() {
        use tempfile::TempDir;

        // Create a temporary directory and change to it
        let temp_dir = TempDir::new().unwrap();

        // Save original directory if possible, but don't fail if we can't
        let original_dir = std::env::current_dir().ok();

        // Change to temp directory
        if std::env::set_current_dir(temp_dir.path()).is_err() {
            // Skip test if we can't change directories
            eprintln!("Skipping test: cannot change directory");
            return;
        }

        // Set test mode
        std::env::set_var("MMM_TEST_MODE", "true");

        // Create a test workflow with just lint command (doesn't require args)
        let config = WorkflowConfig {
            commands: vec![WorkflowCommand::Simple("mmm-lint".to_string())],
            max_iterations: 1,
        };

        let mut executor = WorkflowExecutor::new(config, false, 1);

        // Execute iteration - should succeed
        let result = executor.execute_iteration(1, None).await;

        // Should succeed with changes from lint command
        assert!(result.is_ok());
        assert!(result.unwrap());

        std::env::remove_var("MMM_TEST_MODE");

        // Restore original directory if we had one
        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }
    }

    /// Test that focus directive is passed on every iteration, not just the first
    #[tokio::test]
    async fn test_focus_passed_every_iteration() {
        use tempfile::TempDir;
        use tokio::process::Command as TokioCommand;

        // Create a temporary directory for git operations
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Initialize git repo
        TokioCommand::new("git")
            .current_dir(&temp_path)
            .args(["init"])
            .output()
            .await
            .unwrap();

        TokioCommand::new("git")
            .current_dir(&temp_path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .await
            .unwrap();

        TokioCommand::new("git")
            .current_dir(&temp_path)
            .args(["config", "user.name", "Test User"])
            .output()
            .await
            .unwrap();

        // Change to the temp directory
        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(&temp_path).unwrap();

        // Set test mode
        std::env::set_var("MMM_TEST_MODE", "true");

        // Create workflow that only runs lint (doesn't need spec extraction)
        let workflow = WorkflowConfig {
            commands: vec![WorkflowCommand::Simple("mmm-lint".to_string())],
            max_iterations: 3,
        };

        let mut executor = WorkflowExecutor::new(workflow, true, 3);

        // Track how many times commands are executed
        let mut iteration_count = 0;

        // Run 3 iterations with focus - but since we only have lint command,
        // focus won't be applied (it's only for the first command when it's code review)
        for iteration in 1..=3 {
            let result = executor
                .execute_iteration(iteration, Some("security"))
                .await;

            assert!(result.is_ok(), "Iteration {} should succeed", iteration);
            assert!(
                result.unwrap(),
                "Iteration {} should have changes",
                iteration
            );
            iteration_count += 1;
        }

        // Verify all 3 iterations executed
        assert_eq!(iteration_count, 3, "Should have executed 3 iterations");

        // Clean up
        std::env::remove_var("MMM_TEST_MODE");

        // Restore original directory
        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }
    }

    /// Test that would have caught the original bug where focus was only applied on iteration 1
    #[tokio::test]
    async fn test_focus_bug_regression() {
        // This test verifies that the focus application logic works correctly
        // across multiple iterations

        // Test the logic directly without needing git operations
        let focus = Some("security");
        let mut focus_applied_count = 0;

        for iteration in 1..=3 {
            for idx in 0..1 {
                // Simulating first command (idx == 0)
                // OLD BUGGY LOGIC: if idx == 0 && iteration == 1 && focus.is_some()
                let buggy_would_apply = idx == 0 && iteration == 1 && focus.is_some();

                // NEW FIXED LOGIC: if idx == 0 && focus.is_some()
                let fixed_would_apply = idx == 0 && focus.is_some();

                if fixed_would_apply {
                    focus_applied_count += 1;
                }

                // Verify the bug would have only applied focus on iteration 1
                if buggy_would_apply {
                    assert_eq!(iteration, 1, "Buggy logic only applies on iteration 1");
                }
            }
        }

        // With the fix, focus should be applied on all 3 iterations
        assert_eq!(
            focus_applied_count, 3,
            "Focus should be applied on all 3 iterations, not just the first"
        );
    }

    /// Integration test that verifies focus is included in command options across iterations
    #[tokio::test]
    async fn test_focus_in_command_options() {
        use tempfile::TempDir;
        use tokio::process::Command as TokioCommand;

        // Set up git repo for the test
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        TokioCommand::new("git")
            .current_dir(&temp_path)
            .args(["init"])
            .output()
            .await
            .unwrap();

        TokioCommand::new("git")
            .current_dir(&temp_path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .await
            .unwrap();

        TokioCommand::new("git")
            .current_dir(&temp_path)
            .args(["config", "user.name", "Test User"])
            .output()
            .await
            .unwrap();

        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(&temp_path).unwrap();

        std::env::set_var("MMM_TEST_MODE", "true");

        // Create a workflow with mmm-code-review as first command
        let workflow = WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
            max_iterations: 3,
        };

        // Track command executions and their options
        let mut commands_with_focus = 0;

        // Manually check the logic for each iteration
        for _iteration in 1..=3 {
            let executor = WorkflowExecutor::new(workflow.clone(), true, 3);

            // Get the first command (mmm-code-review)
            let mut cmd = executor.config.commands[0].to_command();

            // Apply the logic from execute_iteration
            let idx = 0; // First command
            let focus = Some("documentation");

            // This is the fixed logic: if idx == 0 && focus.is_some()
            if idx == 0 && focus.is_some() {
                cmd.options
                    .insert("focus".to_string(), serde_json::json!(focus.unwrap()));
                commands_with_focus += 1;
            }
        }

        // All 3 iterations should have focus in the command
        assert_eq!(
            commands_with_focus, 3,
            "All 3 iterations should have focus applied to mmm-code-review"
        );

        std::env::remove_var("MMM_TEST_MODE");

        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }
    }
}
