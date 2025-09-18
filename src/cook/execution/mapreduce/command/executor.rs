//! Command executor trait and router for MapReduce operations

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::workflow::{CommandType, StepResult, WorkflowStep};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Context for command execution
#[derive(Clone)]
pub struct ExecutionContext {
    pub worktree_path: std::path::PathBuf,
    pub worktree_name: String,
    pub item_id: String,
    pub variables: HashMap<String, String>,
    pub captured_outputs: HashMap<String, String>,
    pub environment: HashMap<String, String>,
}

/// Result from command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub output: Option<String>,
    pub exit_code: i32,
    pub variables: HashMap<String, String>,
    pub duration: Duration,
    pub success: bool,
    pub stderr: String,
}

impl From<CommandResult> for StepResult {
    fn from(result: CommandResult) -> Self {
        StepResult {
            success: result.success,
            exit_code: Some(result.exit_code),
            stdout: result.output.unwrap_or_default(),
            stderr: result.stderr,
        }
    }
}

/// Error type for command execution
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Timeout occurred: {0}")]
    Timeout(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Interpolation failed: {0}")]
    InterpolationFailed(String),
}

impl From<CommandError> for MapReduceError {
    fn from(error: CommandError) -> Self {
        MapReduceError::General {
            message: error.to_string(),
            source: None,
        }
    }
}

/// Trait for command executors
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command with the given context
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<CommandResult, CommandError>;

    /// Check if this executor supports the given command type
    fn supports(&self, command_type: &CommandType) -> bool;
}

/// Router for command execution
pub struct CommandRouter {
    executors: HashMap<String, Arc<dyn CommandExecutor>>,
}

impl CommandRouter {
    /// Create a new command router
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Register a command executor
    pub fn register(&mut self, name: String, executor: Arc<dyn CommandExecutor>) {
        self.executors.insert(name, executor);
    }

    /// Execute a workflow step
    pub async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> MapReduceResult<CommandResult> {
        // Determine command type
        let command_type = Self::determine_command_type(step)?;

        // Find executor that supports this command type
        for executor in self.executors.values() {
            if executor.supports(&command_type) {
                return executor.execute(step, context).await.map_err(|e| e.into());
            }
        }

        Err(MapReduceError::InvalidConfiguration {
            reason: "No executor found for command type".to_string(),
            field: "command_type".to_string(),
            value: format!("{:?}", command_type),
        })
    }

    /// Determine command type from a workflow step
    fn determine_command_type(step: &WorkflowStep) -> MapReduceResult<CommandType> {
        // Collect all specified command types
        let commands = Self::collect_command_types(step);

        // Validate exactly one command is specified
        Self::validate_command_count(&commands)?;

        // Extract and return the single command type
        commands
            .into_iter()
            .next()
            .ok_or_else(|| MapReduceError::InvalidConfiguration {
                reason: "No valid command found in step".to_string(),
                field: "command".to_string(),
                value: "<none>".to_string(),
            })
    }

    /// Collect all command types from a workflow step
    fn collect_command_types(step: &WorkflowStep) -> Vec<CommandType> {
        let mut commands = Vec::new();

        if let Some(claude) = &step.claude {
            commands.push(CommandType::Claude(claude.clone()));
        }
        if let Some(shell) = &step.shell {
            commands.push(CommandType::Shell(shell.clone()));
        }
        if let Some(handler) = &step.handler {
            // Convert serde_json::Value to AttributeValue
            let mut converted_attributes = HashMap::new();
            for (key, value) in &handler.attributes {
                let attr_value = match value {
                    serde_json::Value::String(s) => {
                        crate::commands::AttributeValue::from(s.clone())
                    }
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

            commands.push(CommandType::Handler {
                handler_name: handler.name.clone(),
                attributes: converted_attributes,
            });
        }
        if let Some(test) = &step.test {
            commands.push(CommandType::Test(test.clone()));
        }
        if let Some(goal_seek) = &step.goal_seek {
            commands.push(CommandType::GoalSeek(goal_seek.clone()));
        }
        if let Some(foreach) = &step.foreach {
            commands.push(CommandType::Foreach(foreach.clone()));
        }

        commands
    }

    /// Validate that exactly one command is specified
    fn validate_command_count(commands: &[CommandType]) -> MapReduceResult<()> {
        match commands.len() {
            0 => Err(MapReduceError::InvalidConfiguration {
                reason: "No command type specified in step".to_string(),
                field: "command".to_string(),
                value: "<none>".to_string(),
            }),
            1 => Ok(()),
            n => {
                let types: Vec<String> = commands.iter().map(|c| format!("{:?}", c)).collect();
                Err(MapReduceError::InvalidConfiguration {
                    reason: format!("Multiple commands specified in single step: {}", n),
                    field: "commands".to_string(),
                    value: types.join(", "),
                })
            }
        }
    }
}

impl Default for CommandRouter {
    fn default() -> Self {
        Self::new()
    }
}
