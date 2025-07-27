use super::{Config, ProjectConfig, WorkflowConfig};
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::fs;

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
    /// 2. Otherwise, check for .mmm/workflow.yml in project_path
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
                // Check for .mmm/workflow.yml
                let default_path = project_path.join(".mmm").join("workflow.yml");
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
        let content = fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read configuration file: {}", path.display()))?;

        // Determine format based on extension
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

        match extension {
            "yaml" | "yml" => {
                // First try to parse as a full config with workflow section
                if let Ok(full_config) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    if let Some(workflow_value) = full_config.get("workflow") {
                        let workflow_config: WorkflowConfig =
                            serde_yaml::from_value(workflow_value.clone()).with_context(|| {
                                format!(
                                    "Failed to parse workflow configuration from YAML: {}",
                                    path.display()
                                )
                            })?;
                        let mut config = self.config.write().unwrap();
                        config.workflow = Some(workflow_config);
                    } else {
                        // Try to parse as direct WorkflowConfig
                        let workflow_config: WorkflowConfig = serde_yaml::from_str(&content)
                            .with_context(|| {
                                format!("Failed to parse YAML configuration: {}", path.display())
                            })?;
                        let mut config = self.config.write().unwrap();
                        config.workflow = Some(workflow_config);
                    }
                } else {
                    // Try to parse as direct WorkflowConfig
                    let workflow_config: WorkflowConfig = serde_yaml::from_str(&content)
                        .with_context(|| {
                            format!("Failed to parse YAML configuration: {}", path.display())
                        })?;
                    let mut config = self.config.write().unwrap();
                    config.workflow = Some(workflow_config);
                }
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported configuration file format: {}. Use .yaml or .yml",
                    path.display()
                ));
            }
        }

        Ok(())
    }

    pub async fn load_project(&self, project_path: &Path) -> Result<()> {
        let config_path = project_path.join(".mmm").join("config.yml");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let project_config: ProjectConfig = serde_yaml::from_str(&content)?;

            let mut config = self.config.write().unwrap();
            config.project = Some(project_config);
        }

        Ok(())
    }
    pub fn get_config(&self) -> Config {
        self.config.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_new_creates_default_config() {
        let loader = ConfigLoader::new().await.unwrap();
        let config = loader.get_config();

        // Check defaults
        assert!(config.project.is_none());
        assert!(config.workflow.is_none());
        assert_eq!(config.global.log_level, Some("info".to_string()));
        assert_eq!(config.global.max_concurrent_specs, Some(1));
        assert_eq!(config.global.auto_commit, Some(true));
    }

    #[tokio::test]
    async fn test_load_with_explicit_path_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let workflow_path = temp_dir.path().join("workflow.yml");

        // Create a test workflow config
        let workflow_content = r#"
commands:
  - mmm-code-review
  - mmm-implement-spec
  - mmm-lint
max_iterations: 5
"#;
        fs::write(&workflow_path, workflow_content).await.unwrap();

        let loader = ConfigLoader::new().await.unwrap();
        loader
            .load_with_explicit_path(temp_dir.path(), Some(&workflow_path))
            .await
            .unwrap();

        let config = loader.get_config();
        assert!(config.workflow.is_some());
        let workflow = config.workflow.unwrap();
        assert_eq!(workflow.commands.len(), 3);
        assert_eq!(workflow.max_iterations, 5);
    }

    #[tokio::test]
    async fn test_load_with_explicit_path_nested_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yml");

        // Create a config with nested workflow section
        let config_content = r#"
workflow:
  commands:
    - name: mmm-code-review
      options:
        focus: performance
    - mmm-lint
  max_iterations: 3
"#;
        fs::write(&config_path, config_content).await.unwrap();

        let loader = ConfigLoader::new().await.unwrap();
        loader
            .load_with_explicit_path(temp_dir.path(), Some(&config_path))
            .await
            .unwrap();

        let config = loader.get_config();
        assert!(config.workflow.is_some());
        let workflow = config.workflow.unwrap();
        assert_eq!(workflow.commands.len(), 2);
        assert_eq!(workflow.max_iterations, 3);
    }

    #[tokio::test]
    async fn test_load_with_default_path() {
        let temp_dir = TempDir::new().unwrap();
        let mmm_dir = temp_dir.path().join(".mmm");
        fs::create_dir(&mmm_dir).await.unwrap();
        let workflow_path = mmm_dir.join("workflow.yml");

        // Create default workflow config
        let workflow_content = r#"
commands:
  - mmm-test
max_iterations: 7
"#;
        fs::write(&workflow_path, workflow_content).await.unwrap();

        let loader = ConfigLoader::new().await.unwrap();
        // No explicit path, should find .mmm/workflow.yml
        loader
            .load_with_explicit_path(temp_dir.path(), None)
            .await
            .unwrap();

        let config = loader.get_config();
        assert!(config.workflow.is_some());
        let workflow = config.workflow.unwrap();
        assert_eq!(workflow.commands.len(), 1);
        assert_eq!(workflow.max_iterations, 7);
    }

    #[tokio::test]
    async fn test_load_with_no_config_uses_defaults() {
        let temp_dir = TempDir::new().unwrap();

        let loader = ConfigLoader::new().await.unwrap();
        // No config files exist
        loader
            .load_with_explicit_path(temp_dir.path(), None)
            .await
            .unwrap();

        let config = loader.get_config();
        assert!(config.workflow.is_none());
    }

    #[tokio::test]
    async fn test_load_project_config() {
        let temp_dir = TempDir::new().unwrap();
        let mmm_dir = temp_dir.path().join(".mmm");
        fs::create_dir(&mmm_dir).await.unwrap();
        let config_path = mmm_dir.join("config.yml");

        // Create project config
        let project_content = r#"
name: test-project
description: A test project
version: 1.0.0
spec_dir: custom-specs
claude_api_key: test-key
max_iterations: 15
auto_commit: false
"#;
        fs::write(&config_path, project_content).await.unwrap();

        let loader = ConfigLoader::new().await.unwrap();
        loader.load_project(temp_dir.path()).await.unwrap();

        let config = loader.get_config();
        assert!(config.project.is_some());
        let project = config.project.unwrap();
        assert_eq!(project.name, "test-project");
        assert_eq!(project.description, Some("A test project".to_string()));
        assert_eq!(project.version, Some("1.0.0".to_string()));
        assert_eq!(project.spec_dir, Some(PathBuf::from("custom-specs")));
        assert_eq!(project.claude_api_key, Some("test-key".to_string()));
        assert_eq!(project.max_iterations, Some(15));
        assert_eq!(project.auto_commit, Some(false));
    }

    #[tokio::test]
    async fn test_load_from_path_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, "{}").await.unwrap();

        let loader = ConfigLoader::new().await.unwrap();
        let result = loader.load_from_path(&config_path).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported configuration file format"));
    }

    #[tokio::test]
    async fn test_load_from_path_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yml");
        fs::write(&config_path, "invalid: yaml: content:")
            .await
            .unwrap();

        let loader = ConfigLoader::new().await.unwrap();
        let result = loader.load_from_path(&config_path).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_from_path_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.yml");

        let loader = ConfigLoader::new().await.unwrap();
        let result = loader.load_from_path(&config_path).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read configuration file"));
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let loader = ConfigLoader::new().await.unwrap();
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
    }
}
