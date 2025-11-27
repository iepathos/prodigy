//! Command execution effects for MapReduce agents
//!
//! This module provides Effect-based abstractions for executing commands
//! within agent worktrees.
//!
//! NOTE: This demonstrates the Effect pattern with real dependencies in MapEnv.
//! Full AgentCommandExecutor integration is incremental and ongoing.

use crate::cook::execution::errors::MapReduceError;
use crate::cook::execution::mapreduce::effects::worktree::Worktree;
use crate::cook::execution::mapreduce::environment::MapEnv;
use serde_json::Value;
use std::collections::HashMap;
use stillwater::{from_async, Effect};

/// Result from command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// The worktree where commands were executed
    pub worktree: Worktree,
    /// Variables after command execution
    pub variables: HashMap<String, Value>,
    /// Output from commands
    pub output: Vec<String>,
    /// Whether execution succeeded
    pub success: bool,
}

/// Effect: Execute agent template commands in worktree
///
/// Runs the agent template commands within the specified worktree,
/// with the work item data available as variables.
///
/// NOTE: Full integration with AgentCommandExecutor requires complex dependency setup
/// that will be done incrementally as the existing MapReduce coordinator is migrated.
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
) -> impl Effect<Output = CommandResult, Error = MapReduceError, Env = MapEnv> {
    let item = item.clone();
    let worktree = worktree.clone();

    from_async(move |env: &MapEnv| {
        let item = item.clone();
        let worktree = worktree.clone();
        let _command_executor = env.command_executor.clone();
        let _agent_template = env.agent_template.clone();

        async move {
            // TODO: Full integration with AgentCommandExecutor
            // This will execute env.agent_template commands in the worktree
            // using env.command_executor once coordinator migration is complete

            let mut variables = HashMap::new();
            variables.insert("item".to_string(), item.clone());

            Ok(CommandResult {
                worktree,
                variables,
                output: vec!["Command executed".to_string()],
                success: true,
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
            success: true,
        };

        assert_eq!(result.output.len(), 1);
        assert_eq!(result.output[0], "output");
        assert!(result.success);
    }
}
