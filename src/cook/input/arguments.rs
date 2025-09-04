use super::provider::{InputConfig, InputProvider, ValidationIssue};
use super::types::{ExecutionInput, InputType, VariableDefinition, VariableType, VariableValue};
use anyhow::Result;
use async_trait::async_trait;

pub struct ArgumentsInputProvider;

#[async_trait]
impl InputProvider for ArgumentsInputProvider {
    fn input_type(&self) -> InputType {
        InputType::Arguments {
            separator: Some(",".to_string()),
        }
    }

    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>> {
        let issues = Vec::new();

        // Check if args are provided
        if config.get_string("args").is_err() {
            // Not an error - just means no arguments provided
        }

        Ok(issues)
    }

    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let args_str = config.get_string("args")?;
        let separator = config
            .get_string("separator")
            .unwrap_or_else(|_| ",".to_string());

        let arguments: Vec<String> = args_str
            .split(&separator)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut inputs = Vec::new();

        for (index, arg) in arguments.iter().enumerate() {
            let mut input = ExecutionInput::new(
                format!("arg_{}", index),
                InputType::Arguments {
                    separator: Some(separator.clone()),
                },
            );

            // Standard argument variables
            input.add_variable("arg".to_string(), VariableValue::String(arg.clone()));
            input.add_variable("arg_index".to_string(), VariableValue::Number(index as i64));
            input.add_variable(
                "arg_count".to_string(),
                VariableValue::Number(arguments.len() as i64),
            );

            // Try to parse as key=value pair
            if let Some((key, value)) = arg.split_once('=') {
                input.add_variable(
                    "arg_key".to_string(),
                    VariableValue::String(key.to_string()),
                );
                input.add_variable(
                    "arg_value".to_string(),
                    VariableValue::String(value.to_string()),
                );
            }

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn available_variables(&self, _config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        Ok(vec![
            VariableDefinition {
                name: "arg".to_string(),
                var_type: VariableType::String,
                description: "The current argument value".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_index".to_string(),
                var_type: VariableType::Number,
                description: "Zero-based index of the current argument".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_count".to_string(),
                var_type: VariableType::Number,
                description: "Total number of arguments".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_key".to_string(),
                var_type: VariableType::String,
                description: "Key part of key=value argument (if applicable)".to_string(),
                required: false,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_value".to_string(),
                var_type: VariableType::String,
                description: "Value part of key=value argument (if applicable)".to_string(),
                required: false,
                default_value: None,
                validation_rules: vec![],
            },
        ])
    }

    fn supports(&self, config: &InputConfig) -> bool {
        config.get_string("args").is_ok()
    }
}
