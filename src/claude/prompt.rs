//! Prompt template system with variable interpolation

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tera::{Context, Tera};

/// Variable type for prompt templates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Code { language: String },
    File { path: PathBuf },
    Context { source: String },
}

/// Variable definition for prompt templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub description: String,
    #[serde(flatten)]
    pub var_type: VariableType,
    pub required: bool,
    pub default: Option<String>,
}

/// Prompt template configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub template: String,
    pub variables: Vec<Variable>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Prompt template engine
pub struct PromptEngine {
    templates: HashMap<String, PromptTemplate>,
    tera: Tera,
}

impl PromptEngine {
    /// Create a new prompt engine
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();
        tera.autoescape_on(vec![]); // Disable autoescaping for code

        Ok(Self {
            templates: HashMap::new(),
            tera,
        })
    }

    /// Load templates from a directory
    pub fn load_templates(&mut self, dir: &PathBuf) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                self.load_template_file(&path)?;
            }
        }

        Ok(())
    }

    /// Load a single template file
    fn load_template_file(&mut self, path: &PathBuf) -> Result<()> {
        let content = fs::read_to_string(path).map_err(Error::Io)?;

        let template: PromptTemplate = serde_yaml::from_str(&content)
            .map_err(|e| Error::Config(format!("Invalid template YAML: {e}")))?;

        // Add template to Tera
        self.tera
            .add_raw_template(&template.name, &template.template)
            .map_err(|e| Error::Config(format!("Invalid Tera template: {e}")))?;

        self.templates.insert(template.name.clone(), template);
        Ok(())
    }

    /// Render a template with variables
    pub fn render_template(&self, template_name: &str, args: Vec<String>) -> Result<String> {
        let template = self
            .templates
            .get(template_name)
            .ok_or_else(|| Error::NotFound(format!("Template '{template_name}' not found")))?;

        // Build context from arguments
        let mut context = Context::new();

        // Parse arguments and add to context
        for (i, arg) in args.iter().enumerate() {
            if let Some(var) = template.variables.get(i) {
                let value = self.process_variable(var, arg)?;
                context.insert(&var.name, &value);
            }
        }

        // Add defaults for missing optional variables
        for var in &template.variables {
            if !context.contains_key(&var.name) {
                if let Some(default) = &var.default {
                    context.insert(&var.name, default);
                } else if var.required {
                    return Err(Error::Validation(format!(
                        "Required variable '{}' not provided",
                        var.name
                    )));
                }
            }
        }

        // Render template
        self.tera
            .render(&template.name, &context)
            .map_err(|e| Error::Config(format!("Template rendering failed: {e}")))
    }

    /// Process a variable value based on its type
    fn process_variable(&self, var: &Variable, value: &str) -> Result<String> {
        match &var.var_type {
            VariableType::String => Ok(value.to_string()),
            VariableType::Number => value
                .parse::<f64>()
                .map(|_| value.to_string())
                .map_err(|_| {
                    Error::Validation(format!("Invalid number for variable '{}'", var.name))
                }),
            VariableType::Boolean => {
                value
                    .parse::<bool>()
                    .map(|_| value.to_string())
                    .map_err(|_| {
                        Error::Validation(format!("Invalid boolean for variable '{}'", var.name))
                    })
            }
            VariableType::Code { language: _ } => {
                // Could add syntax validation here
                Ok(value.to_string())
            }
            VariableType::File { path: _ } => {
                // Read file content
                let path = PathBuf::from(value);
                fs::read_to_string(&path).map_err(Error::Io)
            }
            VariableType::Context { source } => {
                // This would integrate with context management
                // For now, just return as-is
                Ok(format!("{{context:{source}}}"))
            }
        }
    }

    /// Get a template by name
    pub fn get_template(&self, name: &str) -> Option<&PromptTemplate> {
        self.templates.get(name)
    }

    /// List all available templates
    pub fn list_templates(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }
}
