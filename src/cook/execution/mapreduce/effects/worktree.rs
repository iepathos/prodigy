//! Worktree I/O effects for MapReduce execution
//!
//! This module provides Effect-based abstractions for worktree operations,
//! separating I/O from business logic for better testability.

use crate::cook::execution::errors::MapReduceError;
use crate::cook::execution::mapreduce::environment::MapEnv;
use std::path::PathBuf;
use stillwater::Effect;

/// Worktree information
#[derive(Debug, Clone)]
pub struct Worktree {
    /// Name of the worktree
    pub name: String,
    /// Path to the worktree
    pub path: PathBuf,
    /// Branch name
    pub branch: String,
}

/// Effect: Create git worktree for agent execution
///
/// Creates an isolated git worktree where an agent can execute commands
/// and make commits without affecting other agents or the main repository.
///
/// NOTE: This demonstrates the Effect pattern. Full integration with WorktreeManager
/// builder API is ongoing as the interface evolves.
///
/// # Example
///
/// ```ignore
/// let effect = create_worktree_effect("agent-0", "main");
/// let worktree = effect.run_async(&env).await?;
/// ```
pub fn create_worktree_effect(
    name: &str,
    _parent_branch: &str,
) -> Effect<Worktree, MapReduceError, MapEnv> {
    let name = name.to_string();

    Effect::from_async(move |env: &MapEnv| {
        let name = name.clone();
        let _worktree_manager = env.worktree_manager.clone();

        async move {
            // TODO: Full integration with WorktreeManager builder API
            // Will use worktree_manager.builder().create_session_with_id(&name).await

            let base_path = std::env::temp_dir().join("prodigy-worktrees");
            let worktree_path = base_path.join(&name);

            Ok(Worktree {
                name: name.clone(),
                path: worktree_path,
                branch: format!("agent-{}", name),
            })
        }
    })
}

/// Effect: Remove git worktree after agent completes
///
/// Cleans up the worktree directory and git references after an agent
/// has completed execution and results have been merged.
///
/// # Example
///
/// ```ignore
/// let effect = remove_worktree_effect(&worktree);
/// effect.run_async(&env).await?;
/// ```
pub fn remove_worktree_effect(_worktree: &Worktree) -> Effect<(), MapReduceError, MapEnv> {
    Effect::from_async(move |env: &MapEnv| {
        let _worktree_manager = env.worktree_manager.clone();

        async move {
            // TODO: Full integration with WorktreeManager cleanup
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    // Tests for effects require full environment setup with real dependencies
    // Integration tests with mock environments are in the main test suite
}
