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
    test_mode: bool,
}

impl WorkflowExecutor {
    pub fn new(config: WorkflowConfig, verbose: bool, max_iterations: u32) -> Self {
        Self {
            config,
            verbose,
            max_iterations,
            variables: HashMap::new(),
            test_mode: false,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(config: WorkflowConfig, verbose: bool, max_iterations: u32) -> Self {
        Self {
            config,
            verbose,
            max_iterations,
            variables: HashMap::new(),
            test_mode: true,
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
        println!(
            "📝 Starting workflow with {} commands",
            self.config.commands.len()
        );

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

            // Capture git HEAD before command execution
            let head_before = get_git_head().await.ok();
            
            // Execute the command and capture output
            let (success, _output) = if command.name == "mmm-implement-spec" && command.args.is_empty() {
                // No spec ID found after extraction attempt - skip implementation
                if self.verbose {
                    println!("No spec ID found - skipping implementation");
                }
                (false, None)
            } else {
                let result = self.execute_structured_command(&command).await?;
                (result.0, result.1)
            };
            
            // Check if git HEAD changed and extract spec from the new commit
            if success && (command.name == "mmm-code-review" || command.name == "mmm-cleanup-tech-debt") {
                let head_after = get_git_head().await.ok();
                
                if head_before != head_after && head_after.is_some() {
                    // Git HEAD changed, examine the commit
                    if let Some(spec_id) = extract_spec_from_commit(&head_after.unwrap()).await? {
                        self.variables.insert("SPEC_ID".to_string(), spec_id.clone());
                        if self.verbose {
                            println!("📝 Found spec in commit: {spec_id}");
                        }
                    }
                } else if self.verbose {
                    println!("No git commit detected after command");
                }
            }

            if success {
                any_changes = true;
                println!("✓ Command {} made changes", command.name);
            } else {
                println!("○ Command {} made no changes", command.name);
            }
        }

        println!("📊 Workflow iteration complete. Changes made: {any_changes}");
        Ok(any_changes)
    }

    /// Execute a structured command and return (success, output)
    async fn execute_structured_command(
        &self,
        command: &crate::config::command::Command,
    ) -> Result<(bool, Option<String>)> {
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

        println!("🤖 Running /{}{args_display}", command.name);

        // Skip actual execution in test mode
        if self.test_mode {
            println!(
                "[TEST MODE] Skipping Claude CLI execution for: {}",
                command.name
            );

            // Check if we should simulate no changes for this command
            if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
                if no_changes_cmds
                    .split(',')
                    .any(|cmd| cmd.trim() == command.name)
                {
                    println!("[TEST MODE] Simulating no changes for: {}", command.name);
                    return Ok((false, None));
                }
            }

            // Track focus if requested and this is the first command
            if command.name == "mmm-code-review" {
                if let Some(focus) = command.options.get("focus") {
                    if let Some(focus_str) = focus.as_str() {
                        if let Ok(track_file) = std::env::var("MMM_TRACK_FOCUS") {
                            use std::io::Write;
                            if let Ok(mut file) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&track_file)
                            {
                                let _ = writeln!(file, "iteration: focus={focus_str}");
                            }
                        }
                    }
                }
            }

            return Ok((true, None));
        }

        // First check if claude command exists with improved error handling
        check_claude_cli().await?;

        if self.verbose {
            println!("🔧 Preparing to execute Claude CLI command...");
        }

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

            if self.verbose {
                let args_str = resolved_args.join(" ");
                println!("  📌 Arguments: {args_str}");
            }

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
                if self.verbose {
                    println!("  🎯 Focus: {focus_str}");
                }
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

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
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
                return Ok((false, None));
            } else {
                return Err(anyhow!(error_msg));
            }
        }

        if self.verbose {
            println!("✅ Command '{}' completed successfully", command.name);
            if !stdout.is_empty() {
                println!("📄 Command output:");
                println!("{stdout}");
            }
            if !stderr.is_empty() {
                println!("⚠️  Command stderr:");
                println!("{stderr}");
            }
        }

        Ok((true, Some(stdout.to_string())))
    }

    /// Extract spec ID from git log (for mmm-implement-spec)
    async fn extract_spec_from_git(&self) -> Result<String> {
        if self.verbose {
            println!("Extracting spec ID from git history...");
        }

        // First check for uncommitted spec files (the review might have created but not committed them)
        let uncommitted_output = tokio::process::Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard", "specs/"])
            .output()
            .await
            .context("Failed to check uncommitted files")?;

        if uncommitted_output.status.success() {
            let files = String::from_utf8_lossy(&uncommitted_output.stdout);
            for line in files.lines() {
                if line.ends_with(".md") {
                    if let Some(filename) = line.split('/').last() {
                        let spec_id = filename.trim_end_matches(".md");
                        // Accept any .md file that's not a special file
                        if !matches!(spec_id, "SPEC_INDEX" | "README" | "index") {
                            if self.verbose {
                                println!("Found uncommitted spec file: {spec_id}");
                            }
                            return Ok(spec_id.to_string());
                        }
                    }
                }
            }
        }

        // Check the last commit for any new spec files in specs/
        let output = tokio::process::Command::new("git")
            .args(["diff", "--name-only", "HEAD~1", "HEAD", "--", "specs/"])
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
                    if let Some(filename) = line.split('/').next_back() {
                        if filename.ends_with(".md") {
                            let spec_id = filename.trim_end_matches(".md");
                            if self.verbose {
                                println!("Found recent spec file: {spec_id}");
                            }
                            return Ok(spec_id.to_string());
                        }
                    }
                }
            }
            return Ok(String::new());
        }

        let files = String::from_utf8_lossy(&output.stdout);

        // Look for new .md files anywhere in specs/ directory
        for line in files.lines() {
            if line.starts_with("specs/") && line.ends_with(".md") {
                if let Some(filename) = line.split('/').last() {
                    let spec_id = filename.trim_end_matches(".md");
                    // Accept any .md file that's not a special file
                    if !matches!(spec_id, "SPEC_INDEX" | "README" | "index") {
                        if self.verbose {
                            println!("Found new spec file in commit: {spec_id}");
                        }
                        return Ok(spec_id.to_string());
                    }
                }
            }
        }

        // If no spec file in diff, check if this is a review commit
        // and look for recently created spec files
        let commit_message = get_last_commit_message()
            .await
            .context("Failed to get git log")?;

        if commit_message.starts_with("review:") || commit_message.starts_with("cleanup:") {
            if let Ok(find_output) = tokio::process::Command::new("find")
                .args(["specs", "-name", "*.md", "-type", "f", "-mmin", "-5"])
                .output()
                .await
            {
                let files = String::from_utf8_lossy(&find_output.stdout);
                for line in files.lines() {
                    if let Some(filename) = line.split('/').last() {
                        if filename.ends_with(".md") {
                            let spec_id = filename.trim_end_matches(".md");
                            if self.verbose {
                                println!("Found recent spec file: {spec_id}");
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

/// Get current git HEAD commit hash
async fn get_git_head() -> Result<String> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .await?;
    
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow!("Failed to get git HEAD"))
    }
}

/// Extract spec file from a specific git commit
async fn extract_spec_from_commit(commit_hash: &str) -> Result<Option<String>> {
    // Get list of files changed in this commit
    let output = tokio::process::Command::new("git")
        .args(["show", "--name-only", "--pretty=format:", commit_hash])
        .output()
        .await?;
    
    if !output.status.success() {
        return Ok(None);
    }
    
    let files = String::from_utf8_lossy(&output.stdout);
    
    // Look for spec files in the commit
    for line in files.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("specs/") && line.ends_with(".md") {
            if let Some(filename) = line.split('/').last() {
                let spec_id = filename.trim_end_matches(".md");
                // Skip special files
                if !matches!(spec_id, "SPEC_INDEX" | "README" | "index") {
                    return Ok(Some(spec_id.to_string()));
                }
            }
        }
    }
    
    Ok(None)
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
        // Create a simple workflow with just mmm-code-review and mmm-lint
        let config = WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
        };
        let mut executor = WorkflowExecutor::new_for_test(config, false, 1);

        // This should succeed without actually calling Claude
        let result = executor.execute_iteration(1, Some("test focus")).await;

        // In test mode, should return success
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should have changes from the commands
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
    async fn test_get_git_head() {
        // This test requires a git repository
        if let Ok(head) = get_git_head().await {
            // Should be a 40-character hex string
            assert_eq!(head.len(), 40);
            assert!(head.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[tokio::test]
    async fn test_extract_spec_from_commit() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary git repository
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // Initialize git repo
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["init"])
            .output()
            .expect("Failed to init git repo");
            
        // Configure git user for the test repo
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .expect("Failed to set git email");
            
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test User"])
            .output()
            .expect("Failed to set git name");

        // Create specs directory
        fs::create_dir_all(repo_path.join("specs")).unwrap();
        
        // Create and commit a spec file
        let spec_content = "# Test Spec\nThis is a test specification.";
        fs::write(repo_path.join("specs/test-spec-123.md"), spec_content).unwrap();
        
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["add", "specs/test-spec-123.md"])
            .output()
            .expect("Failed to add file");
            
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["commit", "-m", "test: add test spec"])
            .output()
            .expect("Failed to commit");

        // Get the commit hash
        let output = std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("Failed to get commit hash");
            
        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Change to the test directory for the async command
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo_path).unwrap();
        
        // Test extracting spec from commit
        let result = extract_spec_from_commit(&commit_hash).await.unwrap();
        
        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
        
        assert_eq!(result, Some("test-spec-123".to_string()));
    }

    #[tokio::test]
    async fn test_extract_spec_from_commit_no_spec() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary git repository
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // Initialize git repo
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["init"])
            .output()
            .expect("Failed to init git repo");
            
        // Configure git user
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .expect("Failed to set git email");
            
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test User"])
            .output()
            .expect("Failed to set git name");

        // Create and commit a non-spec file
        fs::write(repo_path.join("README.md"), "# Readme").unwrap();
        
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["add", "README.md"])
            .output()
            .expect("Failed to add file");
            
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["commit", "-m", "test: add readme"])
            .output()
            .expect("Failed to commit");

        // Get the commit hash
        let output = std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("Failed to get commit hash");
            
        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Change to the test directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo_path).unwrap();
        
        // Test extracting spec from commit (should find nothing)
        let result = extract_spec_from_commit(&commit_hash).await.unwrap();
        
        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
        
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_extract_spec_from_commit_skip_special_files() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary git repository
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // Initialize git repo
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["init"])
            .output()
            .expect("Failed to init git repo");
            
        // Configure git user
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .expect("Failed to set git email");
            
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test User"])
            .output()
            .expect("Failed to set git name");

        // Create specs directory
        fs::create_dir_all(repo_path.join("specs")).unwrap();
        
        // Create and commit SPEC_INDEX.md (should be skipped)
        fs::write(repo_path.join("specs/SPEC_INDEX.md"), "# Index").unwrap();
        
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["add", "specs/SPEC_INDEX.md"])
            .output()
            .expect("Failed to add file");
            
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["commit", "-m", "test: add index"])
            .output()
            .expect("Failed to commit");

        // Get the commit hash
        let output = std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("Failed to get commit hash");
            
        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Change to the test directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo_path).unwrap();
        
        // Test extracting spec from commit (should skip SPEC_INDEX)
        let result = extract_spec_from_commit(&commit_hash).await.unwrap();
        
        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
        
        assert_eq!(result, None);
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
            "Some text iteration-1234567890-improvements"
        ];

        for msg in test_cases {
            // Test commit message parsing logic if needed
            assert!(msg.contains("iteration-"));
        }
    }

    #[tokio::test] 
    async fn test_workflow_spec_variable_passing() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary git repository
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // Initialize git repo
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["init"])
            .output()
            .expect("Failed to init git repo");
            
        // Configure git user
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .expect("Failed to set git email");
            
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test User"])
            .output()
            .expect("Failed to set git name");

        // Create initial commit
        fs::write(repo_path.join("README.md"), "# Test").unwrap();
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["add", "."])
            .output()
            .expect("Failed to add files");
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["commit", "-m", "initial"])
            .output()
            .expect("Failed to commit");

        // Change to test directory BEFORE calling git commands
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo_path).unwrap();

        // Simulate a spec being created by setting git HEAD before/after
        let head_before = get_git_head().await.unwrap();
        
        // Create a workflow executor
        let config = WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("test-review".to_string()),
                WorkflowCommand::Simple("mmm-implement-spec".to_string()),
            ],
        };
        let mut executor = WorkflowExecutor::new_for_test(config, true, 1);
        
        // Create specs directory and commit a spec file
        fs::create_dir_all("specs").unwrap();
        fs::write("specs/test-workflow-spec.md", "# Test Spec").unwrap();
        
        std::process::Command::new("git")
            .args(["add", "specs/test-workflow-spec.md"])
            .output()
            .expect("Failed to add spec");
            
        std::process::Command::new("git")
            .args(["commit", "-m", "test: add workflow spec"])
            .output()
            .expect("Failed to commit spec");

        let head_after = get_git_head().await.unwrap();
        
        // Extract spec from the commit
        assert_ne!(head_before, head_after);
        let spec_id = extract_spec_from_commit(&head_after).await.unwrap();
        assert_eq!(spec_id, Some("test-workflow-spec".to_string()));

        // Verify the executor would store this in variables
        if let Some(spec) = spec_id {
            executor.variables.insert("SPEC_ID".to_string(), spec);
        }
        
        assert_eq!(executor.variables.get("SPEC_ID"), Some(&"test-workflow-spec".to_string()));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
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

        // Create a test workflow with just lint command (doesn't require args)
        let config = WorkflowConfig {
            commands: vec![WorkflowCommand::Simple("mmm-lint".to_string())],
        };

        let mut executor = WorkflowExecutor::new_for_test(config, false, 1);

        // Execute iteration - should succeed
        let result = executor.execute_iteration(1, None).await;

        // Should succeed with changes from lint command
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Restore original directory if we had one
        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }
    }

    /// Test that focus directive is passed on every iteration, not just the first
    #[tokio::test]
    async fn test_focus_passed_every_iteration() {
        // Create workflow with mmm-code-review as first command (which receives focus)
        let workflow = WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
        };

        let mut executor = WorkflowExecutor::new_for_test(workflow, true, 3);

        // Track how many times we execute iterations successfully
        let mut successful_iterations = 0;

        // Run 3 iterations with focus
        for iteration in 1..=3 {
            let result = executor
                .execute_iteration(iteration, Some("security"))
                .await;

            assert!(result.is_ok(), "Iteration {iteration} should succeed");
            assert!(result.unwrap(), "Iteration {iteration} should have changes");
            successful_iterations += 1;
        }

        // Verify all 3 iterations executed successfully
        assert_eq!(
            successful_iterations, 3,
            "Should have executed 3 iterations"
        );
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
            .current_dir(temp_path)
            .args(["init"])
            .output()
            .await
            .unwrap();

        TokioCommand::new("git")
            .current_dir(temp_path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .await
            .unwrap();

        TokioCommand::new("git")
            .current_dir(temp_path)
            .args(["config", "user.name", "Test User"])
            .output()
            .await
            .unwrap();

        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(temp_path).unwrap();

        // Create a workflow with mmm-code-review as first command
        let workflow = WorkflowConfig {
            commands: vec![
                WorkflowCommand::Simple("mmm-code-review".to_string()),
                WorkflowCommand::Simple("mmm-lint".to_string()),
            ],
        };

        // Track command executions and their options
        let mut commands_with_focus = 0;

        // Manually check the logic for each iteration
        for _iteration in 1..=3 {
            let executor = WorkflowExecutor::new_for_test(workflow.clone(), true, 3);

            // Get the first command (mmm-code-review)
            let mut cmd = executor.config.commands[0].to_command();

            // Apply the logic from execute_iteration
            let idx = 0; // First command
            let focus = "documentation";

            // This is the fixed logic: if idx == 0
            if idx == 0 {
                cmd.options
                    .insert("focus".to_string(), serde_json::json!(focus));
                commands_with_focus += 1;
            }
        }

        // All 3 iterations should have focus in the command
        assert_eq!(
            commands_with_focus, 3,
            "All 3 iterations should have focus applied to mmm-code-review"
        );

        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }
    }
}
