//! Unit tests for execution coordinator

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::cook::coordinators::execution::RetryConfig;
    use crate::cook::execution::{ClaudeExecutor, CommandExecutor, ExecutionContext};
    use crate::subprocess::SubprocessManager;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    // Create a minimal mock for CommandExecutor
    struct TestCommandExecutor {
        should_fail: bool,
        exit_code: i32,
    }

    #[async_trait]
    impl CommandExecutor for TestCommandExecutor {
        async fn execute(
            &self,
            _command: &str,
            _args: &[String],
            _context: ExecutionContext,
        ) -> Result<crate::cook::execution::ExecutionResult> {
            if self.should_fail {
                return Err(anyhow::anyhow!("Command failed"));
            }
            Ok(crate::cook::execution::ExecutionResult {
                success: self.exit_code == 0,
                stdout: "test output".to_string(),
                stderr: String::new(),
                exit_code: Some(self.exit_code),
            })
        }
    }

    // Create a minimal mock for ClaudeExecutor
    struct TestClaudeExecutor {
        should_fail: bool,
    }

    #[async_trait]
    impl ClaudeExecutor for TestClaudeExecutor {
        async fn execute_claude_command(
            &self,
            _command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
        ) -> Result<crate::cook::execution::ExecutionResult> {
            if self.should_fail {
                return Err(anyhow::anyhow!("Claude command failed"));
            }
            Ok(crate::cook::execution::ExecutionResult {
                success: true,
                stdout: "claude output".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            })
        }

        async fn check_claude_cli(&self) -> Result<bool> {
            Ok(!self.should_fail)
        }

        async fn get_claude_version(&self) -> Result<String> {
            Ok("test-version".to_string())
        }
    }

    #[tokio::test]
    async fn test_execute_command_success() {
        let command_executor = Arc::new(TestCommandExecutor {
            should_fail: false,
            exit_code: 0,
        });
        let claude_executor = Arc::new(TestClaudeExecutor { should_fail: false });
        let subprocess = Arc::new(SubprocessManager::production());

        let coordinator =
            DefaultExecutionCoordinator::new(command_executor, claude_executor, subprocess);

        let result = coordinator
            .execute_command("echo", &["test".to_string()], None, None)
            .await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();
        assert_eq!(exec_result.exit_code, 0);
        assert_eq!(exec_result.stdout, "test output");
    }

    #[tokio::test]
    async fn test_execute_command_failure() {
        let command_executor = Arc::new(TestCommandExecutor {
            should_fail: false,
            exit_code: 1,
        });
        let claude_executor = Arc::new(TestClaudeExecutor { should_fail: false });
        let subprocess = Arc::new(SubprocessManager::production());

        let coordinator =
            DefaultExecutionCoordinator::new(command_executor, claude_executor, subprocess);

        let result = coordinator.execute_command("false", &[], None, None).await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();
        assert_eq!(exec_result.exit_code, 1);
    }

    #[tokio::test]
    async fn test_execute_claude_success() {
        let command_executor = Arc::new(TestCommandExecutor {
            should_fail: false,
            exit_code: 0,
        });
        let claude_executor = Arc::new(TestClaudeExecutor { should_fail: false });
        let subprocess = Arc::new(SubprocessManager::production());

        let coordinator =
            DefaultExecutionCoordinator::new(command_executor, claude_executor, subprocess);

        let result = coordinator.execute_claude("test", &[], None, None).await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();
        assert_eq!(exec_result.exit_code, 0);
        assert_eq!(exec_result.stdout, "claude output");
    }

    #[tokio::test]
    async fn test_execute_claude_failure() {
        let command_executor = Arc::new(TestCommandExecutor {
            should_fail: false,
            exit_code: 0,
        });
        let claude_executor = Arc::new(TestClaudeExecutor { should_fail: true });
        let subprocess = Arc::new(SubprocessManager::production());

        let coordinator =
            DefaultExecutionCoordinator::new(command_executor, claude_executor, subprocess);

        let result = coordinator.execute_claude("fail", &[], None, None).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_retry() {
        let command_executor = Arc::new(TestCommandExecutor {
            should_fail: false,
            exit_code: 0,
        });
        let claude_executor = Arc::new(TestClaudeExecutor { should_fail: false });
        let subprocess = Arc::new(SubprocessManager::production());

        let coordinator =
            DefaultExecutionCoordinator::new(command_executor, claude_executor, subprocess);

        let retry_config = RetryConfig {
            max_attempts: Some(3),
            delay: Some(Duration::from_millis(10)),
        };

        let result = coordinator
            .execute_with_retry("echo", &[], None, None, &retry_config)
            .await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();
        assert_eq!(exec_result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_check_command_available() {
        let command_executor = Arc::new(TestCommandExecutor {
            should_fail: false,
            exit_code: 0,
        });
        let claude_executor = Arc::new(TestClaudeExecutor { should_fail: false });
        let subprocess = Arc::new(SubprocessManager::production());

        let coordinator =
            DefaultExecutionCoordinator::new(command_executor, claude_executor, subprocess);

        let result = coordinator.check_command_available("test").await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}
