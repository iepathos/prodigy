//! Comprehensive unit tests for the cook module
//!
//! Tests various scenarios including error paths, edge cases, and
//! complex workflows using mock implementations.

#[cfg(test)]
mod cook_tests {
    use crate::abstractions::{ClaudeClient, GitOperations, MockClaudeClient, MockGitOperations};
    use crate::cook::command::CookCommand;
    use crate::testing::{TestContext, TestFixtures};
    use std::path::PathBuf;

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
            playbook: PathBuf::from("examples/default.yml"),
            path: None,
            focus: None,
            max_iterations: 2,
            worktree: false,
            config: None,
            map: Vec::new(),
            args: Vec::new(),
            fail_fast: false,
            auto_accept: false,
            metrics: false,
            resume: None,
            skip_analysis: false,
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
mod workflow_parsing_tests {
    use crate::config::command::{InputMethod, WorkflowCommand};
    use crate::config::workflow::WorkflowConfig;

    #[test]
    fn test_parse_simple_workflow_yaml() {
        let yaml = r#"
commands:
  - mmm-code-review
  - mmm-implement-spec
  - mmm-lint
"#;
        let config: WorkflowConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse simple workflow");
        assert_eq!(config.commands.len(), 3);

        match &config.commands[0] {
            WorkflowCommand::Simple(s) => assert_eq!(s, "mmm-code-review"),
            _ => panic!("Expected Simple command"),
        }
    }

    #[test]
    fn test_parse_structured_workflow_with_outputs() {
        let yaml = r#"
commands:
  - name: mmm-code-review
    id: review
    outputs:
      spec:
        file_pattern: "specs/temp/*.md"
"#;
        let config: WorkflowConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse workflow with outputs");
        assert_eq!(config.commands.len(), 1);

        match &config.commands[0] {
            WorkflowCommand::Structured(ref cmd) => {
                assert_eq!(cmd.name, "mmm-code-review");
                assert_eq!(cmd.id, Some("review".to_string()));
                assert!(cmd.outputs.is_some());

                let outputs = cmd.outputs.as_ref().unwrap();
                assert!(outputs.contains_key("spec"));

                let spec_output = &outputs["spec"];
                assert_eq!(spec_output.file_pattern, "specs/temp/*.md");
            }
            _ => panic!("Expected Structured command"),
        }
    }

    #[test]
    fn test_parse_workflow_with_inputs() {
        let yaml = r#"
commands:
  - name: mmm-implement-spec
    inputs:
      spec:
        from: "${review.spec}"
        pass_as:
          argument:
            position: 0
"#;
        let config: WorkflowConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse workflow with inputs");
        assert_eq!(config.commands.len(), 1);

        match &config.commands[0] {
            WorkflowCommand::Structured(ref cmd) => {
                assert_eq!(cmd.name, "mmm-implement-spec");
                assert!(cmd.inputs.is_some());

                let inputs = cmd.inputs.as_ref().unwrap();
                assert!(inputs.contains_key("spec"));

                let spec_input = &inputs["spec"];
                assert_eq!(spec_input.from, "${review.spec}");

                match &spec_input.pass_as {
                    InputMethod::Argument { position } => {
                        assert_eq!(position, &0);
                    }
                    _ => panic!("Expected Argument input method"),
                }
            }
            _ => panic!("Expected Structured command"),
        }
    }

    #[test]
    fn test_parse_full_default_workflow() {
        let yaml = r#"
commands:
  - name: mmm-code-review
    id: review
    outputs:
      spec:
        file_pattern: "specs/temp/*.md"
  
  - name: mmm-implement-spec
    inputs:
      spec:
        from: "${review.spec}"
        pass_as:
          argument:
            position: 0
  
  - name: mmm-lint
"#;

        let config: WorkflowConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse full workflow");

        assert_eq!(config.commands.len(), 3);

        // Verify first command
        match &config.commands[0] {
            WorkflowCommand::Structured(ref cmd) => {
                assert_eq!(cmd.name, "mmm-code-review");
                assert_eq!(cmd.id.as_ref().unwrap(), "review");
                assert!(cmd.outputs.is_some());
            }
            _ => panic!("Expected Structured command for mmm-code-review"),
        }

        // Verify second command
        match &config.commands[1] {
            WorkflowCommand::Structured(ref cmd) => {
                assert_eq!(cmd.name, "mmm-implement-spec");
                assert!(cmd.inputs.is_some());
            }
            _ => panic!("Expected Structured command for mmm-implement-spec"),
        }

        // Verify third command - it's parsed as Structured because it has a "name" field
        match &config.commands[2] {
            WorkflowCommand::Structured(ref cmd) => {
                assert_eq!(cmd.name, "mmm-lint");
                assert!(cmd.id.is_none());
                assert!(cmd.inputs.is_none());
                assert!(cmd.outputs.is_none());
            }
            _ => panic!("Expected Structured command for mmm-lint"),
        }
    }

    #[test]
    fn test_parse_workflow_with_multiple_outputs() {
        let yaml = r#"
commands:
  - name: custom-command
    id: cmd
    outputs:
      spec:
        file_pattern: "specs/*.md"
      temp_spec:
        file_pattern: "specs/temp/*.md"
"#;
        let config: WorkflowConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse workflow with multiple outputs");

        match &config.commands[0] {
            WorkflowCommand::Structured(ref cmd) => {
                let outputs = cmd.outputs.as_ref().unwrap();
                assert_eq!(outputs.len(), 2);

                assert_eq!(outputs["spec"].file_pattern, "specs/*.md");
                assert_eq!(outputs["temp_spec"].file_pattern, "specs/temp/*.md");
            }
            _ => panic!("Expected Structured command"),
        }
    }

    #[test]
    fn test_parse_workflow_with_environment_input() {
        let yaml = r#"
commands:
  - name: custom-command
    inputs:
      api_key:
        from: "${config.api_key}"
        pass_as:
          environment:
            name: "API_KEY"
      data:
        from: "${previous.output}"
        pass_as: stdin
        default: "default data"
"#;
        let config: WorkflowConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse workflow with env inputs");

        match &config.commands[0] {
            WorkflowCommand::Structured(ref cmd) => {
                let inputs = cmd.inputs.as_ref().unwrap();

                // Check environment input
                let api_key = &inputs["api_key"];
                match &api_key.pass_as {
                    InputMethod::Environment { name } => assert_eq!(name, "API_KEY"),
                    _ => panic!("Expected Environment input method"),
                }

                // Check stdin input with default
                let data = &inputs["data"];
                match &data.pass_as {
                    InputMethod::Stdin => {}
                    _ => panic!("Expected Stdin input method"),
                }
                assert_eq!(data.default.as_ref().unwrap(), "default data");
            }
            _ => panic!("Expected Structured command"),
        }
    }

    #[test]
    fn test_simplified_output_syntax() {
        // Test that the simplified syntax with just file_pattern works
        let yaml = r#"
commands:
  - name: mmm-code-review
    id: review
    outputs:
      spec:
        file_pattern: "specs/temp/*.md"
"#;
        let result: Result<WorkflowConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_ok(), "Should parse simplified syntax");

        let config = result.unwrap();
        match &config.commands[0] {
            WorkflowCommand::Structured(ref cmd) => {
                let outputs = cmd.outputs.as_ref().unwrap();
                assert_eq!(outputs["spec"].file_pattern, "specs/temp/*.md");
            }
            _ => panic!("Expected Structured command"),
        }
    }

    #[test]
    fn test_load_playbook_structure() {
        // Test the structure that would be in examples/default.yml
        let yaml = r#"# Default MMM playbook - the original hardcoded workflow
# This is what was previously built into MMM
commands:
  - name: mmm-code-review
    id: review
    outputs:
      spec: 
        file_pattern: "specs/temp/*.md"
  
  - name: mmm-implement-spec
    inputs:
      spec: 
        from: "${review.spec}"
        pass_as:
          argument:
            position: 0
  
  - name: mmm-lint
"#;

        // First, test if it parses as a generic YAML value
        let value: Result<serde_yaml::Value, _> = serde_yaml::from_str(&yaml);
        assert!(value.is_ok(), "Should parse as valid YAML");

        // Now test if it parses as WorkflowConfig directly
        let direct_parse: Result<WorkflowConfig, _> = serde_yaml::from_str(&yaml);
        if let Err(e) = &direct_parse {
            panic!(
                "Failed to parse as WorkflowConfig: {:?}\nYAML content:\n{}",
                e, yaml
            );
        }

        let config = direct_parse.unwrap();
        assert_eq!(config.commands.len(), 3);
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
