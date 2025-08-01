#[cfg(test)]
mod tests {
    use super::super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_production_runner_success() {
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("echo")
            .arg("hello world")
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.trim(), "hello world");
        assert!(output.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_production_runner_failure() {
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("false").build();

        let output = runner.run(command).await.unwrap();
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(1));
    }

    #[tokio::test]
    async fn test_production_runner_command_not_found() {
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("nonexistent-command-12345").build();

        let result = runner.run(command).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProcessError::CommandNotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_production_runner_timeout() {
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sleep")
            .arg("5")
            .timeout(Duration::from_millis(100))
            .build();

        let result = runner.run(command).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProcessError::Timeout(_)));
    }

    #[tokio::test]
    async fn test_mock_runner_basic() {
        let mut mock = MockProcessRunner::new();

        mock.expect_command("git")
            .with_args(|args| args == &["status"])
            .returns_stdout("On branch main\n")
            .returns_success()
            .finish();

        let output = mock
            .run(ProcessCommandBuilder::new("git").arg("status").build())
            .await
            .unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout, "On branch main\n");
        assert!(mock.verify_called("git", 1));
    }

    #[tokio::test]
    async fn test_mock_runner_multiple_calls() {
        let mut mock = MockProcessRunner::new();

        mock.expect_command("git")
            .with_args(|args| args == &["add", "."])
            .returns_success()
            .times(2)
            .finish();

        // First call
        let result1 = mock
            .run(
                ProcessCommandBuilder::new("git")
                    .args(&["add", "."])
                    .build(),
            )
            .await;
        assert!(result1.is_ok());

        // Second call
        let result2 = mock
            .run(
                ProcessCommandBuilder::new("git")
                    .args(&["add", "."])
                    .build(),
            )
            .await;
        assert!(result2.is_ok());

        // Third call should fail
        let result3 = mock
            .run(
                ProcessCommandBuilder::new("git")
                    .args(&["add", "."])
                    .build(),
            )
            .await;
        assert!(result3.is_err());
    }

    #[tokio::test]
    async fn test_subprocess_manager() {
        let (manager, mut mock) = SubprocessManager::mock();

        mock.expect_command("ls")
            .returns_stdout("file1.txt\nfile2.txt\n")
            .returns_success()
            .finish();

        let output = manager
            .runner()
            .run(ProcessCommandBuilder::new("ls").build())
            .await
            .unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout, "file1.txt\nfile2.txt\n");
    }

    #[tokio::test]
    async fn test_git_runner() {
        let (manager, mut mock) = SubprocessManager::mock();

        mock.expect_command("git")
            .with_args(|args| args == &["status", "--porcelain", "--branch"])
            .returns_stdout("## main...origin/main\nM  file.txt\n")
            .returns_success()
            .finish();

        let git = manager.git();
        let status = git.status(std::path::Path::new(".")).await.unwrap();

        assert_eq!(status.branch, Some("main".to_string()));
        assert!(!status.clean);
        assert_eq!(status.modified_files, vec!["file.txt"]);
    }

    #[tokio::test]
    async fn test_claude_runner() {
        let (manager, mut mock) = SubprocessManager::mock();

        mock.expect_command("claude")
            .with_args(|args| args == &["--version"])
            .returns_stdout("Claude CLI version 1.0.0\n")
            .returns_success()
            .finish();

        let claude = manager.claude();
        let available = claude.check_availability().await.unwrap();

        assert!(available);
    }

    #[tokio::test]
    async fn test_process_command_builder() {
        let command = ProcessCommandBuilder::new("test")
            .arg("arg1")
            .args(&["arg2", "arg3"])
            .env("KEY1", "value1")
            .envs([("KEY2", "value2"), ("KEY3", "value3")])
            .current_dir(std::path::Path::new("/tmp"))
            .timeout(Duration::from_secs(30))
            .stdin("input data".to_string())
            .build();

        assert_eq!(command.program, "test");
        assert_eq!(command.args, vec!["arg1", "arg2", "arg3"]);
        assert_eq!(command.env.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(command.env.get("KEY2"), Some(&"value2".to_string()));
        assert_eq!(command.env.get("KEY3"), Some(&"value3".to_string()));
        assert_eq!(command.working_dir, Some(std::path::PathBuf::from("/tmp")));
        assert_eq!(command.timeout, Some(Duration::from_secs(30)));
        assert_eq!(command.stdin, Some("input data".to_string()));
    }
}
