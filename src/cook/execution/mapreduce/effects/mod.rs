//! Effect-based I/O operations for MapReduce execution
//!
//! This module provides Stillwater Effect abstractions for MapReduce I/O operations,
//! following the "pure core, imperative shell" pattern. All I/O is encapsulated in
//! Effects that can be composed, tested with mock environments, and executed with
//! proper error handling.
//!
//! # Architecture
//!
//! The effects module separates concerns:
//! - **Pure logic** lives in `pure/` module (planning, dependency analysis)
//! - **I/O effects** live here (worktree, commands, merge operations)
//! - **Environment** provides dependencies via dependency injection
//!
//! # Effect Composition
//!
//! Effects can be composed using `and_then`, `map`, and parallel combinators:
//!
//! ```ignore
//! use stillwater::Effect;
//!
//! // Sequential composition
//! let agent_effect = create_worktree_effect("agent-0", "main")
//!     .and_then(|worktree| execute_commands_effect(&item, &worktree))
//!     .and_then(|(worktree, result)| merge_to_parent_effect(&worktree, "main"));
//!
//! // Parallel execution
//! let effects = vec![agent_effect_1, agent_effect_2, agent_effect_3];
//! let results = Effect::par_all_limit(effects, max_parallel).run_async(&env).await?;
//! ```
//!
//! # Testing
//!
//! Effects can be tested with mock environments without performing actual I/O:
//!
//! ```ignore
//! let mock_env = MockMapEnv::default();
//! let effect = create_worktree_effect("test-agent", "main");
//! let result = effect.run_async(&mock_env).await;
//! assert!(result.is_ok());
//! ```

pub mod commands;
pub mod merge;
pub mod worktree;

pub use commands::{execute_commands_effect, CommandResult};
pub use merge::{merge_to_parent_effect, MergeResult};
pub use worktree::{create_worktree_effect, remove_worktree_effect};
