//! Immutable environment context for command execution
//!
//! This module provides an immutable environment context pattern that prevents
//! hidden state mutations and makes working directory resolution explicit.
//!
//! # Functional Programming Principles
//!
//! This implementation follows functional programming principles:
//! - **Immutability**: All fields are immutable after construction
//! - **Explicit State**: No hidden mutations, all state visible in function signatures
//! - **Pure Functions**: Working directory resolution is a pure function
//! - **Type Safety**: Use of Arc for efficient cloning of immutable data
//!
//! # Example
//!
//! ```
//! use std::path::PathBuf;
//! use prodigy::cook::environment::ImmutableEnvironmentContext;
//! use prodigy::cook::environment::EnvironmentContextBuilder;
//!
//! // Create immutable context with builder pattern
//! let context = EnvironmentContextBuilder::new(PathBuf::from("/worktree"))
//!     .with_env("KEY".to_string(), "value".to_string())
//!     .with_secret("API_KEY".to_string())
//!     .build();
//!
//! // Context is immutable - no hidden state changes
//! assert_eq!(context.working_dir(), PathBuf::from("/worktree").as_path());
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Immutable environment context for command execution
///
/// This struct contains all environment configuration needed for
/// executing commands. It is immutable after construction, preventing
/// hidden state mutations that can cause bugs in complex workflows.
///
/// # Why Immutable?
///
/// Previous implementation used mutable `EnvironmentManager` with a `set_working_dir()`
/// method that caused bugs where commands executed in wrong directories. This immutable
/// design makes all state explicit and prevents hidden mutations.
///
/// # Fields
///
/// - `base_working_dir`: The primary working directory (e.g., worktree or repo path)
/// - `env_vars`: Immutable environment variables (shared via Arc for cheap cloning)
/// - `secret_keys`: Keys that should be masked in logs (shared via Arc)
/// - `profile`: Active environment profile name (if any)
#[derive(Debug, Clone)]
pub struct ImmutableEnvironmentContext {
    /// Base working directory (typically main repo or worktree)
    pub(crate) base_working_dir: Arc<PathBuf>,

    /// Environment variables (immutable after creation)
    pub(crate) env_vars: Arc<HashMap<String, String>>,

    /// Secret keys for masking in logs
    pub(crate) secret_keys: Arc<Vec<String>>,

    /// Active profile name (if any)
    pub(crate) profile: Option<Arc<str>>,
}

impl ImmutableEnvironmentContext {
    /// Create new environment context with base working directory
    ///
    /// This is the simplest way to create a context. For more complex scenarios,
    /// use `EnvironmentContextBuilder` instead.
    ///
    /// # Arguments
    ///
    /// * `base_working_dir` - The primary working directory for command execution
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use prodigy::cook::environment::ImmutableEnvironmentContext;
    ///
    /// let context = ImmutableEnvironmentContext::new(PathBuf::from("/project"));
    /// assert_eq!(context.working_dir(), PathBuf::from("/project").as_path());
    /// ```
    pub fn new(base_working_dir: PathBuf) -> Self {
        Self {
            base_working_dir: Arc::new(base_working_dir),
            env_vars: Arc::new(HashMap::new()),
            secret_keys: Arc::new(Vec::new()),
            profile: None,
        }
    }

    /// Get base working directory
    ///
    /// Returns an immutable reference to the base working directory.
    /// This directory is used as the default when steps don't specify
    /// an explicit working directory.
    pub fn working_dir(&self) -> &Path {
        &self.base_working_dir
    }

    /// Get environment variables (immutable reference)
    ///
    /// Returns all environment variables configured in this context.
    /// The returned HashMap is immutable and cannot be modified.
    pub fn env_vars(&self) -> &HashMap<String, String> {
        &self.env_vars
    }

    /// Check if a key is a secret (for masking in logs)
    ///
    /// # Arguments
    ///
    /// * `key` - The environment variable key to check
    ///
    /// # Returns
    ///
    /// `true` if the key should be masked in logs, `false` otherwise
    pub fn is_secret(&self, key: &str) -> bool {
        self.secret_keys.contains(&key.to_string())
    }

    /// Get active profile name
    ///
    /// Returns the active profile name if one is configured, `None` otherwise.
    pub fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    /// Get secret keys (for masking)
    ///
    /// Returns all keys that should be masked in log output.
    pub fn secret_keys(&self) -> &[String] {
        &self.secret_keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context() {
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/test"));
        assert_eq!(context.working_dir(), Path::new("/test"));
        assert!(context.env_vars().is_empty());
        assert!(context.secret_keys().is_empty());
        assert!(context.profile().is_none());
    }

    #[test]
    fn test_context_is_immutable() {
        let context = ImmutableEnvironmentContext::new(PathBuf::from("/test"));

        // These should compile (immutable access)
        let _dir = context.working_dir();
        let _env = context.env_vars();
        let _secrets = context.secret_keys();

        // Context cannot be mutated - no mutable methods available
        // This test verifies the immutability at compile time
    }

    #[test]
    fn test_context_clone_is_cheap() {
        let mut vars = HashMap::new();
        vars.insert("KEY".to_string(), "value".to_string());

        let context = ImmutableEnvironmentContext {
            base_working_dir: Arc::new(PathBuf::from("/test")),
            env_vars: Arc::new(vars),
            secret_keys: Arc::new(vec!["SECRET".to_string()]),
            profile: Some(Arc::from("prod")),
        };

        // Cloning should be cheap (Arc clones are cheap)
        let cloned = context.clone();

        // Arc pointers should be the same (not deep copied)
        assert!(Arc::ptr_eq(
            &context.base_working_dir,
            &cloned.base_working_dir
        ));
        assert!(Arc::ptr_eq(&context.env_vars, &cloned.env_vars));
        assert!(Arc::ptr_eq(&context.secret_keys, &cloned.secret_keys));
    }

    #[test]
    fn test_is_secret() {
        let context = ImmutableEnvironmentContext {
            base_working_dir: Arc::new(PathBuf::from("/test")),
            env_vars: Arc::new(HashMap::new()),
            secret_keys: Arc::new(vec!["API_KEY".to_string(), "TOKEN".to_string()]),
            profile: None,
        };

        assert!(context.is_secret("API_KEY"));
        assert!(context.is_secret("TOKEN"));
        assert!(!context.is_secret("NORMAL_VAR"));
    }

    #[test]
    fn test_profile() {
        let context = ImmutableEnvironmentContext {
            base_working_dir: Arc::new(PathBuf::from("/test")),
            env_vars: Arc::new(HashMap::new()),
            secret_keys: Arc::new(Vec::new()),
            profile: Some(Arc::from("production")),
        };

        assert_eq!(context.profile(), Some("production"));
    }
}
