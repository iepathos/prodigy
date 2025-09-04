use super::types::{ExecutionInput, InputType, VariableDefinition};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub field: String,
    pub message: String,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

pub struct InputConfig {
    values: HashMap<String, Value>,
}

impl InputConfig {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn from_values(values: HashMap<String, Value>) -> Self {
        Self { values }
    }

    pub fn get_string(&self, key: &str) -> Result<String> {
        self.values
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid string value for key: {}", key))
    }

    pub fn get_array(&self, key: &str) -> Result<Vec<Value>> {
        self.values
            .get(key)
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid array value for key: {}", key))
    }

    pub fn get_bool(&self, key: &str) -> Result<bool> {
        self.values
            .get(key)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid boolean value for key: {}", key))
    }

    pub fn set(&mut self, key: String, value: Value) {
        self.values.insert(key, value);
    }
}

#[async_trait]
pub trait InputProvider: Send + Sync {
    /// Get the type of input this provider handles
    fn input_type(&self) -> InputType;

    /// Validate input configuration before processing
    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>>;

    /// Generate execution inputs from the configuration
    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>>;

    /// Get available variable names for this input type
    fn available_variables(&self, config: &InputConfig) -> Result<Vec<VariableDefinition>>;

    /// Check if this provider can handle the given configuration
    fn supports(&self, config: &InputConfig) -> bool;
}
