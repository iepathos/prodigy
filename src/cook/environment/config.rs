//! Environment configuration structures
//!
//! Defines the configuration types for environment management including
//! static, dynamic, and conditional environment variables, secrets, and profiles.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Global environment configuration for workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// Global environment variables
    #[serde(default)]
    pub global_env: HashMap<String, EnvValue>,

    /// Secret environment variables (masked in logs)
    #[serde(default)]
    pub secrets: HashMap<String, SecretValue>,

    /// Environment files to load (.env format)
    #[serde(default)]
    pub env_files: Vec<PathBuf>,

    /// Inherit environment from parent process
    #[serde(default = "default_true")]
    pub inherit: bool,

    /// Environment profiles for different contexts
    #[serde(default)]
    pub profiles: HashMap<String, EnvProfile>,

    /// Active profile name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
}

/// Environment value that can be static, dynamic, or conditional
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    /// Static string value
    Static(String),
    /// Dynamically computed value from command
    Dynamic(DynamicEnv),
    /// Conditional value based on expression
    Conditional(ConditionalEnv),
}

impl EnvValue {
    /// Create a static environment value
    pub fn static_value(value: impl Into<String>) -> Self {
        EnvValue::Static(value.into())
    }

    /// Check if this is a static value
    pub fn is_static(&self) -> bool {
        matches!(self, EnvValue::Static(_))
    }
}

/// Dynamic environment value computed from command output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicEnv {
    /// Command to execute for value
    pub command: String,
    /// Whether to cache the result
    #[serde(default)]
    pub cache: bool,
}

/// Conditional environment value based on expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalEnv {
    /// Condition expression to evaluate
    pub condition: String,
    /// Value when condition is true
    pub when_true: String,
    /// Value when condition is false
    pub when_false: String,
}

/// Secret value with provider information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecretValue {
    /// Simple string reference (e.g., environment variable)
    Simple(String),
    /// Provider-based secret
    Provider {
        provider: SecretProvider,
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
}

/// Secret provider types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretProvider {
    /// Environment variable
    Env,
    /// File-based secret
    File,
    /// HashiCorp Vault
    Vault,
    /// AWS Secrets Manager
    Aws,
    /// Custom provider
    Custom(String),
}

/// Environment profile for different contexts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvProfile {
    /// Profile-specific environment variables
    #[serde(flatten)]
    pub env: HashMap<String, String>,

    /// Profile description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Step-specific environment configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepEnvironment {
    /// Step-specific environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory for this step
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<PathBuf>,

    /// Clear parent environment before applying step env
    #[serde(default)]
    pub clear_env: bool,

    /// Temporary environment (restored after step)
    #[serde(default)]
    pub temporary: bool,
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            global_env: HashMap::new(),
            secrets: HashMap::new(),
            env_files: Vec::new(),
            inherit: true,  // This is the key difference - should default to true
            profiles: HashMap::new(),
            active_profile: None,
        }
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_value_static() {
        let value = EnvValue::static_value("test");
        assert!(value.is_static());

        if let EnvValue::Static(s) = value {
            assert_eq!(s, "test");
        } else {
            panic!("Expected static value");
        }
    }

    #[test]
    fn test_env_value_dynamic() {
        let value = EnvValue::Dynamic(DynamicEnv {
            command: "echo hello".to_string(),
            cache: true,
        });
        assert!(!value.is_static());
    }

    #[test]
    fn test_env_value_conditional() {
        let value = EnvValue::Conditional(ConditionalEnv {
            condition: "branch == 'main'".to_string(),
            when_true: "production".to_string(),
            when_false: "staging".to_string(),
        });
        assert!(!value.is_static());
    }

    #[test]
    fn test_environment_config_default() {
        let config = EnvironmentConfig::default();
        assert!(config.inherit);
        assert!(config.global_env.is_empty());
        assert!(config.secrets.is_empty());
        assert!(config.env_files.is_empty());
        assert!(config.profiles.is_empty());
        assert!(config.active_profile.is_none());
    }

    #[test]
    fn test_step_environment_default() {
        let step_env = StepEnvironment::default();
        assert!(!step_env.clear_env);
        assert!(!step_env.temporary);
        assert!(step_env.env.is_empty());
        assert!(step_env.working_dir.is_none());
    }
}
