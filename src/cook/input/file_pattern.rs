use super::provider::{InputConfig, InputProvider, ValidationIssue, ValidationSeverity};
use super::types::{
    ExecutionInput, InputMetadata, InputType, ValidationRule, VariableDefinition, VariableType,
    VariableValue,
};
use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct FilePatternInputProvider;

/// Create file-related variables from a path
fn create_file_variables(file_path: &Path) -> Vec<(String, VariableValue)> {
    vec![
        (
            "file_path".to_string(),
            VariableValue::Path(file_path.to_path_buf()),
        ),
        (
            "file_name".to_string(),
            VariableValue::String(
                file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ),
        ),
        (
            "file_dir".to_string(),
            VariableValue::Path(
                file_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
            ),
        ),
        (
            "file_stem".to_string(),
            VariableValue::String(
                file_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ),
        ),
        (
            "file_extension".to_string(),
            VariableValue::String(
                file_path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ),
        ),
    ]
}

/// Create input metadata from a file path and metadata
fn create_input_metadata(file_path: &Path, metadata: &fs::Metadata) -> InputMetadata {
    InputMetadata {
        source: file_path.to_string_lossy().to_string(),
        created_at: chrono::Utc::now(),
        size_bytes: Some(metadata.len()),
        checksum: None,
        content_type: Some(
            mime_guess::from_path(file_path)
                .first_or_octet_stream()
                .to_string(),
        ),
        custom_fields: std::collections::HashMap::new(),
    }
}

/// Expand a pattern string based on recursive flag
fn expand_pattern(pattern: &str, recursive: bool) -> String {
    if recursive && !pattern.contains("**") {
        format!("**/{}", pattern)
    } else {
        pattern.to_string()
    }
}

impl Default for FilePatternInputProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePatternInputProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl InputProvider for FilePatternInputProvider {
    fn input_type(&self) -> InputType {
        InputType::FilePattern {
            patterns: vec![],
            recursive: false,
        }
    }

    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check if patterns are provided
        if config.get_array("patterns").is_err() {
            issues.push(ValidationIssue {
                field: "patterns".to_string(),
                message: "No file patterns provided".to_string(),
                severity: ValidationSeverity::Warning,
            });
        }

        Ok(issues)
    }

    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let patterns = config.get_array("patterns")?;
        let recursive = config.get_bool("recursive").unwrap_or(false);

        let mut all_files = HashSet::new();

        for pattern in &patterns {
            let pattern_str = pattern
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Pattern must be a string"))?;

            // Use glob to find matching files
            let pattern_to_use = expand_pattern(pattern_str, recursive);

            for entry in glob(&pattern_to_use)? {
                match entry {
                    Ok(path) => {
                        // Check file accessibility once during glob iteration
                        // This avoids race conditions between glob and later metadata checks
                        if let Ok(metadata) = fs::metadata(&path) {
                            if metadata.is_file() {
                                all_files.insert(path);
                            }
                        }
                        // Skip inaccessible files silently (broken symlinks, permission issues)
                    }
                    Err(e) => {
                        // Log but don't fail on individual glob errors
                        eprintln!("Glob error: {}", e);
                    }
                }
            }
        }

        let mut inputs = Vec::new();

        for (index, file_path) in all_files.iter().enumerate() {
            // We already verified file accessibility during glob iteration
            // But double-check here in case filesystem changed between glob and now
            let metadata = match fs::metadata(file_path) {
                Ok(m) => m,
                Err(e) => {
                    // This shouldn't happen often since we checked during glob
                    eprintln!("Skipping inaccessible file {:?}: {}", file_path, e);
                    continue;
                }
            };

            let mut input = ExecutionInput::new(
                format!("file_{}", index),
                InputType::FilePattern {
                    patterns: patterns
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                    recursive,
                },
            );

            // File path variables
            for (name, value) in create_file_variables(file_path) {
                input.add_variable(name, value);
            }

            // File metadata variables
            input.add_variable(
                "file_size".to_string(),
                VariableValue::Number(metadata.len() as i64),
            );

            // Add metadata
            input.with_metadata(create_input_metadata(file_path, &metadata));
            inputs.push(input);
        }

        Ok(inputs)
    }

    fn available_variables(&self, _config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        Ok(vec![
            VariableDefinition {
                name: "file_path".to_string(),
                var_type: VariableType::Path,
                description: "Full path to the matched file".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![ValidationRule::FileExists],
            },
            VariableDefinition {
                name: "file_name".to_string(),
                var_type: VariableType::String,
                description: "File name with extension".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_dir".to_string(),
                var_type: VariableType::Path,
                description: "Directory containing the file".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_stem".to_string(),
                var_type: VariableType::String,
                description: "File name without extension".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_extension".to_string(),
                var_type: VariableType::String,
                description: "File extension (without dot)".to_string(),
                required: false,
                default_value: Some("".to_string()),
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_size".to_string(),
                var_type: VariableType::Number,
                description: "File size in bytes".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![ValidationRule::Range {
                    min: Some(0),
                    max: None,
                }],
            },
        ])
    }

    fn supports(&self, config: &InputConfig) -> bool {
        config.get_array("patterns").is_ok()
    }
}
