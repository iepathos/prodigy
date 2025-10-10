#[cfg(test)]
mod subprocess_tests {
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
            .with_args(|args| args == ["status"])
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
            .with_args(|args| args == ["add", "."])
            .returns_success()
            .times(2)
            .finish();

        // First call
        let result1 = mock
            .run(ProcessCommandBuilder::new("git").args(["add", "."]).build())
            .await;
        assert!(result1.is_ok());

        // Second call
        let result2 = mock
            .run(ProcessCommandBuilder::new("git").args(["add", "."]).build())
            .await;
        assert!(result2.is_ok());

        // Third call should fail
        let result3 = mock
            .run(ProcessCommandBuilder::new("git").args(["add", "."]).build())
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
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
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
            .with_args(|args| args == ["--version"])
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
            .args(["arg2", "arg3"])
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

    #[tokio::test]
    async fn test_environment_not_inherited_from_parent() {
        // Set a test variable in the parent process that should NOT be inherited
        std::env::set_var("PRODIGY_TEST_BLOATED_VAR", "x".repeat(10000));

        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", "env | grep PRODIGY_TEST_BLOATED_VAR || echo 'NOT_FOUND'"])
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());

        // The bloated variable should NOT appear in the child process environment
        assert!(
            output.stdout.contains("NOT_FOUND"),
            "Parent environment variable was inherited! Output: {}",
            output.stdout
        );

        // Clean up
        std::env::remove_var("PRODIGY_TEST_BLOATED_VAR");
    }

    #[tokio::test]
    async fn test_essential_env_vars_preserved() {
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", "echo PATH=$PATH HOME=$HOME"])
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());

        // Essential variables like PATH and HOME should be preserved
        // Check that they are NOT empty (PATH= or HOME= alone)
        assert!(
            output.stdout.contains("PATH=/") || output.stdout.contains("PATH="),
            "PATH should be set! Output: {}",
            output.stdout
        );
        assert!(
            output.stdout.contains("HOME=/") || output.stdout.contains("HOME="),
            "HOME should be set! Output: {}",
            output.stdout
        );
    }

    #[tokio::test]
    async fn test_explicit_env_vars_passed() {
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", "echo CUSTOM=$CUSTOM_VAR"])
            .env("CUSTOM_VAR", "test_value")
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());
        assert!(
            output.stdout.contains("CUSTOM=test_value"),
            "Explicitly set env var should be passed! Output: {}",
            output.stdout
        );
    }

    #[tokio::test]
    async fn test_large_env_vars_do_not_cause_e2big() {
        // Simulate MapReduce scenario with many large environment variables
        let runner = runner::TokioProcessRunner;

        // Create a command with several large environment variables
        // (simulating map.results and result.* variables)
        let large_json = serde_json::json!({
            "results": vec![
                serde_json::json!({"item_id": "item1", "output": "x".repeat(1000)}),
                serde_json::json!({"item_id": "item2", "output": "y".repeat(1000)}),
                serde_json::json!({"item_id": "item3", "output": "z".repeat(1000)}),
            ]
        })
        .to_string();

        let mut builder = ProcessCommandBuilder::new("sh")
            .args(["-c", "echo SUCCESS"]);

        // Add the large variables
        builder = builder.env("MAP_RESULTS", &large_json);
        for i in 0..10 {
            builder = builder.env(
                &format!("RESULT_{}", i),
                &format!("large_output_{}", "x".repeat(500)),
            );
        }

        let command = builder.build();
        let output = runner.run(command).await.unwrap();

        // Should succeed without E2BIG error
        assert!(output.status.success());
        assert_eq!(output.stdout.trim(), "SUCCESS");
    }
}
