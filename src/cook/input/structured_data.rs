use super::provider::{InputConfig, InputProvider, ValidationIssue, ValidationSeverity};
use super::types::{
    DataFormat, ExecutionInput, InputType, VariableDefinition, VariableType,
    VariableValue,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

pub struct StructuredDataInputProvider;

#[async_trait]
impl InputProvider for StructuredDataInputProvider {
    fn input_type(&self) -> InputType {
        InputType::StructuredData {
            format: DataFormat::Auto,
            schema: None,
        }
    }

    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check if data source is provided
        if let Err(_) = config.get_string("file_path") {
            if let Err(_) = config.get_string("data") {
                issues.push(ValidationIssue {
                    field: "source".to_string(),
                    message: "Either 'file_path' or 'data' must be provided".to_string(),
                    severity: ValidationSeverity::Error,
                });
            }
        }

        // Validate format if specified
        if let Ok(format_str) = config.get_string("format") {
            if !["json", "yaml", "toml", "csv", "xml", "text", "auto"].contains(&format_str.as_str()) {
                issues.push(ValidationIssue {
                    field: "format".to_string(),
                    message: format!("Unsupported format: {}", format_str),
                    severity: ValidationSeverity::Error,
                });
            }
        }

        Ok(issues)
    }

    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let format = self.detect_format(config)?;
        let data = self.load_data(config).await?;

        let mut inputs = Vec::new();

        match format {
            DataFormat::Json => {
                let parsed: serde_json::Value = serde_json::from_str(&data)?;
                inputs.extend(self.process_json_data(parsed)?);
            }
            DataFormat::Yaml => {
                let parsed: serde_yaml::Value = serde_yaml::from_str(&data)?;
                let json_value = serde_json::to_value(parsed)?;
                inputs.extend(self.process_json_data(json_value)?);
            }
            DataFormat::Toml => {
                let parsed: toml::Value = toml::from_str(&data)?;
                let json_value = serde_json::to_value(parsed)?;
                inputs.extend(self.process_json_data(json_value)?);
            }
            DataFormat::Csv => {
                inputs.extend(self.process_csv_data(&data)?);
            }
            DataFormat::Xml => {
                inputs.extend(self.process_xml_data(&data)?);
            }
            DataFormat::PlainText => {
                inputs.push(self.process_plain_text(&data)?);
            }
            DataFormat::Auto => {
                return Err(anyhow!("Could not auto-detect data format"));
            }
        }

        Ok(inputs)
    }

    fn available_variables(&self, config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        let format = self.detect_format(config)?;

        let mut vars = vec![
            VariableDefinition {
                name: "data_format".to_string(),
                var_type: VariableType::String,
                description: "The format of the structured data".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
        ];

        match format {
            DataFormat::Json | DataFormat::Yaml | DataFormat::Toml | DataFormat::Xml => {
                vars.push(VariableDefinition {
                    name: "data".to_string(),
                    var_type: VariableType::Object,
                    description: "The parsed data object".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            DataFormat::Csv => {
                vars.push(VariableDefinition {
                    name: "row".to_string(),
                    var_type: VariableType::Object,
                    description: "Current CSV row as an object".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
                vars.push(VariableDefinition {
                    name: "row_index".to_string(),
                    var_type: VariableType::Number,
                    description: "Zero-based index of the current row".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            DataFormat::PlainText => {
                vars.push(VariableDefinition {
                    name: "text".to_string(),
                    var_type: VariableType::String,
                    description: "The plain text content".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
                vars.push(VariableDefinition {
                    name: "line_count".to_string(),
                    var_type: VariableType::Number,
                    description: "Number of lines in the text".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            DataFormat::Auto => {}
        }

        Ok(vars)
    }

    fn supports(&self, config: &InputConfig) -> bool {
        config.get_string("file_path").is_ok() || config.get_string("data").is_ok()
    }
}

impl StructuredDataInputProvider {
    fn detect_format(&self, config: &InputConfig) -> Result<DataFormat> {
        // Check if format is explicitly specified
        if let Ok(format_str) = config.get_string("format") {
            return match format_str.as_str() {
                "json" => Ok(DataFormat::Json),
                "yaml" => Ok(DataFormat::Yaml),
                "toml" => Ok(DataFormat::Toml),
                "csv" => Ok(DataFormat::Csv),
                "xml" => Ok(DataFormat::Xml),
                "text" => Ok(DataFormat::PlainText),
                "auto" => self.auto_detect_format(config),
                _ => Err(anyhow!("Unsupported format: {}", format_str)),
            };
        }

        self.auto_detect_format(config)
    }

    fn auto_detect_format(&self, config: &InputConfig) -> Result<DataFormat> {
        // Try to detect from file extension
        if let Ok(file_path) = config.get_string("file_path") {
            let path = PathBuf::from(&file_path);
            if let Some(ext) = path.extension() {
                return match ext.to_str().unwrap_or("").to_lowercase().as_str() {
                    "json" => Ok(DataFormat::Json),
                    "yaml" | "yml" => Ok(DataFormat::Yaml),
                    "toml" => Ok(DataFormat::Toml),
                    "csv" => Ok(DataFormat::Csv),
                    "xml" => Ok(DataFormat::Xml),
                    "txt" => Ok(DataFormat::PlainText),
                    _ => Ok(DataFormat::PlainText),
                };
            }
        }

        // Try to detect from content
        if let Ok(data) = config.get_string("data") {
            let trimmed = data.trim();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                return Ok(DataFormat::Json);
            }
            if trimmed.starts_with("---") || trimmed.contains(": ") {
                return Ok(DataFormat::Yaml);
            }
            if trimmed.starts_with('<') && trimmed.ends_with('>') {
                return Ok(DataFormat::Xml);
            }
            if trimmed.contains(" = ") {
                return Ok(DataFormat::Toml);
            }
        }

        Ok(DataFormat::PlainText)
    }

    async fn load_data(&self, config: &InputConfig) -> Result<String> {
        // Prefer direct data over file
        if let Ok(data) = config.get_string("data") {
            return Ok(data);
        }

        // Load from file
        let file_path = config.get_string("file_path")?;
        let content = fs::read_to_string(&file_path).await?;
        Ok(content)
    }

    fn process_json_data(&self, value: serde_json::Value) -> Result<Vec<ExecutionInput>> {
        let mut inputs = Vec::new();

        match value {
            serde_json::Value::Array(arr) => {
                for (index, item) in arr.iter().enumerate() {
                    let mut input = ExecutionInput::new(
                        format!("json_item_{}", index),
                        InputType::StructuredData {
                            format: DataFormat::Json,
                            schema: None,
                        },
                    );

                    input.add_variable("data".to_string(), self.json_to_variable_value(item)?);
                    input.add_variable("data_format".to_string(), VariableValue::String("json".to_string()));
                    input.add_variable("item_index".to_string(), VariableValue::Number(index as i64));

                    inputs.push(input);
                }
            }
            _ => {
                let mut input = ExecutionInput::new(
                    "json_data".to_string(),
                    InputType::StructuredData {
                        format: DataFormat::Json,
                        schema: None,
                    },
                );

                input.add_variable("data".to_string(), self.json_to_variable_value(&value)?);
                input.add_variable("data_format".to_string(), VariableValue::String("json".to_string()));

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
                let mut map = HashMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), self.json_to_variable_value(v)?);
                }
                Ok(VariableValue::Object(map))
            }
        }
    }

    fn process_csv_data(&self, data: &str) -> Result<Vec<ExecutionInput>> {
        let mut inputs = Vec::new();
        let mut reader = csv::Reader::from_reader(data.as_bytes());
        
        let headers = reader.headers()?.clone();
        
        for (index, result) in reader.records().enumerate() {
            let record = result?;
            let mut input = ExecutionInput::new(
                format!("csv_row_{}", index),
                InputType::StructuredData {
                    format: DataFormat::Csv,
                    schema: None,
                },
            );

            let mut row_data = HashMap::new();
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    row_data.insert(header.to_string(), VariableValue::String(field.to_string()));
                }
            }

            input.add_variable("row".to_string(), VariableValue::Object(row_data));
            input.add_variable("row_index".to_string(), VariableValue::Number(index as i64));
            input.add_variable("data_format".to_string(), VariableValue::String("csv".to_string()));

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn process_xml_data(&self, data: &str) -> Result<Vec<ExecutionInput>> {
        // For simplicity, convert XML to a basic string representation
        // In a real implementation, you'd use an XML parser
        let mut input = ExecutionInput::new(
            "xml_data".to_string(),
            InputType::StructuredData {
                format: DataFormat::Xml,
                schema: None,
            },
        );

        input.add_variable("data".to_string(), VariableValue::String(data.to_string()));
        input.add_variable("data_format".to_string(), VariableValue::String("xml".to_string()));

        Ok(vec![input])
    }

    fn process_plain_text(&self, data: &str) -> Result<ExecutionInput> {
        let mut input = ExecutionInput::new(
            "text_data".to_string(),
            InputType::StructuredData {
                format: DataFormat::PlainText,
                schema: None,
            },
        );

        let line_count = data.lines().count();

        input.add_variable("text".to_string(), VariableValue::String(data.to_string()));
        input.add_variable("line_count".to_string(), VariableValue::Number(line_count as i64));
        input.add_variable("data_format".to_string(), VariableValue::String("text".to_string()));

        Ok(input)
    }
}