use super::*;
use std::fs;
use tempfile::TempDir;

fn setup_test_dir() -> TempDir {
    TempDir::new().unwrap()
}

fn write_test_config(dir: &TempDir, filename: &str, content: &str) {
    let mmm_dir = dir.path().join(".mmm");
    fs::create_dir_all(&mmm_dir).unwrap();
    fs::write(mmm_dir.join(filename), content).unwrap();
}

#[tokio::test]
async fn test_load_default_config() {
    let loader = ConfigLoader::new().await.unwrap();
    let config = loader.get_config();

    assert!(config.project.is_none());
    assert!(config.workflow.is_none());
    assert_eq!(config.global.log_level, Some("info".to_string()));
    assert_eq!(config.global.max_concurrent_specs, Some(1));
    assert_eq!(config.global.auto_commit, Some(true));
}

#[tokio::test]
async fn test_load_workflow_toml() {
    let temp_dir = setup_test_dir();
    let config_content = r#"
commands = ["/mmm-code-review", "/mmm-implement-spec", "/mmm-lint"]
"#;
    write_test_config(&temp_dir, "config.toml", config_content);

    let loader = ConfigLoader::new().await.unwrap();
    loader
        .load_with_explicit_path(temp_dir.path(), None)
        .await
        .unwrap();
    let config = loader.get_config();

    assert!(config.workflow.is_some());
    let workflow = config.workflow.unwrap();
    assert_eq!(workflow.commands.len(), 3);
    assert_eq!(workflow.commands[0], "/mmm-code-review");
    assert_eq!(workflow.commands[1], "/mmm-implement-spec");
    assert_eq!(workflow.commands[2], "/mmm-lint");
}

#[tokio::test]
async fn test_load_workflow_yaml() {
    let temp_dir = setup_test_dir();
    let custom_config = temp_dir.path().join("custom.yaml");
    let config_content = r#"
commands:
  - /mmm-code-review
  - /mmm-test
"#;
    fs::write(&custom_config, config_content).unwrap();

    let loader = ConfigLoader::new().await.unwrap();
    loader
        .load_with_explicit_path(temp_dir.path(), Some(&custom_config))
        .await
        .unwrap();
    let config = loader.get_config();

    assert!(config.workflow.is_some());
    let workflow = config.workflow.unwrap();
    assert_eq!(workflow.commands.len(), 2);
}

#[tokio::test]
async fn test_load_with_explicit_path() {
    let temp_dir = setup_test_dir();
    let custom_config = temp_dir.path().join("custom.toml");
    let config_content = r#"
commands = ["/mmm-test"]
"#;
    fs::write(&custom_config, config_content).unwrap();

    let loader = ConfigLoader::new().await.unwrap();
    loader
        .load_with_explicit_path(temp_dir.path(), Some(&custom_config))
        .await
        .unwrap();
    let config = loader.get_config();

    assert!(config.workflow.is_some());
    assert_eq!(config.workflow.unwrap().commands[0], "/mmm-test");
}

#[tokio::test]
async fn test_legacy_workflow_toml_backward_compatibility() {
    let temp_dir = setup_test_dir();
    let config_content = r#"
commands = ["/mmm-code-review", "/mmm-lint"]
"#;
    write_test_config(&temp_dir, "workflow.toml", config_content);

    let loader = ConfigLoader::new().await.unwrap();
    loader
        .load_with_explicit_path(temp_dir.path(), None)
        .await
        .unwrap();
    let config = loader.get_config();

    assert!(config.workflow.is_some());
    let workflow = config.workflow.unwrap();
    assert_eq!(workflow.commands.len(), 2);
}

#[tokio::test]
async fn test_invalid_toml() {
    let temp_dir = setup_test_dir();
    let custom_config = temp_dir.path().join("invalid.toml");
    let config_content = r#"
commands = not valid toml
"#;
    fs::write(&custom_config, config_content).unwrap();

    let loader = ConfigLoader::new().await.unwrap();
    let result = loader
        .load_with_explicit_path(temp_dir.path(), Some(&custom_config))
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_unsupported_file_format() {
    let temp_dir = setup_test_dir();
    let custom_config = temp_dir.path().join("config.json");
    let config_content = r#"{"commands": ["/mmm-test"]}"#;
    fs::write(&custom_config, config_content).unwrap();

    let loader = ConfigLoader::new().await.unwrap();
    let result = loader
        .load_with_explicit_path(temp_dir.path(), Some(&custom_config))
        .await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Unsupported configuration file format"));
}

#[tokio::test]
async fn test_merge_env_vars() {
    std::env::set_var("MMM_CLAUDE_API_KEY", "test-key");
    std::env::set_var("MMM_LOG_LEVEL", "debug");
    std::env::set_var("MMM_AUTO_COMMIT", "false");

    let loader = ConfigLoader::new().await.unwrap();
    loader.load_global().await.unwrap();
    let config = loader.get_config();

    assert_eq!(config.global.claude_api_key, Some("test-key".to_string()));
    assert_eq!(config.global.log_level, Some("debug".to_string()));
    assert_eq!(config.global.auto_commit, Some(false));

    // Clean up
    std::env::remove_var("MMM_CLAUDE_API_KEY");
    std::env::remove_var("MMM_LOG_LEVEL");
    std::env::remove_var("MMM_AUTO_COMMIT");
}

#[tokio::test]
async fn test_project_value_getters() {
    let temp_dir = setup_test_dir();
    let mmm_dir = temp_dir.path().join(".mmm");
    fs::create_dir_all(&mmm_dir).unwrap();
    let config_content = r#"
name = "test-project"
description = "Test description"
version = "1.0.0"
max_iterations = 5
auto_commit = false
"#;
    fs::write(mmm_dir.join("config.toml"), config_content).unwrap();

    let loader = ConfigLoader::new().await.unwrap();
    loader.load_project(temp_dir.path()).await.unwrap();

    assert_eq!(loader.get_project_value("name").unwrap(), "test-project");
    assert_eq!(
        loader.get_project_value("description").unwrap(),
        "Test description"
    );
    assert_eq!(loader.get_project_value("version").unwrap(), "1.0.0");
    assert_eq!(loader.get_project_value("max_iterations").unwrap(), "5");
    assert_eq!(loader.get_project_value("auto_commit").unwrap(), "false");
}

#[tokio::test]
async fn test_config_precedence() {
    let loader = ConfigLoader::new().await.unwrap();
    let mut config = loader.get_config();

    // Set global API key
    config.global.claude_api_key = Some("global-key".to_string());

    // No project config, should use global
    assert_eq!(config.get_claude_api_key(), Some("global-key"));

    // Add project config
    config.project = Some(ProjectConfig {
        name: "test".to_string(),
        description: None,
        version: None,
        spec_dir: None,
        claude_api_key: Some("project-key".to_string()),
        max_iterations: Some(15),
        auto_commit: Some(false),
        variables: None,
    });

    // Project should override global
    assert_eq!(config.get_claude_api_key(), Some("project-key"));
    assert_eq!(config.get_max_iterations(), 15);
    assert_eq!(config.get_auto_commit(), false);
}

#[test]
fn test_config_defaults() {
    let config = Config::new();

    assert_eq!(config.get_max_iterations(), 10);
    assert_eq!(config.get_auto_commit(), true);
    assert_eq!(config.get_spec_dir(), PathBuf::from("specs"));
}

#[test]
fn test_validate_valid_workflow_config() {
    let config = Config {
        global: GlobalConfig::default(),
        project: None,
        workflow: Some(WorkflowConfig {
            commands: vec!["/mmm-code-review".to_string()],
            max_iterations: 10,
        }),
    };

    let result = ConfigValidator::validate(&config);
    assert!(result.is_ok());
}

#[test]
fn test_validate_empty_workflow_commands() {
    let config = Config {
        global: GlobalConfig::default(),
        project: None,
        workflow: Some(WorkflowConfig { 
            commands: vec![],
            max_iterations: 10,
        }),
    };

    let result = ConfigValidator::validate(&config);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Workflow must have at least one command"));
}

#[test]
fn test_validate_invalid_workflow_command() {
    let config = Config {
        global: GlobalConfig::default(),
        project: None,
        workflow: Some(WorkflowConfig {
            commands: vec!["not-a-slash-command".to_string()],
            max_iterations: 10,
        }),
    };

    let result = ConfigValidator::validate(&config);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("must start with '/'"));
}

#[test]
fn test_workflow_config_serialization() {
    let workflow = WorkflowConfig {
        commands: vec![
            "/mmm-code-review".to_string(),
            "/mmm-implement-spec".to_string(),
        ],
        max_iterations: 10,
    };

    // Test TOML roundtrip
    let toml_str = toml::to_string(&workflow).unwrap();
    let from_toml: WorkflowConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(workflow.commands, from_toml.commands);

    // Test YAML roundtrip
    let yaml_str = serde_yaml::to_string(&workflow).unwrap();
    let from_yaml: WorkflowConfig = serde_yaml::from_str(&yaml_str).unwrap();
    assert_eq!(workflow.commands, from_yaml.commands);
}
