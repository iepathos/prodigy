//! Unified configuration structure for Prodigy using premortem.
//!
//! This module provides `ProdigyConfig`, a single entry point for all configuration
//! access. It uses premortem's builder pattern for layered source loading and
//! stillwater's `Validation` for comprehensive error accumulation.
//!
//! # Example
//!
//! ```ignore
//! use prodigy::config::prodigy_config::load_prodigy_config;
//!
//! // Load from all sources with real I/O
//! let config = load_prodigy_config().expect("config errors");
//! println!("Max concurrent specs: {}", config.max_concurrent_specs);
//! ```
//!
//! # Testing Example
//!
//! ```ignore
//! use prodigy::config::prodigy_config::load_prodigy_config_with;
//! use premortem::MockEnv;
//!
//! let env = MockEnv::new()
//!     .with_file("~/.prodigy/config.yml", "log_level: info")
//!     .with_env("PRODIGY_LOG_LEVEL", "debug");
//!
//! let config = load_prodigy_config_with(&env).unwrap();
//! assert_eq!(config.log_level, "debug");
//! ```

use premortem::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unified configuration for Prodigy.
///
/// Combines global settings, project settings, and runtime settings into a single
/// struct. Configuration is loaded from multiple sources with layered precedence:
///
/// 1. Hardcoded defaults (lowest priority)
/// 2. Global config file (`~/.prodigy/config.yml`)
/// 3. Project config file (`.prodigy/config.yml`)
/// 4. Environment variables (`PRODIGY_*` prefix) (highest priority)
///
/// # Validation
///
/// All fields are validated during loading. Invalid configurations result in
/// accumulated errors - all issues are reported at once, not just the first one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProdigyConfig {
    /// Logging level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Claude API key for authentication.
    /// Optional - can be set via environment variable PRODIGY_CLAUDE_API_KEY.
    #[serde(default)]
    pub claude_api_key: Option<String>,

    /// Maximum number of specs to process concurrently in MapReduce workflows.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_specs: usize,

    /// Whether to automatically commit changes after successful operations.
    #[serde(default = "default_auto_commit")]
    pub auto_commit: bool,

    /// Default editor for interactive editing.
    #[serde(default)]
    pub default_editor: Option<String>,

    /// Prodigy home directory for storing data and configuration.
    #[serde(default)]
    pub prodigy_home: Option<PathBuf>,

    /// Project-specific settings.
    #[serde(default)]
    pub project: Option<ProjectSettings>,

    /// Storage configuration.
    #[serde(default)]
    pub storage: StorageSettings,
}

/// Project-specific configuration settings.
///
/// These settings apply to a specific project and override global defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectSettings {
    /// Project name.
    #[serde(default)]
    pub name: Option<String>,

    /// Project description.
    #[serde(default)]
    pub description: Option<String>,

    /// Project version.
    #[serde(default)]
    pub version: Option<String>,

    /// Directory containing spec files.
    #[serde(default)]
    pub spec_dir: Option<PathBuf>,

    /// Project-level API key (overrides global).
    #[serde(default)]
    pub claude_api_key: Option<String>,

    /// Project-level auto-commit setting (overrides global).
    #[serde(default)]
    pub auto_commit: Option<bool>,

    /// Custom variables for this project.
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
}

/// Storage backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageSettings {
    /// Storage backend type.
    #[serde(default)]
    pub backend: BackendType,

    /// Base path for storage (if applicable to backend).
    #[serde(default)]
    pub base_path: Option<PathBuf>,

    /// Compression level for checkpoints (0-9, 0 = none).
    #[serde(default)]
    pub compression_level: u8,
}

/// Supported storage backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackendType {
    /// File-system based storage (default).
    #[default]
    FileSystem,
    /// In-memory storage (for testing).
    Memory,
}

impl Default for ProdigyConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            claude_api_key: None,
            max_concurrent_specs: default_max_concurrent(),
            auto_commit: default_auto_commit(),
            default_editor: None,
            prodigy_home: None,
            project: None,
            storage: StorageSettings::default(),
        }
    }
}

// Default value functions for serde
fn default_log_level() -> String {
    "info".to_string()
}

fn default_max_concurrent() -> usize {
    4
}

fn default_auto_commit() -> bool {
    true
}

impl ProdigyConfig {
    /// Get the effective Claude API key (project overrides global).
    pub fn get_claude_api_key(&self) -> Option<&str> {
        self.project
            .as_ref()
            .and_then(|p| p.claude_api_key.as_deref())
            .or(self.claude_api_key.as_deref())
    }

    /// Get the effective auto-commit setting (project overrides global).
    pub fn get_auto_commit(&self) -> bool {
        self.project
            .as_ref()
            .and_then(|p| p.auto_commit)
            .unwrap_or(self.auto_commit)
    }

    /// Get the spec directory path (defaults to "specs").
    pub fn get_spec_dir(&self) -> PathBuf {
        self.project
            .as_ref()
            .and_then(|p| p.spec_dir.clone())
            .unwrap_or_else(|| PathBuf::from("specs"))
    }

    /// Get the prodigy home directory.
    ///
    /// Falls back to `~/.prodigy` if not explicitly set.
    pub fn get_prodigy_home(&self) -> PathBuf {
        self.prodigy_home.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".prodigy"))
                .unwrap_or_else(|| PathBuf::from("~/.prodigy"))
        })
    }
}

impl Validate for ProdigyConfig {
    fn validate(&self) -> ConfigValidation<()> {
        let mut errors = Vec::new();

        // Validate log_level is not empty
        if self.log_level.is_empty() {
            errors.push(ConfigError::ValidationError {
                path: "log_level".to_string(),
                source_location: None,
                value: Some(self.log_level.clone()),
                message: "log_level cannot be empty".to_string(),
            });
        }

        // Validate max_concurrent_specs is in range 1..=100
        if self.max_concurrent_specs == 0 || self.max_concurrent_specs > 100 {
            errors.push(ConfigError::ValidationError {
                path: "max_concurrent_specs".to_string(),
                source_location: None,
                value: Some(self.max_concurrent_specs.to_string()),
                message: "max_concurrent_specs must be between 1 and 100".to_string(),
            });
        }

        // Validate storage.compression_level is in range 0..=9
        if self.storage.compression_level > 9 {
            errors.push(ConfigError::ValidationError {
                path: "storage.compression_level".to_string(),
                source_location: None,
                value: Some(self.storage.compression_level.to_string()),
                message: "storage.compression_level must be between 0 and 9".to_string(),
            });
        }

        match ConfigErrors::from_vec(errors) {
            Some(errs) => Validation::Failure(errs),
            None => Validation::Success(()),
        }
    }
}

/// Returns the global config file path.
///
/// This is `~/.prodigy/config.yml`.
pub fn global_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".prodigy").join("config.yml"))
        .unwrap_or_else(|| PathBuf::from("~/.prodigy/config.yml"))
}

/// Returns the project config file path.
///
/// This is `.prodigy/config.yml` in the current directory.
pub fn project_config_path() -> PathBuf {
    PathBuf::from(".prodigy/config.yml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prodigy_config_default() {
        let config = ProdigyConfig::default();

        assert_eq!(config.log_level, "info");
        assert!(config.claude_api_key.is_none());
        assert_eq!(config.max_concurrent_specs, 4);
        assert!(config.auto_commit);
        assert!(config.default_editor.is_none());
        assert!(config.project.is_none());
        assert_eq!(config.storage.backend, BackendType::FileSystem);
    }

    #[test]
    fn test_get_claude_api_key_precedence() {
        let mut config = ProdigyConfig::default();

        // No API key set
        assert!(config.get_claude_api_key().is_none());

        // Global API key only
        config.claude_api_key = Some("global-key".to_string());
        assert_eq!(config.get_claude_api_key(), Some("global-key"));

        // Project API key takes precedence
        config.project = Some(ProjectSettings {
            claude_api_key: Some("project-key".to_string()),
            ..Default::default()
        });
        assert_eq!(config.get_claude_api_key(), Some("project-key"));
    }

    #[test]
    fn test_get_auto_commit_precedence() {
        let mut config = ProdigyConfig::default();

        // Default value
        assert!(config.get_auto_commit());

        // Global setting
        config.auto_commit = false;
        assert!(!config.get_auto_commit());

        // Project setting takes precedence
        config.project = Some(ProjectSettings {
            auto_commit: Some(true),
            ..Default::default()
        });
        assert!(config.get_auto_commit());
    }

    #[test]
    fn test_get_spec_dir() {
        let mut config = ProdigyConfig::default();

        // Default value
        assert_eq!(config.get_spec_dir(), PathBuf::from("specs"));

        // Project setting
        config.project = Some(ProjectSettings {
            spec_dir: Some(PathBuf::from("custom/specs")),
            ..Default::default()
        });
        assert_eq!(config.get_spec_dir(), PathBuf::from("custom/specs"));
    }

    #[test]
    fn test_yaml_deserialization() {
        let yaml = r#"
log_level: debug
max_concurrent_specs: 8
auto_commit: false
project:
  name: test-project
  spec_dir: my-specs
storage:
  backend: memory
  compression_level: 6
"#;

        let config: ProdigyConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.log_level, "debug");
        assert_eq!(config.max_concurrent_specs, 8);
        assert!(!config.auto_commit);

        let project = config.project.unwrap();
        assert_eq!(project.name, Some("test-project".to_string()));
        assert_eq!(project.spec_dir, Some(PathBuf::from("my-specs")));

        assert_eq!(config.storage.backend, BackendType::Memory);
        assert_eq!(config.storage.compression_level, 6);
    }

    #[test]
    fn test_validation() {
        // Valid config
        let config = ProdigyConfig::default();
        let result = config.validate();
        assert!(matches!(result, Validation::Success(_)));

        // Invalid max_concurrent_specs (0 is out of range 1..=100)
        let invalid_config = ProdigyConfig {
            max_concurrent_specs: 0,
            ..Default::default()
        };
        let result = invalid_config.validate();
        assert!(matches!(result, Validation::Failure(_)));
    }

    #[test]
    fn test_storage_settings_validation_via_config() {
        // Valid compression level through ProdigyConfig
        let config = ProdigyConfig {
            storage: StorageSettings {
                backend: BackendType::FileSystem,
                base_path: None,
                compression_level: 6,
            },
            ..Default::default()
        };
        let result = config.validate();
        assert!(matches!(result, Validation::Success(_)));

        // Invalid compression level (out of range)
        let invalid_config = ProdigyConfig {
            storage: StorageSettings {
                backend: BackendType::FileSystem,
                base_path: None,
                compression_level: 10, // Invalid: max is 9
            },
            ..Default::default()
        };
        let result = invalid_config.validate();
        assert!(matches!(result, Validation::Failure(_)));
    }

    #[test]
    fn test_backend_type_serialization() {
        assert_eq!(
            serde_json::to_string(&BackendType::FileSystem).unwrap(),
            "\"filesystem\""
        );
        assert_eq!(
            serde_json::to_string(&BackendType::Memory).unwrap(),
            "\"memory\""
        );

        let fs: BackendType = serde_json::from_str("\"filesystem\"").unwrap();
        assert_eq!(fs, BackendType::FileSystem);

        let mem: BackendType = serde_json::from_str("\"memory\"").unwrap();
        assert_eq!(mem, BackendType::Memory);
    }
}
