use super::provider::{InputConfig, InputProvider, ValidationIssue};
use super::types::{ExecutionInput, InputType, VariableDefinition, VariableType, VariableValue};
use anyhow::Result;
use async_trait::async_trait;
use std::env;

pub struct EnvironmentInputProvider;

/// Filters environment variables based on prefix and empty value criteria.
/// This is a pure function that takes filtering parameters and returns a vector of filtered vars.
fn filter_env_vars(prefix: Option<&str>, filter_empty: bool) -> Vec<(String, String)> {
    env::vars()
        .filter(|(key, value)| {
            // Apply prefix filter if specified
            if let Some(p) = prefix {
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
        .collect()
}

/// Attempts to parse a string value as a number.
fn try_parse_as_number(value: &str) -> Option<i64> {
    value.parse::<i64>().ok()
}

/// Attempts to parse a string value as a boolean.
fn try_parse_as_boolean(value: &str) -> Option<bool> {
    value.to_lowercase().parse::<bool>().ok()
}

/// Checks if a key represents a path-like variable.
fn is_path_like_key(key: &str) -> bool {
    key.contains("PATH") || key.contains("DIR") || key.contains("HOME")
}

/// Enriches an input with typed values based on the environment variable's value.
fn enrich_input_with_types(input: &mut ExecutionInput, key: &str, value: &str) {
    // Try to parse as number
    if let Some(num) = try_parse_as_number(value) {
        input.add_variable("env_value_number".to_string(), VariableValue::Number(num));
    }

    // Try to parse as boolean
    if let Some(bool_val) = try_parse_as_boolean(value) {
        input.add_variable(
            "env_value_bool".to_string(),
            VariableValue::Boolean(bool_val),
        );
    }

    // Check if it's a path-like variable
    if is_path_like_key(key) {
        input.add_variable(
            "env_value_path".to_string(),
            VariableValue::Path(value.into()),
        );
    }
}

/// Builds a single consolidated input containing all environment variables.
fn build_single_input(env_vars: Vec<(String, String)>, prefix: Option<String>) -> ExecutionInput {
    let mut input = ExecutionInput::new(
        "env_all".to_string(),
        InputType::Environment {
            prefix: prefix.clone(),
        },
    );

    // Build environment object using functional patterns
    let env_object = env_vars
        .iter()
        .map(|(key, value)| (key.clone(), VariableValue::String(value.clone())))
        .collect::<std::collections::HashMap<_, _>>();

    input.add_variable("env".to_string(), VariableValue::Object(env_object));
    input.add_variable(
        "env_count".to_string(),
        VariableValue::Number(env_vars.len() as i64),
    );

    if let Some(p) = prefix {
        input.add_variable("env_prefix".to_string(), VariableValue::String(p));
    }

    input
}

/// Builds multiple inputs, one per environment variable.
fn build_multi_inputs(
    env_vars: Vec<(String, String)>,
    prefix: Option<String>,
) -> Vec<ExecutionInput> {
    env_vars
        .into_iter()
        .map(|(key, value)| {
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

            // Enrich with typed values
            enrich_input_with_types(&mut input, &key, &value);

            input
        })
        .collect()
}

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
        // Extract configuration parameters
        let prefix = config.get_string("prefix").ok();
        let filter_empty = config.get_bool("filter_empty").unwrap_or(true);
        let single_input = config.get_bool("single_input").unwrap_or(false);

        // Filter environment variables
        let env_vars = filter_env_vars(prefix.as_deref(), filter_empty);

        // Build inputs based on mode
        let inputs = if single_input {
            vec![build_single_input(env_vars, prefix)]
        } else {
            build_multi_inputs(env_vars, prefix)
        };

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
