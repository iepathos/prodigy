//! Environment management for workflow execution
//!
//! Provides comprehensive environment variable management and working directory control
//! at global, workflow, and step levels. Supports secret management, dynamic values,
//! conditional environments, and cross-platform path handling.

mod config;
mod manager;
mod path_resolver;
mod secret_store;

pub use config::{
    ConditionalEnv, DynamicEnv, EnvProfile, EnvValue, EnvironmentConfig, SecretProvider,
    SecretValue, StepEnvironment,
};
pub use manager::{EnvironmentContext, EnvironmentManager, EnvironmentSnapshot};
pub use path_resolver::{PathResolver, Platform};
pub use secret_store::{SecretStore, SecretStoreError};

use std::collections::HashMap;

/// Environment utilities for workflow execution
pub struct EnvironmentUtils;

impl EnvironmentUtils {
    /// Mask secrets in text output (pure function)
    pub fn mask_secrets(
        text: &str,
        secret_keys: &[String],
        env: &HashMap<String, String>,
    ) -> String {
        let mut masked = text.to_string();
        for key in secret_keys {
            if let Some(value) = env.get(key) {
                // Only mask if value is not empty and not just whitespace
                if !value.trim().is_empty() {
                    masked = masked.replace(value, "***MASKED***");
                }
            }
        }
        masked
    }

    /// Extract secret keys from environment configuration (pure function)
    pub fn extract_secret_keys(config: &EnvironmentConfig) -> Vec<String> {
        config.secrets.keys().cloned().collect()
    }

    /// Merge environment maps with precedence (pure function)
    /// Later values override earlier ones
    pub fn merge_env_maps(maps: Vec<HashMap<String, String>>) -> HashMap<String, String> {
        let mut result = HashMap::new();
        for map in maps {
            result.extend(map);
        }
        result
    }

    /// Check if a path needs expansion (pure function)
    pub fn needs_path_expansion(path: &str) -> bool {
        path.contains('~') || path.contains("${") || path.contains('$')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secrets() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret123".to_string());
        env.insert("NORMAL".to_string(), "visible".to_string());

        let secret_keys = vec!["API_KEY".to_string()];
        let text = "API key is secret123 and normal is visible";

        let masked = EnvironmentUtils::mask_secrets(text, &secret_keys, &env);
        assert_eq!(masked, "API key is ***MASKED*** and normal is visible");
    }

    #[test]
    fn test_merge_env_maps() {
        let mut map1 = HashMap::new();
        map1.insert("KEY1".to_string(), "value1".to_string());
        map1.insert("KEY2".to_string(), "original".to_string());

        let mut map2 = HashMap::new();
        map2.insert("KEY2".to_string(), "override".to_string());
        map2.insert("KEY3".to_string(), "value3".to_string());

        let merged = EnvironmentUtils::merge_env_maps(vec![map1, map2]);

        assert_eq!(merged.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(merged.get("KEY2"), Some(&"override".to_string()));
        assert_eq!(merged.get("KEY3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_needs_path_expansion() {
        assert!(EnvironmentUtils::needs_path_expansion("~/Documents"));
        assert!(EnvironmentUtils::needs_path_expansion("${HOME}/test"));
        assert!(EnvironmentUtils::needs_path_expansion("$USER/files"));
        assert!(!EnvironmentUtils::needs_path_expansion("/absolute/path"));
    }
}
