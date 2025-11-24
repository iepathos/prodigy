//! Handler command execution effects
//!
//! This module provides Effect-based abstractions for executing registered
//! handler commands using the command registry.
//!
//! # Architecture
//!
//! Handlers are modular command implementations that can be registered
//! with the workflow executor. This module wraps handler execution in
//! Effects for composability and testability.
//!
//! # Example
//!
//! ```ignore
//! use prodigy::cook::workflow::effects::handler::execute_handler_effect;
//! use std::collections::HashMap;
//!
//! let mut attributes = HashMap::new();
//! attributes.insert("path".to_string(), "/tmp/data".into());
//!
//! let effect = execute_handler_effect("file-reader", attributes);
//! let result = effect.run(&env).await?;
//! ```

use super::environment::WorkflowEnv;
use super::{CommandError, CommandOutput};
use crate::commands::AttributeValue;
use crate::cook::workflow::pure::build_command;
use std::collections::HashMap;
use stillwater::Effect;

/// Effect: Execute a registered handler command
///
/// This effect executes a handler from the command registry with the
/// given attributes. Handlers are modular command implementations that
/// can process attributes and return structured output.
///
/// # Arguments
///
/// * `handler_name` - Name of the registered handler
/// * `attributes` - Map of attribute names to values
/// * `variables` - Variables for attribute value interpolation
///
/// # Returns
///
/// An Effect that, when run, will:
/// 1. Pure: Interpolate variables in attribute values
/// 2. I/O: Execute the handler via the registry
/// 3. Return the handler output
///
/// # Example
///
/// ```ignore
/// let mut attrs = HashMap::new();
/// attrs.insert("input".to_string(), AttributeValue::String("${file}".to_string()));
///
/// let mut vars = HashMap::new();
/// vars.insert("file".to_string(), "data.json".to_string());
///
/// let effect = execute_handler_effect("json-processor", attrs, &vars);
/// let output = effect.run(&env).await?;
/// ```
///
/// # Note
///
/// Handler execution is currently a placeholder. Full integration with
/// the CommandRegistry will be implemented when the workflow executor
/// is refactored to use effects (spec 174f).
pub fn execute_handler_effect(
    handler_name: &str,
    attributes: HashMap<String, AttributeValue>,
    variables: &HashMap<String, String>,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let handler_name = handler_name.to_string();
    let attributes = attributes.clone();
    let variables = variables.clone();

    Effect::from_async(move |env: &WorkflowEnv| {
        let handler_name = handler_name.clone();
        let attributes = attributes.clone();
        let variables = variables.clone();
        let working_dir = env.working_dir.clone();
        let _env_vars = env.env_vars.clone();

        async move {
            // Pure: Interpolate string attributes
            let interpolated_attrs: HashMap<String, AttributeValue> = attributes
                .into_iter()
                .map(|(key, value)| {
                    let interpolated_value = interpolate_attribute_value(&value, &variables);
                    (key, interpolated_value)
                })
                .collect();

            // Note: Full handler execution will be integrated in spec 174f
            // For now, return a placeholder output
            //
            // Future implementation will:
            // 1. Look up handler in registry
            // 2. Create ExecutionContext with working_dir and env_vars
            // 3. Execute handler.execute(context, attributes)
            // 4. Convert CommandResult to CommandOutput

            // Placeholder: Return success with handler info
            Ok(CommandOutput {
                stdout: format!(
                    "Handler '{}' executed with {} attributes in {}",
                    handler_name,
                    interpolated_attrs.len(),
                    working_dir.display()
                ),
                stderr: String::new(),
                exit_code: Some(0),
                success: true,
                variables: HashMap::new(),
                json_log_location: None,
            })
        }
    })
}

/// Pure: Interpolate variables in an AttributeValue
///
/// Recursively processes attribute values, interpolating string values
/// using the provided variable map.
fn interpolate_attribute_value(
    value: &AttributeValue,
    variables: &HashMap<String, String>,
) -> AttributeValue {
    match value {
        AttributeValue::String(s) => AttributeValue::String(build_command(s, variables)),
        AttributeValue::Array(arr) => AttributeValue::Array(
            arr.iter()
                .map(|v| interpolate_attribute_value(v, variables))
                .collect(),
        ),
        AttributeValue::Object(obj) => AttributeValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), interpolate_attribute_value(v, variables)))
                .collect(),
        ),
        // Other types don't need interpolation
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::effects::environment::{ClaudeRunner, RunnerOutput, ShellRunner};
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct MockClaudeRunner;

    #[async_trait]
    impl ClaudeRunner for MockClaudeRunner {
        async fn run(
            &self,
            _command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
        ) -> anyhow::Result<RunnerOutput> {
            Ok(RunnerOutput::success("claude output".to_string()))
        }
    }

    struct MockShellRunner;

    #[async_trait]
    impl ShellRunner for MockShellRunner {
        async fn run(
            &self,
            _command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
            _timeout: Option<u64>,
        ) -> anyhow::Result<RunnerOutput> {
            Ok(RunnerOutput::success("shell output".to_string()))
        }
    }

    fn create_test_env() -> WorkflowEnv {
        WorkflowEnv {
            claude_runner: Arc::new(MockClaudeRunner),
            shell_runner: Arc::new(MockShellRunner),
            output_patterns: Vec::new(),
            working_dir: PathBuf::from("/test/dir"),
            env_vars: HashMap::new(),
        }
    }

    #[test]
    fn test_interpolate_attribute_value_string() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "test".to_string());

        let value = AttributeValue::String("hello ${name}".to_string());
        let result = interpolate_attribute_value(&value, &vars);

        assert_eq!(result, AttributeValue::String("hello test".to_string()));
    }

    #[test]
    fn test_interpolate_attribute_value_array() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), "1".to_string());
        vars.insert("y".to_string(), "2".to_string());

        let value = AttributeValue::Array(vec![
            AttributeValue::String("${x}".to_string()),
            AttributeValue::String("${y}".to_string()),
        ]);
        let result = interpolate_attribute_value(&value, &vars);

        match result {
            AttributeValue::Array(arr) => {
                assert_eq!(arr[0], AttributeValue::String("1".to_string()));
                assert_eq!(arr[1], AttributeValue::String("2".to_string()));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_interpolate_attribute_value_object() {
        let mut vars = HashMap::new();
        vars.insert("path".to_string(), "/tmp".to_string());

        let mut obj = HashMap::new();
        obj.insert(
            "location".to_string(),
            AttributeValue::String("${path}/data".to_string()),
        );

        let value = AttributeValue::Object(obj);
        let result = interpolate_attribute_value(&value, &vars);

        match result {
            AttributeValue::Object(obj) => {
                assert_eq!(
                    obj.get("location"),
                    Some(&AttributeValue::String("/tmp/data".to_string()))
                );
            }
            _ => panic!("Expected Object"),
        }
    }

    #[test]
    fn test_interpolate_attribute_value_number() {
        let vars = HashMap::new();
        let value = AttributeValue::Number(42.0);
        let result = interpolate_attribute_value(&value, &vars);

        assert_eq!(result, AttributeValue::Number(42.0));
    }

    #[test]
    fn test_interpolate_attribute_value_boolean() {
        let vars = HashMap::new();
        let value = AttributeValue::Boolean(true);
        let result = interpolate_attribute_value(&value, &vars);

        assert_eq!(result, AttributeValue::Boolean(true));
    }

    #[test]
    fn test_interpolate_attribute_value_null() {
        let vars = HashMap::new();
        let value = AttributeValue::Null;
        let result = interpolate_attribute_value(&value, &vars);

        assert_eq!(result, AttributeValue::Null);
    }

    #[test]
    fn test_interpolate_attribute_value_nested() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "test".to_string());

        let mut inner_obj = HashMap::new();
        inner_obj.insert(
            "value".to_string(),
            AttributeValue::String("${name}".to_string()),
        );

        let value = AttributeValue::Array(vec![AttributeValue::Object(inner_obj)]);
        let result = interpolate_attribute_value(&value, &vars);

        match result {
            AttributeValue::Array(arr) => match &arr[0] {
                AttributeValue::Object(obj) => {
                    assert_eq!(
                        obj.get("value"),
                        Some(&AttributeValue::String("test".to_string()))
                    );
                }
                _ => panic!("Expected Object in Array"),
            },
            _ => panic!("Expected Array"),
        }
    }

    #[tokio::test]
    async fn test_execute_handler_effect_basic() {
        let env = create_test_env();

        let mut attrs = HashMap::new();
        attrs.insert(
            "input".to_string(),
            AttributeValue::String("data".to_string()),
        );

        let effect = execute_handler_effect("test-handler", attrs, &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("test-handler"));
        assert!(output.stdout.contains("1 attributes"));
    }

    #[tokio::test]
    async fn test_execute_handler_effect_with_variable_interpolation() {
        let env = create_test_env();

        let mut attrs = HashMap::new();
        attrs.insert(
            "path".to_string(),
            AttributeValue::String("${base}/file.txt".to_string()),
        );

        let mut vars = HashMap::new();
        vars.insert("base".to_string(), "/home/user".to_string());

        let effect = execute_handler_effect("file-handler", attrs, &vars);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
    }

    #[tokio::test]
    async fn test_execute_handler_effect_empty_attributes() {
        let env = create_test_env();

        let effect = execute_handler_effect("empty-handler", HashMap::new(), &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("0 attributes"));
    }

    #[tokio::test]
    async fn test_execute_handler_effect_multiple_attributes() {
        let env = create_test_env();

        let mut attrs = HashMap::new();
        attrs.insert(
            "attr1".to_string(),
            AttributeValue::String("value1".to_string()),
        );
        attrs.insert("attr2".to_string(), AttributeValue::Number(42.0));
        attrs.insert("attr3".to_string(), AttributeValue::Boolean(true));

        let effect = execute_handler_effect("multi-handler", attrs, &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("3 attributes"));
    }

    #[tokio::test]
    async fn test_execute_handler_effect_includes_working_dir() {
        let env = create_test_env();

        let effect = execute_handler_effect("dir-handler", HashMap::new(), &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.contains("/test/dir"));
    }
}
