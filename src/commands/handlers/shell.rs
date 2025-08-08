//! Shell command handler for executing system commands

use crate::commands::{
    AttributeSchema, AttributeValue, CommandHandler, CommandResult, ExecutionContext,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

/// Handler for executing shell commands
pub struct ShellHandler;

impl ShellHandler {
    /// Creates a new shell handler
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ShellHandler {
    fn name(&self) -> &str {
        "shell"
    }

    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("shell");
        schema.add_required("command", "The shell command to execute");
        schema.add_optional_with_default(
            "shell",
            "The shell to use (bash, sh, zsh)",
            AttributeValue::String("bash".to_string()),
        );
        schema.add_optional_with_default(
            "timeout",
            "Command timeout in seconds",
            AttributeValue::Number(30.0),
        );
        schema.add_optional("working_dir", "Working directory for the command");
        schema.add_optional("env", "Environment variables as key=value pairs");
        schema
    }

    async fn execute(
        &self,
        context: &ExecutionContext,
        mut attributes: HashMap<String, AttributeValue>,
    ) -> CommandResult {
        // Apply defaults
        self.schema().apply_defaults(&mut attributes);

        // Extract command
        let command = match attributes.get("command").and_then(|v| v.as_string()) {
            Some(cmd) => cmd.clone(),
            None => return CommandResult::error("Missing required attribute: command".to_string()),
        };

        // Extract shell
        let shell = attributes
            .get("shell")
            .and_then(|v| v.as_string())
            .map(|s| s.as_str())
            .unwrap_or("bash");

        // Extract timeout
        let timeout = attributes
            .get("timeout")
            .and_then(|v| v.as_number())
            .unwrap_or(30.0) as u64;

        // Extract working directory
        let working_dir = attributes
            .get("working_dir")
            .and_then(|v| v.as_string())
            .map(|s| context.resolve_path(s.as_ref()))
            .unwrap_or_else(|| context.working_dir.clone());

        // Extract additional environment variables
        let mut env = context.full_env();
        if let Some(env_attr) = attributes.get("env").and_then(|v| v.as_object()) {
            for (key, value) in env_attr {
                if let Some(val_str) = value.as_string() {
                    env.insert(key.clone(), val_str.clone());
                }
            }
        }

        // Execute command
        let start = Instant::now();

        if context.dry_run {
            let duration = start.elapsed().as_millis() as u64;
            return CommandResult::success(Value::String(format!(
                "[DRY RUN] Would execute: {shell} -c '{command}'"
            )))
            .with_duration(duration);
        }

        let result = context
            .executor
            .execute(
                shell,
                &["-c", &command],
                Some(&working_dir),
                Some(env),
                Some(std::time::Duration::from_secs(timeout)),
            )
            .await;

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                CommandResult::from_output(stdout, stderr, exit_code).with_duration(duration)
            }
            Err(e) => CommandResult::error(format!("Failed to execute command: {e}"))
                .with_duration(duration),
        }
    }

    fn description(&self) -> &str {
        "Executes shell commands with configurable shell, timeout, and environment"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            r#"{"command": "echo 'Hello, World!'"}"#.to_string(),
            r#"{"command": "ls -la", "working_dir": "/tmp"}"#.to_string(),
            r#"{"command": "npm test", "timeout": 60, "env": {"NODE_ENV": "test"}}"#.to_string(),
        ]
    }
}

impl Default for ShellHandler {
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
    async fn test_shell_handler_schema() {
        let handler = ShellHandler::new();
        let schema = handler.schema();

        assert!(schema.required().contains_key("command"));
        assert!(schema.optional().contains_key("shell"));
        assert!(schema.optional().contains_key("timeout"));
    }

    #[tokio::test]
    async fn test_shell_handler_execute() {
        let handler = ShellHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "bash",
            vec!["-c", "echo test"],
            Some(PathBuf::from("/test")),
            None,
            Some(std::time::Duration::from_secs(30)),
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"test\n".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "command".to_string(),
            AttributeValue::String("echo test".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
        assert_eq!(result.stdout, Some("test\n".to_string()));
    }

    #[tokio::test]
    async fn test_shell_handler_dry_run() {
        let handler = ShellHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test")).with_dry_run(true);

        let mut attributes = HashMap::new();
        attributes.insert(
            "command".to_string(),
            AttributeValue::String("rm -rf /".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
        assert!(result.data.unwrap().as_str().unwrap().contains("[DRY RUN]"));
    }
}
