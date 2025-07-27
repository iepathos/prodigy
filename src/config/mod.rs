use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod command;
pub mod command_parser;
pub mod command_validator;
pub mod loader;
pub mod validator;
pub mod workflow;

pub use command::{Command, CommandMetadata, SimpleCommand, WorkflowCommand};
pub use command_parser::{expand_variables, parse_command_string};
pub use command_validator::{apply_command_defaults, validate_command, CommandRegistry};
pub use loader::ConfigLoader;
pub use validator::ConfigValidator;
pub use workflow::WorkflowConfig;

/// Get the global MMM directory for storing configuration and data
pub fn get_global_mmm_dir() -> Result<PathBuf> {
    ProjectDirs::from("com", "mmm", "mmm")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or_else(|| anyhow!("Could not determine home directory"))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub global: GlobalConfig,
    pub project: Option<ProjectConfig>,
    pub workflow: Option<WorkflowConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub mmm_home: PathBuf,
    pub default_editor: Option<String>,
    pub log_level: Option<String>,
    pub claude_api_key: Option<String>,
    pub max_concurrent_specs: Option<u32>,
    pub auto_commit: Option<bool>,
    pub plugins: Option<PluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub spec_dir: Option<PathBuf>,
    pub claude_api_key: Option<String>,
    pub max_iterations: Option<u32>,
    pub auto_commit: Option<bool>,
    pub variables: Option<toml::Table>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub auto_load: Vec<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            mmm_home: get_global_mmm_dir()
                .unwrap_or_else(|_| PathBuf::from("~/.mmm")),
            default_editor: None,
            log_level: Some("info".to_string()),
            claude_api_key: None,
            max_concurrent_specs: Some(1),
            auto_commit: Some(true),
            plugins: None,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self {
            global: GlobalConfig::default(),
            project: None,
            workflow: None,
        }
    }

    pub fn merge_env_vars(&mut self) {
        if let Ok(api_key) = std::env::var("MMM_CLAUDE_API_KEY") {
            self.global.claude_api_key = Some(api_key);
        }

        if let Ok(log_level) = std::env::var("MMM_LOG_LEVEL") {
            self.global.log_level = Some(log_level);
        }

        if let Ok(editor) = std::env::var("MMM_EDITOR") {
            self.global.default_editor = Some(editor);
        } else if let Ok(editor) = std::env::var("EDITOR") {
            self.global.default_editor = Some(editor);
        }

        if let Ok(auto_commit) = std::env::var("MMM_AUTO_COMMIT") {
            if let Ok(value) = auto_commit.parse::<bool>() {
                self.global.auto_commit = Some(value);
            }
        }
    }

    pub fn get_claude_api_key(&self) -> Option<&str> {
        self.project
            .as_ref()
            .and_then(|p| p.claude_api_key.as_deref())
            .or(self.global.claude_api_key.as_deref())
    }

    pub fn get_auto_commit(&self) -> bool {
        self.project
            .as_ref()
            .and_then(|p| p.auto_commit)
            .or(self.global.auto_commit)
            .unwrap_or(true)
    }

    pub fn get_max_iterations(&self) -> u32 {
        self.project
            .as_ref()
            .and_then(|p| p.max_iterations)
            .unwrap_or(10)
    }

    pub fn get_spec_dir(&self) -> PathBuf {
        self.project
            .as_ref()
            .and_then(|p| p.spec_dir.clone())
            .unwrap_or_else(|| PathBuf::from("specs"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::{Command, WorkflowCommand};
    use crate::config::command_parser::parse_command_string;

    #[test]
    fn test_simple_workflow_config_parsing() {
        // Test simple string format
        let yaml_str = r#"
commands:
  - mmm-code-review
  - mmm-implement-spec
  - mmm-lint
max_iterations: 5
"#;

        let config: WorkflowConfig = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(config.commands.len(), 3);
        assert_eq!(config.max_iterations, 5);

        // Verify commands are parsed as Simple variants
        match &config.commands[0] {
            WorkflowCommand::Simple(s) => assert_eq!(s, "mmm-code-review"),
            _ => unreachable!("Expected Simple command"),
        }
    }

    #[test]
    fn test_structured_workflow_config_parsing() {
        // Test structured format with focus
        let yaml_str = r#"
commands:
  - name: mmm-code-review
    options:
      focus: security
  - name: mmm-implement-spec
    args: ["${SPEC_ID}"]
  - mmm-lint

max_iterations: 3
"#;

        let config: WorkflowConfig = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(config.commands.len(), 3);
        assert_eq!(config.max_iterations, 3);

        // Verify first command (Structured with focus in options)
        let cmd = config.commands[0].to_command();
        assert_eq!(cmd.name, "mmm-code-review");
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );

        // Verify second command (Structured with args)
        let cmd = config.commands[1].to_command();
        assert_eq!(cmd.name, "mmm-implement-spec");
        assert_eq!(cmd.args, vec!["${SPEC_ID}"]);
    }

    #[test]
    fn test_mixed_workflow_config() {
        // Test mixed format (legacy and structured)
        let yaml_str = r#"
max_iterations: 5
commands:
  - "mmm-code-review"
  - name: "mmm-implement-spec"
    args: ["iteration-123"]
  - "mmm-lint"
"#;

        let config: WorkflowConfig = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(config.commands.len(), 3);

        // First command should be Simple
        assert!(matches!(&config.commands[0], WorkflowCommand::Simple(_)));

        // Second command should be Structured
        let cmd = config.commands[1].to_command();
        assert_eq!(cmd.name, "mmm-implement-spec");
        assert_eq!(cmd.args, vec!["iteration-123"]);

        // Third command should be Simple
        assert!(matches!(&config.commands[2], WorkflowCommand::Simple(_)));
    }

    #[test]
    fn test_command_string_parsing() {
        // Test various command string formats
        let test_cases = vec![
            ("mmm-code-review", "mmm-code-review", vec![], vec![]),
            ("/mmm-lint", "mmm-lint", vec![], vec![]),
            (
                "mmm-implement-spec iteration-123",
                "mmm-implement-spec",
                vec!["iteration-123"],
                vec![],
            ),
            (
                "mmm-code-review --focus security",
                "mmm-code-review",
                vec![],
                vec![("focus", "security")],
            ),
            (
                "mmm-test arg1 arg2 --flag",
                "mmm-test",
                vec!["arg1", "arg2"],
                vec![("flag", "true")],
            ),
        ];

        for (input, expected_name, expected_args, expected_options) in test_cases {
            let cmd = parse_command_string(input).unwrap();
            assert_eq!(cmd.name, expected_name);
            assert_eq!(cmd.args, expected_args);

            for (key, value) in expected_options {
                let expected_value = if value == "true" {
                    serde_json::json!(true)
                } else {
                    serde_json::json!(value)
                };
                assert_eq!(
                    cmd.options.get(key),
                    Some(&expected_value),
                    "Failed for input: {input}"
                );
            }
        }
    }

    #[test]
    fn test_command_validation() {
        use crate::config::command_validator::CommandRegistry;

        let registry = CommandRegistry::new();

        // Valid commands
        let valid_commands = vec![
            Command::new("mmm-code-review"),
            Command::new("mmm-implement-spec").with_arg("spec-123"),
            Command::new("mmm-lint"),
        ];

        for cmd in valid_commands {
            assert!(registry.validate_command(&cmd).is_ok());
        }

        // Invalid commands
        let invalid_commands = vec![
            Command::new("unknown-command"),
            Command::new("mmm-implement-spec"), // Missing required arg
        ];

        for cmd in invalid_commands {
            assert!(registry.validate_command(&cmd).is_err());
        }
    }

    #[test]
    fn test_variable_expansion() {
        use crate::config::command_parser::expand_variables;
        use std::collections::HashMap;

        let mut cmd = Command::new("mmm-implement-spec")
            .with_arg("${SPEC_ID}")
            .with_option("path", serde_json::json!("${PROJECT_ROOT}/src"))
            .with_env("CUSTOM_VAR", "${USER_NAME}");

        let mut vars = HashMap::new();
        vars.insert("SPEC_ID".to_string(), "iteration-123".to_string());
        vars.insert("PROJECT_ROOT".to_string(), "/home/user/project".to_string());
        vars.insert("USER_NAME".to_string(), "test-user".to_string());

        expand_variables(&mut cmd, &vars);

        assert_eq!(cmd.args[0], "iteration-123");
        assert_eq!(
            cmd.options.get("path"),
            Some(&serde_json::json!("/home/user/project/src"))
        );
        assert_eq!(
            cmd.metadata.env.get("CUSTOM_VAR"),
            Some(&"test-user".to_string())
        );
    }

    #[test]
    fn test_command_metadata_defaults() {
        use crate::config::command_validator::apply_command_defaults;

        let mut cmd = Command::new("mmm-code-review");

        // Before applying defaults
        assert!(cmd.metadata.retries.is_none());
        assert!(cmd.metadata.timeout.is_none());

        // Apply defaults
        apply_command_defaults(&mut cmd);

        // After applying defaults
        assert_eq!(cmd.metadata.retries, Some(2));
        assert_eq!(cmd.metadata.timeout, Some(300));
        assert_eq!(cmd.metadata.continue_on_error, Some(false));
    }

    #[test]
    fn test_command_serialization_roundtrip() {
        let original = Command::new("mmm-code-review")
            .with_arg("file.rs")
            .with_option("focus", serde_json::json!("performance"))
            .with_retries(3)
            .with_timeout(600)
            .with_continue_on_error(true)
            .with_env("DEBUG", "true");

        // Serialize to JSON
        let json = serde_json::to_string(&original).unwrap();

        // Deserialize back
        let deserialized: Command = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.args, original.args);
        assert_eq!(deserialized.options, original.options);
        assert_eq!(deserialized.metadata.retries, original.metadata.retries);
        assert_eq!(deserialized.metadata.timeout, original.metadata.timeout);
        assert_eq!(
            deserialized.metadata.continue_on_error,
            original.metadata.continue_on_error
        );
        assert_eq!(deserialized.metadata.env, original.metadata.env);
    }
}
