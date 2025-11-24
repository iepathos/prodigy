//! Output parsing functions for workflow command results
//!
//! Provides pure functions for extracting variables from command output:
//! - `parse_output_variables`: Extract variables using multiple patterns
//! - `OutputPattern`: Enum defining extraction patterns (regex, JSON, line)
//!
//! # Examples
//!
//! ```
//! use prodigy::cook::workflow::pure::output_parser::{parse_output_variables, OutputPattern};
//! use regex::Regex;
//!
//! let output = "Result: success\nValue: 42";
//! let patterns = vec![
//!     OutputPattern::Regex {
//!         name: "result".into(),
//!         regex: Regex::new(r"Result: (\w+)").unwrap(),
//!     },
//!     OutputPattern::Line {
//!         name: "second_line".into(),
//!         line_number: 1,
//!     },
//! ];
//!
//! let vars = parse_output_variables(output, &patterns);
//! assert_eq!(vars.get("result").unwrap(), "success");
//! assert_eq!(vars.get("second_line").unwrap(), "Value: 42");
//! ```

use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

/// Pattern types for extracting variables from command output
#[derive(Debug, Clone)]
pub enum OutputPattern {
    /// Extract using a regex pattern with capture group
    Regex {
        /// Name of the variable to extract
        name: String,
        /// Regex pattern with at least one capture group
        regex: Regex,
    },
    /// Extract using a JSON path expression
    Json {
        /// Name of the variable to extract
        name: String,
        /// JSON path expression (e.g., "$.field" or "$.field.nested")
        json_path: String,
    },
    /// Extract a specific line by line number (0-indexed)
    Line {
        /// Name of the variable to extract
        name: String,
        /// Line number to extract (0-indexed)
        line_number: usize,
    },
}

/// Pure: Extract variables from command output using patterns
///
/// Applies each pattern to the output and collects successful matches
/// into a HashMap. Patterns that don't match are silently skipped.
///
/// # Arguments
///
/// * `output` - The command output to parse
/// * `patterns` - A slice of patterns to apply
///
/// # Returns
///
/// A HashMap of variable names to extracted values
///
/// # Examples
///
/// ```
/// use prodigy::cook::workflow::pure::output_parser::{parse_output_variables, OutputPattern};
/// use regex::Regex;
///
/// let output = r#"{"status": "ok", "count": 10}"#;
/// let patterns = vec![
///     OutputPattern::Json {
///         name: "status".into(),
///         json_path: "$.status".into(),
///     },
/// ];
///
/// let vars = parse_output_variables(output, &patterns);
/// assert_eq!(vars.get("status").unwrap(), "ok");
/// ```
pub fn parse_output_variables(output: &str, patterns: &[OutputPattern]) -> HashMap<String, String> {
    patterns
        .iter()
        .filter_map(|pattern| extract_match(output, pattern))
        .collect()
}

/// Pure: Extract single variable match from output
fn extract_match(output: &str, pattern: &OutputPattern) -> Option<(String, String)> {
    match pattern {
        OutputPattern::Regex { name, regex } => regex
            .captures(output)
            .and_then(|cap| cap.get(1).map(|m| (name.clone(), m.as_str().to_string()))),
        OutputPattern::Json { name, json_path } => {
            extract_json_path(output, json_path).map(|value| (name.clone(), value))
        }
        OutputPattern::Line { name, line_number } => output
            .lines()
            .nth(*line_number)
            .map(|line| (name.clone(), line.to_string())),
    }
}

/// Pure: Extract value from JSON using a simple JSON path
///
/// Supports paths like:
/// - `$` - root value
/// - `$.field` - direct field access
/// - `$.field.nested` - nested field access
///
/// Does NOT support array indexing (e.g., `$.items[0]`) - use regex for that.
fn extract_json_path(json_str: &str, path: &str) -> Option<String> {
    let value: Value = serde_json::from_str(json_str).ok()?;

    // Handle empty path or just "$"
    if !path.starts_with('$') {
        return None;
    }

    let path = &path[1..]; // Remove $
    if path.is_empty() {
        return Some(value_to_string(&value));
    }

    // Remove leading dot if present
    let path = path.strip_prefix('.').unwrap_or(path);
    if path.is_empty() {
        return Some(value_to_string(&value));
    }

    // Navigate through the JSON structure
    let mut current = &value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }

    Some(value_to_string(current))
}

/// Convert JSON value to string representation
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_output_regex() {
        let output = "Result: success\nValue: 42";
        let patterns = vec![
            OutputPattern::Regex {
                name: "result".into(),
                regex: Regex::new(r"Result: (\w+)").unwrap(),
            },
            OutputPattern::Regex {
                name: "value".into(),
                regex: Regex::new(r"Value: (\d+)").unwrap(),
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert_eq!(vars.get("result").unwrap(), "success");
        assert_eq!(vars.get("value").unwrap(), "42");
    }

    #[test]
    fn test_parse_output_json() {
        let output = r#"{"status": "ok", "count": 10}"#;
        let patterns = vec![
            OutputPattern::Json {
                name: "status".into(),
                json_path: "$.status".into(),
            },
            OutputPattern::Json {
                name: "count".into(),
                json_path: "$.count".into(),
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert_eq!(vars.get("status").unwrap(), "ok");
        assert_eq!(vars.get("count").unwrap(), "10");
    }

    #[test]
    fn test_parse_output_line() {
        let output = "Line 0\nLine 1\nLine 2";
        let patterns = vec![
            OutputPattern::Line {
                name: "first".into(),
                line_number: 0,
            },
            OutputPattern::Line {
                name: "second".into(),
                line_number: 1,
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert_eq!(vars.get("first").unwrap(), "Line 0");
        assert_eq!(vars.get("second").unwrap(), "Line 1");
    }

    #[test]
    fn test_parse_output_no_match() {
        let output = "No matches here";
        let patterns = vec![OutputPattern::Regex {
            name: "missing".into(),
            regex: Regex::new(r"Result: (\w+)").unwrap(),
        }];

        let vars = parse_output_variables(output, &patterns);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_parse_output_mixed_patterns() {
        let output = "Status: active\n{\"count\": 5}\nThird line";
        let patterns = vec![
            OutputPattern::Regex {
                name: "status".into(),
                regex: Regex::new(r"Status: (\w+)").unwrap(),
            },
            OutputPattern::Line {
                name: "json_line".into(),
                line_number: 1,
            },
            OutputPattern::Line {
                name: "third".into(),
                line_number: 2,
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert_eq!(vars.get("status").unwrap(), "active");
        assert_eq!(vars.get("json_line").unwrap(), r#"{"count": 5}"#);
        assert_eq!(vars.get("third").unwrap(), "Third line");
    }

    #[test]
    fn test_extract_json_path_nested() {
        let json = r#"{"user": {"name": "Alice", "age": 30}}"#;

        let result = extract_json_path(json, "$.user.name");

        assert_eq!(result.unwrap(), "Alice");
    }

    #[test]
    fn test_extract_json_path_root() {
        let json = r#"{"value": 42}"#;

        let result = extract_json_path(json, "$");

        assert!(result.is_some());
        assert!(result.unwrap().contains("42"));
    }

    #[test]
    fn test_extract_json_path_invalid() {
        let json = r#"{"user": {"name": "Alice"}}"#;

        let result = extract_json_path(json, "$.user.missing");

        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_path_invalid_json() {
        let not_json = "this is not json";

        let result = extract_json_path(not_json, "$.field");

        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_path_no_dollar() {
        let json = r#"{"field": "value"}"#;

        // Path must start with $
        let result = extract_json_path(json, "field");

        assert!(result.is_none());
    }

    #[test]
    fn test_parse_output_empty_patterns() {
        let output = "Some output";
        let patterns: Vec<OutputPattern> = vec![];

        let vars = parse_output_variables(output, &patterns);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_parse_output_empty_output() {
        let output = "";
        let patterns = vec![OutputPattern::Line {
            name: "first".into(),
            line_number: 0,
        }];

        let vars = parse_output_variables(output, &patterns);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_parse_output_line_out_of_bounds() {
        let output = "Line 0\nLine 1";
        let patterns = vec![OutputPattern::Line {
            name: "missing".into(),
            line_number: 5,
        }];

        let vars = parse_output_variables(output, &patterns);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_json_path_boolean() {
        let json = r#"{"active": true}"#;

        let result = extract_json_path(json, "$.active");

        assert_eq!(result.unwrap(), "true");
    }

    #[test]
    fn test_extract_json_path_null() {
        let json = r#"{"value": null}"#;

        let result = extract_json_path(json, "$.value");

        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_extract_json_path_array() {
        let json = r#"{"items": [1, 2, 3]}"#;

        let result = extract_json_path(json, "$.items");

        assert!(result.is_some());
        let value = result.unwrap();
        assert!(value.contains("[1,2,3]") || value.contains("[1, 2, 3]"));
    }

    #[test]
    fn test_extract_json_path_object() {
        let json = r#"{"nested": {"a": 1}}"#;

        let result = extract_json_path(json, "$.nested");

        assert!(result.is_some());
        let value = result.unwrap();
        assert!(value.contains("\"a\"") && value.contains("1"));
    }

    #[test]
    fn test_parse_output_regex_no_capture_group() {
        let output = "Result: success";
        // Regex without capture group - should not match anything
        let patterns = vec![OutputPattern::Regex {
            name: "result".into(),
            regex: Regex::new(r"Result: \w+").unwrap(),
        }];

        let vars = parse_output_variables(output, &patterns);

        // No capture group means no match
        assert!(vars.is_empty());
    }

    #[test]
    fn test_parse_output_regex_multiple_capture_groups() {
        let output = "Name: Alice Age: 30";
        // Only first capture group is used
        let patterns = vec![OutputPattern::Regex {
            name: "data".into(),
            regex: Regex::new(r"Name: (\w+) Age: (\d+)").unwrap(),
        }];

        let vars = parse_output_variables(output, &patterns);

        // Should capture first group only
        assert_eq!(vars.get("data").unwrap(), "Alice");
    }

    #[test]
    fn test_parse_output_duplicate_names() {
        let output = "Line 0\nLine 1";
        // Two patterns with same name - last one wins
        let patterns = vec![
            OutputPattern::Line {
                name: "line".into(),
                line_number: 0,
            },
            OutputPattern::Line {
                name: "line".into(),
                line_number: 1,
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        // HashMap behavior - one of them wins
        assert!(vars.contains_key("line"));
        let value = vars.get("line").unwrap();
        assert!(value == "Line 0" || value == "Line 1");
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_parse_output_is_deterministic(
            output in ".*",
            line_num in 0usize..10,
        ) {
            let patterns = vec![OutputPattern::Line {
                name: "line".into(),
                line_number: line_num,
            }];

            let result1 = parse_output_variables(&output, &patterns);
            let result2 = parse_output_variables(&output, &patterns);

            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_parse_output_empty_patterns_returns_empty(output in ".*") {
            let patterns: Vec<OutputPattern> = vec![];

            let result = parse_output_variables(&output, &patterns);

            prop_assert!(result.is_empty());
        }

        #[test]
        fn prop_line_extraction_valid_line(
            lines in prop::collection::vec("[^\n]+", 1..5),  // Use non-empty lines to avoid edge cases
            line_idx in 0usize..5,
        ) {
            let output = lines.join("\n");
            let patterns = vec![OutputPattern::Line {
                name: "line".into(),
                line_number: line_idx,
            }];

            let result = parse_output_variables(&output, &patterns);

            // Note: Rust's lines() iterator doesn't return a trailing empty line
            // So we compare against the actual lines iterator behavior
            let actual_line = output.lines().nth(line_idx);

            match actual_line {
                Some(expected) => {
                    prop_assert_eq!(result.get("line"), Some(&expected.to_string()));
                }
                None => {
                    prop_assert!(result.is_empty());
                }
            }
        }
    }
}
