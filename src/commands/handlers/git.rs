//! Git command handler for version control operations

use crate::commands::{
    AttributeSchema, AttributeValue, CommandHandler, CommandResult, ExecutionContext,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

/// Handler for Git operations
pub struct GitHandler;

impl GitHandler {
    /// Creates a new Git handler
    pub fn new() -> Self {
        Self
    }

    /// Extracts files from attributes, defaulting to ["."] if not specified
    fn extract_files(attributes: &HashMap<String, AttributeValue>) -> Vec<String> {
        attributes
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_string().cloned()).collect())
            .filter(|files: &Vec<String>| !files.is_empty())
            .unwrap_or_else(|| vec![".".to_string()])
    }

    /// Builds commit-specific arguments including message and optional auto-staging
    fn build_commit_args(
        operation: &str,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Result<Vec<String>, String> {
        let msg = attributes
            .get("message")
            .and_then(|v| v.as_string())
            .ok_or_else(|| "Commit operation requires 'message' attribute".to_string())?;

        Ok(vec![operation.to_string(), "-m".to_string(), msg.clone()])
    }

    /// Builds checkout/switch arguments with branch and optional create flag
    fn build_checkout_args(
        operation: &str,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Vec<String> {
        let mut args = vec![operation.to_string()];

        if let Some(branch) = attributes.get("branch").and_then(|v| v.as_string()) {
            // Check if we should create the branch (-b flag)
            let should_create = operation == "checkout"
                && attributes
                    .get("args")
                    .and_then(|v| v.as_string())
                    .map(|s| s.contains("-b"))
                    .unwrap_or(false);

            if should_create {
                args.push("-b".to_string());
            }
            args.push(branch.clone());
        }

        args
    }

    /// Builds push/pull arguments with optional remote and branch
    fn build_push_pull_args(
        operation: &str,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Vec<String> {
        let mut args = vec![operation.to_string()];

        if let Some(remote) = attributes.get("remote").and_then(|v| v.as_string()) {
            args.push(remote.clone());
        }
        if let Some(branch) = attributes.get("branch").and_then(|v| v.as_string()) {
            args.push(branch.clone());
        }

        args
    }

    /// Builds git command arguments for any operation
    fn build_git_args(
        operation: &str,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Result<Vec<String>, String> {
        let mut git_args = match operation {
            "commit" => Self::build_commit_args(operation, attributes)?,
            "checkout" | "switch" => Self::build_checkout_args(operation, attributes),
            "push" | "pull" => Self::build_push_pull_args(operation, attributes),
            _ => vec![operation.to_string()],
        };

        // Add additional args if provided
        if let Some(args) = attributes.get("args").and_then(|v| v.as_string()) {
            git_args.extend(args.split_whitespace().map(String::from));
        }

        // Add files if specified and not a commit operation
        if operation != "commit" {
            if let Some(files) = attributes.get("files").and_then(|v| v.as_array()) {
                git_args.extend(files.iter().filter_map(|v| v.as_string()).map(String::from));
            }
        }

        Ok(git_args)
    }

    /// Determines if auto-staging is required for commit operation
    fn should_auto_stage(operation: &str, attributes: &HashMap<String, AttributeValue>) -> bool {
        operation == "commit"
            && attributes
                .get("auto_stage")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
    }

    /// Validates and extracts the operation attribute
    ///
    /// Returns the operation string if present, or an error message if missing.
    fn validate_operation(attributes: &HashMap<String, AttributeValue>) -> Result<String, String> {
        attributes
            .get("operation")
            .and_then(|v| v.as_string())
            .cloned()
            .ok_or_else(|| "Missing required attribute: operation".to_string())
    }

    /// Executes auto-staging for commit operations if required
    ///
    /// This function checks if auto-staging is enabled and performs the staging operation
    /// by executing `git add` with the appropriate files.
    async fn execute_auto_staging(
        context: &ExecutionContext,
        operation: &str,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Result<(), String> {
        if !Self::should_auto_stage(operation, attributes) {
            return Ok(());
        }

        let files = Self::extract_files(attributes);
        let add_args: Vec<&str> = std::iter::once("add")
            .chain(files.iter().map(|s| s.as_str()))
            .collect();

        context
            .executor
            .execute(
                "git",
                &add_args,
                Some(&context.working_dir),
                Some(context.full_env()),
                None,
            )
            .await
            .map_err(|e| format!("Failed to stage files: {e}"))?;

        Ok(())
    }

    /// Builds a dry-run response for git commands
    ///
    /// Returns a CommandResult indicating what would be executed without actually running it.
    fn build_dry_run_response(git_args: &[String], duration: u64) -> CommandResult {
        CommandResult::success(json!({
            "dry_run": true,
            "command": format!("git {}", git_args.join(" ")),
        }))
        .with_duration(duration)
    }

    /// Executes a git command and processes the result
    ///
    /// This function handles the actual command execution, stdout/stderr processing,
    /// and result transformation into a CommandResult.
    async fn execute_git_command(
        context: &ExecutionContext,
        operation: String,
        git_args: Vec<String>,
        start: Instant,
    ) -> CommandResult {
        let result = context
            .executor
            .execute(
                "git",
                &git_args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                Some(&context.working_dir),
                Some(context.full_env()),
                None,
            )
            .await;

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    CommandResult::success(json!({
                        "output": stdout,
                        "operation": operation,
                    }))
                    .with_duration(duration)
                } else {
                    CommandResult::error(format!("Git command failed: {stderr}"))
                        .with_duration(duration)
                }
            }
            Err(e) => CommandResult::error(format!("Failed to execute git command: {e}"))
                .with_duration(duration),
        }
    }

    /// Validates preconditions and prepares git command arguments
    ///
    /// This pure function performs all validation and argument building without side effects.
    /// It returns the validated operation and prepared git arguments, or an error if validation fails.
    fn validate_and_prepare(
        attributes: &HashMap<String, AttributeValue>,
    ) -> Result<(String, Vec<String>), String> {
        let operation = Self::validate_operation(attributes)?;
        let git_args = Self::build_git_args(&operation, attributes)?;
        Ok((operation, git_args))
    }
}

#[async_trait]
impl CommandHandler for GitHandler {
    fn name(&self) -> &str {
        "git"
    }

    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("git");
        schema.add_required(
            "operation",
            "Git operation to perform (status, diff, commit, etc.)",
        );
        schema.add_optional("args", "Additional arguments for the git command");
        schema.add_optional("message", "Commit message (for commit operation)");
        schema.add_optional("branch", "Branch name (for checkout/create operations)");
        schema.add_optional("remote", "Remote name (for push/pull operations)");
        schema.add_optional("files", "Files to operate on");
        schema.add_optional_with_default(
            "auto_stage",
            "Automatically stage changes before commit",
            AttributeValue::Boolean(false),
        );
        schema
    }

    async fn execute(
        &self,
        context: &ExecutionContext,
        mut attributes: HashMap<String, AttributeValue>,
    ) -> CommandResult {
        // Apply defaults
        self.schema().apply_defaults(&mut attributes);

        // Validate preconditions and prepare git arguments
        let (operation, git_args) = match Self::validate_and_prepare(&attributes) {
            Ok(result) => result,
            Err(e) => return CommandResult::error(e),
        };

        let start = Instant::now();

        // Early return for dry run mode
        if context.dry_run {
            let duration = start.elapsed().as_millis() as u64;
            return Self::build_dry_run_response(&git_args, duration);
        }

        // Handle auto-staging for commits
        if let Err(e) = Self::execute_auto_staging(context, &operation, &attributes).await {
            return CommandResult::error(e);
        }

        // Execute git command
        Self::execute_git_command(context, operation, git_args, start).await
    }

    fn description(&self) -> &str {
        "Handles Git version control operations"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            r#"{"operation": "status"}"#.to_string(),
            r#"{"operation": "commit", "message": "Fix bug", "auto_stage": true}"#.to_string(),
            r#"{"operation": "checkout", "branch": "feature", "args": "-b"}"#.to_string(),
            r#"{"operation": "push", "remote": "origin", "branch": "main"}"#.to_string(),
        ]
    }
}

impl Default for GitHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::adapter::MockSubprocessExecutor;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Output;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_git_handler_schema() {
        let handler = GitHandler::new();
        let schema = handler.schema();

        assert!(schema.required().contains_key("operation"));
        assert!(schema.optional().contains_key("message"));
        assert!(schema.optional().contains_key("branch"));
    }

    #[tokio::test]
    async fn test_git_status() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["status"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"On branch main\nnothing to commit".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("status".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_commit_dry_run() {
        let handler = GitHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test")).with_dry_run(true);

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());

        let data = result.data.unwrap();
        assert_eq!(data.get("dry_run"), Some(&json!(true)));
    }

    #[tokio::test]
    async fn test_git_missing_operation() {
        let handler = GitHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test"));

        let attributes = HashMap::new();

        let result = handler.execute(&context, attributes).await;
        assert!(!result.is_success());
        assert!(result
            .error
            .unwrap()
            .contains("Missing required attribute: operation"));
    }

    #[tokio::test]
    async fn test_git_commit_without_message() {
        let handler = GitHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test"));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(!result.is_success());
        assert!(result
            .error
            .unwrap()
            .contains("Commit operation requires 'message' attribute"));
    }

    #[tokio::test]
    async fn test_git_commit_with_auto_stage() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["add", "."],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
        );

        mock_executor.expect_execute(
            "git",
            vec!["commit", "-m", "Test commit"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"[main abc123] Test commit".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );
        attributes.insert("auto_stage".to_string(), AttributeValue::Boolean(true));

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_commit_auto_stage_failure() {
        let handler = GitHandler::new();
        let mock_executor = MockSubprocessExecutor::new();

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );
        attributes.insert("auto_stage".to_string(), AttributeValue::Boolean(true));

        let result = handler.execute(&context, attributes).await;
        assert!(!result.is_success());
        assert!(result.error.unwrap().contains("Failed to stage files"));
    }

    #[tokio::test]
    async fn test_git_commit_success() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["commit", "-m", "Test commit"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"[main abc123] Test commit\n 1 file changed".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
        let data = result.data.unwrap();
        assert_eq!(data.get("operation"), Some(&json!("commit")));
        assert!(data
            .get("output")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("abc123"));
        assert!(result.duration_ms.is_some());
    }

    #[tokio::test]
    async fn test_git_command_failure() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["commit", "-m", "Test commit"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(1),
                stdout: Vec::new(),
                stderr: b"nothing to commit".to_vec(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(!result.is_success());
        assert!(result.error.unwrap().contains("nothing to commit"));
        assert!(result.duration_ms.is_some());
    }

    #[tokio::test]
    async fn test_git_command_execution_error() {
        let handler = GitHandler::new();
        let mock_executor = MockSubprocessExecutor::new();

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(!result.is_success());
        assert!(result
            .error
            .unwrap()
            .contains("Failed to execute git command"));
        assert!(result.duration_ms.is_some());
    }

    #[tokio::test]
    async fn test_git_checkout_with_branch() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["checkout", "feature"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Switched to branch 'feature'".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("checkout".to_string()),
        );
        attributes.insert(
            "branch".to_string(),
            AttributeValue::String("feature".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_checkout_create_branch() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["checkout", "-b", "feature", "-b"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Switched to a new branch 'feature'".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("checkout".to_string()),
        );
        attributes.insert(
            "branch".to_string(),
            AttributeValue::String("feature".to_string()),
        );
        attributes.insert("args".to_string(), AttributeValue::String("-b".to_string()));

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_push_with_remote_branch() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["push", "origin", "main"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Everything up-to-date".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("push".to_string()),
        );
        attributes.insert(
            "remote".to_string(),
            AttributeValue::String("origin".to_string()),
        );
        attributes.insert(
            "branch".to_string(),
            AttributeValue::String("main".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_status_with_files() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["status", "src/main.rs", "src/lib.rs"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"On branch main".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("status".to_string()),
        );
        attributes.insert(
            "files".to_string(),
            AttributeValue::Array(vec![
                AttributeValue::String("src/main.rs".to_string()),
                AttributeValue::String("src/lib.rs".to_string()),
            ]),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_add_with_files() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["add", "file1.rs", "file2.rs"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("add".to_string()),
        );
        attributes.insert(
            "files".to_string(),
            AttributeValue::Array(vec![
                AttributeValue::String("file1.rs".to_string()),
                AttributeValue::String("file2.rs".to_string()),
            ]),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_switch_to_branch() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["switch", "feature"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Switched to branch 'feature'".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("switch".to_string()),
        );
        attributes.insert(
            "branch".to_string(),
            AttributeValue::String("feature".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_pull_without_remote() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["pull", "main"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Already up to date.".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("pull".to_string()),
        );
        attributes.insert(
            "branch".to_string(),
            AttributeValue::String("main".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_push_without_remote() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["push", "main"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Branch 'main' set up to track remote branch".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("push".to_string()),
        );
        attributes.insert(
            "branch".to_string(),
            AttributeValue::String("main".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_commit_auto_stage_custom_files() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["add", "src/main.rs", "src/lib.rs"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
        );

        mock_executor.expect_execute(
            "git",
            vec!["commit", "-m", "Test commit"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"[main abc123] Test commit".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );
        attributes.insert("auto_stage".to_string(), AttributeValue::Boolean(true));
        attributes.insert(
            "files".to_string(),
            AttributeValue::Array(vec![
                AttributeValue::String("src/main.rs".to_string()),
                AttributeValue::String("src/lib.rs".to_string()),
            ]),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[test]
    fn test_git_handler_description() {
        let handler = GitHandler::new();
        let description = handler.description();
        assert_eq!(description, "Handles Git version control operations");
    }

    #[test]
    fn test_git_handler_examples() {
        let handler = GitHandler::new();
        let examples = handler.examples();
        assert_eq!(examples.len(), 4);
        assert!(examples[0].contains("status"));
        assert!(examples[1].contains("commit"));
        assert!(examples[2].contains("checkout"));
        assert!(examples[3].contains("push"));
    }

    #[test]
    fn test_git_handler_default() {
        let handler = GitHandler;
        assert_eq!(handler.name(), "git");
    }

    #[tokio::test]
    async fn test_git_checkout_without_branch() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["checkout"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Already on 'main'".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("checkout".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_commit_with_additional_args() {
        let handler = GitHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "git",
            vec!["commit", "-m", "Test commit", "--amend", "--no-edit"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"[main abc123] Test commit (amended)".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "operation".to_string(),
            AttributeValue::String("commit".to_string()),
        );
        attributes.insert(
            "message".to_string(),
            AttributeValue::String("Test commit".to_string()),
        );
        attributes.insert(
            "args".to_string(),
            AttributeValue::String("--amend --no-edit".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }
}
