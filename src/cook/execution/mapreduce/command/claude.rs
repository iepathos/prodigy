//! Claude command executor for MapReduce operations

use super::executor::{CommandError, CommandExecutor, CommandResult, ExecutionContext};
use crate::cook::execution::ClaudeExecutor as ClaudeExecutorTrait;
use crate::cook::workflow::{CommandType, WorkflowStep};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Executor for Claude commands
pub struct ClaudeCommandExecutor {
    claude_executor: Arc<dyn ClaudeExecutorTrait>,
}

impl ClaudeCommandExecutor {
    /// Create a new Claude command executor
    pub fn new(claude_executor: Arc<dyn ClaudeExecutorTrait>) -> Self {
        Self { claude_executor }
    }

    /// Build environment variables for Claude execution
    fn build_env_vars(context: &ExecutionContext) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
        env_vars.insert("PRODIGY_WORKTREE".to_string(), context.worktree_name.clone());
        env_vars.insert("PRODIGY_ITEM_ID".to_string(), context.item_id.clone());

        // Add context environment variables
        for (key, value) in &context.environment {
            env_vars.insert(key.clone(), value.clone());
        }

        env_vars
    }

    /// Convert execution result to command result
    fn build_result(
        result: crate::cook::execution::ExecutionResult,
        start: Instant,
    ) -> CommandResult {
        CommandResult {
            output: Some(result.stdout.clone()),
            exit_code: result.exit_code.unwrap_or(0),
            variables: HashMap::new(),
            duration: start.elapsed(),
            success: result.success,
            stderr: result.stderr,
        }
    }
}

#[async_trait]
impl CommandExecutor for ClaudeCommandExecutor {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<CommandResult, CommandError> {
        let start = Instant::now();

        // Extract Claude command
        let command = step.claude.as_ref().ok_or_else(|| {
            CommandError::InvalidConfiguration("No Claude command in step".to_string())
        })?;

        // Build environment variables and execute
        let env_vars = Self::build_env_vars(context);
        let result = self
            .claude_executor
            .execute_claude_command(command, &context.worktree_path, env_vars)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        // Convert to command result
        Ok(Self::build_result(result, start))
    }

    fn supports(&self, command_type: &CommandType) -> bool {
        matches!(
            command_type,
            CommandType::Claude(_) | CommandType::Legacy(_)
        )
    }
}
