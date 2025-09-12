//! Shell command executor for goal-seeking operations

use anyhow::Result;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

use crate::cook::execution::{CommandExecutor, ExecutionContext, ExecutionResult};

/// Shell-based command executor for goal-seeking
pub struct ShellCommandExecutor;

impl Default for ShellCommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellCommandExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandExecutor for ShellCommandExecutor {
    async fn execute(
        &self,
        command: &str,
        _args: &[String],
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // Parse the command string into shell command
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        
        // Add environment variables from context
        for (key, value) in &context.env_vars {
            cmd.env(key, value);
        }
        
        // Set working directory if specified
        cmd.current_dir(&context.working_directory);
        
        // Configure stdio
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        
        // Execute the command
        tracing::debug!("Executing shell command: sh -c '{}'", command);
        let output = cmd.output().await?;
        
        // Convert output to string
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        tracing::debug!("Shell output - stdout: '{}', stderr: '{}', status: {}", 
                       stdout, stderr, output.status.success());
        
        Ok(ExecutionResult {
            success: output.status.success(),
            stdout,
            stderr,
            exit_code: output.status.code(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell_executor_echo() {
        let executor = ShellCommandExecutor::new();
        let context = ExecutionContext::default();
        
        let result = executor
            .execute("echo 'Hello, World!'", &[], context)
            .await
            .unwrap();
        
        assert!(result.success);
        assert!(result.stdout.contains("Hello, World!"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_shell_executor_with_env() {
        let executor = ShellCommandExecutor::new();
        let mut context = ExecutionContext::default();
        context.env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());
        
        let result = executor
            .execute("echo $TEST_VAR", &[], context)
            .await
            .unwrap();
        
        assert!(result.success);
        assert!(result.stdout.contains("test_value"));
    }

    #[tokio::test]
    async fn test_shell_executor_score_output() {
        let executor = ShellCommandExecutor::new();
        let context = ExecutionContext::default();
        
        let result = executor
            .execute("echo 'score: 85'", &[], context)
            .await
            .unwrap();
        
        assert!(result.success);
        assert!(result.stdout.contains("score: 85"));
    }

    #[tokio::test]
    async fn test_shell_executor_failure() {
        let executor = ShellCommandExecutor::new();
        let context = ExecutionContext::default();
        
        let result = executor
            .execute("exit 1", &[], context)
            .await
            .unwrap();
        
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
    }
}