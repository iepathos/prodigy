//! Variable interpolation engine for MapReduce workflows
//!
//! Provides template parsing and variable resolution for dynamic command generation
//! in MapReduce workflows. Supports nested property access, default values, and
//! multiple variable contexts.

use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing;

/// Interpolation engine for processing template strings
pub struct InterpolationEngine {
    /// Whether to fail on undefined variables
    strict_mode: bool,
    /// Cache of parsed templates
    cache: HashMap<String, Arc<Template>>,
    /// Regex for finding variable placeholders
    variable_regex: Regex,
}

impl Default for InterpolationEngine {
    fn default() -> Self {
        Self::new(false)
    }
}

impl InterpolationEngine {
    /// Create a new interpolation engine
    pub fn new(strict_mode: bool) -> Self {
        // Matches both ${variable} and $variable patterns
        // ${variable}, ${variable.path}, ${variable:-default} or $VAR (simple unbraced)
        let variable_regex =
            Regex::new(r"\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*)").expect("Invalid regex pattern");

        Self {
            strict_mode,
            cache: HashMap::new(),
            variable_regex,
        }
    }

    /// Interpolate a template string with the given context
    pub fn interpolate(
        &mut self,
        template_str: &str,
        context: &InterpolationContext,
    ) -> Result<String> {
        self.interpolate_with_debug(template_str, context, false)
    }

    /// Interpolate with enhanced debugging information
    pub fn interpolate_with_debug(
        &mut self,
        template_str: &str,
        context: &InterpolationContext,
        debug_mode: bool,
    ) -> Result<String> {
        let template = self.get_or_parse_template(template_str)?;

        self.resolve_template_segments(&template, context, debug_mode)
    }

    /// Get template from cache or parse new one
    fn get_or_parse_template(&mut self, template_str: &str) -> Result<Arc<Template>> {
        if let Some(cached) = self.cache.get(template_str) {
            Ok(cached.clone())
        } else {
            let parsed = Arc::new(self.parse_template(template_str)?);
            self.cache.insert(template_str.to_string(), parsed.clone());
            Ok(parsed)
        }
    }

    /// Resolve all template segments with enhanced error reporting
    fn resolve_template_segments(
        &self,
        template: &Template,
        context: &InterpolationContext,
        debug_mode: bool,
    ) -> Result<String> {
        let mut result = String::new();
        let mut failed_variables = Vec::new();

        for segment in &template.segments {
            match segment {
                Segment::Literal(text) => result.push_str(text),
                Segment::Variable { path, default } => {
                    match self.resolve_variable_with_context(path, context, debug_mode) {
                        Ok(value) => result.push_str(&value_to_string(&value)),
                        Err(resolution_error) => {
                            if let Some(default_value) = default {
                                if debug_mode {
                                    tracing::debug!(
                                        "Using default value '{}' for variable '{}' due to: {}",
                                        default_value,
                                        path.join("."),
                                        resolution_error
                                    );
                                }
                                result.push_str(default_value);
                            } else if self.strict_mode {
                                failed_variables
                                    .push((path.join("."), resolution_error.to_string()));
                            } else {
                                // In non-strict mode, leave placeholder as-is
                                let placeholder = format!("${{{}}}", path.join("."));
                                result.push_str(&placeholder);
                                if debug_mode {
                                    tracing::warn!(
                                        "Left placeholder '{}' unresolved: {}",
                                        placeholder,
                                        resolution_error
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Report all failed variables in strict mode
        if self.strict_mode && !failed_variables.is_empty() {
            let error_summary =
                self.build_interpolation_error_summary(&failed_variables, context, &template.raw);
            return Err(anyhow!("Variable interpolation failed: {}", error_summary));
        }

        Ok(result)
    }

    /// Resolve variable with enhanced context and debugging
    fn resolve_variable_with_context(
        &self,
        path: &[String],
        context: &InterpolationContext,
        debug_mode: bool,
    ) -> Result<Value> {
        let result = context.resolve_path(path);

        if debug_mode {
            match &result {
                Ok(value) => {
                    tracing::debug!(
                        "Resolved variable '{}' to: {}",
                        path.join("."),
                        match value {
                            serde_json::Value::String(s) => format!("\"{}\" (string)", s),
                            serde_json::Value::Number(n) => format!("{} (number)", n),
                            serde_json::Value::Bool(b) => format!("{} (bool)", b),
                            serde_json::Value::Array(arr) => format!("[...] ({} items)", arr.len()),
                            serde_json::Value::Object(obj) =>
                                format!("{{...}} ({} keys)", obj.len()),
                            serde_json::Value::Null => "null".to_string(),
                        }
                    );
                }
                Err(e) => {
                    tracing::debug!("Failed to resolve variable '{}': {}", path.join("."), e);
                }
            }
        }

        result
    }

    /// Build comprehensive error summary for failed interpolations
    fn build_interpolation_error_summary(
        &self,
        failed_variables: &[(String, String)],
        context: &InterpolationContext,
        template: &str,
    ) -> String {
        let mut summary = String::new();

        summary.push_str(&format!("\nTemplate: '{}'\n", template));

        summary.push_str("\nFailed variables:\n");
        for (var_name, error) in failed_variables {
            summary.push_str(&format!("  - {}: {}\n", var_name, error));
        }

        summary.push_str("\nAvailable variables:\n");
        let available = Self::get_available_variables_summary(context);
        for var in available {
            summary.push_str(&format!("  - {}\n", var));
        }

        summary
    }

    /// Get summary of available variables for debugging
    fn get_available_variables_summary(context: &InterpolationContext) -> Vec<String> {
        let mut variables = Vec::new();

        // Collect variables from current context
        for (key, value) in &context.variables {
            let type_info = match value {
                serde_json::Value::String(s) => format!("string({})", s.len()),
                serde_json::Value::Number(_) => "number".to_string(),
                serde_json::Value::Bool(_) => "bool".to_string(),
                serde_json::Value::Array(arr) => format!("array[{}]", arr.len()),
                serde_json::Value::Object(obj) => format!("object{{{}}}", obj.len()),
                serde_json::Value::Null => "null".to_string(),
            };
            variables.push(format!("{} ({})", key, type_info));
        }

        // Collect from parent context if present
        if let Some(parent) = &context.parent {
            let parent_vars = Self::get_available_variables_summary(parent);
            for var in parent_vars {
                variables.push(format!("{} (from parent)", var));
            }
        }

        variables.sort();
        variables
    }

    /// Parse a template string into segments
    fn parse_template(&self, template_str: &str) -> Result<Template> {
        let mut segments = Vec::new();
        let mut last_end = 0;

        for cap in self.variable_regex.captures_iter(template_str) {
            let full_match = cap.get(0).unwrap();

            // Determine which capture group matched: ${...} or $VAR
            let var_expr = if let Some(braced_match) = cap.get(1) {
                // ${variable} pattern
                braced_match.as_str()
            } else if let Some(unbraced_match) = cap.get(2) {
                // $VAR pattern
                unbraced_match.as_str()
            } else {
                continue; // Should never happen
            };

            // Add literal text before this variable
            if full_match.start() > last_end {
                segments.push(Segment::Literal(
                    template_str[last_end..full_match.start()].to_string(),
                ));
            }

            // Parse variable expression
            let (path, default) = self.parse_variable_expression(var_expr)?;
            segments.push(Segment::Variable { path, default });

            last_end = full_match.end();
        }

        // Add remaining literal text
        if last_end < template_str.len() {
            segments.push(Segment::Literal(template_str[last_end..].to_string()));
        }

        Ok(Template {
            raw: template_str.to_string(),
            segments,
        })
    }

    /// Parse a variable expression like "item.name" or "timeout:-600"
    fn parse_variable_expression(&self, expr: &str) -> Result<(Vec<String>, Option<String>)> {
        // Check for default value syntax (:-default)
        let (path_str, default) = if let Some(idx) = expr.find(":-") {
            let path = &expr[..idx];
            let default = &expr[idx + 2..];
            (path, Some(default.to_string()))
        } else {
            (expr, None)
        };

        // Parse path segments
        let path = self.parse_path(path_str)?;
        Ok((path, default))
    }

    /// Classify a character in the context of path parsing
    fn classify_path_char(ch: char, in_brackets: bool) -> PathCharType {
        match ch {
            '[' => PathCharType::BracketOpen,
            ']' => PathCharType::BracketClose,
            '.' if !in_brackets => PathCharType::Separator,
            _ => PathCharType::Regular,
        }
    }

    /// Parse a variable path into segments
    fn parse_path(&self, path_str: &str) -> Result<Vec<String>> {
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut in_brackets = false;

        for ch in path_str.chars() {
            match Self::classify_path_char(ch, in_brackets) {
                PathCharType::BracketOpen => {
                    if !current.is_empty() {
                        segments.push(current.clone());
                        current.clear();
                    }
                    in_brackets = true;
                    current.push(ch);
                }
                PathCharType::BracketClose => {
                    current.push(ch);
                    in_brackets = false;
                }
                PathCharType::Separator => {
                    if !current.is_empty() {
                        segments.push(current.clone());
                        current.clear();
                    }
                }
                PathCharType::Regular => current.push(ch),
            }
        }

        if !current.is_empty() {
            segments.push(current);
        }

        if segments.is_empty() {
            return Err(anyhow!("Empty variable path"));
        }

        Ok(segments)
    }
}

/// Template representation
#[derive(Debug, Clone)]
pub struct Template {
    /// Original template string
    pub raw: String,
    /// Parsed segments
    pub segments: Vec<Segment>,
}

/// Character type classification for path parsing
#[derive(Debug, Clone, Copy, PartialEq)]
enum PathCharType {
    BracketOpen,
    BracketClose,
    Separator,
    Regular,
}

/// Template segment
#[derive(Debug, Clone)]
pub enum Segment {
    /// Literal text
    Literal(String),
    /// Variable placeholder
    Variable {
        /// Path segments
        path: Vec<String>,
        /// Default value if variable is undefined
        default: Option<String>,
    },
}

/// Interpolation context containing variables
#[derive(Debug, Clone, Default)]
pub struct InterpolationContext {
    /// Variables in this context
    pub variables: HashMap<String, Value>,
    /// Parent context for scoping
    pub parent: Option<Box<InterpolationContext>>,
}

impl InterpolationContext {
    /// Create a new context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a child context
    pub fn child(&self) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Set a variable in the context
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.variables.insert(key.into(), value.into());
    }

    /// Set a variable from a JSON path
    pub fn set_path(&mut self, path: &[String], value: Value) {
        if path.is_empty() {
            return;
        }

        if path.len() == 1 {
            self.variables.insert(path[0].clone(), value);
            return;
        }

        // For nested paths, create the full path as a dotted key
        // This is simpler and avoids the unsafe transmute
        let key = path.join(".");
        self.variables.insert(key, value);
    }

    /// Resolve a path in the context
    pub fn resolve_path(&self, path: &[String]) -> Result<Value> {
        if path.is_empty() {
            return Err(anyhow!("Empty path"));
        }

        // Try to resolve in current context
        let result = self.resolve_in_current(path);

        // If not found and we have a parent, try parent
        if result.is_err() {
            if let Some(parent) = &self.parent {
                return parent.resolve_path(path);
            }
        }

        result
    }

    /// Resolve a path in the current context only
    fn resolve_in_current(&self, path: &[String]) -> Result<Value> {
        if path.is_empty() {
            return Err(anyhow!("Empty path"));
        }

        let root_value = self.get_root_variable(&path[0])?;
        Self::resolve_path_in_value(root_value, &path[1..])
    }

    /// Get root variable from context (pure function)
    fn get_root_variable(&self, root_key: &str) -> Result<Value> {
        self.variables
            .get(root_key)
            .cloned()
            .ok_or_else(|| anyhow!("Variable '{}' not found", root_key))
    }

    /// Resolve remaining path segments in a JSON value (pure function)
    fn resolve_path_in_value(mut current: Value, path: &[String]) -> Result<Value> {
        for segment in path {
            current = if Self::is_array_index(segment) {
                Self::resolve_array_index(current, segment)?
            } else {
                Self::resolve_property_access(current, segment)?
            };
        }
        Ok(current)
    }

    /// Check if segment is an array index like "[0]" (pure function)
    fn is_array_index(segment: &str) -> bool {
        segment.starts_with('[') && segment.ends_with(']')
    }

    /// Resolve array indexing (pure function)
    fn resolve_array_index(value: Value, segment: &str) -> Result<Value> {
        let index_str = &segment[1..segment.len() - 1];
        let index: usize = index_str
            .parse()
            .map_err(|_| anyhow!("Invalid array index: {}", index_str))?;

        match value {
            Value::Array(arr) => arr
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("Array index {} out of bounds", index)),
            _ => Err(anyhow!("Cannot index non-array with [{}]", index)),
        }
    }

    /// Resolve property access (pure function)
    fn resolve_property_access(value: Value, property: &str) -> Result<Value> {
        match value {
            Value::Object(map) => map
                .get(property)
                .cloned()
                .ok_or_else(|| anyhow!("Property '{}' not found", property)),
            _ => Err(anyhow!(
                "Cannot access property '{}' on non-object",
                property
            )),
        }
    }

    /// Add variables from a JSON object
    pub fn add_json_object(&mut self, prefix: &str, obj: &serde_json::Map<String, Value>) {
        for (key, value) in obj {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            self.variables.insert(full_key, value.clone());
        }
    }

    /// Merge another context into this one
    pub fn merge(&mut self, other: &InterpolationContext) {
        for (key, value) in &other.variables {
            self.variables.insert(key.clone(), value.clone());
        }
    }
}

/// Convert a JSON value to a string representation
fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            // For arrays of strings, join with commas for readability
            if arr.iter().all(|v| matches!(v, Value::String(_))) {
                let strings: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                strings.join(", ")
            } else {
                // For mixed arrays, use JSON representation
                serde_json::to_string(value).unwrap_or_else(|_| String::new())
            }
        }
        Value::Object(_) => {
            // For objects, use compact JSON representation
            serde_json::to_string(value).unwrap_or_else(|_| String::new())
        }
    }
}

/// Escape a string for safe use in shell commands
pub fn shell_escape(s: &str) -> String {
    // Simple shell escaping - wrap in single quotes and escape single quotes
    if s.is_empty() {
        return "''".to_string();
    }

    // Check if escaping is needed
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/')
    {
        return s.to_string();
    }

    // Escape single quotes by ending quote, adding escaped quote, and starting new quote
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

#[cfg(test)]
mod mapreduce_variable_tests {
    use super::*;
    use serde_json::json;

    /// Test that InterpolationEngine handles both ${VAR} and $VAR syntax
    #[test]
    fn test_mixed_variable_syntax() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();
        context.set("user", Value::String("alice".to_string()));
        context.set("count", Value::Number(42.into()));

        let template = "User $user processed ${count} items";
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, "User alice processed 42 items");
    }

    /// Test array handling in string conversion (important for MapReduce variables)
    #[test]
    fn test_array_to_string_conversion() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();

        // Test string array - should be comma-separated
        context.set(
            "missing_items",
            json!(["test coverage", "documentation", "error handling"]),
        );

        let template = "Missing: ${missing_items}";
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(
            result,
            "Missing: test coverage, documentation, error handling"
        );

        // Test mixed array - should be JSON
        context.set("mixed_data", json!(["string", 123, true]));

        let template2 = "Data: ${mixed_data}";
        let result2 = engine.interpolate(template2, &context).unwrap();
        assert_eq!(result2, r#"Data: ["string",123,true]"#);
    }

    /// Test nested object access for MapReduce variables
    #[test]
    fn test_nested_mapreduce_variables() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();

        // Set up map object like MapReduce would
        context.set(
            "map",
            json!({
                "successful": 5,
                "failed": 2,
                "total": 7
            }),
        );

        let template = "Processed ${map.total}: ${map.successful} ok, ${map.failed} failed";
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, "Processed 7: 5 ok, 2 failed");
    }

    /// Test unbraced variable parsing edge cases
    #[test]
    fn test_unbraced_variable_edge_cases() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();
        context.set("PATH", Value::String("/usr/bin".to_string()));
        context.set("HOME", Value::String("/home/user".to_string()));

        // Test multiple unbraced variables
        let template = "PATH=$PATH HOME=$HOME";
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, "PATH=/usr/bin HOME=/home/user");

        // Test unbraced variable at end of string
        let template2 = "Current path: $PATH";
        let result2 = engine.interpolate(template2, &context).unwrap();
        assert_eq!(result2, "Current path: /usr/bin");

        // Test unbraced variable at start
        let template3 = "$HOME/documents";
        let result3 = engine.interpolate(template3, &context).unwrap();
        assert_eq!(result3, "/home/user/documents");
    }

    /// Test that the specific MapReduce bug scenario works
    #[test]
    fn test_mapreduce_interpolation_bug_fix() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();

        // Set up context as it would be in reduce phase
        context.set(
            "map",
            json!({
                "successful": 8,
                "failed": 2,
                "total": 10
            }),
        );

        // Test the exact template that was failing
        let shell_template =
            "echo 'Total: ${map.total}, Success: ${map.successful}, Failed: ${map.failed}'";
        let shell_result = engine.interpolate(shell_template, &context).unwrap();
        assert_eq!(shell_result, "echo 'Total: 10, Success: 8, Failed: 2'");

        // Test commit message template
        let commit_template = r#"git commit -m "Processed ${map.successful} items

Total items: ${map.total}
Failed items: ${map.failed}""#;

        let commit_result = engine.interpolate(commit_template, &context).unwrap();
        assert!(commit_result.contains("Processed 8 items"));
        assert!(commit_result.contains("Total items: 10"));
        assert!(commit_result.contains("Failed items: 2"));
    }

    /// Test empty array handling
    #[test]
    fn test_empty_array_handling() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();

        context.set("empty_list", json!([]));

        let template = "Items: ${empty_list}";
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, "Items: ");
    }

    /// Test single item array handling  
    #[test]
    fn test_single_item_array() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();

        context.set("single_item", json!(["only item"]));

        let template = "Item: ${single_item}";
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, "Item: only item");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_interpolation() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();
        context.set("name", json!("Alice"));
        context.set("age", json!(30));

        let result = engine
            .interpolate("Hello, ${name}! You are ${age} years old.", &context)
            .unwrap();
        assert_eq!(result, "Hello, Alice! You are 30 years old.");
    }

    #[test]
    fn test_nested_property_access() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();
        context.set(
            "user",
            json!({
                "name": "Bob",
                "address": {
                    "city": "New York",
                    "zip": "10001"
                }
            }),
        );

        let result = engine
            .interpolate("${user.name} lives in ${user.address.city}", &context)
            .unwrap();
        assert_eq!(result, "Bob lives in New York");
    }

    #[test]
    fn test_array_indexing() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();
        context.set("items", json!(["apple", "banana", "cherry"]));

        let result = engine
            .interpolate("First item: ${items[0]}, Last item: ${items[2]}", &context)
            .unwrap();
        assert_eq!(result, "First item: apple, Last item: cherry");
    }

    #[test]
    fn test_default_values() {
        let mut engine = InterpolationEngine::new(false);
        let context = InterpolationContext::new();

        let result = engine
            .interpolate("Timeout: ${timeout:-600} seconds", &context)
            .unwrap();
        assert_eq!(result, "Timeout: 600 seconds");
    }

    #[test]
    fn test_undefined_variable_strict_mode() {
        let mut engine = InterpolationEngine::new(true);
        let context = InterpolationContext::new();

        let result = engine.interpolate("Value: ${undefined}", &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_undefined_variable_non_strict() {
        let mut engine = InterpolationEngine::new(false);
        let context = InterpolationContext::new();

        let result = engine.interpolate("Value: ${undefined}", &context).unwrap();
        assert_eq!(result, "Value: ${undefined}");
    }

    #[test]
    fn test_context_inheritance() {
        let mut engine = InterpolationEngine::new(false);
        let mut parent = InterpolationContext::new();
        parent.set("global", json!("parent_value"));

        let mut child = parent.child();
        child.set("local", json!("child_value"));

        let result = engine
            .interpolate("${global} and ${local}", &child)
            .unwrap();
        assert_eq!(result, "parent_value and child_value");
    }

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("simple"), "simple");
        assert_eq!(shell_escape("with spaces"), "'with spaces'");
        assert_eq!(shell_escape("with'quote"), "'with'\\''quote'");
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn test_complex_json_interpolation() {
        let mut engine = InterpolationEngine::new(false);
        let mut context = InterpolationContext::new();
        context.set(
            "item",
            json!({
                "id": 123,
                "description": "Fix bug in parser",
                "priority": "high",
                "tags": ["bug", "parser"],
                "metadata": {
                    "created": "2024-01-01",
                    "author": "dev@example.com"
                }
            }),
        );

        let template =
            "Task ${item.id}: ${item.description} [${item.priority}] by ${item.metadata.author}";
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(
            result,
            "Task 123: Fix bug in parser [high] by dev@example.com"
        );
    }

    #[test]
    fn test_literal_segments() {
        let engine = InterpolationEngine::new(false);
        let template = engine.parse_template("No variables here!").unwrap();
        assert_eq!(template.segments.len(), 1);
        match &template.segments[0] {
            Segment::Literal(text) => assert_eq!(text, "No variables here!"),
            _ => panic!("Expected literal segment"),
        }
    }

    #[test]
    fn test_mixed_segments() {
        let engine = InterpolationEngine::new(false);
        let template = engine
            .parse_template("Hello ${name}, you have ${count} messages")
            .unwrap();
        // Should have 5 segments: "Hello ", ${name}, ", you have ", ${count}, " messages"
        assert_eq!(template.segments.len(), 5);
    }

    #[test]
    fn test_parse_path_simple() {
        let engine = InterpolationEngine::new(false);
        let result = engine.parse_path("simple").unwrap();
        assert_eq!(result, vec!["simple"]);
    }

    #[test]
    fn test_parse_path_dotted() {
        let engine = InterpolationEngine::new(false);
        let result = engine.parse_path("user.address.city").unwrap();
        assert_eq!(result, vec!["user", "address", "city"]);
    }

    #[test]
    fn test_parse_path_with_brackets() {
        let engine = InterpolationEngine::new(false);
        let result = engine.parse_path("items[0]").unwrap();
        assert_eq!(result, vec!["items", "[0]"]);
    }

    #[test]
    fn test_parse_path_complex() {
        let engine = InterpolationEngine::new(false);
        let result = engine.parse_path("data.items[0].name").unwrap();
        assert_eq!(result, vec!["data", "items", "[0]", "name"]);
    }

    #[test]
    fn test_parse_path_dot_in_brackets() {
        let engine = InterpolationEngine::new(false);
        let result = engine.parse_path("map[key.with.dots]").unwrap();
        assert_eq!(result, vec!["map", "[key.with.dots]"]);
    }

    #[test]
    fn test_parse_path_empty_error() {
        let engine = InterpolationEngine::new(false);
        let result = engine.parse_path("");
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_path_char() {
        // Test classification of various characters
        assert_eq!(
            InterpolationEngine::classify_path_char('[', false),
            PathCharType::BracketOpen
        );
        assert_eq!(
            InterpolationEngine::classify_path_char(']', false),
            PathCharType::BracketClose
        );
        assert_eq!(
            InterpolationEngine::classify_path_char('.', false),
            PathCharType::Separator
        );
        assert_eq!(
            InterpolationEngine::classify_path_char('.', true),
            PathCharType::Regular
        );
        assert_eq!(
            InterpolationEngine::classify_path_char('a', false),
            PathCharType::Regular
        );
    }
}
