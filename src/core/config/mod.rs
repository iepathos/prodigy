//! Pure configuration processing functions
//!
//! These functions handle configuration parsing, validation, and transformation
//! without performing any I/O operations.

use crate::config::{Config, ProjectConfig, WorkflowConfig};
use anyhow::{anyhow, Result};
use serde_yaml::Value;

/// Parse YAML content into a WorkflowConfig
pub fn parse_workflow_config(content: &str) -> Result<WorkflowConfig> {
    // First try to parse as a full config with workflow section
    if let Ok(full_config) = serde_yaml::from_str::<Value>(content) {
        if let Some(workflow_value) = full_config.get("workflow") {
            return serde_yaml::from_value(workflow_value.clone())
                .map_err(|e| anyhow!("Failed to parse workflow configuration: {}", e));
        }
    }

    // Try to parse as direct WorkflowConfig
    serde_yaml::from_str(content).map_err(|e| anyhow!("Failed to parse YAML configuration: {}", e))
}

/// Parse YAML content into a ProjectConfig
pub fn parse_project_config(content: &str) -> Result<ProjectConfig> {
    serde_yaml::from_str(content)
        .map_err(|e| anyhow!("Failed to parse project configuration: {}", e))
}

/// Merge workflow config into existing config
pub fn merge_workflow_config(mut config: Config, workflow: WorkflowConfig) -> Config {
    config.workflow = Some(workflow);
    config
}

/// Merge project config into existing config
pub fn merge_project_config(mut config: Config, project: ProjectConfig) -> Config {
    config.project = Some(project);
    config
}

/// Validate configuration format based on file extension
pub fn validate_config_format(extension: &str) -> Result<()> {
    match extension {
        "yaml" | "yml" => Ok(()),
        _ => Err(anyhow!(
            "Unsupported configuration file format. Use .yaml or .yml"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direct_workflow_config() {
        let content = r#"
commands:
  - prodigy-code-review
  - prodigy-implement-spec
  - prodigy-lint
"#;

        let result = parse_workflow_config(content);
        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.commands.len(), 3);
    }

    #[test]
    fn test_parse_nested_workflow_config() {
        let content = r#"
workflow:
  commands:
    - name: prodigy-code-review
      options:
        focus: performance
    - prodigy-lint
"#;

        let result = parse_workflow_config(content);
        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.commands.len(), 2);
    }

    #[test]
    fn test_parse_project_config() {
        let content = r#"
name: test-project
description: A test project
version: 1.0.0
spec_dir: custom-specs
claude_api_key: test-key
auto_commit: false
"#;

        let result = parse_project_config(content);
        assert!(result.is_ok());
        let project = result.unwrap();
        assert_eq!(project.name, "test-project");
        assert_eq!(project.description, Some("A test project".to_string()));
    }

    #[test]
    fn test_validate_config_format() {
        assert!(validate_config_format("yaml").is_ok());
        assert!(validate_config_format("yml").is_ok());
        assert!(validate_config_format("json").is_err());
        assert!(validate_config_format("toml").is_err());
    }

    #[test]
    fn test_merge_configs() {
        let mut config = Config::new();
        assert!(config.workflow.is_none());
        assert!(config.project.is_none());

        let workflow = WorkflowConfig {
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        config = merge_workflow_config(config, workflow);
        assert!(config.workflow.is_some());

        let project = ProjectConfig {
            name: "test".to_string(),
            description: None,
            version: None,
            spec_dir: None,
            claude_api_key: None,
            auto_commit: None,
            variables: None,
        };

        config = merge_project_config(config, project);
        assert!(config.project.is_some());
    }
}
