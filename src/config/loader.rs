use super::{Config, GlobalConfig, ProjectConfig};
use crate::{Error, Result};
use notify::{Watcher, RecursiveMode, Event, EventKind};
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
                    let mut project_config = ProjectConfig {
                        name: project.get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unnamed")
                            .to_string(),
                        description: project.get("description")
                            .and_then(|d| d.as_str())
                            .map(|s| s.to_string()),
                        version: project.get("version")
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
                    let is_global = path.parent()
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
                    } else {
                        if let Ok(project_config) = toml::from_str::<ProjectConfig>(&content) {
                            let mut cfg = config.write().unwrap();
                            cfg.project = Some(project_config);
                            tracing::info!("Reloaded project configuration");
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    pub fn watch_config_file(&mut self, path: PathBuf) -> Result<()> {
        let tx = self.reload_tx.clone()
            .ok_or_else(|| Error::Config("Hot reload not enabled".to_string()))?;
        
        let mut watcher = notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_)) {
                    for path in event.paths {
                        if path.extension().map_or(false, |ext| ext == "toml") {
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
}