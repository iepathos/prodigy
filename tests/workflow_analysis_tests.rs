//! Integration tests for per-step analysis configuration in workflows

use mmm::config::command::{AnalysisConfig, Command, WorkflowCommand};
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
    analysis:
      analysis_types: ["context", "metrics"]
      max_cache_age: 300
  
  - name: mmm-cleanup-tech-debt
    analysis:
      analysis_types: ["metrics"]
      force_refresh: true
"#;

    let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.commands.len(), 4);

    // Check first command (no analysis)
    let cmd1 = workflow.commands[0].to_command();
    assert_eq!(cmd1.name, "mmm-implement-spec");
    assert!(cmd1.analysis.is_none());

    // Check second command (no analysis)
    let cmd2 = workflow.commands[1].to_command();
    assert_eq!(cmd2.name, "mmm-lint");
    assert!(!cmd2.metadata.commit_required);
    assert!(cmd2.analysis.is_none());

    // Check third command (with both context and metrics analysis)
    let cmd3 = workflow.commands[2].to_command();
    assert_eq!(cmd3.name, "mmm-code-review");
    assert!(cmd3.analysis.is_some());
    let analysis3 = cmd3.analysis.unwrap();
    assert_eq!(analysis3.analysis_types, vec!["context", "metrics"]);
    assert_eq!(analysis3.max_cache_age, 300);
    assert!(!analysis3.force_refresh);

    // Check fourth command (with metrics analysis and force refresh)
    let cmd4 = workflow.commands[3].to_command();
    assert_eq!(cmd4.name, "mmm-cleanup-tech-debt");
    assert!(cmd4.analysis.is_some());
    let analysis4 = cmd4.analysis.unwrap();
    assert_eq!(analysis4.analysis_types, vec!["metrics"]);
    assert!(analysis4.force_refresh);
}

#[test]
fn test_analysis_config_deserialization() {
    let json = r#"{
        "analysis_types": ["context"],
        "force_refresh": false,
        "max_cache_age": 600
    }"#;

    let config: AnalysisConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.analysis_types, vec!["context"]);
    assert!(!config.force_refresh);
    assert_eq!(config.max_cache_age, 600);

    // Test with multiple types
    let json_multi = r#"{
        "analysis_types": ["context", "metrics"],
        "force_refresh": true,
        "max_cache_age": 300
    }"#;

    let config_multi: AnalysisConfig = serde_json::from_str(json_multi).unwrap();
    assert_eq!(config_multi.analysis_types, vec!["context", "metrics"]);
    assert!(config_multi.force_refresh);
    assert_eq!(config_multi.max_cache_age, 300);

    // Test with default types when not specified
    let json_default = r#"{
        "force_refresh": false,
        "max_cache_age": 600
    }"#;

    let config_default: AnalysisConfig = serde_json::from_str(json_default).unwrap();
    assert_eq!(config_default.analysis_types, vec!["context"]);
}

#[test]
fn test_command_with_top_level_analysis() {
    let mut cmd = Command::new("mmm-test");
    cmd.analysis = Some(AnalysisConfig {
        analysis_types: vec!["context".to_string(), "metrics".to_string()],
        force_refresh: true,
        max_cache_age: 120,
    });

    assert!(cmd.metadata.commit_required); // Default is true
    assert!(cmd.analysis.is_some());

    let analysis = cmd.analysis.as_ref().unwrap();
    assert_eq!(analysis.analysis_types, vec!["context", "metrics"]);
    assert!(analysis.force_refresh);
    assert_eq!(analysis.max_cache_age, 120);
}

#[test]
fn test_structured_command_with_analysis() {
    let mut cmd = Command::new("mmm-test");
    cmd.analysis = Some(AnalysisConfig {
        analysis_types: vec!["metrics".to_string()],
        force_refresh: false,
        max_cache_age: 300,
    });

    let workflow_cmd = WorkflowCommand::Structured(Box::new(cmd.clone()));
    let converted = workflow_cmd.to_command();

    assert_eq!(converted.name, "mmm-test");
    assert!(converted.analysis.is_some());
    assert_eq!(
        converted.analysis.as_ref().unwrap().analysis_types,
        vec!["metrics"]
    );
}

#[test]
fn test_analysis_types_validation() {
    // Test single type
    let config_single = AnalysisConfig {
        analysis_types: vec!["context".to_string()],
        force_refresh: false,
        max_cache_age: 300,
    };

    let json = serde_json::to_string(&config_single).unwrap();
    let deserialized: AnalysisConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.analysis_types, vec!["context"]);

    // Test multiple types
    let config_multi = AnalysisConfig {
        analysis_types: vec!["context".to_string(), "metrics".to_string()],
        force_refresh: false,
        max_cache_age: 300,
    };

    let json_multi = serde_json::to_string(&config_multi).unwrap();
    let deserialized_multi: AnalysisConfig = serde_json::from_str(&json_multi).unwrap();
    assert_eq!(
        deserialized_multi.analysis_types,
        vec!["context", "metrics"]
    );

    // Test empty types (should use default)
    let config_empty = AnalysisConfig {
        analysis_types: vec![],
        force_refresh: false,
        max_cache_age: 300,
    };

    let json_empty = serde_json::to_string(&config_empty).unwrap();
    let deserialized_empty: AnalysisConfig = serde_json::from_str(&json_empty).unwrap();
    assert_eq!(deserialized_empty.analysis_types, Vec::<String>::new());
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
                analysis: None,
            }),
        ],
    };

    let has_analysis = workflow_no_analysis
        .commands
        .iter()
        .any(|cmd| matches!(cmd, WorkflowCommand::Structured(c) if c.analysis.is_some()));
    assert!(!has_analysis);

    // Workflow with analysis
    let mut cmd_with_analysis = Command::new("mmm-code-review");
    cmd_with_analysis.analysis = Some(AnalysisConfig {
        analysis_types: vec!["context".to_string(), "metrics".to_string()],
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
        .any(|cmd| matches!(cmd, WorkflowCommand::Structured(c) if c.analysis.is_some()));
    assert!(has_analysis);
}

#[test]
fn test_simple_command_with_analysis() {
    let yaml = r#"
commands:
  - name: mmm-code-review
    analysis:
      analysis_types: ["context"]
      max_cache_age: 300
  
  - name: mmm-test
    commit_required: false
    analysis:
      analysis_types: ["metrics"]
      force_refresh: true
"#;

    let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.commands.len(), 2);

    // Check first command with analysis
    let cmd1 = workflow.commands[0].to_command();
    assert_eq!(cmd1.name, "mmm-code-review");
    assert!(cmd1.analysis.is_some());
    let analysis1 = cmd1.analysis.unwrap();
    assert_eq!(analysis1.analysis_types, vec!["context"]);
    assert_eq!(analysis1.max_cache_age, 300);
    assert!(!analysis1.force_refresh);

    // Check second command with analysis and commit_required
    let cmd2 = workflow.commands[1].to_command();
    assert_eq!(cmd2.name, "mmm-test");
    assert!(!cmd2.metadata.commit_required);
    assert!(cmd2.analysis.is_some());
    let analysis2 = cmd2.analysis.unwrap();
    assert_eq!(analysis2.analysis_types, vec!["metrics"]);
    assert!(analysis2.force_refresh);
}

#[test]
fn test_backward_compatibility_metadata_analysis() {
    let yaml = r#"
commands:
  - name: mmm-code-review
    metadata:
      analysis:
        analysis_types: ["context"]
        max_cache_age: 300
"#;

    let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.commands.len(), 1);

    // Check that metadata.analysis still works for backward compatibility
    let cmd = workflow.commands[0].to_command();
    assert_eq!(cmd.name, "mmm-code-review");
    assert!(cmd.analysis.is_some());
    assert!(cmd.metadata.analysis.is_some());
    
    let analysis = cmd.analysis.unwrap();
    assert_eq!(analysis.analysis_types, vec!["context"]);
    assert_eq!(analysis.max_cache_age, 300);
}
