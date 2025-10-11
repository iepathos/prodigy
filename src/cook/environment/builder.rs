//! Builder for creating immutable environment contexts
//!
//! Provides a fluent API for constructing `ImmutableEnvironmentContext` instances
//! with all necessary configuration.
//!
//! # Example
//!
//! ```
//! use std::path::PathBuf;
//! use std::collections::HashMap;
//! use prodigy::cook::environment::EnvironmentContextBuilder;
//!
//! let mut vars = HashMap::new();
//! vars.insert("KEY1".to_string(), "value1".to_string());
//! vars.insert("KEY2".to_string(), "value2".to_string());
//!
//! let context = EnvironmentContextBuilder::new(PathBuf::from("/worktree"))
//!     .with_env("CUSTOM".to_string(), "value".to_string())
//!     .with_env_vars(vars)
//!     .with_secret("API_KEY".to_string())
//!     .with_profile("production".to_string())
//!     .build();
//! ```

use super::config::{EnvValue, EnvironmentConfig};
use super::context::ImmutableEnvironmentContext;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

/// Builder for creating immutable EnvironmentContext
///
/// This builder provides a fluent API for constructing environment contexts
/// with incremental configuration. All methods consume and return `self`,
/// allowing for method chaining.
///
/// # Example
///
/// ```
/// use std::path::PathBuf;
/// use prodigy::cook::environment::EnvironmentContextBuilder;
///
/// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
///     .with_env("VAR".to_string(), "value".to_string())
///     .with_secret("SECRET_KEY".to_string())
///     .build();
/// ```
pub struct EnvironmentContextBuilder {
    base_working_dir: PathBuf,
    env_vars: HashMap<String, String>,
    secret_keys: Vec<String>,
    profile: Option<String>,
}

impl EnvironmentContextBuilder {
    /// Create new builder with base working directory
    ///
    /// The builder starts with inherited environment variables from the current process.
    ///
    /// # Arguments
    ///
    /// * `base_working_dir` - The base working directory for command execution
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::EnvironmentContextBuilder;
    ///
    /// let builder = EnvironmentContextBuilder::new(PathBuf::from("/project"));
    /// ```
    pub fn new(base_working_dir: PathBuf) -> Self {
        Self {
            base_working_dir,
            env_vars: std::env::vars().collect(), // Inherit current env
            secret_keys: Vec::new(),
            profile: None,
        }
    }

    /// Add a single environment variable
    ///
    /// # Arguments
    ///
    /// * `key` - Variable name
    /// * `value` - Variable value
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::EnvironmentContextBuilder;
    ///
    /// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
    ///     .with_env("DATABASE_URL".to_string(), "postgres://localhost/db".to_string())
    ///     .build();
    /// ```
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_vars.insert(key, value);
        self
    }

    /// Add multiple environment variables
    ///
    /// # Arguments
    ///
    /// * `vars` - HashMap of variable names to values
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use std::collections::HashMap;
    /// use prodigy::cook::environment::EnvironmentContextBuilder;
    ///
    /// let mut vars = HashMap::new();
    /// vars.insert("A".to_string(), "1".to_string());
    /// vars.insert("B".to_string(), "2".to_string());
    ///
    /// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
    ///     .with_env_vars(vars)
    ///     .build();
    /// ```
    pub fn with_env_vars(mut self, vars: HashMap<String, String>) -> Self {
        self.env_vars.extend(vars);
        self
    }

    /// Mark a key as secret (for masking in logs)
    ///
    /// # Arguments
    ///
    /// * `key` - Environment variable key to mark as secret
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::EnvironmentContextBuilder;
    ///
    /// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
    ///     .with_env("API_KEY".to_string(), "secret123".to_string())
    ///     .with_secret("API_KEY".to_string())
    ///     .build();
    ///
    /// assert!(context.is_secret("API_KEY"));
    /// ```
    pub fn with_secret(mut self, key: String) -> Self {
        if !self.secret_keys.contains(&key) {
            self.secret_keys.push(key);
        }
        self
    }

    /// Add multiple secret keys
    ///
    /// # Arguments
    ///
    /// * `keys` - Vec of environment variable keys to mark as secrets
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::EnvironmentContextBuilder;
    ///
    /// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
    ///     .with_secrets(vec!["API_KEY".to_string(), "TOKEN".to_string()])
    ///     .build();
    /// ```
    pub fn with_secrets(mut self, keys: Vec<String>) -> Self {
        for key in keys {
            if !self.secret_keys.contains(&key) {
                self.secret_keys.push(key);
            }
        }
        self
    }

    /// Set active profile
    ///
    /// # Arguments
    ///
    /// * `profile` - Profile name (e.g., "production", "staging", "development")
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::EnvironmentContextBuilder;
    ///
    /// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
    ///     .with_profile("production".to_string())
    ///     .build();
    ///
    /// assert_eq!(context.profile(), Some("production"));
    /// ```
    pub fn with_profile(mut self, profile: String) -> Self {
        self.profile = Some(profile);
        self
    }

    /// Apply global environment configuration
    ///
    /// Loads environment variables, profiles, and secrets from the global config.
    ///
    /// # Arguments
    ///
    /// * `config` - Global environment configuration
    ///
    /// # Returns
    ///
    /// Result containing the updated builder or an error
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::{EnvironmentConfig, EnvironmentContextBuilder};
    ///
    /// let config = EnvironmentConfig::default();
    /// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
    ///     .with_config(&config)
    ///     .unwrap()
    ///     .build();
    /// ```
    pub fn with_config(mut self, config: &EnvironmentConfig) -> Result<Self> {
        // Apply profile if specified
        if let Some(profile_name) = &config.active_profile {
            self = self.with_profile(profile_name.clone());

            if let Some(profile) = config.profiles.get(profile_name) {
                self = self.apply_profile_vars(&profile.env)?;
            }
        }

        // Apply global env vars from config
        for (key, value) in &config.global_env {
            // Resolve EnvValue to String (static values only for now)
            // Dynamic and conditional values would require async/context
            if let EnvValue::Static(s) = value {
                self = self.with_env(key.clone(), s.clone());
            }
            // Note: Dynamic and Conditional env values are not yet supported here
            // This would require passing workflow context and making this async
        }

        // Mark secrets
        for key in config.secrets.keys() {
            self = self.with_secret(key.clone());
        }

        Ok(self)
    }

    /// Apply environment profile variables
    ///
    /// Internal helper to apply variables from a profile.
    fn apply_profile_vars(mut self, vars: &HashMap<String, String>) -> Result<Self> {
        for (key, value) in vars {
            self = self.with_env(key.clone(), value.clone());
        }
        Ok(self)
    }

    /// Build immutable EnvironmentContext
    ///
    /// Consumes the builder and produces an immutable context.
    ///
    /// # Returns
    ///
    /// Immutable environment context ready for use
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::EnvironmentContextBuilder;
    ///
    /// let context = EnvironmentContextBuilder::new(PathBuf::from("/project"))
    ///     .with_env("VAR".to_string(), "value".to_string())
    ///     .build();
    ///
    /// assert_eq!(context.working_dir(), PathBuf::from("/project").as_path());
    /// ```
    pub fn build(self) -> ImmutableEnvironmentContext {
        use std::sync::Arc;

        ImmutableEnvironmentContext {
            base_working_dir: Arc::new(self.base_working_dir),
            env_vars: Arc::new(self.env_vars),
            secret_keys: Arc::new(self.secret_keys),
            profile: self.profile.map(Arc::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_env("KEY".to_string(), "value".to_string())
            .with_secret("SECRET".to_string())
            .build();

        assert_eq!(context.working_dir(), PathBuf::from("/test").as_path());
        assert_eq!(context.env_vars().get("KEY"), Some(&"value".to_string()));
        assert!(context.is_secret("SECRET"));
    }

    #[test]
    fn test_builder_multiple_env_vars() {
        let mut vars = HashMap::new();
        vars.insert("A".to_string(), "1".to_string());
        vars.insert("B".to_string(), "2".to_string());

        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_env_vars(vars)
            .build();

        assert_eq!(context.env_vars().get("A"), Some(&"1".to_string()));
        assert_eq!(context.env_vars().get("B"), Some(&"2".to_string()));
    }

    #[test]
    fn test_builder_with_profile() {
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_profile("production".to_string())
            .build();

        assert_eq!(context.profile(), Some("production"));
    }

    #[test]
    fn test_builder_with_secrets() {
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_secrets(vec!["SECRET1".to_string(), "SECRET2".to_string()])
            .build();

        assert!(context.is_secret("SECRET1"));
        assert!(context.is_secret("SECRET2"));
        assert!(!context.is_secret("NOT_SECRET"));
    }

    #[test]
    fn test_builder_with_config_empty() {
        let config = EnvironmentConfig::default();
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_config(&config)
            .unwrap()
            .build();

        assert_eq!(context.working_dir(), PathBuf::from("/test").as_path());
    }

    #[test]
    fn test_builder_with_config_global_env() {
        let mut config = EnvironmentConfig::default();
        config.global_env.insert(
            "GLOBAL_VAR".to_string(),
            EnvValue::Static("value".to_string()),
        );

        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_config(&config)
            .unwrap()
            .build();

        assert_eq!(
            context.env_vars().get("GLOBAL_VAR"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_builder_inherits_system_env() {
        // Builder should inherit current process env by default
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test")).build();

        // System environment should be present (PATH is almost always set)
        // We can't test for specific vars, but we can check it's not empty
        assert!(!context.env_vars().is_empty());
    }

    #[test]
    fn test_builder_chain() {
        // Test fluent API chaining
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_env("VAR1".to_string(), "value1".to_string())
            .with_env("VAR2".to_string(), "value2".to_string())
            .with_secret("VAR1".to_string())
            .with_profile("prod".to_string())
            .build();

        assert_eq!(context.env_vars().get("VAR1"), Some(&"value1".to_string()));
        assert_eq!(context.env_vars().get("VAR2"), Some(&"value2".to_string()));
        assert!(context.is_secret("VAR1"));
        assert!(!context.is_secret("VAR2"));
        assert_eq!(context.profile(), Some("prod"));
    }

    #[test]
    fn test_builder_duplicate_secrets() {
        // Adding the same secret multiple times should work
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_secret("SECRET".to_string())
            .with_secret("SECRET".to_string()) // Duplicate
            .build();

        assert!(context.is_secret("SECRET"));
        // Should only appear once in the list
        assert_eq!(
            context
                .secret_keys()
                .iter()
                .filter(|k| *k == "SECRET")
                .count(),
            1
        );
    }
}
