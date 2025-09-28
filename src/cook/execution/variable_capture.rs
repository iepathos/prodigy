//! Variable capture engine for setup phase commands
//!
//! This module implements comprehensive variable capture functionality
//! that allows users to capture command outputs as variables for use
//! throughout MapReduce workflow execution.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{info, warn};

/// Configuration for capturing output from setup commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CaptureConfig {
    /// Simple capture: just command index
    Simple(usize),
    /// Detailed capture configuration
    Detailed {
        /// Command index to capture from
        command_index: usize,
        /// What to capture (stdout, stderr, or both)
        #[serde(default = "default_capture_source")]
        source: CaptureSource,
        /// Optional regex pattern to extract
        pattern: Option<String>,
        /// Optional JSON path for JSON output
        json_path: Option<String>,
        /// Maximum output size to capture (bytes)
        #[serde(default = "default_max_capture_size")]
        max_size: usize,
        /// Default value if capture fails
        default: Option<String>,
        /// How to handle multi-line output
        #[serde(default)]
        multiline: MultilineHandling,
    },
}

impl CaptureConfig {
    /// Get the command index this config applies to
    pub fn command_index(&self) -> usize {
        match self {
            CaptureConfig::Simple(idx) => *idx,
            CaptureConfig::Detailed { command_index, .. } => *command_index,
        }
    }

    /// Extract detailed configuration parameters
    pub fn extract_params(&self) -> (usize, CaptureSource, Option<String>, Option<String>, usize, Option<String>, MultilineHandling) {
        match self {
            CaptureConfig::Simple(idx) => (
                *idx,
                CaptureSource::Stdout,
                None,
                None,
                default_max_capture_size(),
                None,
                MultilineHandling::default(),
            ),
            CaptureConfig::Detailed {
                command_index,
                source,
                pattern,
                json_path,
                max_size,
                default,
                multiline,
            } => (
                *command_index,
                source.clone(),
                pattern.clone(),
                json_path.clone(),
                *max_size,
                default.clone(),
                multiline.clone(),
            ),
        }
    }
}

/// What output to capture from a command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureSource {
    Stdout,
    Stderr,
    Both,
    Combined,
}

fn default_capture_source() -> CaptureSource {
    CaptureSource::Stdout
}

fn default_max_capture_size() -> usize {
    1024 * 1024 // 1MB default limit
}

/// How to handle multi-line output
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultilineHandling {
    /// Keep all lines as single string with newlines
    Preserve,
    /// Join lines with spaces
    Join,
    /// Take only first line
    FirstLine,
    /// Take only last line
    LastLine,
    /// Return as array of lines
    Array,
}

impl Default for MultilineHandling {
    fn default() -> Self {
        MultilineHandling::Preserve
    }
}

/// Captured variable from command output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedVariable {
    pub name: String,
    pub value: Value,
    pub source_command: usize,
    pub captured_at: DateTime<Utc>,
    pub metadata: CaptureMetadata,
}

/// Metadata about the capture process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureMetadata {
    pub source: CaptureSource,
    pub original_size: usize,
    pub truncated: bool,
    pub pattern_matched: bool,
    pub parsing_successful: bool,
}

/// Result from executing a command
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub exit_code: Option<i32>,
}

/// Engine for capturing variables from command outputs
pub struct VariableCaptureEngine {
    config: HashMap<String, CaptureConfig>,
    captured_variables: HashMap<String, CapturedVariable>,
}

impl VariableCaptureEngine {
    /// Create a new variable capture engine
    pub fn new(capture_config: HashMap<String, CaptureConfig>) -> Self {
        Self {
            config: capture_config,
            captured_variables: HashMap::new(),
        }
    }

    /// Capture variables from a command execution result
    pub async fn capture_from_command(
        &mut self,
        command_index: usize,
        command_result: &CommandResult,
    ) -> Result<()> {
        for (var_name, capture_config) in &self.config {
            if capture_config.command_index() == command_index {
                info!(
                    "Capturing output from command {} as variable '{}'",
                    command_index, var_name
                );

                let captured = self.perform_capture(var_name, capture_config, command_result).await?;
                self.captured_variables.insert(var_name.clone(), captured);
            }
        }
        Ok(())
    }

    /// Perform the actual capture of a variable
    async fn perform_capture(
        &self,
        var_name: &str,
        config: &CaptureConfig,
        command_result: &CommandResult,
    ) -> Result<CapturedVariable> {
        let (command_index, source, pattern, json_path, max_size, default, multiline) =
            config.extract_params();

        // Get raw output based on source
        let raw_output = self.get_output_by_source(&source, command_result)?;

        // Apply size limit
        let (limited_output, truncated) = self.apply_size_limit(&raw_output, max_size);

        // Apply pattern extraction if specified
        let pattern_output = if let Some(ref pattern_str) = pattern {
            match self.apply_pattern_extraction(&limited_output, pattern_str) {
                Ok(extracted) => extracted,
                Err(e) => {
                    if let Some(ref default_val) = default {
                        warn!("Pattern extraction failed for '{}': {}, using default", var_name, e);
                        default_val.clone()
                    } else {
                        return Err(e);
                    }
                }
            }
        } else {
            limited_output.clone()
        };

        // Handle multiline processing
        let processed_output = self.handle_multiline(&pattern_output, &multiline);

        // Parse JSON if json_path is specified
        let final_value = if let Some(ref jp) = json_path {
            match self.extract_json_value(&processed_output, jp) {
                Ok(val) => val,
                Err(e) => {
                    if let Some(ref default_val) = default {
                        warn!("JSON extraction failed for '{}': {}, using default", var_name, e);
                        Value::String(default_val.clone())
                    } else {
                        return Err(e);
                    }
                }
            }
        } else if multiline == MultilineHandling::Array {
            // If multiline is set to Array, parse as JSON array
            serde_json::from_str(&processed_output).unwrap_or_else(|_| Value::String(processed_output))
        } else {
            Value::String(processed_output)
        };

        Ok(CapturedVariable {
            name: var_name.to_string(),
            value: final_value,
            source_command: command_index,
            captured_at: Utc::now(),
            metadata: CaptureMetadata {
                source: source.clone(),
                original_size: raw_output.len(),
                truncated,
                pattern_matched: pattern.is_some(),
                parsing_successful: true,
            },
        })
    }

    /// Get output based on the capture source
    fn get_output_by_source(
        &self,
        source: &CaptureSource,
        result: &CommandResult,
    ) -> Result<String> {
        match source {
            CaptureSource::Stdout => Ok(result.stdout.clone()),
            CaptureSource::Stderr => Ok(result.stderr.clone()),
            CaptureSource::Both => Ok(format!("stdout:\n{}\nstderr:\n{}", result.stdout, result.stderr)),
            CaptureSource::Combined => {
                // Interleave stdout and stderr (simplified version)
                Ok(format!("{}{}", result.stdout, result.stderr))
            }
        }
    }

    /// Apply size limit to output
    fn apply_size_limit(&self, output: &str, max_size: usize) -> (String, bool) {
        if output.len() <= max_size {
            (output.to_string(), false)
        } else {
            // Try to cut at a line boundary if possible
            let truncated = if let Some(last_newline) = output[..max_size].rfind('\n') {
                &output[..last_newline]
            } else {
                &output[..max_size]
            };
            (truncated.to_string(), true)
        }
    }

    /// Apply regex pattern extraction
    fn apply_pattern_extraction(&self, input: &str, pattern: &str) -> Result<String> {
        let regex = Regex::new(pattern)
            .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        if let Some(captures) = regex.captures(input) {
            // If there are capture groups, use the first one, otherwise use the whole match
            if captures.len() > 1 {
                Ok(captures.get(1).expect("Capture group 1 exists").as_str().to_string())
            } else {
                Ok(captures.get(0).expect("Match exists").as_str().to_string())
            }
        } else {
            Err(anyhow!("Pattern '{}' did not match any text", pattern))
        }
    }

    /// Handle multiline output based on configuration
    fn handle_multiline(&self, input: &str, handling: &MultilineHandling) -> String {
        match handling {
            MultilineHandling::Preserve => input.to_string(),
            MultilineHandling::Join => input.lines().collect::<Vec<_>>().join(" "),
            MultilineHandling::FirstLine => input.lines().next().unwrap_or("").to_string(),
            MultilineHandling::LastLine => input.lines().last().unwrap_or("").to_string(),
            MultilineHandling::Array => {
                // Return a JSON array as string
                let lines: Vec<&str> = input.lines().collect();
                serde_json::to_string(&lines).unwrap_or_else(|_| input.to_string())
            }
        }
    }

    /// Extract value from JSON using JSONPath
    fn extract_json_value(&self, input: &str, json_path: &str) -> Result<Value> {
        let data: Value = serde_json::from_str(input)
            .with_context(|| format!("Failed to parse JSON from command output"))?;

        // Use simple path extraction for now (can be enhanced with full JSONPath library)
        extract_json_path(&data, json_path)
            .ok_or_else(|| anyhow!("JSONPath '{}' not found in data", json_path))
    }

    /// Get all captured variables
    pub fn get_captured_variables(&self) -> &HashMap<String, CapturedVariable> {
        &self.captured_variables
    }

    /// Get a specific variable value
    pub fn get_variable_value(&self, name: &str) -> Option<&Value> {
        self.captured_variables.get(name).map(|v| &v.value)
    }

    /// Export variables for use in context
    pub fn export_variables(&self) -> HashMap<String, String> {
        self.captured_variables
            .iter()
            .map(|(name, var)| {
                let value_str = match &var.value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => String::new(),
                    v => serde_json::to_string(v).unwrap_or_default(),
                };
                (name.clone(), value_str)
            })
            .collect()
    }
}

/// Extract a value from JSON using a simple path notation
/// Supports:
/// - Simple dot notation: "field.nested.value"
/// - Array indexing: "items[0]" or "items.0"
pub fn extract_json_path(json: &Value, path: &str) -> Option<Value> {
    let mut current = json;

    // Split path on dots
    let parts: Vec<&str> = path.split('.').collect();

    for part in parts {
        // Check for array indexing notation like "items[0]"
        if let Some(bracket_pos) = part.find('[') {
            if let Some(close_bracket) = part.find(']') {
                let field = &part[..bracket_pos];
                let index_str = &part[bracket_pos + 1..close_bracket];

                // Navigate to the field first if field is not empty
                if !field.is_empty() {
                    current = current.get(field)?;
                }

                // Then apply the index
                if let Ok(index) = index_str.parse::<usize>() {
                    current = current.get(index)?;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else if let Ok(index) = part.parse::<usize>() {
            // Handle pure numeric indices (for cases like "items.0")
            current = current.get(index)?;
        } else {
            // Regular field access
            current = current.get(part)?;
        }
    }

    Some(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_capture_config() {
        let config = CaptureConfig::Simple(0);
        assert_eq!(config.command_index(), 0);

        let (idx, source, pattern, json_path, max_size, default, multiline) = config.extract_params();
        assert_eq!(idx, 0);
        assert!(matches!(source, CaptureSource::Stdout));
        assert!(pattern.is_none());
        assert!(json_path.is_none());
        assert_eq!(max_size, 1024 * 1024);
        assert!(default.is_none());
        assert!(matches!(multiline, MultilineHandling::Preserve));
    }

    #[test]
    fn test_detailed_capture_config() {
        let config = CaptureConfig::Detailed {
            command_index: 1,
            source: CaptureSource::Stderr,
            pattern: Some(r"(\d+)".to_string()),
            json_path: Some("$.items[0]".to_string()),
            max_size: 512,
            default: Some("fallback".to_string()),
            multiline: MultilineHandling::FirstLine,
        };

        assert_eq!(config.command_index(), 1);
        let (idx, source, pattern, json_path, max_size, default, multiline) = config.extract_params();
        assert_eq!(idx, 1);
        assert!(matches!(source, CaptureSource::Stderr));
        assert_eq!(pattern, Some(r"(\d+)".to_string()));
        assert_eq!(json_path, Some("$.items[0]".to_string()));
        assert_eq!(max_size, 512);
        assert_eq!(default, Some("fallback".to_string()));
        assert!(matches!(multiline, MultilineHandling::FirstLine));
    }

    #[tokio::test]
    async fn test_variable_capture_simple() {
        let mut config = HashMap::new();
        config.insert("MY_VAR".to_string(), CaptureConfig::Simple(0));

        let mut engine = VariableCaptureEngine::new(config);

        let result = CommandResult {
            stdout: "test output".to_string(),
            stderr: String::new(),
            success: true,
            exit_code: Some(0),
        };

        engine.capture_from_command(0, &result).await.unwrap();

        let captured = engine.get_variable_value("MY_VAR").unwrap();
        assert_eq!(captured, &Value::String("test output".to_string()));
    }

    #[tokio::test]
    async fn test_variable_capture_with_pattern() {
        let mut config = HashMap::new();
        config.insert("COUNT".to_string(), CaptureConfig::Detailed {
            command_index: 0,
            source: CaptureSource::Stdout,
            pattern: Some(r"Total: (\d+)".to_string()),
            json_path: None,
            max_size: 1024,
            default: None,
            multiline: MultilineHandling::Preserve,
        });

        let mut engine = VariableCaptureEngine::new(config);

        let result = CommandResult {
            stdout: "Processing...\nTotal: 42\nDone.".to_string(),
            stderr: String::new(),
            success: true,
            exit_code: Some(0),
        };

        engine.capture_from_command(0, &result).await.unwrap();

        let captured = engine.get_variable_value("COUNT").unwrap();
        assert_eq!(captured, &Value::String("42".to_string()));
    }

    #[tokio::test]
    async fn test_variable_capture_with_json() {
        let mut config = HashMap::new();
        config.insert("STATUS".to_string(), CaptureConfig::Detailed {
            command_index: 0,
            source: CaptureSource::Stdout,
            pattern: None,
            json_path: Some("status.code".to_string()),
            max_size: 1024,
            default: None,
            multiline: MultilineHandling::Preserve,
        });

        let mut engine = VariableCaptureEngine::new(config);

        let result = CommandResult {
            stdout: r#"{"status": {"code": 200, "message": "OK"}}"#.to_string(),
            stderr: String::new(),
            success: true,
            exit_code: Some(0),
        };

        engine.capture_from_command(0, &result).await.unwrap();

        let captured = engine.get_variable_value("STATUS").unwrap();
        assert_eq!(captured, &json!(200));
    }

    #[test]
    fn test_json_path_extraction() {
        let data = json!({
            "items": [
                {"name": "first", "value": 1},
                {"name": "second", "value": 2}
            ],
            "nested": {
                "field": {
                    "value": "deep"
                }
            }
        });

        assert_eq!(extract_json_path(&data, "items[0].name"), Some(json!("first")));
        assert_eq!(extract_json_path(&data, "items.1.value"), Some(json!(2)));
        assert_eq!(extract_json_path(&data, "nested.field.value"), Some(json!("deep")));
        assert_eq!(extract_json_path(&data, "missing.field"), None);
    }

    #[test]
    fn test_multiline_handling() {
        let engine = VariableCaptureEngine::new(HashMap::new());
        let input = "line1\nline2\nline3";

        assert_eq!(
            engine.handle_multiline(input, &MultilineHandling::Preserve),
            "line1\nline2\nline3"
        );
        assert_eq!(
            engine.handle_multiline(input, &MultilineHandling::Join),
            "line1 line2 line3"
        );
        assert_eq!(
            engine.handle_multiline(input, &MultilineHandling::FirstLine),
            "line1"
        );
        assert_eq!(
            engine.handle_multiline(input, &MultilineHandling::LastLine),
            "line3"
        );
        assert_eq!(
            engine.handle_multiline(input, &MultilineHandling::Array),
            r#"["line1","line2","line3"]"#
        );
    }
}