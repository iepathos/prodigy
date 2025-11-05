//! Parameter parsing utilities for workflow templates
//!
//! This module provides functions for parsing CLI parameters and parameter files
//! for use with workflow templates.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Parse CLI parameters in KEY=VALUE format
///
/// Supports automatic type inference:
/// - Numbers (integer and float)
/// - Booleans (true/false)
/// - Strings (default)
pub fn parse_cli_params(params: Vec<String>) -> Result<HashMap<String, Value>> {
    let mut result = HashMap::new();

    for param in params {
        let parts: Vec<&str> = param.splitn(2, '=').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid parameter format: '{}'. Expected KEY=VALUE", param);
        }

        let key = parts[0].to_string();
        let value = parse_param_value(parts[1])?;

        result.insert(key, value);
    }

    Ok(result)
}

/// Load parameters from a JSON or YAML file
pub async fn load_param_file(path: &Path) -> Result<HashMap<String, Value>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read parameter file: {}", path.display()))?;

    // Try JSON first
    if let Ok(params) = serde_json::from_str::<HashMap<String, Value>>(&content) {
        return Ok(params);
    }

    // Try YAML
    serde_yaml::from_str::<HashMap<String, Value>>(&content)
        .with_context(|| "Failed to parse parameter file as JSON or YAML")
}

/// Parse a parameter value with type inference
fn parse_param_value(value: &str) -> Result<Value> {
    // Try to parse as number
    if let Ok(num) = value.parse::<i64>() {
        return Ok(Value::Number(num.into()));
    }

    // Try to parse as float
    if let Ok(num) = value.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(num) {
            return Ok(Value::Number(num));
        }
    }

    // Try to parse as boolean
    if let Ok(b) = value.parse::<bool>() {
        return Ok(Value::Bool(b));
    }

    // Default to string
    Ok(Value::String(value.to_string()))
}

/// Merge CLI parameters with file parameters
///
/// CLI parameters take precedence over file parameters
pub fn merge_params(
    cli_params: HashMap<String, Value>,
    file_params: HashMap<String, Value>,
) -> HashMap<String, Value> {
    let mut result = file_params;

    // CLI params override file params
    for (key, value) in cli_params {
        result.insert(key, value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cli_params() {
        let params = vec![
            "target=src/main.rs".to_string(),
            "timeout=300".to_string(),
            "verbose=true".to_string(),
        ];

        let result = parse_cli_params(params).unwrap();

        assert_eq!(
            result.get("target"),
            Some(&Value::String("src/main.rs".to_string()))
        );
        assert_eq!(result.get("timeout"), Some(&Value::Number(300.into())));
        assert_eq!(result.get("verbose"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_parse_param_value_types() {
        assert_eq!(parse_param_value("123").unwrap(), Value::Number(123.into()));
        assert_eq!(
            parse_param_value("42.5").unwrap(),
            Value::Number(serde_json::Number::from_f64(42.5).unwrap())
        );
        assert_eq!(parse_param_value("true").unwrap(), Value::Bool(true));
        assert_eq!(parse_param_value("false").unwrap(), Value::Bool(false));
        assert_eq!(
            parse_param_value("hello").unwrap(),
            Value::String("hello".to_string())
        );
    }

    #[test]
    fn test_merge_params() {
        let mut cli_params = HashMap::new();
        cli_params.insert("key1".to_string(), Value::String("cli_value".to_string()));
        cli_params.insert("key2".to_string(), Value::Number(123.into()));

        let mut file_params = HashMap::new();
        file_params.insert("key1".to_string(), Value::String("file_value".to_string()));
        file_params.insert("key3".to_string(), Value::Bool(true));

        let result = merge_params(cli_params, file_params);

        // CLI value should override file value for key1
        assert_eq!(
            result.get("key1"),
            Some(&Value::String("cli_value".to_string()))
        );
        // key2 from CLI should be present
        assert_eq!(result.get("key2"), Some(&Value::Number(123.into())));
        // key3 from file should be present
        assert_eq!(result.get("key3"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_invalid_param_format() {
        let params = vec!["invalid".to_string()];
        let result = parse_cli_params(params);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid parameter format"));
    }
}
