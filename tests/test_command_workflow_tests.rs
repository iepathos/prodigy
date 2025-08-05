use anyhow::Result;
use mmm::config::command::{TestCommand, TestDebugConfig, WorkflowCommand, WorkflowStepCommand};
use mmm::config::workflow::WorkflowConfig;

#[test]
fn test_parse_test_command_basic() {
    let yaml = r#"
test:
  command: "cargo test"
"#;

    let step: WorkflowStepCommand = serde_yaml::from_str(yaml).unwrap();
    assert!(step.test.is_some());
    let test_cmd = step.test.unwrap();
    assert_eq!(test_cmd.command, "cargo test");
    assert!(test_cmd.on_failure.is_none());
}

#[test]
fn test_parse_test_command_with_on_failure() {
    let yaml = r#"
test:
  command: "cargo test"
  on_failure:
    claude: "/mmm-debug-test-failure --spec ${spec} --output ${test.output}"
    max_attempts: 3
    fail_workflow: false
"#;

    let step: WorkflowStepCommand = serde_yaml::from_str(yaml).unwrap();
    assert!(step.test.is_some());
    let test_cmd = step.test.unwrap();
    assert_eq!(test_cmd.command, "cargo test");

    assert!(test_cmd.on_failure.is_some());
    let debug_config = test_cmd.on_failure.unwrap();
    assert_eq!(
        debug_config.claude,
        "/mmm-debug-test-failure --spec ${spec} --output ${test.output}"
    );
    assert_eq!(debug_config.max_attempts, 3);
    assert!(!debug_config.fail_workflow);
    assert!(debug_config.commit_required); // Should default to true
}

#[test]
fn test_parse_test_command_with_defaults() {
    let yaml = r#"
test:
  command: "cargo test"
  on_failure:
    claude: "/mmm-debug-test-failure"
"#;

    let step: WorkflowStepCommand = serde_yaml::from_str(yaml).unwrap();
    let test_cmd = step.test.unwrap();
    let debug_config = test_cmd.on_failure.unwrap();

    // Check defaults
    assert_eq!(debug_config.max_attempts, 3); // default
    assert!(!debug_config.fail_workflow); // default false
    assert!(debug_config.commit_required); // default true
}

#[test]
fn test_workflow_with_test_commands() {
    let yaml = r#"
commands:
  - claude: "/mmm-implement-spec ${spec}"
  
  - test:
      command: "cargo test"
      on_failure:
        claude: "/mmm-debug-test-failure --spec ${spec} --output ${test.output}"
        max_attempts: 3
        
  - test:
      command: "cargo test --doc"
      on_failure:
        claude: "/mmm-fix-doc-tests --output ${test.output}"
        max_attempts: 2
        fail_workflow: false
        
  - claude: "/mmm-lint"
    commit_required: false
"#;

    let config: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.commands.len(), 4);

    // Check first command
    match &config.commands[0] {
        WorkflowCommand::WorkflowStep(step) => {
            assert_eq!(step.claude, Some("/mmm-implement-spec ${spec}".to_string()));
        }
        _ => panic!("Expected WorkflowStep"),
    }

    // Check second command (first test)
    match &config.commands[1] {
        WorkflowCommand::WorkflowStep(step) => {
            assert!(step.test.is_some());
            let test_cmd = step.test.as_ref().unwrap();
            assert_eq!(test_cmd.command, "cargo test");
            assert!(test_cmd.on_failure.is_some());
        }
        _ => panic!("Expected WorkflowStep with test command"),
    }

    // Check third command (second test)
    match &config.commands[2] {
        WorkflowCommand::WorkflowStep(step) => {
            assert!(step.test.is_some());
            let test_cmd = step.test.as_ref().unwrap();
            assert_eq!(test_cmd.command, "cargo test --doc");
            let debug_config = test_cmd.on_failure.as_ref().unwrap();
            assert_eq!(debug_config.max_attempts, 2);
            assert!(!debug_config.fail_workflow);
        }
        _ => panic!("Expected WorkflowStep with test command"),
    }
}

#[test]
fn test_coverage_workflow_with_test_command() {
    let yaml = r#"
# Test coverage improvement workflow
commands:
    - claude: "/mmm-coverage"
      id: coverage
      outputs:
        spec:
          file_pattern: "*-coverage-improvements.md"
      analysis:
        max_cache_age: 300
    
    - claude: "/mmm-implement-spec ${coverage.spec}"
    
    - test:
        command: "cargo test"
        on_failure:
          claude: "/mmm-debug-test-failure --spec ${coverage.spec} --output ${test.output}"
          max_attempts: 3
    
    - claude: "/mmm-lint"
      commit_required: false
"#;

    let config: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.commands.len(), 4);

    // Verify the test command is in the right place
    match &config.commands[2] {
        WorkflowCommand::WorkflowStep(step) => {
            assert!(step.test.is_some());
            let test_cmd = step.test.as_ref().unwrap();
            assert_eq!(test_cmd.command, "cargo test");

            let debug_config = test_cmd.on_failure.as_ref().unwrap();
            assert!(debug_config.claude.contains("${coverage.spec}"));
            assert!(debug_config.claude.contains("${test.output}"));
        }
        _ => panic!("Expected WorkflowStep with test command"),
    }
}

#[test]
fn test_invalid_multiple_command_types() {
    // This test now passes because WorkflowStepCommand allows multiple command types
    // The validation is done at the executor level, not at parsing level
    let yaml = r#"
claude: "/mmm-coverage"
test:
  command: "cargo test"
"#;

    let result: Result<WorkflowStepCommand, _> = serde_yaml::from_str(yaml);
    // This now succeeds at parsing level - validation happens during execution
    assert!(result.is_ok());

    // Verify both fields are present
    let cmd = result.unwrap();
    assert!(cmd.claude.is_some());
    assert!(cmd.test.is_some());
}

#[test]
fn test_test_command_serialization() {
    let test_cmd = TestCommand {
        command: "cargo test".to_string(),
        on_failure: Some(TestDebugConfig {
            claude: "/mmm-debug-test-failure".to_string(),
            max_attempts: 5,
            fail_workflow: true,
            commit_required: false,
        }),
    };

    let yaml = serde_yaml::to_string(&test_cmd).unwrap();
    let deserialized: TestCommand = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(deserialized.command, test_cmd.command);
    let debug_config = deserialized.on_failure.unwrap();
    assert_eq!(debug_config.claude, "/mmm-debug-test-failure");
    assert_eq!(debug_config.max_attempts, 5);
    assert!(debug_config.fail_workflow);
    assert!(!debug_config.commit_required);
}
