//! Type definitions for the storage abstraction layer

use chrono::{DateTime, Utc};
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// Session identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionId {
    pub fn new() -> Self {
        Self(format!("session-{}", Uuid::new_v4()))
    }
}

/// Session state enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Persisted session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    pub id: SessionId,
    pub state: SessionState,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub iterations_completed: u32,
    pub files_changed: u32,
    pub worktree_name: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Session filter criteria
#[derive(Debug, Default, Clone)]
pub struct SessionFilter {
    pub state: Option<SessionState>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub worktree_name: Option<String>,
    pub limit: Option<usize>,
}

/// Session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_duration: Duration,
    pub commands_executed: usize,
    pub errors_encountered: usize,
    pub files_modified: usize,
}

/// Event record for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub job_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub correlation_id: Option<String>,
    pub agent_id: Option<String>,
}

/// Event filter criteria
#[derive(Debug, Default, Clone)]
pub struct EventFilter {
    pub job_id: Option<String>,
    pub event_type: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub correlation_id: Option<String>,
    pub agent_id: Option<String>,
    pub limit: Option<usize>,
}

/// Event stream for async iteration
pub type EventStream = BoxStream<'static, Result<EventRecord, anyhow::Error>>;

/// Event subscription for real-time updates
pub struct EventSubscription {
    pub id: String,
    pub filter: EventFilter,
    pub receiver: tokio::sync::mpsc::UnboundedReceiver<EventRecord>,
}

/// Aggregated event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub total_events: usize,
    pub error_count: usize,
    pub success_count: usize,
    pub pending_count: usize,
    pub first_event: Option<DateTime<Utc>>,
    pub last_event: Option<DateTime<Utc>>,
}

/// Workflow checkpoint for resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    pub id: String,
    pub workflow_id: String,
    pub created_at: DateTime<Utc>,
    pub step_index: usize,
    pub completed_steps: Vec<usize>,
    pub variables: HashMap<String, String>,
    pub state: serde_json::Value,
}

/// Checkpoint filter criteria
#[derive(Debug, Default, Clone)]
pub struct CheckpointFilter {
    pub workflow_id: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Checkpoint summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub workflow_id: String,
    pub created_at: DateTime<Utc>,
    pub step_index: usize,
    pub size_bytes: usize,
}

/// Dead Letter Queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DLQItem {
    pub id: String,
    pub job_id: String,
    pub enqueued_at: DateTime<Utc>,
    pub retry_count: u32,
    pub last_error: String,
    pub work_item: serde_json::Value,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// DLQ filter criteria
#[derive(Debug, Default, Clone)]
pub struct DLQFilter {
    pub job_id: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub min_retry_count: Option<u32>,
    pub max_retry_count: Option<u32>,
    pub limit: Option<usize>,
}

/// DLQ statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DLQStats {
    pub total_items: usize,
    pub items_by_retry_count: HashMap<u32, usize>,
    pub oldest_item: Option<DateTime<Utc>>,
    pub newest_item: Option<DateTime<Utc>>,
    pub average_retry_count: f64,
}

/// Workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content: serde_yaml::Value,
    pub metadata: WorkflowMetadata,
}

/// Workflow metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub parameters: HashMap<String, ParameterDefinition>,
}

/// Parameter definition for workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub name: String,
    pub type_name: String,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
}

/// Workflow filter criteria
#[derive(Debug, Default, Clone)]
pub struct WorkflowFilter {
    pub name: Option<String>,
    pub tag: Option<String>,
    pub author: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Workflow summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub execution_count: usize,
}

/// Workflow execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: String,
    pub workflow_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
    pub duration: Option<Duration>,
}

/// Execution status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Storage health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub backend_type: String,
    pub message: Option<String>,
    pub details: Option<serde_json::Value>,
}

/// Connection status for health checks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Degraded,
}

/// Storage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub operations_total: u64,
    pub operations_failed: u64,
    pub average_latency_ms: f64,
    pub storage_size_bytes: u64,
    pub active_connections: u32,
}

// Storage trait definitions for compatibility
use super::error::StorageResult;

/// Session storage interface
pub trait SessionStorage: Send + Sync {
    fn save(&self, session: &SessionState) -> StorageResult<()>;
    fn load(&self, session_id: &SessionId) -> StorageResult<Option<SessionState>>;
    fn list(&self, filter: Option<&SessionFilter>) -> StorageResult<Vec<SessionState>>;
    fn delete(&self, session_id: &SessionId) -> StorageResult<bool>;
}

/// Event storage interface
pub trait EventStorage: Send + Sync {
    fn append(&self, job_id: &str, event: &serde_json::Value) -> StorageResult<()>;
    fn query(&self, filter: &EventFilter) -> StorageResult<Vec<serde_json::Value>>;
    fn stream(&self, filter: &EventFilter) -> StorageResult<EventStream>;
    fn subscribe(&self, filter: &EventFilter) -> StorageResult<EventSubscription>;
    fn stats(&self, job_id: &str) -> StorageResult<EventStats>;
    fn cleanup(&self, retention_days: u32) -> StorageResult<usize>;
}

/// Checkpoint storage interface
pub trait CheckpointStorage: Send + Sync {
    fn save(&self, job_id: &str, checkpoint: &serde_json::Value) -> StorageResult<()>;
    fn load(&self, job_id: &str) -> StorageResult<Option<serde_json::Value>>;
    fn list(&self, filter: Option<&CheckpointFilter>) -> StorageResult<Vec<String>>;
    fn delete(&self, job_id: &str) -> StorageResult<bool>;
    fn exists(&self, job_id: &str) -> StorageResult<bool>;
}

/// DLQ storage interface
pub trait DLQStorage: Send + Sync {
    fn add(&self, job_id: &str, item: &serde_json::Value, error: &str) -> StorageResult<()>;
    fn retry(&self, job_id: &str, item_id: &str) -> StorageResult<Option<serde_json::Value>>;
    fn list(&self, filter: &DLQFilter) -> StorageResult<Vec<serde_json::Value>>;
    fn delete(&self, job_id: &str, item_id: &str) -> StorageResult<bool>;
    fn cleanup(&self, retention_days: u32) -> StorageResult<usize>;
    fn stats(&self, job_id: &str) -> StorageResult<serde_json::Value>;
}

/// Workflow storage interface
pub trait WorkflowStorage: Send + Sync {
    fn save(&self, workflow: &serde_json::Value) -> StorageResult<String>;
    fn load(&self, workflow_id: &str) -> StorageResult<Option<serde_json::Value>>;
    fn list(&self, filter: Option<&WorkflowFilter>) -> StorageResult<Vec<serde_json::Value>>;
    fn delete(&self, workflow_id: &str) -> StorageResult<bool>;
    fn update_status(&self, workflow_id: &str, status: &str) -> StorageResult<()>;
}
