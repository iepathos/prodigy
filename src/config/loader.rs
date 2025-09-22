use super::Config;
use crate::core::config::{
    merge_project_config, merge_workflow_config, parse_project_config, parse_workflow_config,
    validate_config_format,
};
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::fs;

/// Configuration loader responsible for finding and loading MMM configuration files
///
/// This loader supports both TOML and YAML formats and implements a search hierarchy:
/// 1. Explicit path provided by user
/// 2. `.prodigy/config.toml` in the project directory
/// 3. Legacy `.prodigy/workflow.toml` for backward compatibility
/// 4. Default configuration when no file is found
pub struct ConfigLoader {
    config: Arc<RwLock<Config>>,
}

impl ConfigLoader {
    pub async fn new() -> Result<Self> {
        let config = Config::new();

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Load configuration with precedence rules:
    /// 1. If explicit_path is provided, use that file (error if not found)
    /// 2. Otherwise, check for .prodigy/workflow.yml in project_path
    /// 3. Otherwise, return default configuration
    pub async fn load_with_explicit_path(
        &self,
        project_path: &Path,
        explicit_path: Option<&Path>,
    ) -> Result<()> {
        match explicit_path {
            Some(path) => {
                // Load from explicit path, error if not found
                self.load_from_path(path).await?;
            }
            None => {
                // Check for .prodigy/workflow.yml
                let default_path = project_path.join(".prodigy").join("workflow.yml");
                if default_path.exists() {
                    self.load_from_path(&default_path).await?;
                }
                // Otherwise use defaults (already set in new())
            }
        }
        Ok(())
    }

    /// Load configuration from a specific file path
    async fn load_from_path(&self, path: &Path) -> Result<()> {
        // I/O operation: read file
        let content = fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read configuration file: {}", path.display()))?;

        // Extract extension for validation
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

        // Pure validation
        validate_config_format(extension)
            .with_context(|| format!("Invalid configuration file: {}", path.display()))?;

        // Pure parsing
        let workflow_config = parse_workflow_config(&content)
            .with_context(|| format!("Failed to parse configuration: {}", path.display()))?;

        // Update state
        let mut config = self
            .config
            .write()
            .map_err(|_| anyhow!("Failed to acquire write lock for config"))?;
        *config = merge_workflow_config(config.clone(), workflow_config);

        Ok(())
    }

    pub async fn load_project(&self, project_path: &Path) -> Result<()> {
        let config_path = project_path.join(".prodigy").join("config.yml");

        if config_path.exists() {
            // I/O operation: read file
            let content = fs::read_to_string(&config_path).await?;

            // Pure parsing
            let project_config = parse_project_config(&content)?;

            // Update state
            let mut config = self
                .config
                .write()
                .map_err(|_| anyhow!("Failed to acquire write lock for config"))?;
            *config = merge_project_config(config.clone(), project_config);
        }

        Ok(())
    }
    pub fn get_config(&self) -> Config {
        self.config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_new_creates_default_config() -> Result<()> {
        let loader = ConfigLoader::new().await?;
        let config = loader.get_config();

        // Check defaults
        assert!(config.project.is_none());
        assert!(config.workflow.is_none());
        assert_eq!(config.global.log_level, Some("info".to_string()));
        assert_eq!(config.global.max_concurrent_specs, Some(1));
        assert_eq!(config.global.auto_commit, Some(true));
        Ok(())
    }

    #[tokio::test]
    async fn test_load_with_explicit_path_yaml() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let workflow_path = temp_dir.path().join("workflow.yml");

        // Create a test workflow config
        let workflow_content = r#"
commands:
  - prodigy-code-review
  - prodigy-implement-spec
  - prodigy-lint
"#;
        fs::write(&workflow_path, workflow_content).await?;

        let loader = ConfigLoader::new().await?;
        loader
            .load_with_explicit_path(temp_dir.path(), Some(&workflow_path))
            .await?;

        let config = loader.get_config();
        assert!(config.workflow.is_some());
        let workflow = config.workflow.unwrap();
        assert_eq!(workflow.commands.len(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_load_with_explicit_path_nested_workflow() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.yml");

        // Create a config with nested workflow section
        let config_content = r#"
workflow:
  commands:
    - name: prodigy-code-review
      options:
        focus: performance
    - prodigy-lint
"#;
        fs::write(&config_path, config_content).await.unwrap();

        let loader = ConfigLoader::new().await?;
        loader
            .load_with_explicit_path(temp_dir.path(), Some(&config_path))
            .await?;

        let config = loader.get_config();
        assert!(config.workflow.is_some());
        let workflow = config.workflow.unwrap();
        assert_eq!(workflow.commands.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_load_with_default_path() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let prodigy_dir = temp_dir.path().join(".prodigy");
        fs::create_dir(&prodigy_dir).await?;
        let workflow_path = prodigy_dir.join("workflow.yml");

        // Create default workflow config
        let workflow_content = r#"
commands:
  - prodigy-test
"#;
        fs::write(&workflow_path, workflow_content).await?;

        let loader = ConfigLoader::new().await?;
        // No explicit path, should find .prodigy/workflow.yml
        loader
            .load_with_explicit_path(temp_dir.path(), None)
            .await?;

        let config = loader.get_config();
        assert!(config.workflow.is_some());
        let workflow = config.workflow.unwrap();
        assert_eq!(workflow.commands.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_load_with_no_config_uses_defaults() -> Result<()> {
        let temp_dir = TempDir::new()?;

        let loader = ConfigLoader::new().await?;
        // No config files exist
        loader
            .load_with_explicit_path(temp_dir.path(), None)
            .await?;

        let config = loader.get_config();
        assert!(config.workflow.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_load_project_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let prodigy_dir = temp_dir.path().join(".prodigy");
        fs::create_dir(&prodigy_dir).await?;
        let config_path = prodigy_dir.join("config.yml");

        // Create project config
        let project_content = r#"
name: test-project
description: A test project
version: 1.0.0
spec_dir: custom-specs
claude_api_key: test-key
auto_commit: false
"#;
        fs::write(&config_path, project_content).await?;

        let loader = ConfigLoader::new().await?;
        loader.load_project(temp_dir.path()).await?;

        let config = loader.get_config();
        assert!(config.project.is_some());
        let project = config.project.unwrap();
        assert_eq!(project.name, "test-project");
        assert_eq!(project.description, Some("A test project".to_string()));
        assert_eq!(project.version, Some("1.0.0".to_string()));
        assert_eq!(project.spec_dir, Some(PathBuf::from("custom-specs")));
        assert_eq!(project.claude_api_key, Some("test-key".to_string()));
        assert_eq!(project.auto_commit, Some(false));
        Ok(())
    }

    #[tokio::test]
    async fn test_load_from_path_unsupported_format() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, "{}").await?;

        let loader = ConfigLoader::new().await?;
        let result = loader.load_from_path(&config_path).await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid configuration file")
                || error_msg.contains("Unsupported configuration file format"),
            "Expected error message about unsupported format, got: {}",
            error_msg
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_load_from_path_invalid_yaml() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.yml");
        fs::write(&config_path, "invalid: yaml: content:").await?;

        let loader = ConfigLoader::new().await?;
        let result = loader.load_from_path(&config_path).await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_load_from_path_nonexistent_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("nonexistent.yml");

        let loader = ConfigLoader::new().await?;
        let result = loader.load_from_path(&config_path).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read configuration file"));
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_access() -> Result<()> {
        let loader = ConfigLoader::new().await?;
        let loader_arc = Arc::new(loader);

        // Spawn multiple tasks that read config concurrently
        let mut handles = vec![];
        for _ in 0..10 {
            let loader_clone = loader_arc.clone();
            let handle = tokio::spawn(async move {
                let config = loader_clone.get_config();
                assert!(config.global.auto_commit.is_some());
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }
        Ok(())
    }
}
