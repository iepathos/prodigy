//! Checkpoint management for MapReduce workflows
//!
//! This module provides checkpoint structures and management for all MapReduce phases.

pub mod manager;
pub mod reduce;
pub mod types;

// Re-export all types from types module
pub use types::{
    AgentInfo, AgentState, CheckpointConfig, CheckpointId, CheckpointInfo, CheckpointMetadata,
    CheckpointReason, CompletedWorkItem, DlqItem, ErrorState, ExecutionState, FailedWorkItem,
    MapPhaseResults, MapReduceCheckpoint, PhaseResult, PhaseType, ResourceAllocation,
    ResourceState, ResumeState, ResumeStrategy, RetentionPolicy, VariableState, WorkItem,
    WorkItemBatch, WorkItemProgress, WorkItemState,
};

// Re-export from manager
pub use manager::{
    CheckpointManager, CheckpointStorage, CompressionAlgorithm, FileCheckpointStorage,
};

// Re-export types from reduce
pub use reduce::{ReducePhaseCheckpoint, StepResult};
