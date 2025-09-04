use super::types::DataFormat;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    pub sources: Vec<InputSource>,
    pub validation: ValidationConfig,
    pub transformation: TransformationConfig,
    pub caching: CachingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputSource {
    Empty,
    Arguments {
        value: String,
        separator: Option<String>,
        validation: Option<ArgumentValidation>,
    },
    FilePattern {
        patterns: Vec<String>,
        recursive: bool,
        filters: Option<FileFilters>,
    },
    StructuredData {
        source: DataSource,
        format: DataFormat,
        schema: Option<String>,
        path: Option<String>, // JSONPath or similar for extracting subset
    },
    Environment {
        prefix: Option<String>,
        required_vars: Vec<String>,
        optional_vars: Vec<String>,
    },
    StandardInput {
        format: DataFormat,
        buffer_size: Option<usize>,
    },
    Generated {
        generator: GeneratorType,
        count: usize,
        config: serde_json::Value,
    },
    Composite {
        sources: Vec<InputSource>,
        merge_strategy: MergeStrategy,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataSource {
    File(PathBuf),
    Url(String),
    Inline(String),
    StandardInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFilters {
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub extensions: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub modified_since: Option<DateTime<Utc>>,
    pub modified_before: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentValidation {
    pub required_count: Option<usize>,
    pub pattern: Option<String>,
    pub custom_validator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GeneratorType {
    Range { start: i64, end: i64, step: i64 },
    Sequence { values: Vec<String> },
    Random { count: usize, seed: Option<u64> },
    Custom { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    Sequential,
    Interleaved,
    Grouped,
    Custom { handler: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub strict: bool,
    pub custom_rules: Vec<CustomValidationRule>,
    pub stop_on_first_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomValidationRule {
    pub name: String,
    pub expression: String,
    pub error_message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransformationConfig {
    pub variable_transformations: HashMap<String, String>,
    pub input_filters: Vec<InputFilter>,
    pub sorting: Option<SortConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFilter {
    pub filter_type: FilterType,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterType {
    Include { pattern: String },
    Exclude { pattern: String },
    Custom { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortConfig {
    pub field: String,
    pub ascending: bool,
    pub numeric: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachingConfig {
    pub enabled: bool,
    pub ttl_seconds: Option<u64>,
    pub cache_key_template: Option<String>,
    pub invalidate_on_change: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            sources: vec![InputSource::Empty],
            validation: ValidationConfig::default(),
            transformation: TransformationConfig::default(),
            caching: CachingConfig::default(),
        }
    }
}

impl InputConfig {
    pub fn from_command_args(args: &str) -> Self {
        Self {
            sources: vec![InputSource::Arguments {
                value: args.to_string(),
                separator: Some(",".to_string()),
                validation: None,
            }],
            validation: ValidationConfig::default(),
            transformation: TransformationConfig::default(),
            caching: CachingConfig::default(),
        }
    }

    pub fn from_file_patterns(patterns: Vec<String>) -> Self {
        Self {
            sources: vec![InputSource::FilePattern {
                patterns,
                recursive: false,
                filters: None,
            }],
            validation: ValidationConfig::default(),
            transformation: TransformationConfig::default(),
            caching: CachingConfig::default(),
        }
    }

    // Placeholder for future MapReduce support
    // Will be implemented when MapReduce configuration is refactored
}
