//! Parallel execution patterns using Effects
//!
//! This module demonstrates how to use Stillwater's Effect::par_all_limit
//! for bounded parallel execution of agents in MapReduce workflows.
//!
//! Key benefits:
//! - Type-safe parallel execution with error handling
//! - Automatic concurrency limiting (respects max_parallel)
//! - Composable with other effects
//! - Testable without actual I/O

use crate::cook::execution::errors::MapReduceError;
use crate::cook::execution::mapreduce::effects::commands::{
    execute_commands_effect, CommandResult,
};
use crate::cook::execution::mapreduce::effects::merge::{merge_to_parent_effect, MergeResult};
use crate::cook::execution::mapreduce::effects::worktree::{create_worktree_effect, Worktree};
use crate::cook::execution::mapreduce::environment::MapEnv;
use serde_json::Value;
use stillwater::{from_async, par_all_limit, BoxedEffect, Effect, EffectExt};

/// Result from executing a single agent
#[derive(Debug, Clone)]
pub struct AgentExecutionResult {
    pub worktree: Worktree,
    pub command_result: CommandResult,
    pub merge_result: MergeResult,
}

/// Create an effect that executes a single agent (worktree -> commands -> merge)
///
/// This composes three effects sequentially using `and_then`:
/// 1. Create worktree
/// 2. Execute commands in worktree
/// 3. Merge results back
///
/// # Example
///
/// ```ignore
/// let item = json!({"id": 1, "task": "process"});
/// let effect = execute_agent_effect(&item, "agent-0", "main");
/// let result = effect.run_async(&env).await?;
/// ```
pub fn execute_agent_effect(
    item: &Value,
    agent_name: &str,
    parent_branch: &str,
) -> BoxedEffect<AgentExecutionResult, MapReduceError, MapEnv> {
    let item = item.clone();
    let agent_name = agent_name.to_string();
    let parent_branch = parent_branch.to_string();

    // Compose effects sequentially with and_then
    create_worktree_effect(&agent_name, &parent_branch)
        .and_then(move |worktree| {
            let item = item.clone();
            let parent_branch = parent_branch.clone();
            let worktree_clone = worktree.clone();

            execute_commands_effect(&item, &worktree).and_then(move |command_result| {
                let worktree = worktree_clone;
                let worktree_clone2 = worktree.clone();

                merge_to_parent_effect(&worktree, &parent_branch).map(move |merge_result| {
                    AgentExecutionResult {
                        worktree: worktree_clone2,
                        command_result,
                        merge_result,
                    }
                })
            })
        })
        .boxed()
}

/// Execute multiple agents in parallel with bounded concurrency
///
/// This is the KEY function demonstrating Effect::par_all_limit for parallel execution.
/// It takes a list of work items and executes them in parallel with a maximum
/// concurrency limit (respecting max_parallel from config).
///
/// # Benefits over manual tokio::spawn coordination:
/// - Automatic error collection and handling
/// - Respects concurrency limits
/// - Type-safe
/// - Composable with other effects
/// - Testable with mock environments
///
/// # Example
///
/// ```ignore
/// let items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];
/// let effect = execute_agents_parallel(&items, "main", 2);
/// let results = effect.run_async(&env).await?;
/// // Executes agents with max 2 concurrent at a time
/// ```
pub fn execute_agents_parallel(
    items: &[Value],
    parent_branch: &str,
    max_parallel: usize,
) -> impl Effect<Output = Vec<AgentExecutionResult>, Error = Vec<MapReduceError>, Env = MapEnv> {
    let parent_branch = parent_branch.to_string();
    let items: Vec<Value> = items.to_vec();

    // Wrap in from_async to have access to environment for par_all_limit
    from_async(move |env: &MapEnv| {
        let parent_branch = parent_branch.clone();
        let items = items.clone();
        let env = env.clone(); // Clone env for use in async block

        async move {
            // Create an effect for each work item
            let agent_effects: Vec<BoxedEffect<AgentExecutionResult, MapReduceError, MapEnv>> =
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let agent_name = format!("agent-{}", index);
                        execute_agent_effect(item, &agent_name, &parent_branch)
                    })
                    .collect();

            // Execute all effects in parallel with bounded concurrency
            // par_all_limit returns Vec<E> for errors, collecting all failures
            par_all_limit(agent_effects, max_parallel, &env).await
        }
    })
}

/// Pure function to partition work items for parallel batching
///
/// This demonstrates how to plan parallel execution without I/O.
/// Used for testing and validation before actual execution.
pub fn plan_parallel_batches(item_count: usize, max_parallel: usize) -> Vec<Vec<usize>> {
    let mut batches = Vec::new();
    let mut current_batch = Vec::new();

    for i in 0..item_count {
        current_batch.push(i);
        if current_batch.len() >= max_parallel {
            batches.push(current_batch.clone());
            current_batch.clear();
        }
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_parallel_batches() {
        // Test with exact multiple of max_parallel
        let batches = plan_parallel_batches(6, 2);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], vec![0, 1]);
        assert_eq!(batches[1], vec![2, 3]);
        assert_eq!(batches[2], vec![4, 5]);

        // Test with remainder
        let batches = plan_parallel_batches(5, 2);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], vec![0, 1]);
        assert_eq!(batches[1], vec![2, 3]);
        assert_eq!(batches[2], vec![4]);

        // Test with max_parallel larger than item_count
        let batches = plan_parallel_batches(3, 10);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec![0, 1, 2]);
    }

    #[test]
    fn test_plan_parallel_batches_edge_cases() {
        // Empty
        let batches = plan_parallel_batches(0, 2);
        assert_eq!(batches.len(), 0);

        // Single item
        let batches = plan_parallel_batches(1, 2);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec![0]);

        // Max parallel = 1 (sequential)
        let batches = plan_parallel_batches(5, 1);
        assert_eq!(batches.len(), 5);
        for (i, batch) in batches.iter().enumerate() {
            assert_eq!(batch, &vec![i]);
        }
    }
}
