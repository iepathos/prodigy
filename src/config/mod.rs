use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

pub mod loader;
pub mod validator;

pub use loader::ConfigLoader;
pub use validator::ConfigValidator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub global: GlobalConfig,
    pub project: Option<ProjectConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            project: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub mmm_home: PathBuf,
    pub default_editor: Option<String>,
    pub log_level: Option<String>,
    pub claude_api_key: Option<String>,
    pub max_concurrent_specs: Option<u32>,
    pub auto_commit: Option<bool>,
    pub plugins: Option<PluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub spec_dir: Option<PathBuf>,
    pub claude_api_key: Option<String>,
    pub max_iterations: Option<u32>,
    pub auto_commit: Option<bool>,
    pub variables: Option<toml::Table>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub auto_load: Vec<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            mmm_home: crate::project::get_global_mmm_dir()
                .unwrap_or_else(|_| PathBuf::from("~/.mmm")),
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
        }
    }

    pub fn merge_env_vars(&mut self) {
        if let Ok(api_key) = std::env::var("MMM_CLAUDE_API_KEY") {
            self.global.claude_api_key = Some(api_key);
        }

        if let Ok(log_level) = std::env::var("MMM_LOG_LEVEL") {
            self.global.log_level = Some(log_level);
        }

        if let Ok(editor) = std::env::var("MMM_EDITOR") {
            self.global.default_editor = Some(editor);
        } else if let Ok(editor) = std::env::var("EDITOR") {
            self.global.default_editor = Some(editor);
        }

        if let Ok(auto_commit) = std::env::var("MMM_AUTO_COMMIT") {
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

    pub fn get_max_iterations(&self) -> u32 {
        self.project
            .as_ref()
            .and_then(|p| p.max_iterations)
            .unwrap_or(10)
    }

    pub fn get_spec_dir(&self) -> PathBuf {
        self.project
            .as_ref()
            .and_then(|p| p.spec_dir.clone())
            .unwrap_or_else(|| PathBuf::from("specs"))
    }
}
