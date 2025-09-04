use super::provider::{InputConfig, InputProvider, ValidationIssue, ValidationSeverity};
use super::types::{
    DataFormat, ExecutionInput, InputType, VariableDefinition, VariableType, VariableValue,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::io::{self, Read};
use tokio::io::{AsyncReadExt, BufReader};

pub struct StandardInputProvider;

#[async_trait]
impl InputProvider for StandardInputProvider {
    fn input_type(&self) -> InputType {
        InputType::StandardInput {
            format: DataFormat::Auto,
        }
    }

    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check if stdin is available (not in automation mode)
        if std::env::var("PRODIGY_AUTOMATION").unwrap_or_default() == "true" {
            if !config.get_bool("allow_in_automation").unwrap_or(false) {
                issues.push(ValidationIssue {
                    field: "stdin".to_string(),
                    message: "Standard input not available in automation mode".to_string(),
                    severity: ValidationSeverity::Error,
                });
            }
        }

        // Validate format if specified
        if let Ok(format_str) = config.get_string("format") {
            if !["json", "yaml", "csv", "lines", "text", "auto"].contains(&format_str.as_str()) {
                issues.push(ValidationIssue {
                    field: "format".to_string(),
                    message: format!("Unsupported format for stdin: {}", format_str),
                    severity: ValidationSeverity::Error,
                });
            }
        }

        Ok(issues)
    }

    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let format = self.detect_format(config)?;
        let mut inputs: Vec<ExecutionInput> = Vec::new();

        // Check for simulated input in test/automation mode
        if let Ok(simulated_input) = config.get_string("simulated_input") {
            return self.process_input(&simulated_input, format, config).await;
        }

        // Read from actual stdin
        let stdin_content = if config.get_bool("async_read").unwrap_or(true) {
            self.read_stdin_async().await?
        } else {
            self.read_stdin_sync()?
        };

        if stdin_content.is_empty() {
            return Ok(vec![]);
        }

        self.process_input(&stdin_content, format, config).await
    }

    fn available_variables(&self, config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        let format = self.detect_format(config)?;

        let mut vars = vec![
            VariableDefinition {
                name: "stdin_format".to_string(),
                var_type: VariableType::String,
                description: "Format of the standard input data".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
        ];

        match format {
            DataFormat::Json | DataFormat::Yaml | DataFormat::Toml => {
                vars.push(VariableDefinition {
                    name: "stdin_data".to_string(),
                    var_type: VariableType::Object,
                    description: "Parsed standard input as structured data".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            DataFormat::PlainText => {
                vars.push(VariableDefinition {
                    name: "stdin_text".to_string(),
                    var_type: VariableType::String,
                    description: "Raw text from standard input".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
                vars.push(VariableDefinition {
                    name: "stdin_lines".to_string(),
                    var_type: VariableType::Array,
                    description: "Lines from standard input".to_string(),
                    required: false,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            _ => {
                vars.push(VariableDefinition {
                    name: "stdin_content".to_string(),
                    var_type: VariableType::String,
                    description: "Raw content from standard input".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
        }

        Ok(vars)
    }

    fn supports(&self, config: &InputConfig) -> bool {
        config.get_bool("use_stdin").unwrap_or(false)
            || config.get_string("input_type").map(|t| t == "stdin").unwrap_or(false)
            || config.get_string("simulated_input").is_ok()
    }
}

impl StandardInputProvider {
    fn detect_format(&self, config: &InputConfig) -> Result<DataFormat> {
        if let Ok(format_str) = config.get_string("format") {
            return match format_str.as_str() {
                "json" => Ok(DataFormat::Json),
                "yaml" => Ok(DataFormat::Yaml),
                "csv" => Ok(DataFormat::Csv),
                "lines" | "text" => Ok(DataFormat::PlainText),
                "auto" => Ok(DataFormat::Auto),
                _ => Err(anyhow!("Unsupported stdin format: {}", format_str)),
            };
        }

        Ok(DataFormat::PlainText)
    }

    async fn read_stdin_async(&self) -> Result<String> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut buffer = String::new();
        
        // Set a timeout for reading
        let timeout_duration = std::time::Duration::from_secs(5);
        let read_future = reader.read_to_string(&mut buffer);

        match tokio::time::timeout(timeout_duration, read_future).await {
            Ok(Ok(_)) => Ok(buffer),
            Ok(Err(e)) => Err(anyhow!("Failed to read from stdin: {}", e)),
            Err(_) => Err(anyhow!("Timeout reading from stdin")),
        }
    }

    fn read_stdin_sync(&self) -> Result<String> {
        let mut buffer = String::new();
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        
        handle.read_to_string(&mut buffer)
            .map_err(|e| anyhow!("Failed to read from stdin: {}", e))?;
        
        Ok(buffer)
    }

    async fn process_input(
        &self,
        content: &str,
        format: DataFormat,
        config: &InputConfig,
    ) -> Result<Vec<ExecutionInput>> {
        let mut inputs = Vec::new();

        match format {
            DataFormat::Json => {
                let parsed: serde_json::Value = serde_json::from_str(content)?;
                inputs.extend(self.process_json(parsed)?);
            }
            DataFormat::Yaml => {
                let parsed: serde_yaml::Value = serde_yaml::from_str(content)?;
                let json_value = serde_json::to_value(parsed)?;
                inputs.extend(self.process_json(json_value)?);
            }
            DataFormat::PlainText => {
                if config.get_bool("process_lines").unwrap_or(false) {
                    // Process each line as a separate input
                    for (index, line) in content.lines().enumerate() {
                        if line.trim().is_empty() && config.get_bool("skip_empty").unwrap_or(true) {
                            continue;
                        }

                        let mut input = ExecutionInput::new(
                            format!("stdin_line_{}", index),
                            InputType::StandardInput {
                                format: DataFormat::PlainText,
                            },
                        );

                        input.add_variable("stdin_line".to_string(), VariableValue::String(line.to_string()));
                        input.add_variable("line_number".to_string(), VariableValue::Number(index as i64 + 1));
                        input.add_variable("stdin_format".to_string(), VariableValue::String("lines".to_string()));

                        inputs.push(input);
                    }
                } else {
                    // Process as single text block
                    let mut input = ExecutionInput::new(
                        "stdin_text".to_string(),
                        InputType::StandardInput {
                            format: DataFormat::PlainText,
                        },
                    );

                    input.add_variable("stdin_text".to_string(), VariableValue::String(content.to_string()));
                    
                    let lines: Vec<VariableValue> = content
                        .lines()
                        .map(|l| VariableValue::String(l.to_string()))
                        .collect();
                    input.add_variable("stdin_lines".to_string(), VariableValue::Array(lines));
                    input.add_variable("stdin_format".to_string(), VariableValue::String("text".to_string()));

                    inputs.push(input);
                }
            }
            DataFormat::Auto => {
                // Try to auto-detect format from content
                let trimmed = content.trim();
                if trimmed.starts_with('{') || trimmed.starts_with('[') {
                    // Looks like JSON
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
                        return Ok(self.process_json(parsed)?);
                    }
                }
                
                // Default to plain text - inline the logic to avoid recursion
                if config.get_bool("process_lines").unwrap_or(false) {
                    // Process each line as a separate input
                    for (index, line) in content.lines().enumerate() {
                        if line.trim().is_empty() && config.get_bool("skip_empty").unwrap_or(true) {
                            continue;
                        }

                        let mut input = ExecutionInput::new(
                            format!("stdin_line_{}", index),
                            InputType::StandardInput {
                                format: DataFormat::PlainText,
                            },
                        );

                        input.add_variable("stdin_line".to_string(), VariableValue::String(line.to_string()));
                        input.add_variable("line_number".to_string(), VariableValue::Number(index as i64 + 1));
                        input.add_variable("stdin_format".to_string(), VariableValue::String("lines".to_string()));

                        inputs.push(input);
                    }
                } else {
                    // Process as single text block
                    let mut input = ExecutionInput::new(
                        "stdin_text".to_string(),
                        InputType::StandardInput {
                            format: DataFormat::PlainText,
                        },
                    );

                    input.add_variable("stdin_text".to_string(), VariableValue::String(content.to_string()));
                    
                    let lines: Vec<VariableValue> = content
                        .lines()
                        .map(|l| VariableValue::String(l.to_string()))
                        .collect();
                    input.add_variable("stdin_lines".to_string(), VariableValue::Array(lines));
                    input.add_variable("stdin_format".to_string(), VariableValue::String("text".to_string()));

                    inputs.push(input);
                }
            }
            _ => {
                // For other formats, store as raw content
                let mut input = ExecutionInput::new(
                    "stdin_data".to_string(),
                    InputType::StandardInput {
                        format: format.clone(),
                    },
                );

                input.add_variable("stdin_content".to_string(), VariableValue::String(content.to_string()));
                input.add_variable("stdin_format".to_string(), VariableValue::String(format!("{:?}", format)));

                inputs.push(input);
            }
        }

        Ok(inputs)
    }

    fn process_json(&self, value: serde_json::Value) -> Result<Vec<ExecutionInput>> {
        let mut inputs = Vec::new();

        match value {
            serde_json::Value::Array(arr) => {
                for (index, item) in arr.iter().enumerate() {
                    let mut input = ExecutionInput::new(
                        format!("stdin_item_{}", index),
                        InputType::StandardInput {
                            format: DataFormat::Json,
                        },
                    );

                    input.add_variable("stdin_data".to_string(), self.json_to_variable_value(item)?);
                    input.add_variable("item_index".to_string(), VariableValue::Number(index as i64));
                    input.add_variable("stdin_format".to_string(), VariableValue::String("json".to_string()));

                    inputs.push(input);
                }
            }
            _ => {
                let mut input = ExecutionInput::new(
                    "stdin_json".to_string(),
                    InputType::StandardInput {
                        format: DataFormat::Json,
                    },
                );

                input.add_variable("stdin_data".to_string(), self.json_to_variable_value(&value)?);
                input.add_variable("stdin_format".to_string(), VariableValue::String("json".to_string()));

                inputs.push(input);
            }
        }

        Ok(inputs)
    }

    fn json_to_variable_value(&self, value: &serde_json::Value) -> Result<VariableValue> {
        match value {
            serde_json::Value::Null => Ok(VariableValue::Null),
            serde_json::Value::Bool(b) => Ok(VariableValue::Boolean(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(VariableValue::Number(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(VariableValue::Float(f))
                } else {
                    Err(anyhow!("Could not convert number"))
                }
            }
            serde_json::Value::String(s) => Ok(VariableValue::String(s.clone())),
            serde_json::Value::Array(arr) => {
                let values: Result<Vec<_>> = arr.iter().map(|v| self.json_to_variable_value(v)).collect();
                Ok(VariableValue::Array(values?))
            }
            serde_json::Value::Object(obj) => {
                let mut map = std::collections::HashMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), self.json_to_variable_value(v)?);
                }
                Ok(VariableValue::Object(map))
            }
        }
    }
}