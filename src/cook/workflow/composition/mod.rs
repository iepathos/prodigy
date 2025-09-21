//! Workflow composition and reusability module
//!
//! Enables building complex workflows from reusable components,
//! supporting workflow imports, templates, and parameterization.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod composer;
pub mod registry;
pub mod sub_workflow;

pub use composer::WorkflowComposer;
pub use registry::{TemplateRegistry, TemplateStorage};
pub use sub_workflow::{SubWorkflow, SubWorkflowExecutor, SubWorkflowResult};

/// A composable workflow with support for imports, templates, and parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposableWorkflow {
    /// Base workflow configuration
    #[serde(flatten)]
    pub config: crate::config::WorkflowConfig,

    /// Import other workflow files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<Vec<WorkflowImport>>,

    /// Extend from base workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,

    /// Use workflow template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<WorkflowTemplate>,

    /// Define parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterDefinitions>,

    /// Default values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<HashMap<String, Value>>,

    /// Sub-workflows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflows: Option<HashMap<String, SubWorkflow>>,
}

/// Import configuration for external workflow files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowImport {
    /// Path to the workflow file to import
    pub path: PathBuf,

    /// Optional alias for the import
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// Selective import of specific workflows
    #[serde(default)]
    pub selective: Vec<String>,
}

/// Template configuration for workflow reuse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    /// Name of the template
    pub name: String,

    /// Source of the template
    pub source: TemplateSource,

    /// Parameters to pass to the template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with: Option<HashMap<String, Value>>,

    /// Override specific template values
    #[serde(skip_serializing_if = "Option::is_none", rename = "override")]
    pub override_field: Option<HashMap<String, Value>>,
}

/// Source location for templates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TemplateSource {
    /// Local file path
    File(PathBuf),
    /// Registry name
    Registry(String),
    /// Remote URL
    Url(String),
}

/// Parameter definitions for workflow composition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinitions {
    /// Required parameters
    #[serde(default)]
    pub required: Vec<Parameter>,

    /// Optional parameters
    #[serde(default)]
    pub optional: Vec<Parameter>,
}

/// Individual parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name
    pub name: String,

    /// Type hint for the parameter
    #[serde(rename = "type")]
    pub type_hint: ParameterType,

    /// Description of the parameter
    pub description: String,

    /// Default value for optional parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,

    /// Validation expression or pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
}

/// Type hints for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    Any,
}

/// Result of workflow composition
#[derive(Debug, Clone)]
pub struct ComposedWorkflow {
    /// The composed workflow
    pub workflow: ComposableWorkflow,

    /// Metadata about the composition
    pub metadata: CompositionMetadata,
}

/// Metadata about workflow composition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionMetadata {
    /// Source files involved in composition
    pub sources: Vec<PathBuf>,

    /// Templates used
    pub templates: Vec<String>,

    /// Parameters applied
    pub parameters: HashMap<String, Value>,

    /// Composition timestamp
    pub composed_at: chrono::DateTime<chrono::Utc>,

    /// Dependency graph
    pub dependencies: Vec<DependencyInfo>,
}

/// Information about workflow dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    /// Source of the dependency
    pub source: PathBuf,

    /// Type of dependency
    pub dep_type: DependencyType,

    /// Resolved path or name
    pub resolved: String,
}

/// Type of workflow dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Import,
    Extends,
    Template,
    SubWorkflow,
}

impl ComposableWorkflow {
    /// Create a new composable workflow from a base configuration
    pub fn from_config(config: crate::config::WorkflowConfig) -> Self {
        Self {
            config,
            imports: None,
            extends: None,
            template: None,
            parameters: None,
            defaults: None,
            workflows: None,
        }
    }

    /// Check if the workflow uses composition features
    pub fn uses_composition(&self) -> bool {
        self.imports.is_some()
            || self.extends.is_some()
            || self.template.is_some()
            || self.workflows.is_some()
    }

    /// Get all required parameters
    pub fn required_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .as_ref()
            .map(|p| p.required.iter().collect())
            .unwrap_or_default()
    }

    /// Validate provided parameters against definitions
    pub fn validate_parameters(&self, provided: &HashMap<String, Value>) -> Result<()> {
        if let Some(params) = &self.parameters {
            // Check all required parameters are provided
            for param in &params.required {
                if !provided.contains_key(&param.name) && param.default.is_none() {
                    anyhow::bail!("Required parameter '{}' not provided", param.name);
                }
            }

            // Validate parameter types and constraints
            for (name, value) in provided {
                if let Some(param) = params
                    .required
                    .iter()
                    .chain(params.optional.iter())
                    .find(|p| p.name == *name)
                {
                    self.validate_parameter_value(param, value)
                        .with_context(|| format!("Invalid value for parameter '{}'", name))?;
                }
            }
        }

        Ok(())
    }

    fn validate_parameter_value(&self, param: &Parameter, value: &Value) -> Result<()> {
        // Type validation
        match (&param.type_hint, value) {
            (ParameterType::String, Value::String(_)) => {}
            (ParameterType::Number, Value::Number(_)) => {}
            (ParameterType::Boolean, Value::Bool(_)) => {}
            (ParameterType::Array, Value::Array(_)) => {}
            (ParameterType::Object, Value::Object(_)) => {}
            (ParameterType::Any, _) => {}
            _ => anyhow::bail!(
                "Type mismatch: expected {:?}, got {}",
                param.type_hint,
                value
            ),
        }

        // Custom validation
        if let Some(validation) = &param.validation {
            // TODO: Implement custom validation expression evaluation
            tracing::debug!(
                "Custom validation for parameter '{}': {}",
                param.name,
                validation
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composable_workflow_creation() {
        let config = crate::config::WorkflowConfig {
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let workflow = ComposableWorkflow::from_config(config);
        assert!(!workflow.uses_composition());
    }

    #[test]
    fn test_parameter_validation() {
        let mut workflow = ComposableWorkflow::from_config(crate::config::WorkflowConfig {
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        });

        workflow.parameters = Some(ParameterDefinitions {
            required: vec![Parameter {
                name: "target".to_string(),
                type_hint: ParameterType::String,
                description: "Target file".to_string(),
                default: None,
                validation: None,
            }],
            optional: vec![],
        });

        let mut params = HashMap::new();
        params.insert("target".to_string(), Value::String("file.js".to_string()));

        assert!(workflow.validate_parameters(&params).is_ok());

        let empty_params = HashMap::new();
        assert!(workflow.validate_parameters(&empty_params).is_err());
    }
}
