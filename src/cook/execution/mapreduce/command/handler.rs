//! Handler command executor for MapReduce operations

use super::executor::{
    CommandError, CommandExecutor, CommandResult, ExecutionContext as MapReduceContext,
};
use crate::commands::{CommandRegistry, ExecutionContext};
use crate::cook::workflow::{CommandType, WorkflowStep};
use async_trait::async_trait;
use std::collections::HashMap;
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

        // Create execution context for the handler
        let mut exec_context = ExecutionContext::new(context.worktree_path.clone());

        // Add environment variables
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

        // Convert serde_json::Value to AttributeValue
        let mut converted_attributes = HashMap::new();
        for (key, value) in &handler.attributes {
            let attr_value = match value {
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
            };
            converted_attributes.insert(key.clone(), attr_value);
        }

        // Execute the handler
        let result = self
            .command_registry
            .execute(&handler.name, &exec_context, converted_attributes)
            .await;

        // Convert CommandResult to our CommandResult
        let stdout = result.stdout.as_ref().cloned().unwrap_or_else(|| {
            result
                .data
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        });

        Ok(CommandResult {
            output: Some(stdout),
            exit_code: result.exit_code.unwrap_or(0),
            variables: HashMap::new(),
            duration: start.elapsed(),
            success: result.is_success(),
            stderr: result.stderr.unwrap_or_default(),
        })
    }

    fn supports(&self, command_type: &CommandType) -> bool {
        matches!(command_type, CommandType::Handler { .. })
    }
}
