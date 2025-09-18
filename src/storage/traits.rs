//! Core trait definitions for the storage abstraction layer

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use super::error::StorageResult;
use super::lock::StorageLockGuard;
use super::types::*;

/// Unified storage interface providing access to all storage subsystems
#[async_trait]
pub trait UnifiedStorage: Send + Sync {
    /// Get the session storage implementation
    fn session_storage(&self) -> &dyn SessionStorage;

    /// Get the event storage implementation
    fn event_storage(&self) -> &dyn EventStorage;

    /// Get the checkpoint storage implementation
    fn checkpoint_storage(&self) -> &dyn CheckpointStorage;

    /// Get the DLQ storage implementation
    fn dlq_storage(&self) -> &dyn DLQStorage;

    /// Get the workflow storage implementation
    fn workflow_storage(&self) -> &dyn WorkflowStorage;

    /// Acquire a distributed lock for coordination
    async fn acquire_lock(
        &self,
        key: &str,
        ttl: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>>;

    /// Check the health of the storage backend
    async fn health_check(&self) -> StorageResult<HealthStatus>;

    /// Get backend-specific metrics
    async fn get_metrics(&self) -> StorageResult<StorageMetrics>;
}

/// Session storage operations
#[async_trait]
pub trait SessionStorage: Send + Sync {
    /// Save a session
    async fn save(&self, session: &PersistedSession) -> StorageResult<()>;

    /// Load a session by ID
    async fn load(&self, id: &SessionId) -> StorageResult<Option<PersistedSession>>;

    /// List sessions matching filter criteria
    async fn list(&self, filter: SessionFilter) -> StorageResult<Vec<SessionId>>;

    /// Delete a session
    async fn delete(&self, id: &SessionId) -> StorageResult<()>;

    /// Update session state
    async fn update_state(&self, id: &SessionId, state: SessionState) -> StorageResult<()>;

    /// Get session statistics
    async fn get_stats(&self, id: &SessionId) -> StorageResult<SessionStats>;
}

/// Event storage operations with streaming support
#[async_trait]
pub trait EventStorage: Send + Sync {
    /// Append events to storage
    async fn append(&self, events: Vec<EventRecord>) -> StorageResult<()>;

    /// Query events with filter, returns a stream
    async fn query(&self, filter: EventFilter) -> StorageResult<EventStream>;

    /// Aggregate event statistics for a job
    async fn aggregate(&self, job_id: &str) -> StorageResult<EventStats>;

    /// Subscribe to events matching filter
    async fn subscribe(&self, filter: EventFilter) -> StorageResult<EventSubscription>;

    /// Get event count for a filter
    async fn count(&self, filter: EventFilter) -> StorageResult<usize>;

    /// Archive old events
    async fn archive(&self, before: DateTime<Utc>) -> StorageResult<usize>;
}

/// Checkpoint storage for workflow resumption
#[async_trait]
pub trait CheckpointStorage: Send + Sync {
    /// Save a checkpoint
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> StorageResult<()>;

    /// Load a checkpoint by ID
    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowCheckpoint>>;

    /// List checkpoints matching filter
    async fn list(&self, filter: CheckpointFilter) -> StorageResult<Vec<CheckpointInfo>>;

    /// Delete a checkpoint
    async fn delete(&self, id: &str) -> StorageResult<()>;

    /// Get the latest checkpoint for a workflow
    async fn get_latest(&self, workflow_id: &str) -> StorageResult<Option<WorkflowCheckpoint>>;

    /// Clean up old checkpoints
    async fn cleanup(&self, keep_last: usize) -> StorageResult<usize>;
}

/// Dead Letter Queue storage for failed items
#[async_trait]
pub trait DLQStorage: Send + Sync {
    /// Enqueue a failed item
    async fn enqueue(&self, item: DLQItem) -> StorageResult<()>;

    /// Dequeue items for reprocessing
    async fn dequeue(&self, limit: usize) -> StorageResult<Vec<DLQItem>>;

    /// List items matching filter
    async fn list(&self, filter: DLQFilter) -> StorageResult<Vec<DLQItem>>;

    /// Delete an item by ID
    async fn delete(&self, id: &str) -> StorageResult<()>;

    /// Mark item as processed
    async fn mark_processed(&self, id: &str) -> StorageResult<()>;

    /// Get statistics for a job
    async fn get_stats(&self, job_id: &str) -> StorageResult<DLQStats>;

    /// Purge old items
    async fn purge(&self, older_than: Duration) -> StorageResult<usize>;
}

/// Workflow storage for templates and definitions
#[async_trait]
pub trait WorkflowStorage: Send + Sync {
    /// Save a workflow definition
    async fn save(&self, workflow: &WorkflowDefinition) -> StorageResult<()>;

    /// Load a workflow by ID
    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowDefinition>>;

    /// List workflows matching filter
    async fn list(&self, filter: WorkflowFilter) -> StorageResult<Vec<WorkflowInfo>>;

    /// Delete a workflow
    async fn delete(&self, id: &str) -> StorageResult<()>;

    /// Update workflow metadata
    async fn update_metadata(&self, id: &str, metadata: WorkflowMetadata) -> StorageResult<()>;

    /// Get workflow execution history
    async fn get_history(&self, id: &str) -> StorageResult<Vec<WorkflowExecution>>;
}

// Transaction support will be added in a future iteration
// when database backends are implemented
