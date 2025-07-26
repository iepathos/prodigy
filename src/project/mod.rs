use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

pub mod health;
pub mod manager;
pub mod template;

pub use health::{HealthCheck, HealthStatus, ProjectHealth, Severity};
pub use manager::ProjectManager;
pub use template::TemplateManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub created: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub template: Option<String>,
    pub version: Option<String>,
    pub archived: bool,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub team: Vec<String>,
    pub repository: Option<String>,
    pub total_specs: usize,
    pub completed_specs: usize,
    pub total_iterations: usize,
    pub success_rate: f64,
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
            created: now,
            last_accessed: now,
            template: None,
            version: Some("0.1.0".to_string()),
            archived: false,
            description: None,
            tags: Vec::new(),
            team: Vec::new(),
            repository: None,
            total_specs: 0,
            completed_specs: 0,
            total_iterations: 0,
            success_rate: 0.0,
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
                description: self.description.clone(),
                version: self.version.clone(),
                claude_api_key: None,
                max_iterations: Some(10),
                auto_commit: Some(true),
            };

            let config_content =
                toml::to_string_pretty(&config).context("Failed to serialize project config")?;
            fs::write(&config_path, config_content).await?;
        }

        let manifest_path = self.path.join("mmm.toml");
        if !manifest_path.exists() {
            let manifest = ProjectManifest {
                project: ManifestProject {
                    name: self.name.clone(),
                    version: self.version.clone().unwrap_or_else(|| "0.1.0".to_string()),
                    description: self.description.clone(),
                },
            };

            let manifest_content = toml::to_string_pretty(&manifest)
                .context("Failed to serialize project manifest")?;
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
        .ok_or_else(|| anyhow!("Could not determine home directory"))
}
