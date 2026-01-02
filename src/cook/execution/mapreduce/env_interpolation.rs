//! Pure functions for environment variable interpolation in MapReduce workflows
//!
//! This module provides testable, pure functions for handling environment variable
//! interpolation with positional arguments. All functions are side-effect free and
//! can be tested independently.

use crate::cook::environment::EnvValue;
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Build an interpolation context from positional arguments
///
/// Creates a context where positional args are available as both:
/// - Numbered variables: `1`, `2`, `3`, etc. (for `$1`, `$2` syntax)
/// - Named variables: `ARG_1`, `ARG_2`, `ARG_3`, etc. (for `${ARG_1}` syntax)
///
/// # Examples
/// ```no_run
/// use prodigy::cook::execution::mapreduce::env_interpolation::build_positional_args_context;
/// let args = vec!["file.txt".to_string(), "output.txt".to_string()];
/// let context = build_positional_args_context(&args);
///
/// // Context contains: {"1": "file.txt", "2": "output.txt", "ARG_1": "file.txt", "ARG_2": "output.txt"}
/// ```
pub fn build_positional_args_context(args: &[String]) -> InterpolationContext {
    let mut variables = HashMap::new();

    for (index, arg) in args.iter().enumerate() {
        let position = index + 1;

        // Add numbered variable for $1, $2 syntax
        variables.insert(position.to_string(), Value::String(arg.clone()));

        // Add named variable for ${ARG_1}, ${ARG_2} syntax
        variables.insert(format!("ARG_{}", position), Value::String(arg.clone()));
    }

    InterpolationContext {
        variables,
        parent: None,
    }
}

/// Interpolate a single environment variable value with positional arguments
///
/// # Examples
/// ```no_run
/// use prodigy::cook::execution::mapreduce::env_interpolation::{build_positional_args_context, interpolate_env_value};
/// use prodigy::cook::execution::interpolation::InterpolationEngine;
///
/// let value = "$1";
/// let args = vec!["file.txt".to_string()];
/// let context = build_positional_args_context(&args);
/// let mut engine = InterpolationEngine::new(false);
///
/// let result = interpolate_env_value(value, &context, &mut engine).unwrap();
/// assert_eq!(result, "file.txt");
/// ```
pub fn interpolate_env_value(
    value: &str,
    context: &InterpolationContext,
    engine: &mut InterpolationEngine,
) -> Result<String> {
    engine
        .interpolate(value, context)
        .map_err(|e| anyhow::anyhow!("Interpolation failed: {}", e))
}

/// Interpolate all environment variables in a map with positional arguments
///
/// This is a pure function that takes environment variables and positional args,
/// and returns a new map with interpolated values.
///
/// # Examples
/// ```no_run
/// use std::collections::HashMap;
/// use prodigy::cook::execution::mapreduce::env_interpolation::interpolate_workflow_env_with_positional_args;
///
/// let mut env = HashMap::new();
/// env.insert("BLOG_POST".to_string(), "$1".to_string());
/// env.insert("OUTPUT_DIR".to_string(), "out".to_string());
///
/// let args = vec!["content/blog/my-post.md".to_string()];
/// let result = interpolate_workflow_env_with_positional_args(Some(&env), &args).unwrap();
///
/// // result contains:
/// // "BLOG_POST" => "content/blog/my-post.md"
/// // "OUTPUT_DIR" => "out"
/// # assert_eq!(result.len(), 2);
/// ```
pub fn interpolate_workflow_env_with_positional_args(
    workflow_env: Option<&HashMap<String, String>>,
    positional_args: &[String],
) -> Result<HashMap<String, EnvValue>> {
    let Some(env_map) = workflow_env else {
        return Ok(HashMap::new());
    };

    // Build context with positional args
    let context = build_positional_args_context(positional_args);

    // Create interpolation engine (non-strict mode allows undefined variables)
    let mut engine = InterpolationEngine::new(false);

    // Interpolate each environment variable value
    let mut result = HashMap::new();
    for (key, value) in env_map {
        let interpolated_value =
            interpolate_env_value(value, &context, &mut engine).with_context(|| {
                format!(
                    "Failed to interpolate workflow env variable '{}' with value '{}'",
                    key, value
                )
            })?;

        result.insert(key.clone(), EnvValue::Static(interpolated_value));
    }

    Ok(result)
}

/// Add positional arguments as environment variables
///
/// Returns a map of `ARG_1`, `ARG_2`, etc. that can be merged with workflow env vars.
/// This is separate from interpolation - these are raw positional arg values.
///
/// # Examples
/// ```no_run
/// use prodigy::cook::execution::mapreduce::env_interpolation::positional_args_as_env_vars;
///
/// let args = vec!["file.txt".to_string(), "output.txt".to_string()];
/// let result = positional_args_as_env_vars(&args);
///
/// // result contains:
/// // "ARG_1" => EnvValue::Static("file.txt")
/// // "ARG_2" => EnvValue::Static("output.txt")
/// # assert_eq!(result.len(), 2);
/// ```
pub fn positional_args_as_env_vars(positional_args: &[String]) -> HashMap<String, EnvValue> {
    positional_args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let arg_name = format!("ARG_{}", index + 1);
            (arg_name, EnvValue::Static(arg.clone()))
        })
        .collect()
}

/// Convert EnvValue map to plain string map (for MapPhase.workflow_env)
///
/// # Examples
/// ```no_run
/// use std::collections::HashMap;
/// use prodigy::cook::environment::EnvValue;
/// use prodigy::cook::execution::mapreduce::env_interpolation::env_values_to_plain_map;
///
/// let mut env_values = HashMap::new();
/// env_values.insert("KEY".to_string(), EnvValue::Static("value".to_string()));
///
/// let result = env_values_to_plain_map(&env_values);
/// assert_eq!(result.get("KEY"), Some(&"value".to_string()));
/// ```
pub fn env_values_to_plain_map(env_values: &HashMap<String, EnvValue>) -> HashMap<String, String> {
    env_values
        .iter()
        .filter_map(|(k, v)| match v {
            EnvValue::Static(val) => Some((k.clone(), val.clone())),
            _ => None, // Skip non-static values
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_positional_args_context_empty() {
        let args: Vec<String> = vec![];
        let context = build_positional_args_context(&args);

        assert_eq!(context.variables.len(), 0);
    }

    #[test]
    fn test_build_positional_args_context_single_arg() {
        let args = vec!["file.txt".to_string()];
        let context = build_positional_args_context(&args);

        assert_eq!(context.variables.len(), 2); // "1" and "ARG_1"
        assert_eq!(
            context.variables.get("1"),
            Some(&Value::String("file.txt".to_string()))
        );
        assert_eq!(
            context.variables.get("ARG_1"),
            Some(&Value::String("file.txt".to_string()))
        );
    }

    #[test]
    fn test_build_positional_args_context_multiple_args() {
        let args = vec![
            "content/blog/my-post.md".to_string(),
            "output".to_string(),
            "format=markdown".to_string(),
        ];
        let context = build_positional_args_context(&args);

        assert_eq!(context.variables.len(), 6); // 3 args × 2 forms each
        assert_eq!(
            context.variables.get("1"),
            Some(&Value::String("content/blog/my-post.md".to_string()))
        );
        assert_eq!(
            context.variables.get("ARG_2"),
            Some(&Value::String("output".to_string()))
        );
        assert_eq!(
            context.variables.get("3"),
            Some(&Value::String("format=markdown".to_string()))
        );
    }

    #[test]
    fn test_interpolate_env_value_simple_reference() {
        let args = vec!["file.txt".to_string()];
        let context = build_positional_args_context(&args);
        let mut engine = InterpolationEngine::new(false);

        let result = interpolate_env_value("$1", &context, &mut engine).unwrap();
        assert_eq!(result, "file.txt");
    }

    #[test]
    fn test_interpolate_env_value_braced_reference() {
        let args = vec!["file.txt".to_string()];
        let context = build_positional_args_context(&args);
        let mut engine = InterpolationEngine::new(false);

        let result = interpolate_env_value("${ARG_1}", &context, &mut engine).unwrap();
        assert_eq!(result, "file.txt");
    }

    #[test]
    fn test_interpolate_env_value_embedded_in_string() {
        let args = vec!["my-post.md".to_string()];
        let context = build_positional_args_context(&args);
        let mut engine = InterpolationEngine::new(false);

        let result = interpolate_env_value("content/blog/$1", &context, &mut engine).unwrap();
        assert_eq!(result, "content/blog/my-post.md");
    }

    #[test]
    fn test_interpolate_env_value_multiple_references() {
        let args = vec!["input.txt".to_string(), "output.txt".to_string()];
        let context = build_positional_args_context(&args);
        let mut engine = InterpolationEngine::new(false);

        let result = interpolate_env_value("cp $1 $2", &context, &mut engine).unwrap();
        assert_eq!(result, "cp input.txt output.txt");
    }

    #[test]
    fn test_interpolate_workflow_env_empty() {
        let args = vec!["file.txt".to_string()];
        let result = interpolate_workflow_env_with_positional_args(None, &args).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_interpolate_workflow_env_no_references() {
        let mut env = HashMap::new();
        env.insert("SITE_URL".to_string(), "https://example.com".to_string());
        env.insert("OUTPUT_DIR".to_string(), "out".to_string());

        let args = vec!["file.txt".to_string()];
        let result = interpolate_workflow_env_with_positional_args(Some(&env), &args).unwrap();

        assert_eq!(result.len(), 2);
        assert!(matches!(
            result.get("SITE_URL"),
            Some(EnvValue::Static(val)) if val == "https://example.com"
        ));
        assert!(matches!(
            result.get("OUTPUT_DIR"),
            Some(EnvValue::Static(val)) if val == "out"
        ));
    }

    #[test]
    fn test_interpolate_workflow_env_with_positional_args() {
        let mut env = HashMap::new();
        env.insert("BLOG_POST".to_string(), "$1".to_string());
        env.insert("OUTPUT_DIR".to_string(), "cross-posts".to_string());
        env.insert("POST_SLUG".to_string(), "$(basename $1 .md)".to_string()); // Will NOT be interpolated by our engine

        let args = vec!["content/blog/rethinking-code-quality-analysis.md".to_string()];
        let result = interpolate_workflow_env_with_positional_args(Some(&env), &args).unwrap();

        assert_eq!(result.len(), 3);
        assert!(matches!(
            result.get("BLOG_POST"),
            Some(EnvValue::Static(val)) if val == "content/blog/rethinking-code-quality-analysis.md"
        ));
        assert!(matches!(
            result.get("OUTPUT_DIR"),
            Some(EnvValue::Static(val)) if val == "cross-posts"
        ));
    }

    #[test]
    fn test_positional_args_as_env_vars() {
        let args = vec!["file.txt".to_string(), "output.txt".to_string()];
        let result = positional_args_as_env_vars(&args);

        assert_eq!(result.len(), 2);
        assert!(matches!(
            result.get("ARG_1"),
            Some(EnvValue::Static(val)) if val == "file.txt"
        ));
        assert!(matches!(
            result.get("ARG_2"),
            Some(EnvValue::Static(val)) if val == "output.txt"
        ));
    }

    #[test]
    fn test_env_values_to_plain_map() {
        let mut env_values = HashMap::new();
        env_values.insert("KEY1".to_string(), EnvValue::Static("value1".to_string()));
        env_values.insert("KEY2".to_string(), EnvValue::Static("value2".to_string()));

        let result = env_values_to_plain_map(&env_values);

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
    }

    // Integration test for full workflow
    #[test]
    fn test_full_workflow_env_interpolation() {
        // Setup: workflow env with positional arg references
        let mut workflow_env = HashMap::new();
        workflow_env.insert("BLOG_POST".to_string(), "$1".to_string());
        workflow_env.insert(
            "SITE_URL".to_string(),
            "https://entropicdrift.com".to_string(),
        );
        workflow_env.insert("OUTPUT_DIR".to_string(), "cross-posts".to_string());

        let args = vec!["content/blog/rethinking-code-quality-analysis.md".to_string()];

        // Step 1: Interpolate workflow env
        let mut interpolated_env =
            interpolate_workflow_env_with_positional_args(Some(&workflow_env), &args).unwrap();

        // Step 2: Add positional args as env vars
        let positional_env = positional_args_as_env_vars(&args);
        interpolated_env.extend(positional_env);

        // Step 3: Convert to plain map for MapPhase
        let plain_map = env_values_to_plain_map(&interpolated_env);

        // Verify final result
        assert_eq!(
            plain_map.get("BLOG_POST"),
            Some(&"content/blog/rethinking-code-quality-analysis.md".to_string())
        );
        assert_eq!(
            plain_map.get("ARG_1"),
            Some(&"content/blog/rethinking-code-quality-analysis.md".to_string())
        );
        assert_eq!(
            plain_map.get("SITE_URL"),
            Some(&"https://entropicdrift.com".to_string())
        );
    }
}
