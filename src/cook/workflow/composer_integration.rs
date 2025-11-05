//! Integration layer between workflow composer and executor
//!
//! This module bridges the gap between the workflow composition system
//! and the workflow execution runtime, enabling template-based workflows
//! to be composed and executed seamlessly.

use crate::config::WorkflowConfig;
use crate::cook::workflow::composition::{
    ComposableWorkflow, ComposedWorkflow, TemplateRegistry, WorkflowComposer,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Errors that can occur during workflow composition
#[derive(Debug, thiserror::Error)]
pub enum CompositionError {
    #[error("Template '{0}' not found in registry or file system")]
    TemplateNotFound(String),

    #[error("Required parameter '{name}' not provided")]
    MissingParameter { name: String },

    #[error("Parameter '{name}' has invalid value: {reason}")]
    InvalidParameter { name: String, reason: String },

    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Failed to load workflow from {path}: {source}")]
    WorkflowLoadError {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    #[error("Parameter substitution failed in command '{command}': {reason}")]
    SubstitutionError { command: String, reason: String },
}

/// Detects if a workflow file uses composition features
pub fn is_composable_workflow(content: &str) -> bool {
    content.contains("template:")
        || content.contains("imports:")
        || content.contains("extends:")
        || content.contains("workflows:")
        || content.contains("parameters:")
}

/// Parse and compose a composable workflow file
pub async fn parse_composable_workflow(
    path: &Path,
    content: &str,
) -> Result<(
    WorkflowConfig,
    Option<crate::config::MapReduceWorkflowConfig>,
)> {
    // Parse as ComposableWorkflow
    let composable: ComposableWorkflow = serde_yaml::from_str(content)
        .with_context(|| format!("Failed to parse composable workflow: {}", path.display()))?;

    // Extract parameters from workflow defaults
    let params = extract_workflow_parameters(&composable)?;

    // Initialize template registry
    let registry = Arc::new(create_template_registry()?);

    // Create composer
    let composer = WorkflowComposer::new(registry);

    // Compose the workflow
    let composed = composer
        .compose(path, params)
        .await
        .context("Failed to compose workflow")?;

    // Convert to WorkflowConfig
    let workflow_config = convert_composed_to_config(composed)?;

    Ok((workflow_config, None))
}

/// Create a template registry with standard search paths
fn create_template_registry() -> Result<TemplateRegistry> {
    // Look for templates in standard locations (following Prodigy global storage pattern)
    // Priority order:
    // 1. ~/.prodigy/templates/ (global, shared across repos)
    // 2. .prodigy/templates/ (project-local)
    // 3. templates/ (legacy, project-local)
    let template_dirs = vec![
        directories::ProjectDirs::from("com", "prodigy", "prodigy")
            .map(|dirs| dirs.data_dir().join("templates"))
            .unwrap_or_else(|| {
                // Fallback for when ProjectDirs fails
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".prodigy/templates")
            }),
        PathBuf::from(".prodigy/templates"),
        PathBuf::from("templates"),
    ];

    // Use first existing directory, or create global default
    let template_dir = template_dirs
        .into_iter()
        .find(|dir| dir.exists())
        .unwrap_or_else(|| {
            // Default to global storage location
            directories::ProjectDirs::from("com", "prodigy", "prodigy")
                .map(|dirs| dirs.data_dir().join("templates"))
                .unwrap_or_else(|| {
                    let home = std::env::var("HOME")
                        .or_else(|_| std::env::var("USERPROFILE"))
                        .unwrap_or_else(|_| ".".to_string());
                    PathBuf::from(home).join(".prodigy/templates")
                })
        });

    // Use file-based template storage
    let storage = Box::new(
        crate::cook::workflow::composition::registry::FileTemplateStorage::new(template_dir),
    );
    let registry = TemplateRegistry::with_storage(storage);

    Ok(registry)
}

/// Extract workflow parameters from composable workflow
fn extract_workflow_parameters(composable: &ComposableWorkflow) -> Result<HashMap<String, Value>> {
    let mut params = HashMap::new();

    // Start with defaults from the workflow file
    if let Some(defaults) = &composable.defaults {
        for (key, value) in defaults {
            params.insert(key.clone(), value.clone());
        }
    }

    // Also extract parameters from template.with if present
    if let Some(template) = &composable.template {
        if let Some(template_params) = &template.with {
            for (key, value) in template_params {
                params.insert(key.clone(), value.clone());
            }
        }
    }

    // NOTE: CLI parameter overrides will be implemented in Phase 2
    // Phase 1 uses only defaults from workflow file

    // Validate required parameters
    composable
        .validate_parameters(&params)
        .context("Parameter validation failed")?;

    Ok(params)
}

/// Convert composed workflow to executable WorkflowConfig
fn convert_composed_to_config(composed: ComposedWorkflow) -> Result<WorkflowConfig> {
    let workflow = composed.workflow;

    Ok(WorkflowConfig {
        name: workflow.config.name,
        commands: workflow.config.commands,
        env: workflow.config.env,
        secrets: workflow.config.secrets,
        env_files: workflow.config.env_files,
        profiles: workflow.config.profiles,
        merge: workflow.config.merge,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_composable_workflow() {
        assert!(is_composable_workflow("template:\n  name: foo"));
        assert!(is_composable_workflow("imports:\n  - path: bar.yml"));
        assert!(is_composable_workflow("extends: base"));
        assert!(is_composable_workflow("workflows:\n  test: {}"));
        assert!(is_composable_workflow("parameters:\n  required: []"));
        assert!(!is_composable_workflow("commands:\n  - shell: test"));
    }

    #[test]
    fn test_extract_workflow_parameters() {
        let mut composable = ComposableWorkflow::from_config(WorkflowConfig {
            name: None,
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        });

        let mut defaults = HashMap::new();
        defaults.insert("target".to_string(), Value::String("app.js".to_string()));
        defaults.insert("style".to_string(), Value::String("functional".to_string()));
        composable.defaults = Some(defaults);

        let result = extract_workflow_parameters(&composable);
        assert!(result.is_ok());

        let params = result.unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(
            params.get("target"),
            Some(&Value::String("app.js".to_string()))
        );
        assert_eq!(
            params.get("style"),
            Some(&Value::String("functional".to_string()))
        );
    }
}
