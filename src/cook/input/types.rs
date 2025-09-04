use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInput {
    pub id: String,
    pub input_type: InputType,
    pub variables: HashMap<String, VariableValue>,
    pub metadata: InputMetadata,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputType {
    Empty,
    Arguments {
        separator: Option<String>,
    },
    FilePattern {
        patterns: Vec<String>,
        recursive: bool,
    },
    StructuredData {
        format: DataFormat,
        schema: Option<String>,
    },
    Environment {
        prefix: Option<String>,
    },
    StandardInput {
        format: DataFormat,
    },
    Generated {
        generator: String,
        config: serde_json::Value,
    },
    Composite {
        sources: Vec<Box<InputType>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMetadata {
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub size_bytes: Option<u64>,
    pub checksum: Option<String>,
    pub content_type: Option<String>,
    pub custom_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub name: String,
    pub var_type: VariableType,
    pub description: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub validation_rules: Vec<ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Path,
    Url,
    Array,
    Object,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VariableValue {
    String(String),
    Number(i64),
    Float(f64),
    Boolean(bool),
    Path(PathBuf),
    Url(url::Url),
    Array(Vec<VariableValue>),
    Object(HashMap<String, VariableValue>),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationRule {
    FileExists,
    Range { min: Option<i64>, max: Option<i64> },
    Pattern { regex: String },
    OneOf { values: Vec<String> },
    Custom { validator: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFormat {
    Json,
    Yaml,
    Toml,
    Csv,
    Xml,
    PlainText,
    Auto,
}

impl Default for InputMetadata {
    fn default() -> Self {
        Self {
            source: String::new(),
            created_at: Utc::now(),
            size_bytes: None,
            checksum: None,
            content_type: None,
            custom_fields: HashMap::new(),
        }
    }
}

impl ExecutionInput {
    pub fn new(id: String, input_type: InputType) -> Self {
        Self {
            id,
            input_type,
            variables: HashMap::new(),
            metadata: InputMetadata::default(),
            dependencies: Vec::new(),
        }
    }

    pub fn add_variable(&mut self, name: String, value: VariableValue) -> &mut Self {
        self.variables.insert(name, value);
        self
    }

    pub fn with_metadata(&mut self, metadata: InputMetadata) -> &mut Self {
        self.metadata = metadata;
        self
    }

    pub fn substitute_in_template(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();

        // Standard variables available to all inputs
        result = result.replace("{input_id}", &self.id);
        result = result.replace("{input_type}", &format!("{:?}", self.input_type));

        // Input-specific variables
        for (key, value) in &self.variables {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, &value.to_string());
        }

        // Apply helper functions
        result = self.apply_helper_functions(&result)?;

        Ok(result)
    }

    fn apply_helper_functions(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();

        // Example: {file_path|basename} -> filename without path
        let re = regex::Regex::new(r"\{([^|}]+)\|([^}]+)\}")?;

        for capture in re.captures_iter(template) {
            let var_name = &capture[1];
            let function = &capture[2];
            let full_match = &capture[0];

            if let Some(value) = self.variables.get(var_name) {
                let transformed = self.apply_transformation(value, function)?;
                result = result.replace(full_match, &transformed);
            }
        }

        Ok(result)
    }

    fn apply_transformation(&self, value: &VariableValue, function: &str) -> Result<String> {
        match function {
            "basename" => Ok(PathBuf::from(value.to_string())
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()),
            "dirname" => Ok(PathBuf::from(value.to_string())
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string())),
            "uppercase" => Ok(value.to_string().to_uppercase()),
            "lowercase" => Ok(value.to_string().to_lowercase()),
            "trim" => Ok(value.to_string().trim().to_string()),
            _ => Err(anyhow!("Unknown transformation function: {}", function)),
        }
    }
}

impl VariableValue {
    pub fn to_string(&self) -> String {
        match self {
            VariableValue::String(s) => s.clone(),
            VariableValue::Number(n) => n.to_string(),
            VariableValue::Float(f) => f.to_string(),
            VariableValue::Boolean(b) => b.to_string(),
            VariableValue::Path(p) => p.to_string_lossy().to_string(),
            VariableValue::Url(u) => u.to_string(),
            VariableValue::Array(arr) => format!(
                "[{}]",
                arr.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            VariableValue::Object(obj) => serde_json::to_string(obj).unwrap_or_default(),
            VariableValue::Null => "null".to_string(),
        }
    }

    pub fn as_path(&self) -> Result<PathBuf> {
        match self {
            VariableValue::Path(p) => Ok(p.clone()),
            VariableValue::String(s) => Ok(PathBuf::from(s)),
            _ => Err(anyhow!("Cannot convert {:?} to path", self)),
        }
    }

    pub fn as_number(&self) -> Result<i64> {
        match self {
            VariableValue::Number(n) => Ok(*n),
            VariableValue::Float(f) => Ok(*f as i64),
            VariableValue::String(s) => s.parse().map_err(anyhow::Error::from),
            _ => Err(anyhow!("Cannot convert {:?} to number", self)),
        }
    }
}
