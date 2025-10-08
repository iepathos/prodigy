//! Command executor trait and router for MapReduce operations
//!
//! This module provides the core abstraction for executing commands
//! and routing them to appropriate executors.

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::workflow::{StepResult, WorkflowStep};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// Re-export context types
pub use super::context::ExecutionContext;
use super::types;

/// Result from command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub output: Option<String>,
    pub exit_code: i32,
    pub variables: HashMap<String, String>,
    pub duration: Duration,
    pub success: bool,
    pub stderr: String,
    pub json_log_location: Option<String>,
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
    fn supports(&self, command_type: &crate::cook::workflow::CommandType) -> bool;
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
        // Determine command type using extracted function
        let command_type = types::determine_command_type(step)?;

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
}

impl Default for CommandRouter {
    fn default() -> Self {
        Self::new()
    }
}
