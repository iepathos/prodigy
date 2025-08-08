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

        // Build git command based on operation
        let mut git_args = vec![operation.clone()];

        // Handle special operations
        match operation.as_str() {
            "commit" => {
                if let Some(msg) = attributes.get("message").and_then(|v| v.as_string()) {
                    git_args.push("-m".to_string());
                    git_args.push(msg.clone());
                } else {
                    return CommandResult::error(
                        "Commit operation requires 'message' attribute".to_string(),
                    );
                }

                // Auto-stage if requested
                if attributes
                    .get("auto_stage")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    // First run git add
                    let files = attributes
                        .get("files")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_string().cloned())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_else(|| vec![".".to_string()]);

                    if !context.dry_run {
                        let add_result = context
                            .executor
                            .execute(
                                "git",
                                &["add"]
                                    .iter()
                                    .copied()
                                    .chain(files.iter().map(|s| s.as_str()))
                                    .collect::<Vec<_>>(),
                                Some(&context.working_dir),
                                Some(context.full_env()),
                                None,
                            )
                            .await;

                        if let Err(e) = add_result {
                            return CommandResult::error(format!("Failed to stage files: {e}"));
                        }
                    }
                }
            }
            "checkout" | "switch" => {
                if let Some(branch) = attributes.get("branch").and_then(|v| v.as_string()) {
                    git_args.push(branch.clone());

                    // Check if we should create the branch
                    if operation == "checkout"
                        && attributes
                            .get("args")
                            .and_then(|v| v.as_string())
                            .map(|s| s.contains("-b"))
                            .unwrap_or(false)
                    {
                        git_args.insert(1, "-b".to_string());
                    }
                }
            }
            "push" | "pull" => {
                if let Some(remote) = attributes.get("remote").and_then(|v| v.as_string()) {
                    git_args.push(remote.clone());
                }
                if let Some(branch) = attributes.get("branch").and_then(|v| v.as_string()) {
                    git_args.push(branch.clone());
                }
            }
            _ => {}
        }

        // Add additional args if provided
        if let Some(args) = attributes.get("args").and_then(|v| v.as_string()) {
            for arg in args.split_whitespace() {
                git_args.push(arg.to_string());
            }
        }

        // Add files if specified and not already handled
        if operation != "commit" {
            if let Some(files) = attributes.get("files").and_then(|v| v.as_array()) {
                for file_val in files {
                    if let Some(file) = file_val.as_string() {
                        git_args.push(file.clone());
                    }
                }
            }
        }

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
}
