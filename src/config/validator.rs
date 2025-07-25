use super::{Config, GlobalConfig, ProjectConfig};
use crate::{Error, Result};
use std::path::Path;

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate_config(config: &Config) -> Result<()> {
        Self::validate_global(&config.global)?;

        if let Some(project) = &config.project {
            Self::validate_project(project)?;
        }

        Ok(())
    }

    pub fn validate_global(config: &GlobalConfig) -> Result<()> {
        if !config.mmm_home.exists() {
            return Err(Error::Config(format!(
                "MMM home directory does not exist: {}",
                config.mmm_home.display()
            )));
        }

        if let Some(log_level) = &config.log_level {
            let valid_levels = ["trace", "debug", "info", "warn", "error"];
            if !valid_levels.contains(&log_level.as_str()) {
                return Err(Error::Config(format!(
                    "Invalid log level: {}. Must be one of: {:?}",
                    log_level, valid_levels
                )));
            }
        }

        if let Some(max_concurrent) = config.max_concurrent_specs {
            if max_concurrent == 0 {
                return Err(Error::Config(
                    "max_concurrent_specs must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(plugins) = &config.plugins {
            if plugins.enabled && !plugins.directory.exists() {
                return Err(Error::Config(format!(
                    "Plugin directory does not exist: {}",
                    plugins.directory.display()
                )));
            }
        }

        Ok(())
    }

    pub fn validate_project(config: &ProjectConfig) -> Result<()> {
        if config.name.is_empty() {
            return Err(Error::Config("Project name cannot be empty".to_string()));
        }

        if config
            .name
            .contains(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        {
            return Err(Error::Config(
                "Project name can only contain alphanumeric characters, hyphens, and underscores"
                    .to_string(),
            ));
        }

        if let Some(max_iterations) = config.max_iterations {
            if max_iterations == 0 {
                return Err(Error::Config(
                    "max_iterations must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(spec_dir) = &config.spec_dir {
            if spec_dir.is_absolute() {
                return Err(Error::Config(
                    "spec_dir must be a relative path".to_string(),
                ));
            }
        }

        Ok(())
    }

    pub fn validate_api_key(api_key: &str) -> Result<()> {
        if api_key.is_empty() {
            return Err(Error::Config("Claude API key cannot be empty".to_string()));
        }

        if !api_key.starts_with("sk-") {
            return Err(Error::Config(
                "Claude API key must start with 'sk-'".to_string(),
            ));
        }

        if api_key.len() < 20 {
            return Err(Error::Config(
                "Claude API key appears to be too short".to_string(),
            ));
        }

        Ok(())
    }
}
