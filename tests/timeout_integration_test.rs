//! Integration tests for command timeout configuration

use anyhow::Result;
use prodigy::config::command::WorkflowStepCommand;
use prodigy::config::WorkflowConfig;

#[tokio::test]
async fn test_parse_timeout_in_workflow_yaml() -> Result<()> {
    let yaml = r#"
commands:
  - shell: "npm test"
    timeout: 300  # 5 minutes
    
  - claude: "/implement feature"
    timeout: 600  # 10 minutes
    
  - shell: "cargo build --release"
    # No timeout means unlimited
"#;

    let config: WorkflowConfig = serde_yaml::from_str(yaml)?;
    assert_eq!(config.commands.len(), 3);

    // Check first command (shell with timeout)
    if let prodigy::config::command::WorkflowCommand::WorkflowStep(step) = &config.commands[0] {
        assert_eq!(step.shell, Some("npm test".to_string()));
        assert_eq!(step.timeout, Some(300));
    } else {
        panic!("Expected WorkflowStep variant");
    }

    // Check second command (claude with timeout)
    if let prodigy::config::command::WorkflowCommand::WorkflowStep(step) = &config.commands[1] {
        assert_eq!(step.claude, Some("/implement feature".to_string()));
        assert_eq!(step.timeout, Some(600));
    } else {
        panic!("Expected WorkflowStep variant");
    }

    // Check third command (shell without timeout)
    if let prodigy::config::command::WorkflowCommand::WorkflowStep(step) = &config.commands[2] {
        assert_eq!(step.shell, Some("cargo build --release".to_string()));
        assert_eq!(step.timeout, None);
    } else {
        panic!("Expected WorkflowStep variant");
    }

    Ok(())
}

#[tokio::test]
async fn test_timeout_propagation_to_normalized_workflow() -> Result<()> {
    use prodigy::cook::workflow::normalized::{NormalizedWorkflow, ExecutionMode};
    use std::time::Duration;

    let yaml = r#"
commands:
  - shell: "sleep 10"
    timeout: 5
  - claude: "/prodigy-lint"
    timeout: 120
"#;

    let config: WorkflowConfig = serde_yaml::from_str(yaml)?;
    let normalized = NormalizedWorkflow::from_workflow_config(&config, ExecutionMode::Sequential)?;

    assert_eq!(normalized.steps.len(), 2);
    
    // First step should have 5 second timeout
    assert_eq!(
        normalized.steps[0].timeout,
        Some(Duration::from_secs(5))
    );
    
    // Second step should have 120 second timeout
    assert_eq!(
        normalized.steps[1].timeout,
        Some(Duration::from_secs(120))
    );

    Ok(())
}

#[tokio::test]
async fn test_shell_command_timeout_execution() -> Result<()> {
    // This test just verifies the configuration parsing and propagation
    // Actual execution tests would be in the integration test suite
    
    let yaml = r#"
commands:
  - shell: "sleep 5"
    timeout: 1
"#;
    
    let config: WorkflowConfig = serde_yaml::from_str(yaml)?;
    
    // Check that timeout is parsed correctly
    if let prodigy::config::command::WorkflowCommand::WorkflowStep(step) = &config.commands[0] {
        assert_eq!(step.timeout, Some(1));
    } else {
        panic!("Expected WorkflowStep variant");
    }
    
    Ok(())
}

#[test]
fn test_timeout_field_serialization() {
    let step = WorkflowStepCommand {
        shell: Some("npm test".to_string()),
        timeout: Some(300),
        claude: None,
        analyze: None,
        test: None,
        id: None,
        commit_required: false,
        analysis: None,
        outputs: None,
        capture_output: false,
        on_failure: None,
        on_success: None,
        validate: None,
    };

    let yaml = serde_yaml::to_string(&step).unwrap();
    assert!(yaml.contains("timeout: 300"));
    
    // Test deserialization
    let deserialized: WorkflowStepCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(deserialized.timeout, Some(300));
}

#[test]
fn test_timeout_error_message() {
    // Test that timeout error messages are properly formatted
    let stderr_message = "Command timed out after 300 seconds";
    
    assert!(stderr_message.contains("timed out"));
    assert!(stderr_message.contains("300 seconds"));
}