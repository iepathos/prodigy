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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub workflow_name: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_session_id_creation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();

        // Each ID should be unique
        assert_ne!(id1, id2);

        // IDs should have the correct prefix
        assert!(id1.as_str().starts_with("session-"));
        assert!(id2.as_str().starts_with("session-"));
    }

    #[test]
    fn test_session_id_from_string() {
        let custom_id = "custom-session-123";
        let session_id = SessionId::from_string(custom_id.to_string());

        assert_eq!(session_id.as_str(), custom_id);
        assert_eq!(session_id.to_string(), custom_id);
    }

    #[test]
    fn test_checkpoint_id_creation() {
        let id1 = CheckpointId::new();
        let id2 = CheckpointId::new();

        // Each ID should be unique
        assert_ne!(id1, id2);

        // IDs should have the correct prefix
        assert!(id1.as_str().starts_with("checkpoint-"));
        assert!(id2.as_str().starts_with("checkpoint-"));
    }

    #[test]
    fn test_session_type_equality() {
        assert_eq!(SessionType::Workflow, SessionType::Workflow);
        assert_eq!(SessionType::MapReduce, SessionType::MapReduce);
        assert_ne!(SessionType::Workflow, SessionType::MapReduce);
    }

    #[test]
    fn test_session_status_equality() {
        assert_eq!(SessionStatus::Initializing, SessionStatus::Initializing);
        assert_eq!(SessionStatus::Running, SessionStatus::Running);
        assert_eq!(SessionStatus::Paused, SessionStatus::Paused);
        assert_eq!(SessionStatus::Completed, SessionStatus::Completed);
        assert_eq!(SessionStatus::Failed, SessionStatus::Failed);
        assert_ne!(SessionStatus::Running, SessionStatus::Completed);
    }

    // Status methods don't exist, remove these tests

    #[test]
    fn test_unified_session_new_workflow() {
        let workflow_id = "test-workflow-123";
        let workflow_name = "test-workflow";
        let session =
            UnifiedSession::new_workflow(workflow_id.to_string(), workflow_name.to_string());

        assert_eq!(session.session_type, SessionType::Workflow);
        assert_eq!(session.status, SessionStatus::Initializing);
        assert!(session.workflow_data.is_some());
        assert!(session.mapreduce_data.is_none());

        let workflow = session.workflow_data.unwrap();
        assert_eq!(workflow.workflow_id, workflow_id);
        assert_eq!(workflow.workflow_name, workflow_name);
        assert_eq!(workflow.iterations_completed, 0);
        assert_eq!(workflow.files_changed, 0);
        assert_eq!(workflow.worktree_name, None);
    }

    #[test]
    fn test_unified_session_new_mapreduce() {
        let job_id = "mapreduce-job-456";
        let total_items = 100;
        let session = UnifiedSession::new_mapreduce(job_id.to_string(), total_items);

        assert_eq!(session.session_type, SessionType::MapReduce);
        assert_eq!(session.status, SessionStatus::Initializing);
        assert!(session.workflow_data.is_none());
        assert!(session.mapreduce_data.is_some());

        let mapreduce = session.mapreduce_data.unwrap();
        assert_eq!(mapreduce.job_id, job_id);
        assert_eq!(mapreduce.total_items, total_items);
        assert_eq!(mapreduce.processed_items, 0);
        assert_eq!(mapreduce.failed_items, 0);
        assert_eq!(mapreduce.phase, MapReducePhase::Setup);
    }

    #[test]
    fn test_unified_session_duration() {
        let mut session = UnifiedSession::new_workflow("test".to_string(), "workflow".to_string());

        // When not completed, duration should be small (time since created)
        let duration = session.duration();
        assert!(duration.as_millis() < 100); // Should be very small

        // Set completed time
        let now = Utc::now();
        session.started_at = now - chrono::Duration::seconds(10);
        session.completed_at = Some(now);

        // Should have a duration of approximately 10 seconds
        let duration = session.duration();
        assert!(duration.as_secs() >= 9 && duration.as_secs() <= 11);
    }

    #[test]
    fn test_unified_session_to_summary() {
        let mut session =
            UnifiedSession::new_workflow("workflow-1".to_string(), "workflow-1".to_string());
        session.status = SessionStatus::Running;
        session
            .metadata
            .insert("key".to_string(), serde_json::json!("value"));

        let summary = session.to_summary();

        assert_eq!(summary.id, session.id);
        assert_eq!(summary.session_type, SessionType::Workflow);
        assert_eq!(summary.status, SessionStatus::Running);
        assert_eq!(summary.started_at, session.started_at);
        assert!(summary.duration.is_some());
        // Duration should be very small since we just created the session
        assert!(summary.duration.unwrap().as_millis() < 100);
        assert_eq!(summary.metadata, session.metadata);
    }

    #[test]
    fn test_workflow_session_defaults() {
        let workflow = WorkflowSession {
            workflow_id: "test".to_string(),
            workflow_name: "workflow".to_string(),
            current_step: 0,
            total_steps: 0,
            completed_steps: vec![],
            variables: HashMap::new(),
            iterations_completed: 0,
            files_changed: 0,
            worktree_name: Some("worktree".to_string()),
        };

        assert_eq!(workflow.iterations_completed, 0);
        assert_eq!(workflow.files_changed, 0);
        assert_eq!(workflow.current_step, 0);
        assert_eq!(workflow.total_steps, 0);
    }

    #[test]
    fn test_mapreduce_session_phases() {
        let mut mapreduce = MapReduceSession {
            job_id: "job-1".to_string(),
            total_items: 100,
            processed_items: 0,
            failed_items: 0,
            agent_count: 0,
            phase: MapReducePhase::Setup,
            reduce_results: None,
        };

        // Test phase transitions
        assert_eq!(mapreduce.phase, MapReducePhase::Setup);

        mapreduce.phase = MapReducePhase::Map;
        assert_eq!(mapreduce.phase, MapReducePhase::Map);

        mapreduce.phase = MapReducePhase::Reduce;
        assert_eq!(mapreduce.phase, MapReducePhase::Reduce);

        mapreduce.phase = MapReducePhase::Complete;
        assert_eq!(mapreduce.phase, MapReducePhase::Complete);
    }

    #[test]
    fn test_mapreduce_phase_equality() {
        assert_eq!(MapReducePhase::Setup, MapReducePhase::Setup);
        assert_eq!(MapReducePhase::Map, MapReducePhase::Map);
        assert_eq!(MapReducePhase::Reduce, MapReducePhase::Reduce);
        assert_eq!(MapReducePhase::Complete, MapReducePhase::Complete);
        assert_ne!(MapReducePhase::Setup, MapReducePhase::Map);
    }

    #[test]
    fn test_session_filter_default() {
        let filter = SessionFilter::default();
        // Default filter should have no restrictions
        assert!(filter.status.is_none());
        assert!(filter.session_type.is_none());
        assert!(filter.after.is_none());
        assert!(filter.before.is_none());
        assert!(filter.worktree_name.is_none());
        assert!(filter.limit.is_none());
    }

    #[test]
    fn test_session_config_creation() {
        let mut metadata = HashMap::new();
        metadata.insert("env".to_string(), serde_json::json!("production"));

        let config = SessionConfig {
            session_type: SessionType::Workflow,
            workflow_id: Some("workflow-1".to_string()),
            job_id: None,
            metadata: metadata.clone(),
        };

        assert_eq!(config.session_type, SessionType::Workflow);
        assert_eq!(config.workflow_id, Some("workflow-1".to_string()));
        assert!(config.job_id.is_none());
        assert_eq!(
            config.metadata.get("env"),
            Some(&serde_json::json!("production"))
        );
    }

    #[test]
    fn test_checkpoint_creation() {
        let checkpoint = Checkpoint {
            id: CheckpointId::new(),
            created_at: Utc::now(),
            state: serde_json::json!({"test": "data"}),
            metadata: HashMap::new(),
        };

        assert!(checkpoint.id.as_str().starts_with("checkpoint-"));
        assert_eq!(checkpoint.state, serde_json::json!({"test": "data"}));
        assert!(checkpoint.metadata.is_empty());
    }

    #[test]
    fn test_session_summary_with_completed_session() {
        let mut session =
            UnifiedSession::new_workflow("workflow-1".to_string(), "workflow-1".to_string());
        let now = Utc::now();
        session.started_at = now - chrono::Duration::seconds(60);
        session.completed_at = Some(now);
        session.status = SessionStatus::Completed;

        let summary = session.to_summary();

        assert_eq!(summary.status, SessionStatus::Completed);
        assert!(summary.duration.is_some());
        assert!(summary.duration.is_some());

        // Duration should be approximately 60 seconds
        let duration = summary.duration.unwrap();
        assert!(duration.as_secs() >= 59 && duration.as_secs() <= 61);
    }

    #[test]
    fn test_unified_session_serialization() {
        let session = UnifiedSession::new_workflow("test".to_string(), "workflow".to_string());

        // Serialize to JSON
        let json = serde_json::to_string(&session).unwrap();

        // Deserialize back
        let deserialized: UnifiedSession = serde_json::from_str(&json).unwrap();

        // Should be equal (except for potentially different timestamps due to precision)
        assert_eq!(deserialized.id, session.id);
        assert_eq!(deserialized.session_type, session.session_type);
        assert_eq!(deserialized.status, session.status);
        assert_eq!(deserialized.workflow_data, session.workflow_data);
    }

    #[test]
    fn test_session_with_checkpoints() {
        let mut session = UnifiedSession::new_workflow("test".to_string(), "workflow".to_string());

        // Add some checkpoints
        for i in 0..3 {
            session.checkpoints.push(Checkpoint {
                id: CheckpointId::new(),
                created_at: Utc::now(),
                state: serde_json::json!({"iteration": i}),
                metadata: HashMap::new(),
            });
        }

        assert_eq!(session.checkpoints.len(), 3);

        // Verify checkpoint states
        for (i, checkpoint) in session.checkpoints.iter().enumerate() {
            assert_eq!(checkpoint.state["iteration"], i);
        }
    }

    #[test]
    fn test_session_with_timings() {
        let mut session = UnifiedSession::new_workflow("test".to_string(), "workflow".to_string());

        // Add some timing entries
        session
            .timings
            .insert("step1".to_string(), Duration::from_secs(10));
        session
            .timings
            .insert("step2".to_string(), Duration::from_secs(20));
        session
            .timings
            .insert("step3".to_string(), Duration::from_secs(15));

        assert_eq!(session.timings.len(), 3);
        assert_eq!(session.timings.get("step1"), Some(&Duration::from_secs(10)));
        assert_eq!(session.timings.get("step2"), Some(&Duration::from_secs(20)));
        assert_eq!(session.timings.get("step3"), Some(&Duration::from_secs(15)));
    }

    #[test]
    fn test_session_error_handling() {
        let mut session = UnifiedSession::new_workflow("test".to_string(), "workflow".to_string());

        assert!(session.error.is_none());

        // Set an error
        session.error = Some("Test error message".to_string());
        session.status = SessionStatus::Failed;

        assert!(session.error.is_some());
        assert_eq!(session.error.unwrap(), "Test error message");
        assert_eq!(session.status, SessionStatus::Failed);
    }

    #[test]
    fn test_mapreduce_progress_tracking() {
        let mut mapreduce = MapReduceSession {
            job_id: "job-1".to_string(),
            total_items: 100,
            processed_items: 0,
            failed_items: 0,
            agent_count: 0,
            phase: MapReducePhase::Map,
            reduce_results: None,
        };

        // Simulate processing
        mapreduce.processed_items = 50;
        mapreduce.failed_items = 5;
        mapreduce.agent_count = 10;

        assert_eq!(mapreduce.processed_items, 50);
        assert_eq!(mapreduce.failed_items, 5);
        assert_eq!(mapreduce.agent_count, 10);

        // Calculate success rate
        let success_rate = (mapreduce.processed_items - mapreduce.failed_items) as f64
            / mapreduce.processed_items as f64;
        assert!(success_rate > 0.89 && success_rate < 0.91); // ~90% success
    }
}
