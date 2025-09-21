//! Unified session state definitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use uuid::Uuid;

/// Unique identifier for a session
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Create a new session ID
    pub fn new() -> Self {
        Self(format!("session-{}", Uuid::new_v4()))
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

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a checkpoint
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(String);

impl CheckpointId {
    /// Create a new checkpoint ID
    pub fn new() -> Self {
        Self(format!("checkpoint-{}", Uuid::new_v4()))
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

/// Unified session structure representing all session types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSession {
    pub id: SessionId,
    pub session_type: SessionType,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub checkpoints: Vec<Checkpoint>,
    pub timings: BTreeMap<String, Duration>,
    pub error: Option<String>,

    // Session-specific fields
    pub workflow_data: Option<WorkflowSession>,
    pub mapreduce_data: Option<MapReduceSession>,
}

/// Session type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionType {
    Workflow,
    MapReduce,
}

/// Session status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Initializing,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// Workflow-specific session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSession {
    pub workflow_id: String,
    pub workflow_name: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub completed_steps: Vec<usize>,
    pub variables: HashMap<String, String>,
    pub iterations_completed: u32,
    pub files_changed: u32,
    pub worktree_name: Option<String>,
}

/// MapReduce-specific session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceSession {
    pub job_id: String,
    pub total_items: usize,
    pub processed_items: usize,
    pub failed_items: usize,
    pub agent_count: usize,
    pub phase: MapReducePhase,
    pub reduce_results: Option<serde_json::Value>,
}

/// MapReduce execution phase
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapReducePhase {
    Setup,
    Map,
    Reduce,
    Complete,
}

/// Session checkpoint for resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub created_at: DateTime<Utc>,
    pub state: serde_json::Value,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session_type: SessionType,
    pub workflow_id: Option<String>,
    pub job_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Session metadata
pub type SessionMetadata = HashMap<String, serde_json::Value>;

/// Session timings
pub type SessionTimings = BTreeMap<String, Duration>;

/// Session filter criteria
#[derive(Debug, Default, Clone)]
pub struct SessionFilter {
    pub status: Option<SessionStatus>,
    pub session_type: Option<SessionType>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub worktree_name: Option<String>,
    pub limit: Option<usize>,
}

/// Session summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub session_type: SessionType,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub duration: Option<Duration>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl UnifiedSession {
    /// Create a new workflow session
    pub fn new_workflow(workflow_id: String, workflow_name: String) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            session_type: SessionType::Workflow,
            status: SessionStatus::Initializing,
            started_at: now,
            updated_at: now,
            completed_at: None,
            metadata: HashMap::new(),
            checkpoints: Vec::new(),
            timings: BTreeMap::new(),
            error: None,
            workflow_data: Some(WorkflowSession {
                workflow_id,
                workflow_name,
                current_step: 0,
                total_steps: 0,
                completed_steps: Vec::new(),
                variables: HashMap::new(),
                iterations_completed: 0,
                files_changed: 0,
                worktree_name: None,
            }),
            mapreduce_data: None,
        }
    }

    /// Create a new MapReduce session
    pub fn new_mapreduce(job_id: String, total_items: usize) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            session_type: SessionType::MapReduce,
            status: SessionStatus::Initializing,
            started_at: now,
            updated_at: now,
            completed_at: None,
            metadata: HashMap::new(),
            checkpoints: Vec::new(),
            timings: BTreeMap::new(),
            error: None,
            workflow_data: None,
            mapreduce_data: Some(MapReduceSession {
                job_id,
                total_items,
                processed_items: 0,
                failed_items: 0,
                agent_count: 0,
                phase: MapReducePhase::Setup,
                reduce_results: None,
            }),
        }
    }

    /// Get the current duration of the session
    pub fn duration(&self) -> Duration {
        let end = self.completed_at.unwrap_or_else(Utc::now);
        (end - self.started_at)
            .to_std()
            .unwrap_or(Duration::from_secs(0))
    }

    /// Convert to summary for listing
    pub fn to_summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id.clone(),
            session_type: self.session_type.clone(),
            status: self.status.clone(),
            started_at: self.started_at,
            updated_at: self.updated_at,
            duration: Some(self.duration()),
            metadata: self.metadata.clone(),
        }
    }
}
