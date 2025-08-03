//! Integration tests for per-step analysis configuration in workflows

use mmm::config::command::{AnalysisConfig, Command, CommandMetadata, WorkflowCommand};
use mmm::config::WorkflowConfig;
use serde_yaml;

#[test]
fn test_workflow_yaml_with_analysis_config() {
    let yaml = r#"
commands:
  - name: mmm-implement-spec
    args: ["$ARG"]
    # No analysis needed, uses initial context
  
  - name: mmm-lint
    commit_required: false
    # No analysis needed for linting
  
  - name: mmm-code-review
    metadata:
      analysis:
        analysis_type: "all"
        max_cache_age: 300
  
  - name: mmm-cleanup-tech-debt
    metadata:
      analysis:
        analysis_type: "metrics"
        force_refresh: true
"#;

    let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.commands.len(), 4);

    // Check first command (no analysis)
    let cmd1 = workflow.commands[0].to_command();
    assert_eq!(cmd1.name, "mmm-implement-spec");
    assert!(cmd1.metadata.analysis.is_none());

    // Check second command (no analysis)
    let cmd2 = workflow.commands[1].to_command();
    assert_eq!(cmd2.name, "mmm-lint");
    assert!(!cmd2.metadata.commit_required);
    assert!(cmd2.metadata.analysis.is_none());

    // Check third command (with full analysis)
    let cmd3 = workflow.commands[2].to_command();
    assert_eq!(cmd3.name, "mmm-code-review");
    assert!(cmd3.metadata.analysis.is_some());
    let analysis3 = cmd3.metadata.analysis.unwrap();
    assert_eq!(analysis3.analysis_type, "all");
    assert_eq!(analysis3.max_cache_age, 300);
    assert!(!analysis3.force_refresh);

    // Check fourth command (with metrics analysis and force refresh)
    let cmd4 = workflow.commands[3].to_command();
    assert_eq!(cmd4.name, "mmm-cleanup-tech-debt");
    assert!(cmd4.metadata.analysis.is_some());
    let analysis4 = cmd4.metadata.analysis.unwrap();
    assert_eq!(analysis4.analysis_type, "metrics");
    assert!(analysis4.force_refresh);
}

#[test]
fn test_analysis_config_deserialization() {
    let json = r#"{
        "analysis_type": "context",
        "force_refresh": false,
        "max_cache_age": 600
    }"#;

    let config: AnalysisConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.analysis_type, "context");
    assert!(!config.force_refresh);
    assert_eq!(config.max_cache_age, 600);
}

#[test]
fn test_command_metadata_with_analysis() {
    let mut metadata = CommandMetadata::default();
    metadata.analysis = Some(AnalysisConfig {
        analysis_type: "all".to_string(),
        force_refresh: true,
        max_cache_age: 120,
    });

    assert!(metadata.commit_required); // Default is true
    assert!(metadata.analysis.is_some());

    let analysis = metadata.analysis.as_ref().unwrap();
    assert_eq!(analysis.analysis_type, "all");
    assert!(analysis.force_refresh);
    assert_eq!(analysis.max_cache_age, 120);
}

#[test]
fn test_structured_command_with_analysis() {
    let mut cmd = Command::new("mmm-test");
    cmd.metadata.analysis = Some(AnalysisConfig {
        analysis_type: "metrics".to_string(),
        force_refresh: false,
        max_cache_age: 300,
    });

    let workflow_cmd = WorkflowCommand::Structured(Box::new(cmd.clone()));
    let converted = workflow_cmd.to_command();

    assert_eq!(converted.name, "mmm-test");
    assert!(converted.metadata.analysis.is_some());
    assert_eq!(
        converted.metadata.analysis.as_ref().unwrap().analysis_type,
        "metrics"
    );
}

#[test]
fn test_analysis_type_validation() {
    let valid_types = vec!["context", "metrics", "all"];

    for analysis_type in valid_types {
        let config = AnalysisConfig {
            analysis_type: analysis_type.to_string(),
            force_refresh: false,
            max_cache_age: 300,
        };

        // Ensure it serializes and deserializes correctly
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AnalysisConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.analysis_type, analysis_type);
    }
}

#[test]
fn test_workflow_has_analysis_detection() {
    // Workflow without analysis
    let workflow_no_analysis = WorkflowConfig {
        commands: vec![
            WorkflowCommand::Simple("mmm-lint".to_string()),
            WorkflowCommand::SimpleObject(mmm::config::command::SimpleCommand {
                name: "mmm-test".to_string(),
                commit_required: Some(false),
                args: None,
            }),
        ],
    };

    let has_analysis = workflow_no_analysis
        .commands
        .iter()
        .any(|cmd| matches!(cmd, WorkflowCommand::Structured(c) if c.metadata.analysis.is_some()));
    assert!(!has_analysis);

    // Workflow with analysis
    let mut cmd_with_analysis = Command::new("mmm-code-review");
    cmd_with_analysis.metadata.analysis = Some(AnalysisConfig {
        analysis_type: "all".to_string(),
        force_refresh: false,
        max_cache_age: 300,
    });

    let workflow_with_analysis = WorkflowConfig {
        commands: vec![
            WorkflowCommand::Simple("mmm-lint".to_string()),
            WorkflowCommand::Structured(Box::new(cmd_with_analysis)),
        ],
    };

    let has_analysis = workflow_with_analysis
        .commands
        .iter()
        .any(|cmd| matches!(cmd, WorkflowCommand::Structured(c) if c.metadata.analysis.is_some()));
    assert!(has_analysis);
}
