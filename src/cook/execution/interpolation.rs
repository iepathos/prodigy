//! Variable interpolation engine for MapReduce workflows
//!
//! Provides template parsing and variable resolution for dynamic command generation
//! in MapReduce workflows. Supports nested property access, default values, and
//! multiple variable contexts.

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

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
        // Matches ${variable}, ${variable.path}, ${variable:-default}
        let variable_regex = Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex pattern");

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
        // Check cache for parsed template
        let template = if let Some(cached) = self.cache.get(template_str) {
            cached.clone()
        } else {
            let parsed = Arc::new(self.parse_template(template_str)?);
            self.cache.insert(template_str.to_string(), parsed.clone());
            parsed
        };

        // Resolve all segments
        let mut result = String::new();
        for segment in &template.segments {
            match segment {
                Segment::Literal(text) => result.push_str(text),
                Segment::Variable { path, default } => {
                    match self.resolve_variable(path, context) {
                        Ok(value) => result.push_str(&value_to_string(&value)),
                        Err(_) if default.is_some() => {
                            result.push_str(default.as_ref().unwrap());
                        }
                        Err(e) if self.strict_mode => {
                            return Err(e).context(format!(
                                "Failed to resolve variable: {}",
                                path.join(".")
                            ));
                        }
                        Err(_) => {
                            // In non-strict mode, leave placeholder as-is
                            result.push_str(&format!("${{{}}}", path.join(".")));
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// Parse a template string into segments
    fn parse_template(&self, template_str: &str) -> Result<Template> {
        let mut segments = Vec::new();
        let mut last_end = 0;

        for cap in self.variable_regex.captures_iter(template_str) {
            let full_match = cap.get(0).unwrap();
            let var_expr = cap.get(1).unwrap().as_str();

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

    /// Parse a variable path into segments
    fn parse_path(&self, path_str: &str) -> Result<Vec<String>> {
        let mut segments = Vec::new();

        // Split by dots, but handle array indexing
        let mut current = String::new();
        let mut in_brackets = false;

        for ch in path_str.chars() {
            match ch {
                '[' => {
                    if !current.is_empty() {
                        segments.push(current.clone());
                        current.clear();
                    }
                    in_brackets = true;
                    current.push(ch);
                }
                ']' => {
                    current.push(ch);
                    in_brackets = false;
                }
                '.' if !in_brackets => {
                    if !current.is_empty() {
                        segments.push(current.clone());
                        current.clear();
                    }
                }
                _ => current.push(ch),
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

    /// Resolve a variable from the context
    fn resolve_variable(&self, path: &[String], context: &InterpolationContext) -> Result<Value> {
        context.resolve_path(path)
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
        let mut current = if let Some(root_val) = self.variables.get(&path[0]) {
            root_val.clone()
        } else {
            return Err(anyhow!("Variable '{}' not found", path[0]));
        };

        for segment in &path[1..] {
            // Handle array indexing
            if segment.starts_with('[') && segment.ends_with(']') {
                let index_str = &segment[1..segment.len() - 1];
                let index: usize = index_str
                    .parse()
                    .map_err(|_| anyhow!("Invalid array index: {}", index_str))?;

                current = match current {
                    Value::Array(arr) => arr
                        .get(index)
                        .ok_or_else(|| anyhow!("Array index {} out of bounds", index))?
                        .clone(),
                    _ => return Err(anyhow!("Cannot index non-array with [{}]", index)),
                };
            } else {
                // Regular property access
                current = match current {
                    Value::Object(map) => map
                        .get(segment)
                        .ok_or_else(|| anyhow!("Property '{}' not found", segment))?
                        .clone(),
                    _ => {
                        return Err(anyhow!(
                            "Cannot access property '{}' on non-object",
                            segment
                        ))
                    }
                };
            }
        }

        Ok(current)
    }

    /// Add variables from a JSON object
    pub fn add_json_object(&mut self, prefix: &str, obj: &serde_json::Map<String, Value>) {
        for (key, value) in obj {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
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
        Value::Array(_) | Value::Object(_) => {
            // For complex types, use compact JSON representation
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
}
