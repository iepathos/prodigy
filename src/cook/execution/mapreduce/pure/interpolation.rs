//! Pure functions for variable interpolation
//!
//! These functions handle building variable contexts from JSON items,
//! flattening nested objects, and extracting variable references.

use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::LazyLock;

// Static regex patterns for variable extraction
static BRACED_VAR_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex pattern"));
static UNBRACED_VAR_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").expect("Invalid regex pattern"));

/// Build item variables from JSON value
///
/// # Arguments
///
/// * `item` - JSON item to extract variables from
/// * `item_id` - ID to assign to the item
///
/// # Returns
///
/// HashMap of variable names to string values
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::mapreduce::pure::interpolation::build_item_variables;
/// use serde_json::json;
///
/// let item = json!({"name": "test", "priority": 5});
/// let vars = build_item_variables(&item, "item-1");
/// assert_eq!(vars.get("item.id").unwrap(), "item-1");
/// assert_eq!(vars.get("item.name").unwrap(), "test");
/// assert_eq!(vars.get("item.priority").unwrap(), "5");
/// ```
pub fn build_item_variables(item: &Value, item_id: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Always include item ID
    vars.insert("item.id".to_string(), item_id.to_string());

    // Flatten JSON object to variables
    if let Some(obj) = item.as_object() {
        for (key, value) in obj {
            let var_name = format!("item.{}", key);
            if let Some(string_value) = value_to_string(value) {
                vars.insert(var_name, string_value);
            }
        }
    }

    vars
}

/// Convert JSON value to string
fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".to_string()),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).ok(),
    }
}

/// Flatten nested JSON object to dot-notation variables
///
/// # Arguments
///
/// * `obj` - JSON object to flatten
///
/// # Returns
///
/// HashMap of flattened variable names to string values
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::mapreduce::pure::interpolation::flatten_json_to_vars;
/// use serde_json::json;
///
/// let obj = json!({"user": {"name": "Alice", "age": 30}}).as_object().unwrap().clone();
/// let vars = flatten_json_to_vars(&obj);
/// assert_eq!(vars.get("user.name").unwrap(), "Alice");
/// assert_eq!(vars.get("user.age").unwrap(), "30");
/// ```
pub fn flatten_json_to_vars(obj: &Map<String, Value>) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    for (key, value) in obj {
        flatten_value(&mut vars, key, value);
    }

    vars
}

fn flatten_value(vars: &mut HashMap<String, String>, prefix: &str, value: &Value) {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj {
                let new_prefix = format!("{}.{}", prefix, key);
                flatten_value(vars, &new_prefix, val);
            }
        }
        _ => {
            if let Some(string_value) = value_to_string(value) {
                vars.insert(prefix.to_string(), string_value);
            }
        }
    }
}

/// Extract variable names from template
///
/// # Arguments
///
/// * `template` - Template string containing variable references
///
/// # Returns
///
/// Vector of variable names found in template
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::mapreduce::pure::interpolation::extract_variable_names;
///
/// let template = "Process ${item.name} with priority $PRIORITY";
/// let vars = extract_variable_names(template);
/// assert!(vars.contains(&"item.name".to_string()));
/// assert!(vars.contains(&"PRIORITY".to_string()));
/// ```
pub fn extract_variable_names(template: &str) -> Vec<String> {
    let mut vars = Vec::new();

    // Match ${var} patterns
    for cap in BRACED_VAR_PATTERN.captures_iter(template) {
        vars.push(cap[1].to_string());
    }

    // Match $var patterns (but not if already captured by braced pattern)
    for cap in UNBRACED_VAR_PATTERN.captures_iter(template) {
        let var_name = cap[1].to_string();
        // Only add if not already in list (from braced pattern)
        if !vars.contains(&var_name) {
            vars.push(var_name);
        }
    }

    vars
}

/// Validate interpolation context has required variables
///
/// # Arguments
///
/// * `template` - Template string to validate
/// * `context` - Variable context to check against
///
/// # Returns
///
/// Ok if all variables present, Err with list of missing variables
pub fn validate_context(
    template: &str,
    context: &HashMap<String, String>,
) -> Result<(), Vec<String>> {
    let required = extract_variable_names(template);
    let missing: Vec<String> = required
        .into_iter()
        .filter(|var| !context.contains_key(var))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_item_variables_simple() {
        let item = json!({"name": "test", "priority": 5});
        let vars = build_item_variables(&item, "item-1");

        assert_eq!(vars.get("item.id").unwrap(), "item-1");
        assert_eq!(vars.get("item.name").unwrap(), "test");
        assert_eq!(vars.get("item.priority").unwrap(), "5");
    }

    #[test]
    fn test_build_item_variables_with_null() {
        let item = json!({"name": "test", "value": null});
        let vars = build_item_variables(&item, "item-1");

        assert_eq!(vars.get("item.value").unwrap(), "null");
    }

    #[test]
    fn test_build_item_variables_with_boolean() {
        let item = json!({"enabled": true, "disabled": false});
        let vars = build_item_variables(&item, "item-1");

        assert_eq!(vars.get("item.enabled").unwrap(), "true");
        assert_eq!(vars.get("item.disabled").unwrap(), "false");
    }

    #[test]
    fn test_build_item_variables_with_array() {
        let item = json!({"tags": ["a", "b", "c"]});
        let vars = build_item_variables(&item, "item-1");

        assert!(vars.get("item.tags").unwrap().contains("\"a\""));
    }

    #[test]
    fn test_flatten_json_to_vars_nested() {
        let obj = json!({"user": {"name": "Alice", "age": 30}})
            .as_object()
            .unwrap()
            .clone();
        let vars = flatten_json_to_vars(&obj);

        assert_eq!(vars.get("user.name").unwrap(), "Alice");
        assert_eq!(vars.get("user.age").unwrap(), "30");
    }

    #[test]
    fn test_flatten_json_to_vars_deep_nesting() {
        let obj = json!({"a": {"b": {"c": "value"}}})
            .as_object()
            .unwrap()
            .clone();
        let vars = flatten_json_to_vars(&obj);

        assert_eq!(vars.get("a.b.c").unwrap(), "value");
    }

    #[test]
    fn test_flatten_json_to_vars_mixed_types() {
        let obj = json!({"str": "text", "num": 42, "bool": true})
            .as_object()
            .unwrap()
            .clone();
        let vars = flatten_json_to_vars(&obj);

        assert_eq!(vars.get("str").unwrap(), "text");
        assert_eq!(vars.get("num").unwrap(), "42");
        assert_eq!(vars.get("bool").unwrap(), "true");
    }

    #[test]
    fn test_extract_variable_names_braced() {
        let template = "Process ${item.name} and ${item.priority}";
        let vars = extract_variable_names(template);

        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&"item.name".to_string()));
        assert!(vars.contains(&"item.priority".to_string()));
    }

    #[test]
    fn test_extract_variable_names_unbraced() {
        let template = "Use $VAR1 and $VAR2 here";
        let vars = extract_variable_names(template);

        assert!(vars.len() >= 2);
        assert!(vars.contains(&"VAR1".to_string()));
        assert!(vars.contains(&"VAR2".to_string()));
    }

    #[test]
    fn test_extract_variable_names_mixed() {
        let template = "Process ${item.name} with $PRIORITY";
        let vars = extract_variable_names(template);

        assert!(vars.contains(&"item.name".to_string()));
        assert!(vars.contains(&"PRIORITY".to_string()));
    }

    #[test]
    fn test_extract_variable_names_none() {
        let template = "No variables here";
        let vars = extract_variable_names(template);

        assert_eq!(vars.len(), 0);
    }

    #[test]
    fn test_validate_context_all_present() {
        let template = "Use ${var1} and ${var2}";
        let mut context = HashMap::new();
        context.insert("var1".to_string(), "value1".to_string());
        context.insert("var2".to_string(), "value2".to_string());

        assert!(validate_context(template, &context).is_ok());
    }

    #[test]
    fn test_validate_context_missing_variables() {
        let template = "Use ${var1} and ${var2}";
        let mut context = HashMap::new();
        context.insert("var1".to_string(), "value1".to_string());

        let result = validate_context(template, &context);
        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert!(missing.contains(&"var2".to_string()));
    }

    #[test]
    fn test_validate_context_empty_template() {
        let template = "No variables";
        let context = HashMap::new();

        assert!(validate_context(template, &context).is_ok());
    }
}
