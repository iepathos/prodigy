use crate::{Error, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

pub mod manager;
pub mod template;

pub use manager::ProjectManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub metadata: ProjectMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectMetadata {
    pub description: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub claude_api_key: Option<String>,
    pub max_iterations: Option<u32>,
    pub auto_commit: Option<bool>,
}

impl Project {
    pub fn new(name: String, path: PathBuf) -> Self {
        let now = chrono::Utc::now();
        Self {
            name,
            path,
            created_at: now,
            updated_at: now,
            metadata: ProjectMetadata::default(),
        }
    }
    
    pub async fn init_structure(&self) -> Result<()> {
        let mmm_dir = self.path.join(".mmm");
        fs::create_dir_all(&mmm_dir).await?;
        
        let specs_dir = self.path.join("specs");
        fs::create_dir_all(&specs_dir).await?;
        
        let config_path = mmm_dir.join("config.toml");
        if !config_path.exists() {
            let config = ProjectConfig {
                name: self.name.clone(),
                description: self.metadata.description.clone(),
                version: self.metadata.version.clone(),
                claude_api_key: None,
                max_iterations: Some(10),
                auto_commit: Some(true),
            };
            
            let config_content = toml::to_string_pretty(&config)
                .map_err(|e| Error::Config(e.to_string()))?;
            fs::write(&config_path, config_content).await?;
        }
        
        let manifest_path = self.path.join("mmm.toml");
        if !manifest_path.exists() {
            let manifest = ProjectManifest {
                project: ManifestProject {
                    name: self.name.clone(),
                    version: self.metadata.version.clone().unwrap_or_else(|| "0.1.0".to_string()),
                    description: self.metadata.description.clone(),
                },
            };
            
            let manifest_content = toml::to_string_pretty(&manifest)
                .map_err(|e| Error::Config(e.to_string()))?;
            fs::write(&manifest_path, manifest_content).await?;
        }
        
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectManifest {
    project: ManifestProject,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestProject {
    name: String,
    version: String,
    description: Option<String>,
}

pub fn get_global_mmm_dir() -> Result<PathBuf> {
    ProjectDirs::from("com", "mmm", "mmm")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or_else(|| Error::Config("Could not determine home directory".to_string()))
}