use super::provider::{InputConfig, InputProvider, ValidationIssue};
use super::types::{ExecutionInput, InputType, VariableDefinition, VariableType, VariableValue};
use anyhow::Result;
use async_trait::async_trait;
use std::env;

pub struct EnvironmentInputProvider;

#[async_trait]
impl InputProvider for EnvironmentInputProvider {
    fn input_type(&self) -> InputType {
        InputType::Environment { prefix: None }
    }

    async fn validate(&self, _config: &InputConfig) -> Result<Vec<ValidationIssue>> {
        // Environment variables are always valid
        Ok(Vec::new())
    }

    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let prefix = config.get_string("prefix").ok();
        let filter_empty = config.get_bool("filter_empty").unwrap_or(true);

        let mut inputs = Vec::new();

        // Check if we should create a single input with all vars, or one per var
        let single_input = config.get_bool("single_input").unwrap_or(false);

        if single_input {
            // Create a single input with all environment variables
            let mut input = ExecutionInput::new(
                "env_all".to_string(),
                InputType::Environment {
                    prefix: prefix.clone(),
                },
            );

            let env_vars: Vec<(String, String)> = env::vars()
                .filter(|(key, value)| {
                    // Apply prefix filter if specified
                    if let Some(ref p) = prefix {
                        if !key.starts_with(p) {
                            return false;
                        }
                    }
                    // Filter empty values if requested
                    if filter_empty && value.is_empty() {
                        return false;
                    }
                    true
                })
                .collect();

            // Add all environment variables as a single object
            let mut env_object = std::collections::HashMap::new();
            for (key, value) in &env_vars {
                env_object.insert(key.clone(), VariableValue::String(value.clone()));
            }

            input.add_variable("env".to_string(), VariableValue::Object(env_object));
            input.add_variable(
                "env_count".to_string(),
                VariableValue::Number(env_vars.len() as i64),
            );

            if let Some(ref p) = prefix {
                input.add_variable("env_prefix".to_string(), VariableValue::String(p.clone()));
            }

            inputs.push(input);
        } else {
            // Create one input per environment variable
            let env_vars: Vec<(String, String)> = env::vars()
                .filter(|(key, value)| {
                    // Apply prefix filter if specified
                    if let Some(ref p) = prefix {
                        if !key.starts_with(p) {
                            return false;
                        }
                    }
                    // Filter empty values if requested
                    if filter_empty && value.is_empty() {
                        return false;
                    }
                    true
                })
                .collect();

            for (key, value) in env_vars {
                let mut input = ExecutionInput::new(
                    format!("env_{}", key.to_lowercase()),
                    InputType::Environment {
                        prefix: prefix.clone(),
                    },
                );

                // Add the environment variable
                input.add_variable("env_key".to_string(), VariableValue::String(key.clone()));
                input.add_variable(
                    "env_value".to_string(),
                    VariableValue::String(value.clone()),
                );

                // Strip prefix from key if specified
                if let Some(ref p) = prefix {
                    let stripped_key = key.strip_prefix(p).unwrap_or(&key);
                    input.add_variable(
                        "env_key_stripped".to_string(),
                        VariableValue::String(stripped_key.to_string()),
                    );
                }

                // Try to parse common types
                if let Ok(num) = value.parse::<i64>() {
                    input.add_variable("env_value_number".to_string(), VariableValue::Number(num));
                }

                if let Ok(bool_val) = value.to_lowercase().parse::<bool>() {
                    input.add_variable(
                        "env_value_bool".to_string(),
                        VariableValue::Boolean(bool_val),
                    );
                }

                // Parse path-like variables
                if key.contains("PATH") || key.contains("DIR") || key.contains("HOME") {
                    input.add_variable(
                        "env_value_path".to_string(),
                        VariableValue::Path(value.into()),
                    );
                }

                inputs.push(input);
            }
        }

        Ok(inputs)
    }

    fn available_variables(&self, config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        let single_input = config.get_bool("single_input").unwrap_or(false);

        if single_input {
            Ok(vec![
                VariableDefinition {
                    name: "env".to_string(),
                    var_type: VariableType::Object,
                    description: "All environment variables as an object".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                },
                VariableDefinition {
                    name: "env_count".to_string(),
                    var_type: VariableType::Number,
                    description: "Number of environment variables".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                },
                VariableDefinition {
                    name: "env_prefix".to_string(),
                    var_type: VariableType::String,
                    description: "Prefix filter applied to environment variables".to_string(),
                    required: false,
                    default_value: None,
                    validation_rules: vec![],
                },
            ])
        } else {
            Ok(vec![
                VariableDefinition {
                    name: "env_key".to_string(),
                    var_type: VariableType::String,
                    description: "Environment variable name".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                },
                VariableDefinition {
                    name: "env_value".to_string(),
                    var_type: VariableType::String,
                    description: "Environment variable value".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                },
                VariableDefinition {
                    name: "env_key_stripped".to_string(),
                    var_type: VariableType::String,
                    description: "Environment variable name with prefix removed".to_string(),
                    required: false,
                    default_value: None,
                    validation_rules: vec![],
                },
                VariableDefinition {
                    name: "env_value_number".to_string(),
                    var_type: VariableType::Number,
                    description: "Environment variable value as number (if parseable)".to_string(),
                    required: false,
                    default_value: None,
                    validation_rules: vec![],
                },
                VariableDefinition {
                    name: "env_value_bool".to_string(),
                    var_type: VariableType::Boolean,
                    description: "Environment variable value as boolean (if parseable)".to_string(),
                    required: false,
                    default_value: None,
                    validation_rules: vec![],
                },
                VariableDefinition {
                    name: "env_value_path".to_string(),
                    var_type: VariableType::Path,
                    description: "Environment variable value as path (for PATH-like variables)"
                        .to_string(),
                    required: false,
                    default_value: None,
                    validation_rules: vec![],
                },
            ])
        }
    }

    fn supports(&self, config: &InputConfig) -> bool {
        // Environment provider can always run, but check for configuration hints
        config
            .get_string("input_type")
            .map(|t| t == "environment")
            .unwrap_or(false)
            || config.get_string("env_prefix").is_ok()
            || config.get_bool("use_environment").unwrap_or(false)
    }
}
