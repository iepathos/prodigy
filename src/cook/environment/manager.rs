//! Environment manager for workflow execution
//!
//! Manages environment variables, working directories, and secrets for workflow steps.
//! Provides isolation, inheritance, and restoration of environment contexts.

use super::config::{
    ConditionalEnv, DynamicEnv, EnvValue, EnvironmentConfig, SecretProvider, SecretValue,
    StepEnvironment,
};
use super::path_resolver::PathResolver;
use super::secret_store::SecretStore;
use crate::cook::expression::{
    ExpressionEvaluator, Value, VariableContext as ExpressionVariableContext,
};
use crate::cook::workflow::WorkflowStep;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info};

/// Environment context for command execution
#[derive(Debug, Clone)]
pub struct EnvironmentContext {
    /// Environment variables for execution
    pub env: HashMap<String, String>,
    /// Working directory for execution
    pub working_dir: PathBuf,
    /// Keys of secret environment variables (for masking)
    pub secrets: Vec<String>,
}

/// Snapshot of environment state for restoration
#[derive(Debug, Clone)]
pub struct EnvironmentSnapshot {
    /// Saved environment variables
    pub env: HashMap<String, String>,
    /// Saved working directory
    pub working_dir: PathBuf,
}

/// Manages environment configuration for workflow execution
pub struct EnvironmentManager {
    base_env: HashMap<String, String>,
    secrets: SecretStore,
    #[allow(dead_code)]
    profiles: HashMap<String, HashMap<String, String>>,
    current_dir: PathBuf,
    env_stack: Vec<EnvironmentSnapshot>,
    path_resolver: PathResolver,
    cache: HashMap<String, String>,
}

impl EnvironmentManager {
    /// Create a new environment manager
    pub fn new(current_dir: PathBuf) -> Result<Self> {
        let base_env = std::env::vars().collect();
        Ok(Self {
            base_env,
            secrets: SecretStore::new(),
            profiles: HashMap::new(),
            current_dir,
            env_stack: Vec::new(),
            path_resolver: PathResolver::new(),
            cache: HashMap::new(),
        })
    }

    /// Set up environment for a workflow step
    pub async fn setup_step_environment(
        &mut self,
        step: &WorkflowStep,
        global_config: Option<&EnvironmentConfig>,
        variables: &HashMap<String, String>,
    ) -> Result<EnvironmentContext> {
        // Build step environment configuration
        let step_env = StepEnvironment {
            env: step.env.clone(),
            working_dir: step.working_dir.clone(),
            clear_env: false, // Could be extended to support from step config
            temporary: false, // Could be extended to support from step config
        };

        self.setup_environment(&step_env, global_config, variables)
            .await
    }

    /// Set up environment from configuration
    pub async fn setup_environment(
        &mut self,
        step_env: &StepEnvironment,
        global_config: Option<&EnvironmentConfig>,
        variables: &HashMap<String, String>,
    ) -> Result<EnvironmentContext> {
        // Start with base environment
        let mut env = if step_env.clear_env {
            HashMap::new()
        } else if global_config.is_none_or(|c| c.inherit) {
            self.get_inherited_env()?
        } else {
            HashMap::new()
        };

        // Apply global environment if provided
        if let Some(config) = global_config {
            // Load environment files
            for env_file in &config.env_files {
                self.load_env_file(&mut env, env_file)?;
            }

            // Apply global environment variables
            for (key, value) in &config.global_env {
                let resolved = self.resolve_env_value(value, variables).await?;
                env.insert(key.clone(), resolved);
            }

            // Apply active profile if specified
            if let Some(profile_name) = &config.active_profile {
                self.apply_profile(&mut env, profile_name, &config.profiles)?;
            }
        }

        // Apply step-specific environment
        for (key, value) in &step_env.env {
            let interpolated = self.interpolate_value(value, variables)?;
            env.insert(key.clone(), interpolated);
        }

        // Load secrets if configured
        let mut secret_keys = Vec::new();
        if let Some(config) = global_config {
            for (key, secret) in &config.secrets {
                let value = self.resolve_secret(secret).await?;
                env.insert(key.clone(), value);
                secret_keys.push(key.clone());
            }
        }

        // Resolve working directory
        let working_dir = if let Some(dir) = &step_env.working_dir {
            self.resolve_path(dir, variables)?
        } else {
            self.current_dir.clone()
        };

        // Save snapshot if temporary
        if step_env.temporary {
            self.push_snapshot();
        }

        Ok(EnvironmentContext {
            env,
            working_dir,
            secrets: secret_keys,
        })
    }

    /// Restore environment from snapshot
    pub fn restore_environment(&mut self) -> Result<()> {
        if let Some(snapshot) = self.env_stack.pop() {
            self.base_env = snapshot.env;
            self.current_dir = snapshot.working_dir;
            debug!("Restored environment from snapshot");
        }
        Ok(())
    }

    /// Push current environment to stack
    fn push_snapshot(&mut self) {
        self.env_stack.push(EnvironmentSnapshot {
            env: self.base_env.clone(),
            working_dir: self.current_dir.clone(),
        });
        debug!("Saved environment snapshot");
    }

    /// Get inherited environment from parent process
    fn get_inherited_env(&self) -> Result<HashMap<String, String>> {
        Ok(self.base_env.clone())
    }

    /// Load environment variables from file
    fn load_env_file(&self, env: &mut HashMap<String, String>, path: &Path) -> Result<()> {
        if !path.exists() {
            debug!("Environment file not found: {}", path.display());
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().to_string();

                // Remove quotes if present
                let value = if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value[1..value.len() - 1].to_string()
                } else {
                    value
                };

                env.insert(key, value);
            }
        }

        info!("Loaded environment from: {}", path.display());
        Ok(())
    }

    /// Apply environment profile
    fn apply_profile(
        &self,
        env: &mut HashMap<String, String>,
        profile_name: &str,
        profiles: &HashMap<String, super::config::EnvProfile>,
    ) -> Result<()> {
        let profile = profiles
            .get(profile_name)
            .ok_or_else(|| anyhow!("Profile not found: {}", profile_name))?;

        // Apply profile environment using functional extension
        env.extend(profile.env.iter().map(|(k, v)| (k.clone(), v.clone())));

        info!("Applied environment profile: {}", profile_name);
        Ok(())
    }

    /// Resolve environment value (static, dynamic, or conditional)
    async fn resolve_env_value(
        &mut self,
        value: &EnvValue,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        match value {
            EnvValue::Static(s) => Ok(self.interpolate_value(s, variables)?),
            EnvValue::Dynamic(d) => self.resolve_dynamic_env(d).await,
            EnvValue::Conditional(c) => self.resolve_conditional_env(c, variables),
        }
    }

    /// Resolve dynamic environment value from command
    async fn resolve_dynamic_env(&mut self, dynamic: &DynamicEnv) -> Result<String> {
        // Check cache if enabled
        if dynamic.cache {
            if let Some(cached) = self.cache.get(&dynamic.command) {
                debug!("Using cached value for dynamic env: {}", dynamic.command);
                return Ok(cached.clone());
            }
        }

        // Execute command
        let output = Command::new("sh")
            .arg("-c")
            .arg(&dynamic.command)
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "Dynamic environment command failed: {}",
                dynamic.command
            ));
        }

        let value = String::from_utf8(output.stdout)?.trim().to_string();

        // Cache if enabled
        if dynamic.cache {
            self.cache.insert(dynamic.command.clone(), value.clone());
        }

        debug!("Resolved dynamic env '{}' to '{}'", dynamic.command, value);
        Ok(value)
    }

    /// Resolve conditional environment value
    fn resolve_conditional_env(
        &self,
        conditional: &ConditionalEnv,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        // Create variable context using functional fold
        let var_context =
            variables
                .iter()
                .fold(ExpressionVariableContext::new(), |mut ctx, (key, value)| {
                    ctx.set(key.clone(), Value::String(value.clone()));
                    ctx
                });

        // Evaluate condition
        let evaluator = ExpressionEvaluator::new();
        let condition_met = evaluator.evaluate(&conditional.condition, &var_context)?;

        let value = if condition_met {
            &conditional.when_true
        } else {
            &conditional.when_false
        };

        debug!(
            "Resolved conditional env '{}' to '{}' (condition: {})",
            conditional.condition, value, condition_met
        );

        self.interpolate_value(value, variables)
    }

    /// Resolve secret value
    async fn resolve_secret(&mut self, secret: &SecretValue) -> Result<String> {
        match secret {
            SecretValue::Simple(key) => {
                // Try to get from environment
                std::env::var(key).map_err(|_| anyhow!("Secret not found in environment: {}", key))
            }
            SecretValue::Provider { provider, key, .. } => {
                match provider {
                    SecretProvider::Env => std::env::var(key)
                        .map_err(|_| anyhow!("Secret not found in environment: {}", key)),
                    SecretProvider::File => {
                        let path = PathBuf::from(key);
                        std::fs::read_to_string(path)
                            .map(|s| s.trim().to_string())
                            .map_err(|e| anyhow!("Failed to read secret file: {}", e))
                    }
                    _ => {
                        // Use secret store for other providers
                        self.secrets.get_secret(key).await
                    }
                }
            }
        }
    }

    /// Interpolate variables in a value
    fn interpolate_value(
        &self,
        value: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        // Chain variable interpolations using fold
        let with_vars = variables.iter().fold(value.to_string(), |acc, (key, val)| {
            acc.replace(&format!("${{{}}}", key), val)
                .replace(&format!("${}", key), val)
        });

        // Apply environment variable interpolation
        let result = self.base_env.iter().fold(with_vars, |acc, (key, val)| {
            acc.replace(&format!("${{env.{}}}", key), val)
        });

        Ok(result)
    }

    /// Resolve path with variable expansion
    fn resolve_path(&self, path: &Path, variables: &HashMap<String, String>) -> Result<PathBuf> {
        let path_str = path.to_string_lossy().to_string();
        let interpolated = self.interpolate_value(&path_str, variables)?;
        let resolved = self.path_resolver.resolve(&interpolated);

        if resolved.is_absolute() {
            Ok(resolved)
        } else {
            Ok(self.current_dir.join(resolved))
        }
    }

    /// Update current working directory
    pub fn set_working_dir(&mut self, dir: PathBuf) {
        self.current_dir = dir;
    }

    /// Get current working directory
    pub fn working_dir(&self) -> &Path {
        &self.current_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_environment_manager_new() {
        let manager = EnvironmentManager::new(PathBuf::from("/test")).unwrap();
        assert_eq!(manager.working_dir(), Path::new("/test"));
        assert!(!manager.base_env.is_empty());
    }

    #[tokio::test]
    async fn test_setup_environment_basic() {
        let mut manager = EnvironmentManager::new(PathBuf::from("/test")).unwrap();

        let mut step_env = StepEnvironment::default();
        step_env
            .env
            .insert("TEST_VAR".to_string(), "test_value".to_string());

        let variables = HashMap::new();
        let context = manager
            .setup_environment(&step_env, None, &variables)
            .await
            .unwrap();

        assert_eq!(context.env.get("TEST_VAR"), Some(&"test_value".to_string()));
        assert_eq!(context.working_dir, PathBuf::from("/test"));
        assert!(context.secrets.is_empty());
    }

    #[tokio::test]
    async fn test_interpolate_value() {
        let manager = EnvironmentManager::new(PathBuf::from("/test")).unwrap();

        let mut variables = HashMap::new();
        variables.insert("NAME".to_string(), "world".to_string());

        let result = manager
            .interpolate_value("Hello ${NAME}!", &variables)
            .unwrap();
        assert_eq!(result, "Hello world!");
    }

    #[tokio::test]
    async fn test_environment_snapshot_restore() {
        let mut manager = EnvironmentManager::new(PathBuf::from("/test")).unwrap();

        manager.push_snapshot();
        manager.set_working_dir(PathBuf::from("/other"));

        assert_eq!(manager.working_dir(), Path::new("/other"));

        manager.restore_environment().unwrap();
        assert_eq!(manager.working_dir(), Path::new("/test"));
    }
}
