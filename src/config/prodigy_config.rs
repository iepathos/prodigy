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
//!
//! # Migration from GlobalConfig/ProjectConfig
//!
//! This module replaces the older `GlobalConfig` and `ProjectConfig` types with
//! a unified `ProdigyConfig`. The old types still exist for backward compatibility
//! but delegate to this new system.
//!
//! ```ignore
//! // Old approach (deprecated)
//! let global = GlobalConfig::load()?;
//! let project = ProjectConfig::load()?;
//! let api_key = project
//!     .and_then(|p| p.claude_api_key.clone())
//!     .or_else(|| global.claude_api_key.clone());
//!
//! // New approach
//! let config = load_prodigy_config()?;
//! let api_key = config.effective_api_key();  // Precedence handled internally
//! ```

use premortem::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Valid log levels for configuration validation.
pub const VALID_LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

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

    /// Plugin configuration.
    #[serde(default)]
    pub plugins: PluginConfig,
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

/// Plugin configuration for extending Prodigy functionality.
///
/// Plugins are loaded from a directory and can provide custom commands
/// and workflows.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginConfig {
    /// Whether plugins are enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Directory containing plugin files.
    #[serde(default)]
    pub directory: Option<PathBuf>,

    /// List of plugins to auto-load on startup.
    #[serde(default)]
    pub auto_load: Vec<String>,
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
            plugins: PluginConfig::default(),
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
    ///
    /// This method implements proper precedence handling:
    /// 1. Project-level API key (if set)
    /// 2. Global API key (if set)
    ///
    /// Note: Environment variables are already merged into these fields
    /// during configuration loading.
    #[deprecated(since = "0.6.0", note = "Use effective_api_key() instead")]
    pub fn get_claude_api_key(&self) -> Option<&str> {
        self.effective_api_key()
    }

    /// Get the effective Claude API key with proper precedence.
    ///
    /// Precedence (highest to lowest):
    /// 1. Project-level API key
    /// 2. Global API key
    ///
    /// Environment variables are merged during loading, so they're already
    /// reflected in these fields based on when they were applied.
    pub fn effective_api_key(&self) -> Option<&str> {
        self.project
            .as_ref()
            .and_then(|p| p.claude_api_key.as_deref())
            .or(self.claude_api_key.as_deref())
    }

    /// Get the effective auto-commit setting (project overrides global).
    #[deprecated(since = "0.6.0", note = "Use effective_auto_commit() instead")]
    pub fn get_auto_commit(&self) -> bool {
        self.effective_auto_commit()
    }

    /// Get the effective auto-commit setting with proper precedence.
    ///
    /// Precedence (highest to lowest):
    /// 1. Project-level setting (if set)
    /// 2. Global setting
    pub fn effective_auto_commit(&self) -> bool {
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

    /// Get the effective default editor.
    ///
    /// Returns the configured editor or None if not set.
    pub fn effective_editor(&self) -> Option<&str> {
        self.default_editor.as_deref()
    }

    /// Get the effective max concurrent specs.
    pub fn effective_max_concurrent(&self) -> usize {
        self.max_concurrent_specs
    }

    /// Get the effective log level.
    pub fn effective_log_level(&self) -> &str {
        &self.log_level
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
        } else if !VALID_LOG_LEVELS.contains(&self.log_level.as_str()) {
            // Validate log_level is one of the allowed values
            errors.push(ConfigError::ValidationError {
                path: "log_level".to_string(),
                source_location: None,
                value: Some(self.log_level.clone()),
                message: format!("log_level must be one of: {}", VALID_LOG_LEVELS.join(", ")),
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

        // Validate project settings if present
        if let Some(ref project) = self.project {
            // Validate project.name is non-empty when provided
            if let Some(ref name) = project.name {
                if name.is_empty() {
                    errors.push(ConfigError::ValidationError {
                        path: "project.name".to_string(),
                        source_location: None,
                        value: Some(name.clone()),
                        message: "project.name cannot be empty when provided".to_string(),
                    });
                }
            }

            // Cross-field validation: spec_dir should be a relative path
            if let Some(ref spec_dir) = project.spec_dir {
                if spec_dir.is_absolute() {
                    errors.push(ConfigError::ValidationError {
                        path: "project.spec_dir".to_string(),
                        source_location: None,
                        value: Some(spec_dir.display().to_string()),
                        message: "project.spec_dir should be a relative path".to_string(),
                    });
                }
            }
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

    #[test]
    fn test_validation_log_level_valid_values() {
        for level in VALID_LOG_LEVELS {
            let config = ProdigyConfig {
                log_level: level.to_string(),
                ..Default::default()
            };
            let result = config.validate();
            assert!(
                matches!(result, Validation::Success(_)),
                "log_level '{}' should be valid",
                level
            );
        }
    }

    #[test]
    fn test_validation_log_level_invalid() {
        let config = ProdigyConfig {
            log_level: "invalid_level".to_string(),
            ..Default::default()
        };
        let result = config.validate();

        match result {
            Validation::Failure(errors) => {
                assert!(
                    errors.iter().any(|e| {
                        matches!(e, ConfigError::ValidationError { path, .. } if path == "log_level")
                    }),
                    "Expected validation error for log_level"
                );
            }
            Validation::Success(_) => panic!("Expected validation to fail for invalid log_level"),
        }
    }

    #[test]
    fn test_validation_project_name_empty() {
        let config = ProdigyConfig {
            project: Some(ProjectSettings {
                name: Some("".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = config.validate();

        match result {
            Validation::Failure(errors) => {
                assert!(
                    errors.iter().any(|e| {
                        matches!(e, ConfigError::ValidationError { path, .. } if path == "project.name")
                    }),
                    "Expected validation error for project.name"
                );
            }
            Validation::Success(_) => {
                panic!("Expected validation to fail for empty project.name")
            }
        }
    }

    #[test]
    fn test_validation_spec_dir_absolute_path() {
        let config = ProdigyConfig {
            project: Some(ProjectSettings {
                spec_dir: Some(PathBuf::from("/absolute/path/to/specs")),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = config.validate();

        match result {
            Validation::Failure(errors) => {
                assert!(
                    errors.iter().any(|e| {
                        matches!(e, ConfigError::ValidationError { path, .. } if path == "project.spec_dir")
                    }),
                    "Expected validation error for project.spec_dir"
                );
            }
            Validation::Success(_) => {
                panic!("Expected validation to fail for absolute spec_dir")
            }
        }
    }

    #[test]
    fn test_validation_spec_dir_relative_path_valid() {
        let config = ProdigyConfig {
            project: Some(ProjectSettings {
                spec_dir: Some(PathBuf::from("specs")),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = config.validate();
        assert!(
            matches!(result, Validation::Success(_)),
            "Relative spec_dir should be valid"
        );
    }

    #[test]
    fn test_validation_error_accumulation() {
        // Create a config with multiple validation errors
        let config = ProdigyConfig {
            log_level: "invalid".to_string(),
            max_concurrent_specs: 0,
            storage: StorageSettings {
                compression_level: 15,
                ..Default::default()
            },
            project: Some(ProjectSettings {
                name: Some("".to_string()),
                spec_dir: Some(PathBuf::from("/absolute/path")),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = config.validate();

        match result {
            Validation::Failure(errors) => {
                // Should have accumulated all errors
                assert!(
                    errors.len() >= 4,
                    "Expected at least 4 errors, got {}",
                    errors.len()
                );
            }
            Validation::Success(_) => panic!("Expected validation to fail with multiple errors"),
        }
    }

    #[test]
    fn test_effective_api_key_precedence() {
        let mut config = ProdigyConfig::default();

        // No API key set
        assert!(config.effective_api_key().is_none());

        // Global API key only
        config.claude_api_key = Some("global-key".to_string());
        assert_eq!(config.effective_api_key(), Some("global-key"));

        // Project API key takes precedence
        config.project = Some(ProjectSettings {
            claude_api_key: Some("project-key".to_string()),
            ..Default::default()
        });
        assert_eq!(config.effective_api_key(), Some("project-key"));
    }

    #[test]
    fn test_effective_auto_commit_precedence() {
        let mut config = ProdigyConfig::default();

        // Default value
        assert!(config.effective_auto_commit());

        // Global setting
        config.auto_commit = false;
        assert!(!config.effective_auto_commit());

        // Project setting takes precedence
        config.project = Some(ProjectSettings {
            auto_commit: Some(true),
            ..Default::default()
        });
        assert!(config.effective_auto_commit());
    }

    #[test]
    fn test_effective_methods() {
        let config = ProdigyConfig {
            log_level: "debug".to_string(),
            max_concurrent_specs: 16,
            default_editor: Some("vim".to_string()),
            ..Default::default()
        };

        assert_eq!(config.effective_log_level(), "debug");
        assert_eq!(config.effective_max_concurrent(), 16);
        assert_eq!(config.effective_editor(), Some("vim"));
    }

    #[test]
    fn test_plugin_config_default() {
        let plugins = PluginConfig::default();

        assert!(!plugins.enabled);
        assert!(plugins.directory.is_none());
        assert!(plugins.auto_load.is_empty());
    }

    #[test]
    fn test_plugin_config_deserialization() {
        let yaml = r#"
plugins:
  enabled: true
  directory: /path/to/plugins
  auto_load:
    - plugin1
    - plugin2
"#;

        let config: ProdigyConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(config.plugins.enabled);
        assert_eq!(
            config.plugins.directory,
            Some(PathBuf::from("/path/to/plugins"))
        );
        assert_eq!(config.plugins.auto_load, vec!["plugin1", "plugin2"]);
    }
}
