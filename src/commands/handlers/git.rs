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

        // Extract operation
        let operation = match attributes.get("operation").and_then(|v| v.as_string()) {
            Some(op) => op.clone(),
            None => {
                return CommandResult::error("Missing required attribute: operation".to_string())
            }
        };

        let start = Instant::now();

        // Build git command arguments using pure functions
        let git_args = match Self::build_git_args(&operation, &attributes) {
            Ok(args) => args,
            Err(e) => return CommandResult::error(e),
        };

        // Handle auto-staging for commits
        if Self::should_auto_stage(&operation, &attributes) && !context.dry_run {
            let files = Self::extract_files(&attributes);
            let add_args: Vec<&str> = std::iter::once("add")
                .chain(files.iter().map(|s| s.as_str()))
                .collect();

            if let Err(e) = context
                .executor
                .execute(
                    "git",
                    &add_args,
                    Some(&context.working_dir),
                    Some(context.full_env()),
                    None,
                )
                .await
            {
                return CommandResult::error(format!("Failed to stage files: {e}"));
            }
        }

        // Handle dry run
        if context.dry_run {
            let duration = start.elapsed().as_millis() as u64;
            return CommandResult::success(json!({
                "dry_run": true,
                "command": format!("git {}", git_args.join(" ")),
            }))
            .with_duration(duration);
        }

        // Execute git command
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
}
