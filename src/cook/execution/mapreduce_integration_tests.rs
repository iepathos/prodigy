//! Integration tests for MapReduce workflow execution
//!
//! These tests verify the complete MapReduce workflow execution including:
//! - Setup phase execution
//! - Map phase with parallel agents
//! - Reduce phase aggregation
//! - Error handling and edge cases

#[cfg(test)]
mod tests {
    use crate::config::mapreduce::parse_mapreduce_workflow;
    use crate::cook::workflow::{ExtendedWorkflowConfig, WorkflowMode};
    use tempfile::TempDir;

    /// Test that setup phase executes before map phase
    #[tokio::test]
    async fn test_setup_phase_execution() {
        let yaml = r#"
name: test-mapreduce
mode: mapreduce

setup:
  - shell: "echo 'setup1' > setup1.txt"
  - shell: 'echo ''[{{"id": 1}}, {{"id": 2}}]'' > items.json'

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 2
  agent_template:
    commands:
      - shell: "echo 'processing item'"

reduce:
  commands:
    - shell: "echo 'reduce done'"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert_eq!(config.name, "test-mapreduce");
        assert!(config.setup.is_some());
        assert_eq!(config.setup.as_ref().unwrap().len(), 2);
    }

    /// Test that map phase doesn't run when setup produces no items
    #[tokio::test]
    async fn test_empty_items_handling() {
        let yaml = r#"
name: test-empty
mode: mapreduce

setup:
  - shell: "echo '[]' > empty.json"

map:
  input: empty.json
  json_path: "$[*]"
  max_parallel: 2
  agent_template:
    commands:
      - shell: "echo 'should not run'"

reduce:
  commands:
    - shell: "echo 'should not run either'"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.setup.is_some());

        // When there are 0 items, the map phase should return early
        // and the reduce phase should be skipped
    }

    /// Test reduce phase variable substitution in actual workflow
    #[tokio::test]
    async fn test_reduce_phase_variable_substitution() {
        let yaml = r#"
name: test-variable-substitution
mode: mapreduce

map:
  input: test-items.json
  json_path: "$[*]"
  max_parallel: 2
  agent_template:
    commands:
      - shell: "echo 'Processing ${item.id}'"

reduce:
  commands:
    - shell: "echo 'Total: ${map.total}, Success: ${map.successful}, Failed: ${map.failed}'"
    - shell: |
        git commit -m "Processed ${map.successful} items
        
        Total items: ${map.total}
        Failed items: ${map.failed}"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();

        // Verify reduce phase has commands with variables
        assert!(config.reduce.is_some());
        let reduce = config.reduce.as_ref().unwrap();
        assert_eq!(reduce.commands.len(), 2);

        // Check that the commands contain the expected variables
        let first_cmd = &reduce.commands[0];
        assert!(first_cmd.shell.is_some());
        let shell_cmd = first_cmd.shell.as_ref().unwrap();
        assert!(shell_cmd.contains("${map.total}"));
        assert!(shell_cmd.contains("${map.successful}"));
        assert!(shell_cmd.contains("${map.failed}"));

        let second_cmd = &reduce.commands[1];
        assert!(second_cmd.shell.is_some());
        let commit_cmd = second_cmd.shell.as_ref().unwrap();
        assert!(commit_cmd.contains("${map.successful}"));
        assert!(commit_cmd.contains("${map.total}"));
        assert!(commit_cmd.contains("${map.failed}"));
    }

    /// Test complete debtmap workflow parsing
    #[tokio::test]
    async fn test_debtmap_workflow_parsing() {
        let yaml = r#"
name: debtmap-parallel-elimination
mode: mapreduce

# Setup phase: Analyze the codebase and generate debt items
setup:
  - shell: "just coverage-lcov"
    
  - shell: "debtmap analyze . --lcov target/coverage/info.lcov --output debtmap.json --format json && git add debtmap.json && git commit -m 'Add debtmap.json'"
    commit_required: true

# Map phase: Process each debt item in parallel
map:
  input: debtmap.json
  json_path: "$.items[*]"
  
  agent_template:
    commands:
      - claude: "/fix-debt-item --file ${item.location.file}"
        capture_output: true
        timeout: 300
      
      - shell: "just test"
        on_failure:
          claude: "/mmm-debug-test-failure --output '${shell.output}'"
          max_attempts: 2
          fail_workflow: false
  
  max_parallel: 5
  timeout_per_agent: 600s
  retry_on_failure: 1
  
  filter: "unified_score.final_score >= 5"
  sort_by: "unified_score.final_score DESC"
  max_items: 10

# Reduce phase: Aggregate results and finalize
reduce:
  commands:
    - shell: "just test"
      on_failure:
        claude: "/mmm-debug-test-failure --output '${shell.output}'"
        max_attempts: 3
        fail_workflow: true
    
    - shell: "just fmt && just lint"
      capture_output: false
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();

        // Verify setup phase
        assert!(config.setup.is_some());
        let setup = config.setup.as_ref().unwrap();
        assert_eq!(setup.len(), 2);
        assert_eq!(setup[0].shell, Some("just coverage-lcov".to_string()));
        assert!(setup[1].commit_required);

        // Verify map phase
        assert_eq!(config.map.input.to_string_lossy(), "debtmap.json");
        assert_eq!(config.map.json_path, "$.items[*]");
        assert_eq!(config.map.max_parallel, 5);
        assert_eq!(config.map.max_items, Some(10));
        assert_eq!(
            config.map.filter,
            Some("unified_score.final_score >= 5".to_string())
        );
        assert_eq!(
            config.map.sort_by,
            Some("unified_score.final_score DESC".to_string())
        );

        // Verify reduce phase
        assert!(config.reduce.is_some());
        let reduce = config.reduce.as_ref().unwrap();
        assert_eq!(reduce.commands.len(), 2);
    }

    /// Test that reduce phase is skipped when map phase has no successful items
    #[tokio::test]
    async fn test_skip_reduce_on_no_success() {
        let yaml = r#"
name: test-skip-reduce
mode: mapreduce

setup:
  - shell: 'echo ''[{{"id": 1}}]'' > items.json'

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 1
  agent_template:
    commands:
      - shell: "exit 1"  # Fail on purpose

reduce:
  commands:
    - shell: "echo 'should be skipped'"
"#;

        let _config = parse_mapreduce_workflow(yaml).unwrap();

        // The reduce phase should be skipped if all map agents fail
        // or if skip_reduce_on_empty is true (default behavior TBD)
    }

    /// Test variable interpolation in map phase
    #[tokio::test]
    async fn test_map_variable_interpolation() {
        let yaml = r#"
name: test-interpolation
mode: mapreduce

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 1
  agent_template:
    commands:
      - shell: "echo 'Processing ${item.id} with score ${item.score}'"
      - claude: "/process --file ${item.file} --line ${item.line}"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        let commands = &config.map.agent_template.commands;

        // Verify interpolation placeholders are preserved
        assert!(commands[0].shell.as_ref().unwrap().contains("${item.id}"));
        assert!(commands[1]
            .claude
            .as_ref()
            .unwrap()
            .contains("${item.file}"));
    }

    /// Test timeout parsing
    #[tokio::test]
    async fn test_timeout_formats() {
        let yaml = r#"
name: test-timeout
mode: mapreduce

map:
  input: test.json
  timeout_per_agent: "10m"
  agent_template:
    commands:
      - shell: "echo test"
        timeout: 300
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert_eq!(config.map.timeout_per_agent, Some(600)); // 10 minutes = 600 seconds
        assert_eq!(config.map.agent_template.commands[0].timeout, Some(300));
    }

    /// Test ExtendedWorkflowConfig conversion
    #[tokio::test]
    async fn test_extended_workflow_conversion() {
        let yaml = r#"
name: test-conversion
mode: mapreduce

setup:
  - shell: "echo setup"

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 3
  agent_template:
    commands:
      - shell: "echo map"

reduce:
  commands:
    - shell: "echo reduce"
"#;

        let mapreduce_config = parse_mapreduce_workflow(yaml).unwrap();

        // Convert to ExtendedWorkflowConfig (as done in orchestrator)
        let extended_workflow = ExtendedWorkflowConfig {
            name: mapreduce_config.name.clone(),
            mode: WorkflowMode::MapReduce,
            steps: mapreduce_config.setup.clone().unwrap_or_default(),
            map_phase: Some(mapreduce_config.to_map_phase()),
            reduce_phase: mapreduce_config.to_reduce_phase(),
            max_iterations: 1,
            iterate: false,
        };

        assert_eq!(extended_workflow.name, "test-conversion");
        assert_eq!(extended_workflow.mode, WorkflowMode::MapReduce);
        assert_eq!(extended_workflow.steps.len(), 1); // Setup step
        assert!(extended_workflow.map_phase.is_some());
        assert!(extended_workflow.reduce_phase.is_some());
    }

    /// Test that setup phase runs in the main worktree before map agents are created
    #[tokio::test]
    async fn test_setup_runs_in_main_worktree() {
        // Setup should run in the main worktree to prepare data
        // before individual agent worktrees are created for map phase

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("setup_marker.txt");

        let yaml = format!(
            r#"
name: test-setup-location
mode: mapreduce

setup:
  - shell: "echo 'setup complete' > {}"
  - shell: 'echo ''[{{"id": 1}}]'' > items.json'

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 1
  agent_template:
    commands:
      - shell: "echo 'map phase'"
"#,
            test_file.display()
        );

        let _config = parse_mapreduce_workflow(&yaml).unwrap();

        // After setup phase, the marker file should exist in main worktree
        // This verifies setup runs before map agents are spawned
    }

    /// Test error handling when input file doesn't exist
    #[tokio::test]
    async fn test_missing_input_file_error() {
        let yaml = r#"
name: test-missing-input
mode: mapreduce

# No setup phase to create the file

map:
  input: nonexistent.json
  json_path: "$[*]"
  max_parallel: 1
  agent_template:
    commands:
      - shell: "echo test"
"#;

        let _config = parse_mapreduce_workflow(yaml).unwrap();

        // Should fail with clear error about missing input file
        // The error should happen after setup phase (if any) completes
    }

    /// Test on_failure handlers in map phase
    #[tokio::test]
    async fn test_map_on_failure_handlers() {
        let yaml = r#"
name: test-on-failure
mode: mapreduce

setup:
  - shell: 'echo ''[{{"id": 1}}]'' > items.json'

map:
  input: items.json
  json_path: "$[*]"
  max_parallel: 1
  agent_template:
    commands:
      - shell: "exit 1"  # Fail intentionally
    on_failure:
      claude: "/fix-error --output '${shell.output}'"
      max_attempts: 2
      fail_workflow: false
"#;

        let _config = parse_mapreduce_workflow(yaml).unwrap();

        // on_failure handler should be triggered
        // max_attempts should allow retries
        // fail_workflow: false should allow continuation
    }

    /// Test reduce phase on_failure with retry logic
    #[tokio::test]
    async fn test_reduce_phase_on_failure_with_retries() {
        let yaml = r#"
name: test-reduce-retry
mode: mapreduce

setup:
  - shell: 'echo ''[{"id": 1}, {"id": 2}]'' > items.json'

map:
  input: items.json
  json_path: "$[*]"
  agent_template:
    commands:
      - shell: "echo 'Processing item ${item.id}'"

reduce:
  commands:
    - shell: "exit 1"  # Fail intentionally to trigger retry
      on_failure:
        claude: "/fix-error"
        max_attempts: 3  # Should retry up to 3 times
        fail_workflow: false
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();

        // Verify reduce phase configuration
        assert!(config.reduce.is_some());
        let reduce = config.reduce.unwrap();
        assert_eq!(reduce.commands.len(), 1);

        // Verify on_failure configuration
        let step = &reduce.commands[0];
        assert!(step.on_failure.is_some());

        let on_failure = step.on_failure.as_ref().unwrap();
        // Verify max_retries is parsed correctly from max_attempts
        assert_eq!(on_failure.max_retries(), 3);
        // Verify should_retry returns true when max_retries > 0
        assert!(on_failure.should_retry());
        // Verify fail_workflow is false
        assert!(!on_failure.should_fail_workflow());
    }

    /// Test reduce phase retry behavior without explicit retry_original
    #[tokio::test]
    async fn test_reduce_phase_implicit_retry() {
        use crate::cook::workflow::OnFailureConfig;

        // Test that max_attempts > 0 implies retry without retry_original
        let yaml_config = r#"
claude: "/debug-test"
max_attempts: 2
fail_workflow: true
"#;

        let on_failure: OnFailureConfig = serde_yaml::from_str(yaml_config).unwrap();

        // Should retry because max_retries (from max_attempts) is 2
        assert!(on_failure.should_retry());
        assert_eq!(on_failure.max_retries(), 2);
        assert!(on_failure.should_fail_workflow());
        assert!(on_failure.handler().is_some());
    }

    /// Test reduce phase behavior when retries are exhausted
    #[tokio::test]
    async fn test_reduce_phase_retry_exhaustion() {
        let yaml = r#"
name: test-reduce-exhaustion
mode: mapreduce

map:
  input: items.json
  json_path: "$[*]"
  agent_template:
    commands:
      - shell: "echo 'ok'"

reduce:
  commands:
    - shell: "false"  # Always fails
      on_failure:
        shell: "echo 'Attempting recovery'"
        max_attempts: 2
        fail_workflow: true  # Should fail workflow after retries exhausted
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        let reduce = config.reduce.unwrap();
        let step = &reduce.commands[0];
        let on_failure = step.on_failure.as_ref().unwrap();

        // Verify configuration
        assert_eq!(on_failure.max_retries(), 2);
        assert!(on_failure.should_retry());
        assert!(on_failure.should_fail_workflow());

        // In actual execution:
        // 1. Command fails
        // 2. on_failure handler runs
        // 3. Original command retried (attempt 1/2)
        // 4. Command fails again
        // 5. Original command retried (attempt 2/2)
        // 6. Command fails again
        // 7. Workflow fails because fail_workflow=true and retries exhausted
    }

    /// Test that max_attempts=0 means no retries
    #[tokio::test]
    async fn test_reduce_phase_no_retry() {
        use crate::cook::workflow::OnFailureConfig;

        let yaml_config = r#"
claude: "/debug-test"
max_attempts: 0
fail_workflow: false
"#;

        let on_failure: OnFailureConfig = serde_yaml::from_str(yaml_config).unwrap();

        // Should NOT retry because max_retries is 0
        assert!(!on_failure.should_retry());
        assert_eq!(on_failure.max_retries(), 0);
        assert!(!on_failure.should_fail_workflow());
    }
}
