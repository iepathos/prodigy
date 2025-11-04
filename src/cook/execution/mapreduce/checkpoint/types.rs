//! Checkpoint data structures and state types
//!
//! This module contains all data structures used for checkpoint state representation,
//! including execution state, work item tracking, agent management, and metadata.

use crate::cook::execution::mapreduce::{AgentResult, AgentStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Unique identifier for a checkpoint
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(String);

impl CheckpointId {
    /// Create a new checkpoint ID
    pub fn new() -> Self {
        Self(format!("cp-{}", uuid::Uuid::new_v4()))
    }

    /// Create from an existing string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Enhanced MapReduce checkpoint with comprehensive state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceCheckpoint {
    /// Basic checkpoint metadata
    pub metadata: CheckpointMetadata,
    /// Complete execution state
    pub execution_state: ExecutionState,
    /// Work item processing status
    pub work_item_state: WorkItemState,
    /// Agent execution state
    pub agent_state: AgentState,
    /// Variable and context state
    pub variable_state: VariableState,
    /// Resource allocation state
    pub resource_state: ResourceState,
    /// Error and DLQ state
    pub error_state: ErrorState,
}

/// Checkpoint metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    pub checkpoint_id: String,
    pub job_id: String,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub phase: PhaseType,
    pub total_work_items: usize,
    pub completed_items: usize,
    pub checkpoint_reason: CheckpointReason,
    pub integrity_hash: String,
}

/// Reason for creating a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointReason {
    Interval,
    PhaseTransition,
    Manual,
    BeforeShutdown,
    BatchComplete,
    ErrorRecovery,
}

/// Phase types in MapReduce execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseType {
    Setup,
    Map,
    Reduce,
    Complete,
}

/// Complete execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    pub current_phase: PhaseType,
    pub phase_start_time: DateTime<Utc>,
    pub setup_results: Option<PhaseResult>,
    pub map_results: Option<MapPhaseResults>,
    pub reduce_results: Option<PhaseResult>,
    pub workflow_variables: HashMap<String, Value>,
}

/// Results from a phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub success: bool,
    pub outputs: Vec<String>,
    pub duration: Duration,
}

/// Results from map phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhaseResults {
    pub successful_count: usize,
    pub failed_count: usize,
    pub total_duration: Duration,
}

/// Work item processing state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemState {
    pub pending_items: Vec<WorkItem>,
    pub in_progress_items: HashMap<String, WorkItemProgress>,
    pub completed_items: Vec<CompletedWorkItem>,
    pub failed_items: Vec<FailedWorkItem>,
    pub current_batch: Option<WorkItemBatch>,
}

/// A work item to be processed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub data: Value,
}

/// Progress tracking for a work item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemProgress {
    pub work_item: WorkItem,
    pub agent_id: String,
    pub started_at: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
}

/// A completed work item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedWorkItem {
    pub work_item: WorkItem,
    pub result: AgentResult,
    pub completed_at: DateTime<Utc>,
}

/// A failed work item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedWorkItem {
    pub work_item: WorkItem,
    pub error: String,
    pub failed_at: DateTime<Utc>,
    pub retry_count: usize,
}

/// Batch of work items being processed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemBatch {
    pub batch_id: String,
    pub items: Vec<String>,
    pub started_at: DateTime<Utc>,
}

/// Agent execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub active_agents: HashMap<String, AgentInfo>,
    pub agent_assignments: HashMap<String, Vec<String>>,
    pub agent_results: HashMap<String, AgentResult>,
    pub resource_allocation: HashMap<String, ResourceAllocation>,
}

/// Information about an active agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub worktree_path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub status: AgentStatus,
}

/// Resource allocation for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub cpu_cores: Option<usize>,
    pub memory_mb: Option<usize>,
    pub disk_mb: Option<usize>,
}

/// Variable state for interpolation and context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableState {
    pub workflow_variables: HashMap<String, String>,
    pub captured_outputs: HashMap<String, String>,
    pub environment_variables: HashMap<String, String>,
    pub item_variables: HashMap<String, HashMap<String, String>>,
}

/// Resource state for the job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceState {
    pub total_agents_allowed: usize,
    pub current_agents_active: usize,
    pub worktrees_created: Vec<String>,
    pub worktrees_cleaned: Vec<String>,
    pub disk_usage_bytes: Option<u64>,
}

/// Error and DLQ state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorState {
    pub error_count: usize,
    pub dlq_items: Vec<DlqItem>,
    pub error_threshold_reached: bool,
    pub last_error: Option<String>,
}

/// Dead letter queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqItem {
    pub item_id: String,
    pub error: String,
    pub timestamp: DateTime<Utc>,
    pub retry_count: usize,
}

/// Options for checkpoint configuration
#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    pub interval_items: Option<usize>,
    pub interval_duration: Option<Duration>,
    pub enable_compression: bool,
    pub retention_policy: Option<RetentionPolicy>,
    pub validate_on_save: bool,
    pub validate_on_load: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval_items: Some(100),
            interval_duration: Some(Duration::from_secs(300)),
            enable_compression: true,
            retention_policy: Some(RetentionPolicy::default()),
            validate_on_save: true,
            validate_on_load: true,
        }
    }
}

/// Retention policy for checkpoints
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub max_checkpoints: Option<usize>,
    pub max_age: Option<Duration>,
    pub keep_final: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_checkpoints: Some(10),
            max_age: Some(Duration::from_secs(7 * 24 * 3600)), // 7 days
            keep_final: true,
        }
    }
}

/// State for resuming execution
#[derive(Debug)]
pub struct ResumeState {
    pub execution_state: ExecutionState,
    pub work_items: WorkItemState,
    pub agents: AgentState,
    pub variables: VariableState,
    pub resources: ResourceState,
    pub resume_strategy: ResumeStrategy,
    pub checkpoint: MapReduceCheckpoint,
}

/// Strategy for resuming execution
#[derive(Debug, Clone)]
pub enum ResumeStrategy {
    ContinueFromCheckpoint,
    RestartCurrentPhase,
    RestartFromMapPhase,
    ValidateAndContinue,
}

/// Information about a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub job_id: String,
    pub created_at: DateTime<Utc>,
    pub phase: PhaseType,
    pub completed_items: usize,
    pub total_items: usize,
    pub is_final: bool,
}
