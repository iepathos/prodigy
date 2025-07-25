//! Model selection based on task type

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub display_name: String,
    pub context_window: usize,
    pub max_output: usize,
    pub cost_per_1k_input: f64,
    pub cost_per_1k_output: f64,
    pub capabilities: Vec<String>,
}

/// Task-specific model selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskModelConfig {
    pub task: String,
    pub model: String,
    pub reason: String,
}

/// Model selector configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSelectorConfig {
    pub default: String,
    pub models: HashMap<String, ModelConfig>,
    pub tasks: HashMap<String, TaskModelConfig>,
}

impl Default for ModelSelectorConfig {
    fn default() -> Self {
        let mut models = HashMap::new();
        let mut tasks = HashMap::new();

        // Define available models
        models.insert(
            "claude-3-opus".to_string(),
            ModelConfig {
                name: "claude-3-opus-20240229".to_string(),
                display_name: "Claude 3 Opus".to_string(),
                context_window: 200000,
                max_output: 4096,
                cost_per_1k_input: 0.015,
                cost_per_1k_output: 0.075,
                capabilities: vec![
                    "complex-reasoning".to_string(),
                    "code-generation".to_string(),
                    "debugging".to_string(),
                    "planning".to_string(),
                ],
            },
        );

        models.insert(
            "claude-3-sonnet".to_string(),
            ModelConfig {
                name: "claude-3-sonnet-20240229".to_string(),
                display_name: "Claude 3 Sonnet".to_string(),
                context_window: 200000,
                max_output: 4096,
                cost_per_1k_input: 0.003,
                cost_per_1k_output: 0.015,
                capabilities: vec![
                    "code-generation".to_string(),
                    "general-tasks".to_string(),
                    "review".to_string(),
                ],
            },
        );

        models.insert(
            "claude-3-haiku".to_string(),
            ModelConfig {
                name: "claude-3-haiku-20240307".to_string(),
                display_name: "Claude 3 Haiku".to_string(),
                context_window: 200000,
                max_output: 4096,
                cost_per_1k_input: 0.00025,
                cost_per_1k_output: 0.00125,
                capabilities: vec![
                    "simple-tasks".to_string(),
                    "quick-responses".to_string(),
                    "formatting".to_string(),
                ],
            },
        );

        // Define task mappings
        tasks.insert(
            "planning".to_string(),
            TaskModelConfig {
                task: "planning".to_string(),
                model: "claude-3-opus".to_string(),
                reason: "Complex reasoning required for architectural planning".to_string(),
            },
        );

        tasks.insert(
            "implementation".to_string(),
            TaskModelConfig {
                task: "implementation".to_string(),
                model: "claude-3-sonnet".to_string(),
                reason: "Good balance of capability and speed for code generation".to_string(),
            },
        );

        tasks.insert(
            "review".to_string(),
            TaskModelConfig {
                task: "review".to_string(),
                model: "claude-3-haiku".to_string(),
                reason: "Fast iteration for simple code reviews".to_string(),
            },
        );

        tasks.insert(
            "debug".to_string(),
            TaskModelConfig {
                task: "debug".to_string(),
                model: "claude-3-opus".to_string(),
                reason: "Deep analysis needed for complex debugging".to_string(),
            },
        );

        tasks.insert(
            "explanation".to_string(),
            TaskModelConfig {
                task: "explanation".to_string(),
                model: "claude-3-sonnet".to_string(),
                reason: "Clear explanations with reasonable speed".to_string(),
            },
        );

        Self {
            default: "claude-3-sonnet".to_string(),
            models,
            tasks,
        }
    }
}

/// Model selector for choosing appropriate models
pub struct ModelSelector {
    config: ModelSelectorConfig,
}

impl ModelSelector {
    /// Create a new model selector
    pub fn new(default_model: String) -> Self {
        let mut config = ModelSelectorConfig::default();
        config.default = default_model;

        // Try to load custom configuration
        if let Ok(custom_config) = Self::load_config() {
            config = custom_config;
        }

        Self { config }
    }

    /// Select model for a specific task
    pub fn select_for_task(&self, task_type: &str) -> Result<String> {
        // Check task-specific configuration
        if let Some(task_config) = self.config.tasks.get(task_type) {
            // Verify model exists
            if self.config.models.contains_key(&task_config.model) {
                return Ok(self.config.models[&task_config.model].name.clone());
            }
        }

        // Fall back to default
        self.get_default_model()
    }

    /// Get the default model
    pub fn get_default_model(&self) -> Result<String> {
        self.config
            .models
            .get(&self.config.default)
            .map(|m| m.name.clone())
            .ok_or_else(|| {
                Error::Config(format!("Default model '{}' not found", self.config.default))
            })
    }

    /// Get model configuration
    pub fn get_model_config(&self, model_key: &str) -> Option<&ModelConfig> {
        self.config.models.get(model_key)
    }

    /// List available models
    pub fn list_models(&self) -> Vec<(&str, &ModelConfig)> {
        self.config
            .models
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }

    /// Estimate cost for a task
    pub fn estimate_cost(
        &self,
        model_key: &str,
        input_tokens: usize,
        output_tokens: usize,
    ) -> Result<f64> {
        let model = self
            .config
            .models
            .get(model_key)
            .ok_or_else(|| Error::NotFound(format!("Model '{model_key}' not found")))?;

        let input_cost = (input_tokens as f64 / 1000.0) * model.cost_per_1k_input;
        let output_cost = (output_tokens as f64 / 1000.0) * model.cost_per_1k_output;

        Ok(input_cost + output_cost)
    }

    /// Check if a model supports a capability
    pub fn supports_capability(&self, model_key: &str, capability: &str) -> bool {
        self.config
            .models
            .get(model_key)
            .map(|m| m.capabilities.iter().any(|c| c == capability))
            .unwrap_or(false)
    }

    /// Load custom configuration
    fn load_config() -> Result<ModelSelectorConfig> {
        let config_path = PathBuf::from(".mmm/models.yaml");
        if !config_path.exists() {
            return Err(Error::NotFound("Models config not found".to_string()));
        }

        let content = fs::read_to_string(&config_path).map_err(Error::Io)?;

        serde_yaml::from_str(&content)
            .map_err(|e| Error::Config(format!("Invalid models YAML: {e}")))
    }

    /// Save current configuration
    pub fn save_config(&self) -> Result<()> {
        let config_path = PathBuf::from(".mmm/models.yaml");

        // Create directory if needed
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        let yaml = serde_yaml::to_string(&self.config)
            .map_err(|e| Error::Parse(format!("Failed to serialize config: {e}")))?;

        fs::write(&config_path, yaml).map_err(Error::Io)?;

        Ok(())
    }
}
