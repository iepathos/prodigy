//! Handler command executor for MapReduce operations

use super::executor::{
    CommandError, CommandExecutor, CommandResult, ExecutionContext as MapReduceContext,
};
use crate::commands::{CommandRegistry, ExecutionContext};
use crate::cook::workflow::{CommandType, WorkflowStep};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;

/// Executor for handler commands
pub struct HandlerCommandExecutor {
    command_registry: Arc<CommandRegistry>,
}

impl HandlerCommandExecutor {
    /// Create a new handler command executor
    pub fn new(command_registry: Arc<CommandRegistry>) -> Self {
        Self { command_registry }
    }

    /// Build execution context for handler
    fn build_exec_context(context: &MapReduceContext) -> ExecutionContext {
        let mut exec_context = ExecutionContext::new(context.worktree_path.clone());

        // Add standard environment variables
        exec_context.add_env_var(
            "PRODIGY_WORKTREE".to_string(),
            context.worktree_name.clone(),
        );
        exec_context.add_env_var("PRODIGY_ITEM_ID".to_string(), context.item_id.clone());
        exec_context.add_env_var("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Add context environment variables
        for (key, value) in &context.environment {
            exec_context.add_env_var(key.clone(), value.clone());
        }

        exec_context
    }

    /// Convert JSON attributes to handler attributes
    fn convert_attributes(
        attributes: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, crate::commands::AttributeValue> {
        let mut converted = std::collections::HashMap::new();

        for (key, value) in attributes {
            let attr_value = Self::json_to_attribute_value(value);
            converted.insert(key.clone(), attr_value);
        }

        converted
    }

    /// Convert JSON value to AttributeValue
    fn json_to_attribute_value(value: &serde_json::Value) -> crate::commands::AttributeValue {
        match value {
            serde_json::Value::String(s) => crate::commands::AttributeValue::from(s.clone()),
            serde_json::Value::Bool(b) => crate::commands::AttributeValue::from(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    crate::commands::AttributeValue::from(i as i32)
                } else if let Some(f) = n.as_f64() {
                    crate::commands::AttributeValue::from(f)
                } else {
                    crate::commands::AttributeValue::from(n.to_string())
                }
            }
            _ => crate::commands::AttributeValue::from(value.to_string()),
        }
    }

    /// Convert handler result to command result
    fn build_result(result: crate::commands::CommandResult, start: Instant) -> CommandResult {
        let stdout = result.stdout.as_ref().cloned().unwrap_or_else(|| {
            result
                .data
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        });

        CommandResult {
            output: Some(stdout),
            exit_code: result.exit_code.unwrap_or(0),
            variables: std::collections::HashMap::new(),
            duration: start.elapsed(),
            success: result.is_success(),
            stderr: result.stderr.unwrap_or_default(),
        }
    }
}

#[async_trait]
impl CommandExecutor for HandlerCommandExecutor {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &MapReduceContext,
    ) -> Result<CommandResult, CommandError> {
        let start = Instant::now();

        // Extract handler information
        let handler = step
            .handler
            .as_ref()
            .ok_or_else(|| CommandError::InvalidConfiguration("No handler in step".to_string()))?;

        // Build execution context and convert attributes
        let exec_context = Self::build_exec_context(context);
        let converted_attributes = Self::convert_attributes(&handler.attributes);

        // Execute the handler
        let result = self
            .command_registry
            .execute(&handler.name, &exec_context, converted_attributes)
            .await;

        // Convert to command result
        Ok(Self::build_result(result, start))
    }

    fn supports(&self, command_type: &CommandType) -> bool {
        matches!(command_type, CommandType::Handler { .. })
    }
}
