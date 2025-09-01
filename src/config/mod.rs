use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod command;
pub mod command_discovery;
pub mod command_parser;
pub mod command_validator;
pub mod dynamic_registry;
pub mod loader;
pub mod mapreduce;
pub mod metadata_parser;
pub mod workflow;

pub use command::{
    Command, CommandArg, CommandMetadata, OutputDeclaration, SimpleCommand, WorkflowCommand,
};
pub use command_parser::{expand_variables, parse_command_string};
pub use command_validator::{apply_command_defaults, validate_command, CommandRegistry};
pub use dynamic_registry::DynamicCommandRegistry;
pub use loader::ConfigLoader;
pub use mapreduce::{parse_mapreduce_workflow, MapReduceWorkflowConfig};
pub use workflow::WorkflowConfig;

/// Get the global Prodigy directory for storing configuration and data
pub fn get_global_prodigy_dir() -> Result<PathBuf> {
    ProjectDirs::from("com", "prodigy", "prodigy")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or_else(|| anyhow!("Could not determine home directory"))
}

/// Root configuration structure for Prodigy
///
/// Contains global settings, project-specific configuration,
/// and workflow definitions. Supports hierarchical configuration
/// with project settings overriding global defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub global: GlobalConfig,
    pub project: Option<ProjectConfig>,
    pub workflow: Option<WorkflowConfig>,
}

/// Global configuration settings for MMM
///
/// These settings apply across all projects and can be overridden
/// by project-specific configuration. Stored in the user's home
/// directory under ~/.prodigy/config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub prodigy_home: PathBuf,
    pub default_editor: Option<String>,
    pub log_level: Option<String>,
    pub claude_api_key: Option<String>,
    pub max_concurrent_specs: Option<u32>,
    pub auto_commit: Option<bool>,
    pub plugins: Option<PluginConfig>,
}

/// Project-specific configuration settings
///
/// These settings override global configuration for a specific
/// project. Stored in the project's .prodigy/config.toml file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub spec_dir: Option<PathBuf>,
    pub claude_api_key: Option<String>,
    pub auto_commit: Option<bool>,
    pub variables: Option<toml::Table>,
}

/// Plugin system configuration
///
/// Controls plugin discovery, loading, and execution.
/// Plugins extend MMM's functionality with custom commands
/// and workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub auto_load: Vec<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            prodigy_home: get_global_prodigy_dir().unwrap_or_else(|_| PathBuf::from("~/.prodigy")),
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
        if let Ok(api_key) = std::env::var("PRODIGY_CLAUDE_API_KEY") {
            self.global.claude_api_key = Some(api_key);
        }

        if let Ok(log_level) = std::env::var("PRODIGY_LOG_LEVEL") {
            self.global.log_level = Some(log_level);
        }

        if let Ok(editor) = std::env::var("PRODIGY_EDITOR") {
            self.global.default_editor = Some(editor);
        } else if let Ok(editor) = std::env::var("EDITOR") {
            self.global.default_editor = Some(editor);
        }

        if let Ok(auto_commit) = std::env::var("PRODIGY_AUTO_COMMIT") {
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
    use std::sync::Mutex;

    // Shared mutex for environment variable tests to prevent race conditions
    static ENV_TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_simple_workflow_config_parsing() {
        // Test simple string format
        let yaml_str = r#"
commands:
  - prodigy-code-review
  - prodigy-implement-spec
  - prodigy-lint
"#;

        let config: WorkflowConfig = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(config.commands.len(), 3);

        // Verify commands are parsed as Simple variants
        match &config.commands[0] {
            WorkflowCommand::Simple(s) => assert_eq!(s, "prodigy-code-review"),
            _ => unreachable!("Expected Simple command"),
        }
    }

    #[test]
    fn test_structured_workflow_config_parsing() {
        // Test structured format with focus
        let yaml_str = r#"
commands:
  - name: prodigy-code-review
    options:
      focus: security
  - name: prodigy-implement-spec
    args: ["${SPEC_ID}"]
  - prodigy-lint
"#;

        let config: WorkflowConfig = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(config.commands.len(), 3);

        // Verify first command (Structured with focus in options)
        let cmd = config.commands[0].to_command();
        assert_eq!(cmd.name, "prodigy-code-review");
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );

        // Verify second command (Structured with args)
        let cmd = config.commands[1].to_command();
        assert_eq!(cmd.name, "prodigy-implement-spec");
        assert_eq!(cmd.args, vec![CommandArg::parse("${SPEC_ID}")]);
    }

    #[test]
    fn test_mixed_workflow_config() {
        // Test mixed format (legacy and structured)
        let yaml_str = r#"
max_iterations: 5
commands:
  - "prodigy-code-review"
  - name: "prodigy-implement-spec"
    args: ["iteration-123"]
  - "prodigy-lint"
"#;

        let config: WorkflowConfig = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(config.commands.len(), 3);

        // First command should be Simple
        assert!(matches!(&config.commands[0], WorkflowCommand::Simple(_)));

        // Second command should be Structured
        let cmd = config.commands[1].to_command();
        assert_eq!(cmd.name, "prodigy-implement-spec");
        assert_eq!(cmd.args, vec![CommandArg::parse("iteration-123")]);

        // Third command should be Simple
        assert!(matches!(&config.commands[2], WorkflowCommand::Simple(_)));
    }

    #[test]
    fn test_command_string_parsing() {
        // Test various command string formats
        let test_cases = vec![
            ("prodigy-code-review", "prodigy-code-review", vec![], vec![]),
            ("/prodigy-lint", "prodigy-lint", vec![], vec![]),
            (
                "prodigy-implement-spec iteration-123",
                "prodigy-implement-spec",
                vec!["iteration-123"],
                vec![],
            ),
            (
                "prodigy-code-review --focus security",
                "prodigy-code-review",
                vec![],
                vec![("focus", "security")],
            ),
            (
                "prodigy-test arg1 arg2 --flag",
                "prodigy-test",
                vec!["arg1", "arg2"],
                vec![("flag", "true")],
            ),
        ];

        for (input, expected_name, expected_args, expected_options) in test_cases {
            let cmd = parse_command_string(input).unwrap();
            assert_eq!(cmd.name, expected_name);
            let expected_args_cmd: Vec<CommandArg> =
                expected_args.into_iter().map(CommandArg::parse).collect();
            assert_eq!(cmd.args, expected_args_cmd);

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
            Command::new("prodigy-code-review"),
            Command::new("prodigy-implement-spec").with_arg("spec-123"),
            Command::new("prodigy-lint"),
        ];

        for cmd in valid_commands {
            assert!(registry.validate_command(&cmd).is_ok());
        }

        // Invalid commands
        let invalid_commands = vec![
            Command::new("unknown-command"),
            Command::new("prodigy-implement-spec"), // Missing required arg
        ];

        for cmd in invalid_commands {
            assert!(registry.validate_command(&cmd).is_err());
        }
    }

    #[test]
    fn test_variable_expansion() {
        use crate::config::command_parser::expand_variables;
        use std::collections::HashMap;

        let mut cmd = Command::new("prodigy-implement-spec")
            .with_arg("${SPEC_ID}")
            .with_option("path", serde_json::json!("${PROJECT_ROOT}/src"))
            .with_env("CUSTOM_VAR", "${USER_NAME}");

        let mut vars = HashMap::new();
        vars.insert("SPEC_ID".to_string(), "iteration-123".to_string());
        vars.insert("PROJECT_ROOT".to_string(), "/home/user/project".to_string());
        vars.insert("USER_NAME".to_string(), "test-user".to_string());

        expand_variables(&mut cmd, &vars);

        // expand_variables doesn't change CommandArg anymore, so this test doesn't apply the same way
        // The variable would be resolved at execution time
        assert!(matches!(&cmd.args[0], CommandArg::Variable(var) if var == "SPEC_ID"));
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

        let mut cmd = Command::new("prodigy-code-review");

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
        let original = Command::new("prodigy-code-review")
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

    #[test]
    fn test_config_new_creates_defaults() {
        let config = Config::new();

        assert!(config.project.is_none());
        assert!(config.workflow.is_none());
        assert_eq!(config.global.log_level, Some("info".to_string()));
        assert_eq!(config.global.max_concurrent_specs, Some(1));
        assert_eq!(config.global.auto_commit, Some(true));
    }

    #[test]
    fn test_get_claude_api_key_precedence() {
        let mut config = Config::new();

        // No API key set
        assert!(config.get_claude_api_key().is_none());

        // Global API key only
        config.global.claude_api_key = Some("global-key".to_string());
        assert_eq!(config.get_claude_api_key(), Some("global-key"));

        // Project API key takes precedence
        config.project = Some(ProjectConfig {
            name: "test".to_string(),
            description: None,
            version: None,
            spec_dir: None,
            claude_api_key: Some("project-key".to_string()),
            auto_commit: None,
            variables: None,
        });
        assert_eq!(config.get_claude_api_key(), Some("project-key"));
    }

    #[test]
    fn test_get_claude_api_key_from_config() {
        let mut config = Config::default();
        config.global.claude_api_key = Some("test-key-123".to_string());

        let result = config.get_claude_api_key();
        assert_eq!(result, Some("test-key-123"));
    }

    #[test]
    fn test_get_claude_api_key_from_env() {
        let _guard = ENV_TEST_MUTEX.lock().unwrap();

        // Save original env value
        let original = std::env::var("PRODIGY_CLAUDE_API_KEY").ok();

        unsafe { std::env::set_var("PRODIGY_CLAUDE_API_KEY", "env-key-456") };
        let mut config = Config::default();
        config.merge_env_vars();

        let result = config.get_claude_api_key();
        assert_eq!(result, Some("env-key-456"));

        // Restore original
        unsafe { std::env::remove_var("PRODIGY_CLAUDE_API_KEY") };
        if let Some(val) = original {
            unsafe { std::env::set_var("PRODIGY_CLAUDE_API_KEY", val) };
        }
    }

    #[test]
    fn test_get_claude_api_key_config_precedence_over_env() {
        let _guard = ENV_TEST_MUTEX.lock().unwrap();

        // Save original env value
        let original = std::env::var("PRODIGY_CLAUDE_API_KEY").ok();

        unsafe { std::env::set_var("PRODIGY_CLAUDE_API_KEY", "env-key") };
        let mut config = Config::default();
        config.merge_env_vars();

        // Now set config value - it should take precedence
        config.project = Some(ProjectConfig {
            name: "test".to_string(),
            description: None,
            version: None,
            spec_dir: None,
            claude_api_key: Some("config-key".to_string()),
            auto_commit: None,
            variables: None,
        });

        // Project config takes precedence over env
        let result = config.get_claude_api_key();
        assert_eq!(result, Some("config-key"));

        // Restore original
        unsafe { std::env::remove_var("PRODIGY_CLAUDE_API_KEY") };
        if let Some(val) = original {
            unsafe { std::env::set_var("PRODIGY_CLAUDE_API_KEY", val) };
        }
    }

    #[test]
    fn test_get_auto_commit_precedence() {
        let mut config = Config::new();

        // Default value
        assert!(config.get_auto_commit());

        // Global setting
        config.global.auto_commit = Some(false);
        assert!(!config.get_auto_commit());

        // Project setting takes precedence
        config.project = Some(ProjectConfig {
            name: "test".to_string(),
            description: None,
            version: None,
            spec_dir: None,
            claude_api_key: None,
            auto_commit: Some(true),
            variables: None,
        });
        assert!(config.get_auto_commit());
    }

    #[test]
    fn test_get_spec_dir() {
        let mut config = Config::new();

        // Default value
        assert_eq!(config.get_spec_dir(), PathBuf::from("specs"));

        // Project setting
        config.project = Some(ProjectConfig {
            name: "test".to_string(),
            description: None,
            version: None,
            spec_dir: Some(PathBuf::from("custom/specs")),
            claude_api_key: None,
            auto_commit: None,
            variables: None,
        });
        assert_eq!(config.get_spec_dir(), PathBuf::from("custom/specs"));
    }

    #[test]
    fn test_merge_env_vars() {
        // Use the shared mutex to ensure test isolation
        let _guard = ENV_TEST_MUTEX.lock().unwrap();

        // Save original env values
        let original_api_key = std::env::var("PRODIGY_CLAUDE_API_KEY").ok();
        let original_log_level = std::env::var("PRODIGY_LOG_LEVEL").ok();
        let original_editor = std::env::var("PRODIGY_EDITOR").ok();
        let original_auto_commit = std::env::var("PRODIGY_AUTO_COMMIT").ok();

        let mut config = Config::new();

        // Test environment variables override defaults
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("PRODIGY_CLAUDE_API_KEY", "env-api-key") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("PRODIGY_LOG_LEVEL", "debug") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("PRODIGY_EDITOR", "vim") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("PRODIGY_AUTO_COMMIT", "false") };

        config.merge_env_vars();

        assert_eq!(
            config.global.claude_api_key,
            Some("env-api-key".to_string())
        );
        assert_eq!(config.global.log_level, Some("debug".to_string()));
        assert_eq!(config.global.default_editor, Some("vim".to_string()));
        assert_eq!(config.global.auto_commit, Some(false));

        // Clean up
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("PRODIGY_CLAUDE_API_KEY") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("PRODIGY_LOG_LEVEL") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("PRODIGY_EDITOR") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("PRODIGY_AUTO_COMMIT") };

        // Restore original values if they existed
        if let Some(val) = original_api_key {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("PRODIGY_CLAUDE_API_KEY", val) };
        }
        if let Some(val) = original_log_level {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("PRODIGY_LOG_LEVEL", val) };
        }
        if let Some(val) = original_editor {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("PRODIGY_EDITOR", val) };
        }
        if let Some(val) = original_auto_commit {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("PRODIGY_AUTO_COMMIT", val) };
        }
    }

    #[test]
    fn test_merge_env_vars_editor_fallback() {
        // Use the shared mutex to ensure test isolation
        let _guard = ENV_TEST_MUTEX.lock().unwrap();

        // Save original env values
        let original_editor = std::env::var("EDITOR").ok();
        let original_prodigy_editor = std::env::var("PRODIGY_EDITOR").ok();

        // Test 1: EDITOR fallback when PRODIGY_EDITOR is not set
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("EDITOR") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("PRODIGY_EDITOR") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("EDITOR", "nano") };

        let mut config = Config::new();
        config.merge_env_vars();
        assert_eq!(
            config.global.default_editor,
            Some("nano".to_string()),
            "EDITOR fallback should work when PRODIGY_EDITOR is not set"
        );

        // Test 2: PRODIGY_EDITOR takes precedence over EDITOR
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("PRODIGY_EDITOR") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("EDITOR", "nano") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("PRODIGY_EDITOR", "emacs") };

        let mut config2 = Config::new();
        config2.merge_env_vars();
        assert_eq!(
            config2.global.default_editor,
            Some("emacs".to_string()),
            "PRODIGY_EDITOR should take precedence over EDITOR"
        );

        // Clean up - remove our test env vars
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("EDITOR") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("PRODIGY_EDITOR") };

        // Restore original values if they existed
        if let Some(val) = original_editor {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("EDITOR", val) };
        }
        if let Some(val) = original_prodigy_editor {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("PRODIGY_EDITOR", val) };
        }
    }

    #[test]
    fn test_global_config_default() {
        let global = GlobalConfig::default();

        // The home directory should be set to something
        assert!(!global.prodigy_home.as_os_str().is_empty());
        assert_eq!(global.log_level, Some("info".to_string()));
        assert_eq!(global.max_concurrent_specs, Some(1));
        assert_eq!(global.auto_commit, Some(true));
        assert!(global.default_editor.is_none());
        assert!(global.claude_api_key.is_none());
        assert!(global.plugins.is_none());
    }

    #[test]
    fn test_get_global_prodigy_dir_success() {
        // Test normal operation
        let result = get_global_prodigy_dir();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("prodigy"));
    }

    #[test]
    fn test_get_global_prodigy_dir_path_structure() {
        // Test path structure is correct
        let path = get_global_prodigy_dir().unwrap();

        // The path ends with the full app identifier "com.prodigy.prodigy" on most platforms
        let file_name = path.file_name().unwrap().to_string_lossy();
        assert!(
            file_name == "com.prodigy.prodigy" || file_name == "prodigy",
            "Expected directory name to be 'com.prodigy.prodigy' or 'prodigy', got '{file_name}'"
        );

        // Should be an absolute path
        assert!(path.is_absolute());
    }
}
