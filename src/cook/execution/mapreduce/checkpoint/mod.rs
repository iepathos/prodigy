//! Checkpoint management for MapReduce workflows
//!
//! This module provides checkpoint structures and management for all MapReduce phases.
//!
//! ## Architecture
//!
//! The checkpoint system follows a "pure core, imperative shell" pattern:
//!
//! - `pure/` - Pure functions for validation, preparation, triggers, state transitions
//! - `effects/` - Effect-based I/O operations for storage and signals
//! - `types.rs` - Data structures for checkpoints
//! - `storage.rs` - Storage trait and file implementation
//! - `manager.rs` - High-level checkpoint manager
//!
//! ## Incremental Checkpointing (Spec 162)
//!
//! Checkpoints are created:
//! - After every N agent completions (configurable)
//! - At time intervals (configurable)
//! - On signal (SIGINT/SIGTERM) for graceful shutdown
//! - At phase transitions (setup -> map -> reduce)
//!
//! All checkpoints are stored in `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`

pub mod effects;
pub mod environment;
pub mod incremental;
pub mod manager;
pub mod pure;
pub mod reduce;
pub mod storage;
pub mod types;

// Re-export all types from types module
pub use types::{
    AgentInfo, AgentState, CheckpointConfig, CheckpointId, CheckpointInfo, CheckpointMetadata,
    CheckpointReason, CompletedWorkItem, DlqItem, ErrorState, ExecutionState, FailedWorkItem,
    MapPhaseResults, MapReduceCheckpoint, PhaseResult, PhaseType, ResourceAllocation,
    ResourceState, ResumeState, ResumeStrategy, RetentionPolicy, VariableState, WorkItem,
    WorkItemBatch, WorkItemProgress, WorkItemState,
};

// Re-export from storage
pub use storage::{CheckpointStorage, CompressionAlgorithm, FileCheckpointStorage};

// Re-export from manager
pub use manager::CheckpointManager;

// Re-export types from reduce
pub use reduce::{ReducePhaseCheckpoint, StepResult};

// Re-export pure functions
pub use pure::{
    calculate_integrity_hash, prepare_checkpoint, reset_in_progress_items, should_checkpoint,
    transition_work_item, CheckpointTriggerConfig, CheckpointValidationError, WorkItemEvent,
    WorkItemStatus,
};

// Re-export effects
pub use effects::{
    load_checkpoint_effect, save_checkpoint_effect, save_checkpoint_on_shutdown, shutdown_signal,
    CheckpointOnShutdown, CheckpointStorageEnv, CheckpointStorageError, ShutdownSignal,
};

// Re-export environment
pub use environment::{
    get_checkpoint_job_id, get_checkpoint_storage, get_checkpoint_storage_path,
    get_items_since_checkpoint, get_trigger_config, is_checkpointing_enabled,
    with_checkpointing_disabled, with_trigger_config, CheckpointEnv, CheckpointError,
    MockCheckpointEnvBuilder,
};

// Re-export incremental checkpoint controller
pub use incremental::{CheckpointStats, IncrementalCheckpointController};
