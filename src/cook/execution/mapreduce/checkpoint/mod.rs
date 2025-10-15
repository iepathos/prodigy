//! Checkpoint management for MapReduce workflows
//!
//! This module provides checkpoint structures and management for all MapReduce phases.

pub mod manager;
pub mod reduce;

// Re-export all types from manager
pub use manager::{
    AgentInfo, AgentState, CheckpointConfig, CheckpointId, CheckpointInfo, CheckpointManager,
    CheckpointMetadata, CheckpointReason, CheckpointStorage, CompletedWorkItem,
    CompressionAlgorithm, DlqItem, ErrorState, ExecutionState, FailedWorkItem,
    FileCheckpointStorage, MapPhaseResults, MapReduceCheckpoint, PhaseResult, PhaseType,
    ResourceAllocation, ResourceState, ResumeState, ResumeStrategy, RetentionPolicy,
    VariableState, WorkItem, WorkItemBatch, WorkItemProgress, WorkItemState,
};

// Re-export types from reduce
pub use reduce::{ReducePhaseCheckpoint, StepResult};
