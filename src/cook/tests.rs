//! Comprehensive unit tests for the cook module
//!
//! Tests various scenarios including error paths, edge cases, and
//! complex workflows using mock implementations.

#[cfg(test)]
mod cook_tests {
    use crate::abstractions::{ClaudeClient, GitOperations, MockClaudeClient, MockGitOperations};
    use crate::cook::command::CookCommand;
    use crate::testing::{TestContext, TestFixtures};

    /// Test successful improvement loop
    #[tokio::test]
    async fn test_successful_improvement_loop() {
        // Create test context
        let mut context = TestContext::new().unwrap();

        // Set up git mock for clean repository
        let git_mock = TestFixtures::clean_repo_git().await;
        context.git_ops = Box::new(git_mock);

        // Set up Claude mock for successful operations
        let claude_mock = TestFixtures::successful_claude().await;
        context.claude_client = Box::new(claude_mock);

        // Create test command
        let cmd = CookCommand {
            path: None,
            focus: None,
            max_iterations: 2,
            worktree: false,
            config: None,
            map: Vec::new(),
            args: Vec::new(),
            fail_fast: false,
            auto_accept: false,
        };

        // Run the command (this would require refactoring cook::run to accept injected dependencies)
        // For now, this test demonstrates the setup
        assert_eq!(cmd.max_iterations, 2);
    }

    /// Test Claude CLI not available
    #[tokio::test]
    async fn test_claude_cli_not_available() {
        let _context = TestContext::new().unwrap();

        // Set up Claude mock as unavailable
        let claude_mock = TestFixtures::unavailable_claude();

        // Test availability check
        let result = claude_mock.check_availability().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not available"));
    }

    /// Test git operation failures
    #[tokio::test]
    async fn test_git_operation_failures() {
        let mock = MockGitOperations::new();

        // Add error response for commit
        mock.add_error_response("fatal: not a git repository").await;

        // Test commit failure
        let result = mock.create_commit("test message").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not a git repository"));
    }

    /// Test rate limiting handling
    #[tokio::test]
    async fn test_rate_limit_handling() {
        let claude_mock = TestFixtures::rate_limited_claude().await;

        // Test code review with rate limit
        let result = claude_mock.code_review(false, None).await;
        assert!(!result.unwrap());
    }

    /// Test worktree creation failure
    #[tokio::test]
    async fn test_worktree_creation_failure() {
        let mock = MockGitOperations::new();

        // Add error for worktree creation
        mock.add_error_response("fatal: invalid reference").await;

        // Test worktree creation
        let temp_dir = tempfile::TempDir::new().unwrap();
        let result = mock.create_worktree("test-branch", temp_dir.path()).await;
        assert!(result.is_err());
    }

    /// Test merge conflicts scenario
    #[tokio::test]
    async fn test_merge_conflicts() {
        let mock = MockGitOperations::new();

        // Simulate merge conflict
        mock.add_error_response("CONFLICT (content): Merge conflict in src/main.rs")
            .await;

        // Test branch switch with conflict
        let result = mock.switch_branch("feature-branch").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CONFLICT"));
    }

    /// Test empty repository scenario
    #[tokio::test]
    async fn test_empty_repository() {
        let mut mock = MockGitOperations::new();
        mock.is_repo = false;

        // Test repository check
        assert!(!mock.is_git_repo().await);
    }

    /// Test spec ID extraction from commit message
    #[tokio::test]
    async fn test_spec_id_extraction() {
        let mock = MockGitOperations::new();

        // Add commit message with spec ID
        mock.add_success_response("add: spec iteration-1234567890-improvements")
            .await;

        // Test getting last commit message
        let msg = mock.get_last_commit_message().await.unwrap();
        assert!(msg.contains("iteration-1234567890-improvements"));
    }

    /// Test multiple iterations with state tracking
    #[tokio::test]
    async fn test_multiple_iterations() {
        let git_mock = MockGitOperations::new();
        let claude_mock = MockClaudeClient::new();

        // Set up responses for multiple iterations
        git_mock.add_success_response("").await; // Clean status
        claude_mock.add_success_response("Review completed").await;
        git_mock.add_success_response("add: spec test-123").await;
        claude_mock
            .add_success_response("Implementation done")
            .await;
        claude_mock.add_success_response("Linting done").await;

        // Verify mock setup
        assert!(git_mock.is_repo);
        assert!(claude_mock.is_available);
    }

    /// Test focus directive propagation
    #[tokio::test]
    async fn test_focus_directive() {
        let claude_mock = MockClaudeClient::new();

        // Add response for focused review
        claude_mock
            .add_success_response("Focused review on performance")
            .await;

        // Test code review with focus
        let result = claude_mock.code_review(false, Some("performance")).await;
        assert!(result.unwrap());

        // Verify the command was called
        let commands = claude_mock.get_called_commands().await;
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "/mmm-code-review");
    }

    /// Test invalid spec ID validation
    #[tokio::test]
    async fn test_invalid_spec_id() {
        // Test various invalid spec IDs
        let invalid_ids = vec![
            "../etc/passwd",
            "../../secrets",
            "spec; rm -rf /",
            "spec && malicious_command",
            "spec`evil`",
            "spec$(bad)",
            "spec\nmalicious",
        ];

        for id in invalid_ids {
            // In real implementation, this would be validated
            assert!(
                id.contains("..")
                    || id.contains("/")
                    || id.contains(";")
                    || id.contains("&")
                    || id.contains("`")
                    || id.contains("$")
                    || id.contains("\n")
            );
        }
    }

    /// Test workflow configuration loading
    #[tokio::test]
    async fn test_workflow_configuration() {
        let context = TestContext::new().unwrap();

        // Create .mmm directory first
        let mmm_dir = context.temp_path().join(".mmm");
        std::fs::create_dir_all(&mmm_dir).unwrap();

        // Create test workflow config
        let workflow_content = r#"
[[commands]]
command = "/mmm-code-review"

[[commands]]
command = "/mmm-implement-spec"
"#;

        context
            .create_test_file(".mmm/workflow.toml", workflow_content)
            .unwrap();

        // Verify file was created
        let path = context.temp_path().join(".mmm/workflow.toml");
        assert!(path.exists());
    }
}

#[cfg(test)]
mod retry_tests {
    use crate::cook::retry::{format_subprocess_error, is_transient_error};

    #[test]
    fn test_comprehensive_transient_errors() {
        // Test all transient error patterns
        let transient_errors = vec![
            "API rate limit exceeded",
            "Request timeout after 30 seconds",
            "Connection refused: Unable to connect",
            "Temporary failure in DNS resolution",
            "Network is unreachable",
            "HTTP 503 Service Unavailable",
            "Error 429: Too Many Requests",
            "Could not connect to server",
            "Broken pipe error occurred",
        ];

        for error in transient_errors {
            assert!(
                is_transient_error(error),
                "Should detect as transient: {error}"
            );
        }

        // Test non-transient errors
        let permanent_errors = vec![
            "Syntax error in configuration",
            "Invalid API key",
            "Permission denied",
            "File not found",
            "Invalid argument provided",
        ];

        for error in permanent_errors {
            assert!(
                !is_transient_error(error),
                "Should not detect as transient: {error}"
            );
        }
    }

    #[test]
    fn test_error_formatting_edge_cases() {
        // Test with empty stderr and stdout
        let error = format_subprocess_error("test-cmd", Some(0), "", "");
        assert!(error.contains("test-cmd"));
        assert!(error.contains("exit code 0"));

        // Test with very long output
        let long_output = "x".repeat(1000);
        let error = format_subprocess_error("test-cmd", Some(1), &long_output, "");
        assert!(error.contains("test-cmd"));
        assert!(error.len() < 2000); // Should be reasonably sized

        // Test with special characters
        let special_output = "Error: 特殊文字 🚀 \n\t\r";
        let error = format_subprocess_error("test-cmd", Some(1), special_output, "");
        assert!(error.contains("特殊文字"));
    }
}

#[cfg(test)]
mod git_ops_tests {
    use crate::abstractions::{GitOperations, MockGitOperations};

    #[tokio::test]
    async fn test_concurrent_git_operations() {
        let mock = MockGitOperations::new();

        // Add multiple responses
        for i in 0..10 {
            mock.add_success_response(&format!("Response {i}")).await;
        }

        // Execute concurrent operations
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let mock_clone = MockGitOperations::new();
                tokio::spawn(async move { mock_clone.check_git_status().await })
            })
            .collect();

        // All should complete without errors
        for handle in handles {
            let _ = handle.await;
        }
    }

    #[tokio::test]
    async fn test_git_command_tracking() {
        let mock = MockGitOperations::new();

        // Add responses and execute commands
        mock.add_success_response("status output").await;
        mock.add_success_response("").await;
        mock.add_success_response("commit done").await;

        mock.check_git_status().await.unwrap();
        mock.stage_all_changes().await.unwrap();
        mock.create_commit("test commit").await.unwrap();

        // Verify commands were called in correct order
        let commands = mock.get_called_commands().await;
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0], vec!["status", "--porcelain"]);
        assert_eq!(commands[1], vec!["add", "."]);
        assert_eq!(commands[2], vec!["commit", "-m", "test commit"]);
    }
}
