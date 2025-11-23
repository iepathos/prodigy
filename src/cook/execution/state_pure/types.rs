//! Type definitions for MapReduce job state

use crate::cook::execution::mapreduce::{AgentResult, MapReduceConfig};
use crate::cook::workflow::WorkflowStep;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// State of the reduce phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhaseState {
    /// Whether reduce phase has started
    pub started: bool,
    /// Whether reduce phase completed successfully
    pub completed: bool,
    /// Commands executed in reduce phase
    pub executed_commands: Vec<String>,
    /// Output from reduce phase
    pub output: Option<String>,
    /// Error if reduce phase failed
    pub error: Option<String>,
    /// Timestamp of reduce phase start
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp of reduce phase completion
    pub completed_at: Option<DateTime<Utc>>,
}

/// Information about a worktree used by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    /// Path to the worktree
    pub path: PathBuf,
    /// Name of the worktree
    pub name: String,
    /// Branch created for this worktree
    pub branch: Option<String>,
    /// Session ID for cleanup tracking
    pub session_id: Option<String>,
}

/// Record of a failed agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    /// Identifier of the failed work item
    pub item_id: String,
    /// Number of retry attempts made
    pub attempts: u32,
    /// Last error message
    pub last_error: String,
    /// Timestamp of last attempt
    pub last_attempt: DateTime<Utc>,
    /// Worktree information if available
    pub worktree_info: Option<WorktreeInfo>,
}

/// Complete state of a MapReduce job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceJobState {
    /// Unique job identifier
    pub job_id: String,
    /// Job configuration
    pub config: MapReduceConfig,
    /// When the job started
    pub started_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// All work items to process
    pub work_items: Vec<Value>,
    /// Results from completed agents
    pub agent_results: HashMap<String, AgentResult>,
    /// Set of completed agent IDs
    pub completed_agents: HashSet<String>,
    /// Failed agents with retry information
    pub failed_agents: HashMap<String, FailureRecord>,
    /// Items still pending execution
    pub pending_items: Vec<String>,
    /// Version number for this checkpoint
    pub checkpoint_version: u32,
    /// Format version of the checkpoint (for migration support)
    #[serde(default = "default_format_version")]
    pub checkpoint_format_version: u32,
    /// Parent worktree if job is running in isolated mode
    pub parent_worktree: Option<String>,
    /// State of the reduce phase
    pub reduce_phase_state: Option<ReducePhaseState>,
    /// Total number of work items (for progress tracking)
    pub total_items: usize,
    /// Number of successful completions
    pub successful_count: usize,
    /// Number of failures
    pub failed_count: usize,
    /// Whether the job has completed
    pub is_complete: bool,
    /// Agent template commands (needed for resumption)
    pub agent_template: Vec<WorkflowStep>,
    /// Reduce phase commands (needed for resumption)
    pub reduce_commands: Option<Vec<WorkflowStep>>,
    /// Workflow variables for interpolation
    #[serde(default)]
    pub variables: HashMap<String, Value>,
    /// Setup phase output if available
    #[serde(default)]
    pub setup_output: Option<String>,
    /// Whether setup phase has been completed
    #[serde(default)]
    pub setup_completed: bool,
    /// Track retry attempts per work item
    /// Key: item_id, Value: number of attempts so far
    #[serde(default)]
    pub item_retry_counts: HashMap<String, u32>,
}

/// Default checkpoint format version
fn default_format_version() -> u32 {
    1
}

/// Information about a checkpoint file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    /// Path to the checkpoint file
    pub path: PathBuf,
    /// Version number of this checkpoint
    pub version: u32,
    /// When this checkpoint was created
    pub created_at: DateTime<Utc>,
    /// Size of the checkpoint file
    pub size_bytes: u64,
}

/// Phase of MapReduce execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Setup phase
    Setup,
    /// Map phase (processing work items)
    Map,
    /// Reduce phase (aggregating results)
    Reduce,
    /// Completed
    Complete,
}

impl MapReduceJobState {
    /// Create a new job state
    pub fn new(job_id: String, config: MapReduceConfig, work_items: Vec<Value>) -> Self {
        let total_items = work_items.len();
        let pending_items: Vec<String> = work_items
            .iter()
            .enumerate()
            .map(|(i, _)| format!("item_{}", i))
            .collect();

        Self {
            job_id,
            config,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items,
            agent_results: HashMap::new(),
            completed_agents: HashSet::new(),
            failed_agents: HashMap::new(),
            pending_items,
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        }
    }
}
