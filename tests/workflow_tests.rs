use mmm::config::workflow::WorkflowConfig;

/// Test workflow configuration loading
#[test]
fn test_workflow_config_parsing() {
    // Test valid workflow config
    let valid_toml = r#"
commands = [
    "/mmm-code-review",
    "/mmm-implement-spec",
    "/mmm-lint"
]
"#;

    let config: Result<WorkflowConfig, _> = toml::from_str(valid_toml);
    assert!(config.is_ok());

    let config = config.unwrap();
    assert_eq!(config.commands.len(), 3);
    assert_eq!(config.commands[0], "/mmm-code-review");
    assert_eq!(config.commands[1], "/mmm-implement-spec");
    assert_eq!(config.commands[2], "/mmm-lint");
}

/// Test workflow config with empty commands
#[test]
fn test_empty_workflow_config() {
    let empty_toml = r#"commands = []"#;

    let config: Result<WorkflowConfig, _> = toml::from_str(empty_toml);
    assert!(config.is_ok());

    let config = config.unwrap();
    assert!(config.commands.is_empty());
}

/// Test invalid workflow config
#[test]
fn test_invalid_workflow_config() {
    let invalid_toml = r#"
commands = "not an array"
"#;

    let config: Result<WorkflowConfig, _> = toml::from_str(invalid_toml);
    assert!(config.is_err());
}

/// Test spec ID extraction from commands
#[test]
fn test_spec_id_extraction_logic() {
    // Test the logic for extracting spec ID from git commit
    let test_messages = vec![
        (
            "review: generate improvement spec for iteration-1234567890-improvements",
            Some("iteration-1234567890-improvements"),
        ),
        ("fix: some other commit", None),
        (
            "review: generate improvement spec for iteration-9999999999-improvements with notes",
            Some("iteration-9999999999-improvements"),
        ),
        ("", None),
    ];

    for (message, expected) in test_messages {
        let extracted = if message.contains("iteration-") && message.contains("-improvements") {
            message
                .split_whitespace()
                .find(|word| word.starts_with("iteration-") && word.ends_with("-improvements"))
        } else {
            None
        };

        assert_eq!(extracted, expected);
    }
}

/// Test command parsing for mmm-implement-spec
#[test]
fn test_implement_spec_command_parsing() {
    let commands = vec!["/mmm-code-review", "/mmm-implement-spec", "/mmm-lint"];

    for cmd in &commands {
        let needs_spec_id = cmd.trim() == "/mmm-implement-spec";

        if cmd == &"/mmm-implement-spec" {
            assert!(needs_spec_id);
        } else {
            assert!(!needs_spec_id);
        }
    }
}

/// Test workflow execution order
#[test]
fn test_workflow_execution_order() {
    let workflow = WorkflowConfig {
        commands: vec![
            "/mmm-code-review".to_string(),
            "/mmm-implement-spec".to_string(),
            "/mmm-lint".to_string(),
        ],
        max_iterations: 10,
    };

    // Simulate workflow execution tracking
    let mut executed_commands = Vec::new();

    for (i, cmd) in workflow.commands.iter().enumerate() {
        executed_commands.push(cmd.clone());

        // Verify order
        match i {
            0 => assert_eq!(cmd, "/mmm-code-review"),
            1 => assert_eq!(cmd, "/mmm-implement-spec"),
            2 => assert_eq!(cmd, "/mmm-lint"),
            _ => panic!("Unexpected command index"),
        }
    }

    assert_eq!(executed_commands.len(), 3);
}

/// Test custom workflow configurations
#[test]
fn test_custom_workflow_configs() {
    // Test security-focused workflow
    let security_workflow = WorkflowConfig {
        commands: vec![
            "/mmm-security-audit".to_string(),
            "/mmm-implement-spec".to_string(),
            "/mmm-security-verify".to_string(),
        ],
        max_iterations: 10,
    };

    assert_eq!(security_workflow.commands.len(), 3);
    assert!(security_workflow.commands[0].contains("security"));

    // Test documentation workflow
    let docs_workflow = WorkflowConfig {
        commands: vec![
            "/mmm-docs-review".to_string(),
            "/mmm-implement-spec".to_string(),
            "/mmm-docs-generate".to_string(),
        ],
        max_iterations: 10,
    };

    assert_eq!(docs_workflow.commands.len(), 3);
    assert!(docs_workflow.commands[0].contains("docs"));
}

/// Test workflow with single command
#[test]
fn test_single_command_workflow() {
    let minimal_workflow = WorkflowConfig {
        commands: vec!["/mmm-quick-fix".to_string()],
        max_iterations: 10,
    };

    assert_eq!(minimal_workflow.commands.len(), 1);
    assert_eq!(minimal_workflow.commands[0], "/mmm-quick-fix");
}

/// Test workflow commands with arguments like --focus
#[test]
fn test_workflow_commands_with_arguments() {
    let focus_workflow_toml = r#"
commands = [
    "mmm-code-review --focus architecture",
    "mmm-implement-spec",
    "mmm-code-review --focus error-handling",
    "mmm-implement-spec"
]
max_iterations = 2
"#;

    let config: Result<WorkflowConfig, _> = toml::from_str(focus_workflow_toml);
    assert!(config.is_ok());

    let config = config.unwrap();
    assert_eq!(config.commands.len(), 4);
    assert_eq!(config.commands[0], "mmm-code-review --focus architecture");
    assert_eq!(config.commands[2], "mmm-code-review --focus error-handling");
}
