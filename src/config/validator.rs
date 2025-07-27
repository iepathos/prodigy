use super::{Config, GlobalConfig, ProjectConfig, WorkflowConfig};
use anyhow::{anyhow, Result};

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(config: &Config) -> Result<()> {
        Self::validate_config(config)
    }

    pub fn validate_config(config: &Config) -> Result<()> {
        Self::validate_global(&config.global)?;

        if let Some(project) = &config.project {
            Self::validate_project(project)?;
        }

        if let Some(workflow) = &config.workflow {
            Self::validate_workflow(workflow)?;
        }

        Ok(())
    }

    pub fn validate_global(config: &GlobalConfig) -> Result<()> {
        if !config.mmm_home.exists() {
            return Err(anyhow!(
                "MMM home directory does not exist: {}",
                config.mmm_home.display()
            ));
        }

        if let Some(log_level) = &config.log_level {
            let valid_levels = ["trace", "debug", "info", "warn", "error"];
            if !valid_levels.contains(&log_level.as_str()) {
                return Err(anyhow!(
                    "Invalid log level: {log_level}. Must be one of: {valid_levels:?}"
                ));
            }
        }

        if let Some(max_concurrent) = config.max_concurrent_specs {
            if max_concurrent == 0 {
                return Err(anyhow!("max_concurrent_specs must be greater than 0"));
            }
        }

        if let Some(plugins) = &config.plugins {
            if plugins.enabled && !plugins.directory.exists() {
                return Err(anyhow!(
                    "Plugin directory does not exist: {}",
                    plugins.directory.display()
                ));
            }
        }

        Ok(())
    }

    pub fn validate_project(config: &ProjectConfig) -> Result<()> {
        if config.name.is_empty() {
            return Err(anyhow!("Project name cannot be empty"));
        }

        if config
            .name
            .contains(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        {
            return Err(anyhow!(
                "Project name can only contain alphanumeric characters, hyphens, and underscores"
            ));
        }

        if let Some(max_iterations) = config.max_iterations {
            if max_iterations == 0 {
                return Err(anyhow!("max_iterations must be greater than 0"));
            }
        }

        if let Some(spec_dir) = &config.spec_dir {
            if spec_dir.is_absolute() {
                return Err(anyhow!("spec_dir must be a relative path"));
            }
        }

        Ok(())
    }

    pub fn validate_api_key(api_key: &str) -> Result<()> {
        if api_key.is_empty() {
            return Err(anyhow!("Claude API key cannot be empty"));
        }

        if !api_key.starts_with("sk-") {
            return Err(anyhow!("Claude API key must start with 'sk-'"));
        }

        if api_key.len() < 20 {
            return Err(anyhow!("Claude API key appears to be too short"));
        }

        Ok(())
    }

    pub fn validate_workflow(workflow: &WorkflowConfig) -> Result<()> {
        if workflow.commands.is_empty() {
            return Err(anyhow!("Workflow must have at least one command"));
        }

        // Commands are now WorkflowCommand enums, which are already validated
        // during parsing and conversion to Command objects

        Ok(())
    }
}
