//! Command execution effects for MapReduce agents
//!
//! This module provides Effect-based abstractions for executing commands
//! within agent worktrees.
//!
//! NOTE (Spec 173): This is a foundational implementation demonstrating the Effect pattern.
//! Full integration with AgentCommandExecutor will be done in follow-up work.

use crate::cook::execution::errors::MapReduceError;
use crate::cook::execution::mapreduce::environment::MapEnv;
use crate::cook::execution::mapreduce::effects::worktree::Worktree;
use serde_json::Value;
use std::collections::HashMap;
use stillwater::Effect;

/// Result from command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// The worktree where commands were executed
    pub worktree: Worktree,
    /// Variables after command execution
    pub variables: HashMap<String, Value>,
    /// Output from commands
    pub output: Vec<String>,
}

/// Effect: Execute agent template commands in worktree
///
/// Runs the agent template commands within the specified worktree,
/// with the work item data available as variables.
///
/// This is a placeholder demonstrating the Effect pattern.
/// Full implementation will integrate with AgentCommandExecutor.
///
/// # Example
///
/// ```ignore
/// let item = json!({"id": 1, "task": "process"});
/// let effect = execute_commands_effect(&item, &worktree);
/// let result = effect.run_async(&env).await?;
/// ```
pub fn execute_commands_effect(
    item: &Value,
    worktree: &Worktree,
) -> Effect<CommandResult, MapReduceError, MapEnv> {
    let item = item.clone();
    let worktree = worktree.clone();

    Effect::from_async(move |_env: &MapEnv| {
        let item = item.clone();
        let worktree = worktree.clone();

        async move {
            // Placeholder implementation
            // Real implementation will use env.command_executor
            let mut variables = HashMap::new();
            variables.insert("item".to_string(), item.clone());

            Ok(CommandResult {
                worktree,
                variables,
                output: vec!["Command executed".to_string()],
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_command_result_structure() {
        let worktree = Worktree {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/test"),
            branch: "test-branch".to_string(),
        };

        let result = CommandResult {
            worktree,
            variables: HashMap::new(),
            output: vec!["output".to_string()],
        };

        assert_eq!(result.output.len(), 1);
        assert_eq!(result.output[0], "output");
    }

    #[tokio::test]
    async fn test_execute_commands_effect() {
        let env = MapEnv::new(HashMap::new(), HashMap::new());
        let worktree = Worktree {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/test"),
            branch: "test-branch".to_string(),
        };
        let item = json!({"id": 1});

        let effect = execute_commands_effect(&item, &worktree);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
    }
}
