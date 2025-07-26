use super::{Config, GlobalConfig, ProjectConfig};
use anyhow::{anyhow, Result};
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
        let global_dir = crate::project::get_global_mmm_dir()?;
        let config_path = global_dir.join("config.toml");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let global_config: GlobalConfig = toml::from_str(&content)?;

            let mut config = self.config.write().unwrap();
            config.global = global_config;
        }

        let mut config = self.config.write().unwrap();
        config.merge_env_vars();

        Ok(())
    }

    pub async fn load_project(&self, project_path: &Path) -> Result<()> {
        let config_path = project_path.join(".mmm").join("config.toml");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let project_config: ProjectConfig = toml::from_str(&content)?;

            let mut config = self.config.write().unwrap();
            config.project = Some(project_config);
        }

        let manifest_path = project_path.join("mmm.toml");
        if manifest_path.exists() && !config_path.exists() {
            let content = fs::read_to_string(&manifest_path).await?;
            if let Ok(manifest) = toml::from_str::<toml::Table>(&content) {
                if let Some(project) = manifest.get("project").and_then(|p| p.as_table()) {
                    let project_config = ProjectConfig {
                        name: project
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unnamed")
                            .to_string(),
                        description: project
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(|s| s.to_string()),
                        version: project
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        spec_dir: None,
                        claude_api_key: None,
                        max_iterations: None,
                        auto_commit: None,
                        variables: None,
                    };

                    let mut config = self.config.write().unwrap();
                    config.project = Some(project_config);
                }
            }
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
                        if let Ok(global_config) = toml::from_str::<GlobalConfig>(&content) {
                            let mut cfg = config.write().unwrap();
                            cfg.global = global_config;
                            cfg.merge_env_vars();
                            tracing::info!("Reloaded global configuration");
                        }
                    } else if let Ok(project_config) = toml::from_str::<ProjectConfig>(&content) {
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
                            if path.extension().is_some_and(|ext| ext == "toml") {
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
                    if project.variables.is_none() {
                        project.variables = Some(toml::Table::new());
                    }
                    if let Some(variables) = &mut project.variables {
                        variables.insert(key.to_string(), toml::Value::String(value.to_string()));
                    }
                }
            }

            // Save to disk
            // TODO: Get project path from somewhere

            Ok(())
        } else {
            Err(anyhow!("No project loaded"))
        }
    }
}
