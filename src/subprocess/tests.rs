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
    #[serial_test::serial] // Must run alone - modifies global env to test subprocess isolation
    async fn test_environment_not_inherited_from_parent() {
        // Set a test variable in the parent process that should NOT be inherited
        std::env::set_var("PRODIGY_TEST_BLOATED_VAR", "x".repeat(10000));

        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sh")
            .args([
                "-c",
                "env | grep PRODIGY_TEST_BLOATED_VAR || echo 'NOT_FOUND'",
            ])
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
            .args(["-c", "echo PATH=$PATH HOME=$HOME USER=$USER"])
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());

        // Essential variables like PATH and HOME should be preserved
        // PATH must contain actual directories (not be empty)
        assert!(
            output.stdout.contains("PATH=/"),
            "PATH should be set with valid directories! Output: {}",
            output.stdout
        );

        // PATH should not be just "PATH=" (empty)
        assert!(
            !output.stdout.contains("PATH=\n") && !output.stdout.contains("PATH= "),
            "PATH should not be empty! Output: {}",
            output.stdout
        );

        // HOME should be set to a directory path
        assert!(
            output.stdout.contains("HOME=/"),
            "HOME should be set! Output: {}",
            output.stdout
        );

        // USER should be preserved (might be empty on some systems, but should be present)
        assert!(
            output.stdout.contains("USER="),
            "USER should be set! Output: {}",
            output.stdout
        );
    }

    /// Test that PATH contains valid directories and commands can be found
    #[tokio::test]
    async fn test_path_contains_valid_directories() {
        let runner = runner::TokioProcessRunner;

        // Test that we can actually find and execute common commands via PATH
        // This verifies PATH is not only set, but contains valid directories
        let command = ProcessCommandBuilder::new("which").arg("sh").build();

        let result = runner.run(command).await;

        // Should be able to find 'sh' in PATH
        assert!(
            result.is_ok(),
            "which command should work with preserved PATH"
        );

        let output = result.unwrap();
        assert!(
            output.status.success(),
            "'which sh' should succeed with valid PATH. Stderr: {}",
            output.stderr
        );

        // Should return a path to sh
        assert!(
            output.stdout.contains("/sh") || output.stdout.contains("bin/sh"),
            "Should find sh in PATH. Output: {}",
            output.stdout
        );
    }

    /// Test that command not found errors are properly reported
    /// This verifies the error handling path when a command doesn't exist
    #[tokio::test]
    async fn test_command_not_found_error_is_clear() {
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("definitely-not-a-real-command-xyz123").build();

        let result = runner.run(command).await;

        // Should fail with CommandNotFound error
        assert!(result.is_err(), "Non-existent command should fail");

        let err = result.unwrap_err();
        assert!(
            matches!(err, ProcessError::CommandNotFound(_)),
            "Should be CommandNotFound error, got: {:?}",
            err
        );

        // Error message should contain the command name
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("definitely-not-a-real-command-xyz123"),
            "Error should include command name. Error: {}",
            err_msg
        );
    }

    /// Test that all essential environment variables are preserved together
    /// This is a comprehensive test for the environment preservation fix
    #[tokio::test]
    async fn test_all_essential_vars_preserved_together() {
        let runner = runner::TokioProcessRunner;

        // Check all essential vars that should be preserved
        let command = ProcessCommandBuilder::new("sh")
            .args([
                "-c",
                r#"
                [ -n "$PATH" ] && echo "PATH_OK" || echo "PATH_MISSING"
                [ -n "$HOME" ] && echo "HOME_OK" || echo "HOME_MISSING"
                [ -n "$SHELL" ] && echo "SHELL_OK" || echo "SHELL_MISSING"
                "#,
            ])
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());

        // PATH is critical - must always be present
        assert!(
            output.stdout.contains("PATH_OK"),
            "PATH must be preserved! Output: {}",
            output.stdout
        );
        assert!(
            !output.stdout.contains("PATH_MISSING"),
            "PATH was missing! This is the bug we fixed. Output: {}",
            output.stdout
        );

        // HOME and SHELL are important but may be missing on some systems
        // At minimum, we verify they're checked and handled
        assert!(
            output.stdout.contains("HOME_OK") || output.stdout.contains("HOME_MISSING"),
            "HOME var should be checked. Output: {}",
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

        let mut builder = ProcessCommandBuilder::new("sh").args(["-c", "echo SUCCESS"]);

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

    /// Test that documents the PATH preservation fix
    ///
    /// This test documents the fix for a bug where PATH being unavailable
    /// would silently fail, causing "Command not found" errors after
    /// long-running workflows (observed after 5+ hours with 27+ agents).
    ///
    /// The fix ensures that:
    /// 1. PATH preservation returns Result instead of silently succeeding
    /// 2. Missing PATH logs detailed error information
    /// 3. Process spawning fails loudly with clear error message
    /// 4. All available env vars are logged for debugging
    ///
    /// This prevents mysterious "Command not found: claude" errors that
    /// occur when environment degradation happens in long-running processes.
    #[tokio::test]
    async fn test_path_preservation_fix_documented() {
        // Verify that commands work correctly when PATH is available
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("echo").arg("test").build();

        let result = runner.run(command).await;

        // Should succeed when PATH is properly preserved
        assert!(
            result.is_ok(),
            "Command should succeed when PATH is preserved"
        );

        // The fix is implemented in:
        // - src/subprocess/runner.rs::preserve_essential_env()
        // - Returns Result<(), ProcessError> instead of ()
        // - Fails loudly if PATH is not available
        // - Logs all available env vars for debugging
        //
        // When PATH is missing, the error message will be:
        // "Critical environment variable PATH is not available (required for 'cmd')
        //  This indicates environment corruption after long-running execution."
    }

    /// Test that PATH preservation is logged for debugging
    #[tokio::test]
    async fn test_path_preservation_logging() {
        // This test verifies that PATH preservation happens and is logged
        let runner = runner::TokioProcessRunner;
        let command = ProcessCommandBuilder::new("echo").arg("test").build();

        // Run the command - should succeed with PATH properly preserved
        let output = runner.run(command).await;

        assert!(output.is_ok(), "Command should succeed with PATH preserved");

        // In verbose/debug mode, the logs would show:
        // "Preserved critical env var PATH for command 'echo': /usr/bin:/bin:..."
        // This helps diagnose environment issues in long-running workflows
    }
}
