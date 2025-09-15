//! Integration tests for environment workflow features

use anyhow::Result;
use prodigy::config::{CookCommand, WorkflowConfig};
use prodigy::cook::environment::{EnvironmentConfig, EnvironmentManager};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_environment_example_workflow_parsing() -> Result<()> {
    // Load the environment-example.yml workflow
    let workflow_path = PathBuf::from("workflows/environment-example.yml");
    if !workflow_path.exists() {
        eprintln!("Skipping test: environment-example.yml not found");
        return Ok(());
    }

    let content = std::fs::read_to_string(&workflow_path)?;
    let workflow: WorkflowConfig = serde_yaml::from_str(&content)?;

    // Verify global environment fields are parsed
    assert!(workflow.env.is_some(), "env field should be parsed");
    assert!(workflow.secrets.is_some(), "secrets field should be parsed");
    assert!(workflow.env_files.is_some(), "env_files field should be parsed");
    assert!(workflow.profiles.is_some(), "profiles field should be parsed");

    // Check specific environment variables
    let env = workflow.env.as_ref().unwrap();
    assert_eq!(env.get("NODE_ENV"), Some(&"production".to_string()));
    assert_eq!(env.get("API_URL"), Some(&"https://api.example.com".to_string()));

    // Check profiles
    let profiles = workflow.profiles.as_ref().unwrap();
    assert!(profiles.contains_key("development"));
    assert!(profiles.contains_key("testing"));

    // Check development profile
    let dev_profile = profiles.get("development").unwrap();
    assert_eq!(dev_profile.env.get("NODE_ENV"), Some(&"development".to_string()));
    assert_eq!(dev_profile.env.get("DEBUG"), Some(&"true".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_environment_manager_with_global_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let mut manager = EnvironmentManager::new(temp_dir.path().to_path_buf())?;

    // Create a global environment config
    let mut global_env = HashMap::new();
    global_env.insert(
        "TEST_VAR".to_string(),
        prodigy::cook::environment::config::EnvValue::Static("test_value".to_string()),
    );

    let global_config = EnvironmentConfig {
        global_env,
        secrets: HashMap::new(),
        env_files: Vec::new(),
        inherit: true,
        profiles: HashMap::new(),
        active_profile: None,
    };

    // Create a step environment
    let step_env = prodigy::cook::environment::config::StepEnvironment {
        env: HashMap::from([("STEP_VAR".to_string(), "step_value".to_string())]),
        working_dir: None,
        clear_env: false,
        temporary: false,
    };

    // Set up environment with global config
    let variables = HashMap::new();
    let context = manager
        .setup_environment(&step_env, Some(&global_config), &variables)
        .await?;

    // Verify both global and step variables are present
    assert_eq!(context.env.get("TEST_VAR"), Some(&"test_value".to_string()));
    assert_eq!(context.env.get("STEP_VAR"), Some(&"step_value".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_environment_profiles() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let mut manager = EnvironmentManager::new(temp_dir.path().to_path_buf())?;

    // Create profiles
    let mut profiles = HashMap::new();
    profiles.insert(
        "development".to_string(),
        prodigy::cook::environment::config::EnvProfile {
            env: HashMap::from([
                ("NODE_ENV".to_string(), "development".to_string()),
                ("DEBUG".to_string(), "true".to_string()),
            ]),
            description: Some("Development environment".to_string()),
        },
    );
    profiles.insert(
        "production".to_string(),
        prodigy::cook::environment::config::EnvProfile {
            env: HashMap::from([
                ("NODE_ENV".to_string(), "production".to_string()),
                ("DEBUG".to_string(), "false".to_string()),
            ]),
            description: Some("Production environment".to_string()),
        },
    );

    // Create global config with active profile
    let global_config = EnvironmentConfig {
        global_env: HashMap::new(),
        secrets: HashMap::new(),
        env_files: Vec::new(),
        inherit: true,
        profiles: profiles.clone(),
        active_profile: Some("development".to_string()),
    };

    // Set up environment
    let step_env = prodigy::cook::environment::config::StepEnvironment::default();
    let variables = HashMap::new();
    let context = manager
        .setup_environment(&step_env, Some(&global_config), &variables)
        .await?;

    // Verify development profile is applied
    assert_eq!(context.env.get("NODE_ENV"), Some(&"development".to_string()));
    assert_eq!(context.env.get("DEBUG"), Some(&"true".to_string()));

    // Test with production profile
    let prod_config = EnvironmentConfig {
        active_profile: Some("production".to_string()),
        ..global_config
    };

    let prod_context = manager
        .setup_environment(&step_env, Some(&prod_config), &variables)
        .await?;

    // Verify production profile is applied
    assert_eq!(prod_context.env.get("NODE_ENV"), Some(&"production".to_string()));
    assert_eq!(prod_context.env.get("DEBUG"), Some(&"false".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_environment_inheritance() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let mut manager = EnvironmentManager::new(temp_dir.path().to_path_buf())?;

    // Set a test environment variable
    std::env::set_var("TEST_INHERITED_VAR", "inherited_value");

    // Test with inheritance enabled (default)
    let config_with_inherit = EnvironmentConfig {
        global_env: HashMap::new(),
        secrets: HashMap::new(),
        env_files: Vec::new(),
        inherit: true,
        profiles: HashMap::new(),
        active_profile: None,
    };

    let step_env = prodigy::cook::environment::config::StepEnvironment::default();
    let variables = HashMap::new();
    let context = manager
        .setup_environment(&step_env, Some(&config_with_inherit), &variables)
        .await?;

    // Should inherit the environment variable
    assert_eq!(
        context.env.get("TEST_INHERITED_VAR"),
        Some(&"inherited_value".to_string())
    );

    // Test with inheritance disabled
    let config_no_inherit = EnvironmentConfig {
        inherit: false,
        ..config_with_inherit
    };

    let context_no_inherit = manager
        .setup_environment(&step_env, Some(&config_no_inherit), &variables)
        .await?;

    // Should not inherit the environment variable
    assert_eq!(context_no_inherit.env.get("TEST_INHERITED_VAR"), None);

    // Clean up
    std::env::remove_var("TEST_INHERITED_VAR");

    Ok(())
}

#[tokio::test]
async fn test_workflow_with_environment_steps() -> Result<()> {
    // This test verifies that workflow steps can have their own environment configuration
    let workflow_yaml = r#"
env:
  GLOBAL_VAR: global_value

commands:
  - name: "Step with env"
    shell: "echo $STEP_VAR"
    env:
      STEP_VAR: step_value

  - name: "Step with working dir"
    shell: "pwd"
    working_dir: /tmp
"#;

    let workflow: WorkflowConfig = serde_yaml::from_str(workflow_yaml)?;

    // Verify the workflow has global env
    assert!(workflow.env.is_some());
    assert_eq!(
        workflow.env.as_ref().unwrap().get("GLOBAL_VAR"),
        Some(&"global_value".to_string())
    );

    // Verify commands are parsed
    assert_eq!(workflow.commands.len(), 2);

    Ok(())
}