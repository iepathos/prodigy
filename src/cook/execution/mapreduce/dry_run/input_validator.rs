//! Input source validation for dry-run mode
//!
//! Validates input sources and JSONPath expressions without executing commands.

use super::types::{DryRunError, InputValidation, JsonPathValidation};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

/// Validator for input sources and JSONPath expressions
pub struct InputValidator;

impl InputValidator {
    /// Create a new input validator
    pub fn new() -> Self {
        Self
    }

    /// Validate an input source
    pub async fn validate_input_source(&self, input: &str) -> Result<InputValidation, DryRunError> {
        debug!("Validating input source: {}", input);

        if input.starts_with("shell:") {
            self.validate_command_input(input).await
        } else if Path::new(input).exists() {
            self.validate_file_input(input).await
        } else {
            Ok(InputValidation {
                source: input.to_string(),
                valid: false,
                size_bytes: 0,
                item_count_estimate: 0,
                data_structure: "unknown".to_string(),
            })
        }
    }

    /// Validate file-based input
    async fn validate_file_input(&self, path: &str) -> Result<InputValidation, DryRunError> {
        debug!("Validating file input: {}", path);

        let content = fs::read_to_string(path).await.map_err(|e| {
            DryRunError::InputError(format!("Failed to read input file {}: {}", path, e))
        })?;

        let data: Value = serde_json::from_str(&content).map_err(|e| DryRunError::JsonError(e))?;

        let item_count = self.estimate_item_count(&data);
        let structure = self.analyze_structure(&data);

        Ok(InputValidation {
            source: path.to_string(),
            valid: true,
            size_bytes: content.len(),
            item_count_estimate: item_count,
            data_structure: structure,
        })
    }

    /// Validate command-based input
    async fn validate_command_input(&self, command: &str) -> Result<InputValidation, DryRunError> {
        debug!("Validating command input: {}", command);

        // Strip "shell:" prefix
        let cmd = command.strip_prefix("shell:").unwrap_or(command).trim();

        // In dry-run mode, we don't actually execute the command
        // Just validate that it's not empty and appears to be a valid command
        if cmd.is_empty() {
            return Ok(InputValidation {
                source: command.to_string(),
                valid: false,
                size_bytes: 0,
                item_count_estimate: 0,
                data_structure: "invalid command".to_string(),
            });
        }

        // Check if it's a potentially dangerous command
        let dangerous_commands = ["rm", "del", "format", "dd", "mkfs"];
        let first_word = cmd.split_whitespace().next().unwrap_or("");

        if dangerous_commands
            .iter()
            .any(|&danger| first_word == danger)
        {
            warn!("Potentially dangerous command detected: {}", cmd);
        }

        Ok(InputValidation {
            source: command.to_string(),
            valid: true,
            size_bytes: 0,          // Unknown in dry-run
            item_count_estimate: 0, // Unknown in dry-run
            data_structure: "command output (not executed in dry-run)".to_string(),
        })
    }

    /// Load work items from input source
    pub async fn load_work_items(
        &self,
        input: &str,
        json_path: Option<&str>,
    ) -> Result<Vec<Value>, DryRunError> {
        if input.starts_with("shell:") {
            // In dry-run mode, return mock data for command inputs
            warn!("Command input in dry-run mode, returning empty work items");
            return Ok(Vec::new());
        }

        // Load from file
        if !Path::new(input).exists() {
            return Err(DryRunError::InputError(format!(
                "Input file does not exist: {}",
                input
            )));
        }

        let content = fs::read_to_string(input)
            .await
            .map_err(|e| DryRunError::InputError(format!("Failed to read input file: {}", e)))?;

        let data: Value = serde_json::from_str(&content)?;

        // Apply JSONPath if provided
        if let Some(path) = json_path {
            self.extract_with_jsonpath(&data, path)
        } else if let Value::Array(items) = data {
            Ok(items)
        } else {
            Ok(vec![data])
        }
    }

    /// Validate a JSONPath expression
    pub async fn validate_jsonpath(
        &self,
        path: &str,
        sample_data: &Value,
    ) -> Result<JsonPathValidation, DryRunError> {
        debug!("Validating JSONPath: {}", path);

        // Simple validation - just check basic syntax
        if path.is_empty() {
            return Err(DryRunError::JsonPathError("Empty JSONPath".to_string()));
        }

        // Extract items using simple pattern matching for common cases
        let match_values = self.extract_with_jsonpath(sample_data, path)?;
        let data_types = self.analyze_data_types(&match_values);

        Ok(JsonPathValidation {
            path: path.to_string(),
            valid: true,
            match_count: match_values.len(),
            sample_matches: match_values.into_iter().take(5).collect(),
            data_types,
        })
    }

    /// Extract items using JSONPath (simplified version)
    fn extract_with_jsonpath(&self, data: &Value, path: &str) -> Result<Vec<Value>, DryRunError> {
        // Simple JSONPath extraction for common patterns
        if path == "$" {
            return Ok(vec![data.clone()]);
        }

        // Handle $.items[*] pattern
        if path == "$.items[*]" || path == "$.items" {
            if let Value::Object(obj) = data {
                if let Some(Value::Array(items)) = obj.get("items") {
                    return Ok(items.clone());
                }
            }
        }

        // Handle $[*] pattern
        if path == "$[*]" {
            if let Value::Array(items) = data {
                return Ok(items.clone());
            }
        }

        // Handle $.field pattern
        if let Some(field) = path.strip_prefix("$.") {
            if !field.contains('[') && !field.contains('.') {
                if let Value::Object(obj) = data {
                    if let Some(value) = obj.get(field) {
                        if let Value::Array(items) = value {
                            return Ok(items.clone());
                        }
                        return Ok(vec![value.clone()]);
                    }
                }
            }
        }

        // For unsupported patterns, return the data as-is
        warn!("Complex JSONPath '{}' not fully supported in dry-run mode", path);
        Ok(vec![data.clone()])
    }

    /// Estimate number of items in data structure
    fn estimate_item_count(&self, data: &Value) -> usize {
        match data {
            Value::Array(items) => items.len(),
            Value::Object(map) => {
                // Look for common patterns like "items", "data", "results"
                for key in ["items", "data", "results", "records"] {
                    if let Some(Value::Array(items)) = map.get(key) {
                        return items.len();
                    }
                }
                1 // Single object
            }
            _ => 1,
        }
    }

    /// Analyze the structure of the data
    fn analyze_structure(&self, data: &Value) -> String {
        match data {
            Value::Array(items) => {
                if items.is_empty() {
                    "empty array".to_string()
                } else {
                    format!("array of {} items", items.len())
                }
            }
            Value::Object(map) => {
                let keys: Vec<String> = map.keys().take(5).cloned().collect();
                if keys.is_empty() {
                    "empty object".to_string()
                } else {
                    format!("object with keys: {}", keys.join(", "))
                }
            }
            Value::String(_) => "string".to_string(),
            Value::Number(_) => "number".to_string(),
            Value::Bool(_) => "boolean".to_string(),
            Value::Null => "null".to_string(),
        }
    }

    /// Analyze data types in a collection of values
    fn analyze_data_types(&self, values: &[Value]) -> HashMap<String, usize> {
        let mut types = HashMap::new();

        for value in values {
            let type_name = match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };

            *types.entry(type_name.to_string()).or_insert(0) += 1;
        }

        types
    }
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}
