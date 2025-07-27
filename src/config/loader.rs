use super::{Config, GlobalConfig, ProjectConfig, WorkflowConfig};
use anyhow::{anyhow, Context, Result};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::fs;
use tokio::sync::mpsc;

pub struct ConfigLoader {
    config: Arc<RwLock<Config>>,
    watcher: Option<notify::RecommendedWatcher>,
    reload_tx: Option<mpsc::Sender<PathBuf>>,
}

impl ConfigLoader {
    pub async fn new() -> Result<Self> {
        let config = Config::new();

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            watcher: None,
            reload_tx: None,
        })
    }

    pub async fn load_global(&self) -> Result<()> {
        let global_dir = super::get_global_mmm_dir()?;
        let yaml_config_path = global_dir.join("config.yml");

        if yaml_config_path.exists() {
            let content = fs::read_to_string(&yaml_config_path).await?;
            let global_config: GlobalConfig = serde_yaml::from_str(&content)?;

            let mut config = self.config.write().unwrap();
            config.global = global_config;
        }

        let mut config = self.config.write().unwrap();
        config.merge_env_vars();

        Ok(())
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

    pub async fn enable_hot_reload(&mut self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(10);
        self.reload_tx = Some(tx);

        let config = self.config.clone();
        tokio::spawn(async move {
            while let Some(path) = rx.recv().await {
                if let Ok(content) = fs::read_to_string(&path).await {
                    let is_global = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .map(|n| n != ".mmm")
                        .unwrap_or(false);

                    if is_global {
                        if let Ok(global_config) = serde_yaml::from_str::<GlobalConfig>(&content) {
                            let mut cfg = config.write().unwrap();
                            cfg.global = global_config;
                            cfg.merge_env_vars();
                            tracing::info!("Reloaded global configuration");
                        }
                    } else if let Ok(project_config) =
                        serde_yaml::from_str::<ProjectConfig>(&content)
                    {
                        let mut cfg = config.write().unwrap();
                        cfg.project = Some(project_config);
                        tracing::info!("Reloaded project configuration");
                    }
                }
            }
        });

        Ok(())
    }

    pub fn watch_config_file(&mut self, path: PathBuf) -> Result<()> {
        let tx = self
            .reload_tx
            .clone()
            .ok_or_else(|| anyhow!("Hot reload not enabled"))?;

        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        for path in event.paths {
                            if path
                                .extension()
                                .is_some_and(|ext| ext == "yaml" || ext == "yml")
                            {
                                let _ = tx.blocking_send(path);
                            }
                        }
                    }
                }
            })?;

        watcher.watch(&path, RecursiveMode::NonRecursive)?;
        self.watcher = Some(watcher);

        Ok(())
    }

    pub fn get_config(&self) -> Config {
        self.config.read().unwrap().clone()
    }

    pub fn get_project_value(&self, key: &str) -> Result<String> {
        let config = self.config.read().unwrap();

        if let Some(project) = &config.project {
            match key {
                "name" => Ok(project.name.clone()),
                "description" => Ok(project.description.clone().unwrap_or_default()),
                "version" => Ok(project.version.clone().unwrap_or_default()),
                "spec_dir" => Ok(project
                    .spec_dir
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()),
                "claude_api_key" => Ok(project.claude_api_key.clone().unwrap_or_default()),
                "max_iterations" => Ok(project
                    .max_iterations
                    .map(|v| v.to_string())
                    .unwrap_or_default()),
                "auto_commit" => Ok(project
                    .auto_commit
                    .map(|v| v.to_string())
                    .unwrap_or_default()),
                _ => {
                    // Check in variables table
                    if let Some(variables) = &project.variables {
                        if let Some(value) = variables.get(key) {
                            return Ok(value.to_string());
                        }
                    }
                    Err(anyhow!("Unknown configuration key: {key}"))
                }
            }
        } else {
            Err(anyhow!("No project loaded"))
        }
    }

    pub async fn set_project_value(&self, key: &str, value: &str) -> Result<()> {
        let mut config = self.config.write().unwrap();

        if let Some(project) = &mut config.project {
            match key {
                "name" => project.name = value.to_string(),
                "description" => project.description = Some(value.to_string()),
                "version" => project.version = Some(value.to_string()),
                "spec_dir" => project.spec_dir = Some(PathBuf::from(value)),
                "claude_api_key" => project.claude_api_key = Some(value.to_string()),
                "max_iterations" => {
                    project.max_iterations = value.parse().ok();
                }
                "auto_commit" => {
                    project.auto_commit = value.parse().ok();
                }
                _ => {
                    // Store in variables table
                    // Variables table functionality removed for YAML-only approach
                    return Err(anyhow!(
                        "Custom variables not supported in current YAML configuration"
                    ));
                }
            }

            // Note: Changes are only made in-memory. Saving to disk is not implemented
            // as this appears to be unused functionality.

            Ok(())
        } else {
            Err(anyhow!("No project loaded"))
        }
    }
}
