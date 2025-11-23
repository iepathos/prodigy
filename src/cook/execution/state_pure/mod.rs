//! MapReduce job state management using Stillwater's Effect pattern
//!
//! This module provides pure, testable state management for MapReduce jobs,
//! separating state transition logic from I/O operations.
//!
//! # Architecture
//!
//! The state management is organized into three layers:
//!
//! - **Pure Functions** (`pure.rs`): State transition logic with no side effects
//! - **Effect-Based I/O** (`io.rs`): I/O operations wrapped in Effect types
//! - **Type Definitions** (`types.rs`): Data structures for job state
//!
//! # Pure Core, Imperative Shell
//!
//! This design follows the "pure core, imperative shell" pattern:
//!
//! ```text
//! Pure Logic (pure.rs)  →  Effect Composition (io.rs)  →  Orchestration
//!     ↓                          ↓                              ↓
//! No I/O, instant tests    Mock environments            Real dependencies
//! ```
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use prodigy::cook::execution::state::{pure, io, StateEnv};
//!
//! // Pure state transition (testable without I/O)
//! let new_state = pure::apply_agent_result(state, result);
//!
//! // Effect-based I/O (lazy, composable)
//! let effect = io::update_with_agent_result(state, result);
//! let new_state = effect.run(&env).await?;
//! ```

pub mod io;
pub mod pure;
pub mod types;

// Re-export commonly used types
pub use types::{
    CheckpointInfo, FailureRecord, MapReduceJobState, Phase, ReducePhaseState, WorktreeInfo,
};

// Re-export pure functions for easy access
pub use pure::{
    apply_agent_result, complete_reduce_phase, find_work_item, get_retriable_items,
    is_job_complete, is_map_phase_complete, mark_complete, mark_setup_complete,
    record_agent_failure, set_parent_worktree, should_transition_to_reduce, start_reduce_phase,
    update_variables,
};

// Re-export I/O functions and environment
pub use io::{
    complete_batch, complete_reduce_phase_with_save, load_checkpoint, mark_complete_with_save,
    mark_setup_complete_with_save, save_checkpoint, set_parent_worktree_with_save,
    start_reduce_phase_with_save, update_variables_with_save, update_with_agent_result, EventLog,
    StateEffect, StateEnv, StorageBackend,
};
