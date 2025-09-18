//! In-memory storage backend for testing

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::{self};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::storage::{
    config::{BackendConfig, MemoryConfig, StorageConfig},
    error::{StorageError, StorageResult},
    lock::{StorageLock, StorageLockGuard},
    traits::*,
    types::*,
};

/// In-memory storage backend for testing
pub struct MemoryBackend {
    config: MemoryConfig,
    sessions: Arc<RwLock<HashMap<SessionId, PersistedSession>>>,
    events: Arc<RwLock<Vec<EventRecord>>>,
    checkpoints: Arc<RwLock<HashMap<String, WorkflowCheckpoint>>>,
    dlq: Arc<RwLock<HashMap<String, DLQItem>>>,
    workflows: Arc<RwLock<HashMap<String, WorkflowDefinition>>>,
    locks: Arc<RwLock<HashMap<String, StorageLock>>>,
}

impl MemoryBackend {
    /// Create a new memory backend
    pub fn new(config: &StorageConfig) -> StorageResult<Self> {
        let memory_config = match &config.backend_config {
            BackendConfig::Memory(cfg) => cfg.clone(),
            _ => {
                return Err(StorageError::configuration(
                    "Invalid backend config for memory storage",
                ))
            }
        };

        Ok(Self {
            config: memory_config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            dlq: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            locks: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new memory backend directly from MemoryConfig (for testing)
    pub fn from_memory_config(config: &MemoryConfig) -> StorageResult<Self> {
        Ok(Self {
            config: config.clone(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            dlq: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            locks: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

/// Simple lock guard for memory backend
struct MemoryLockGuard {
    lock: StorageLock,
    locks: Arc<RwLock<HashMap<String, StorageLock>>>,
}

#[async_trait]
impl StorageLockGuard for MemoryLockGuard {
    fn lock_info(&self) -> &StorageLock {
        &self.lock
    }

    async fn release(self: Box<Self>) -> StorageResult<()> {
        self.locks.write().await.remove(&self.lock.key);
        Ok(())
    }

    async fn extend(&mut self, additional_ttl: Duration) -> StorageResult<()> {
        self.lock.ttl = self.lock.ttl + additional_ttl;
        self.locks
            .write()
            .await
            .insert(self.lock.key.clone(), self.lock.clone());
        Ok(())
    }

    async fn is_valid(&self) -> StorageResult<bool> {
        Ok(!self.lock.is_expired())
    }
}

#[async_trait]
impl UnifiedStorage for MemoryBackend {
    fn session_storage(&self) -> &dyn SessionStorage {
        self
    }

    fn event_storage(&self) -> &dyn EventStorage {
        self
    }

    fn checkpoint_storage(&self) -> &dyn CheckpointStorage {
        self
    }

    fn dlq_storage(&self) -> &dyn DLQStorage {
        self
    }

    fn workflow_storage(&self) -> &dyn WorkflowStorage {
        self
    }

    async fn acquire_lock(
        &self,
        key: &str,
        ttl: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>> {
        let mut locks = self.locks.write().await;

        if let Some(existing) = locks.get(key) {
            if !existing.is_expired() {
                return Err(StorageError::conflict(format!(
                    "Lock already held: {}",
                    key
                )));
            }
        }

        let lock = StorageLock::new(key.to_string(), "memory-backend".to_string(), ttl);
        locks.insert(key.to_string(), lock.clone());

        Ok(Box::new(MemoryLockGuard {
            lock,
            locks: Arc::clone(&self.locks),
        }))
    }

    async fn health_check(&self) -> StorageResult<HealthStatus> {
        Ok(HealthStatus {
            healthy: true,
            backend_type: "memory".to_string(),
            connection_status: ConnectionStatus::Connected,
            latency_ms: 0,
            errors: vec![],
        })
    }

    async fn get_metrics(&self) -> StorageResult<StorageMetrics> {
        let sessions_count = self.sessions.read().await.len();
        let events_count = self.events.read().await.len();
        let checkpoints_count = self.checkpoints.read().await.len();
        let dlq_count = self.dlq.read().await.len();
        let workflows_count = self.workflows.read().await.len();

        Ok(StorageMetrics {
            operations_total: (sessions_count
                + events_count
                + checkpoints_count
                + dlq_count
                + workflows_count) as u64,
            operations_failed: 0,
            average_latency_ms: 0.1,
            storage_size_bytes: 0,
            active_connections: 1,
        })
    }
}

#[async_trait]
impl SessionStorage for MemoryBackend {
    async fn save(&self, session: &PersistedSession) -> StorageResult<()> {
        self.sessions
            .write()
            .await
            .insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn load(&self, id: &SessionId) -> StorageResult<Option<PersistedSession>> {
        Ok(self.sessions.read().await.get(id).cloned())
    }

    async fn list(&self, filter: SessionFilter) -> StorageResult<Vec<SessionId>> {
        let sessions = self.sessions.read().await;
        let mut result = Vec::new();

        for (id, session) in sessions.iter() {
            if let Some(ref state) = filter.state {
                if session.state != *state {
                    continue;
                }
            }
            if let Some(after) = filter.after {
                if session.started_at < after {
                    continue;
                }
            }
            if let Some(before) = filter.before {
                if session.started_at > before {
                    continue;
                }
            }
            if let Some(ref worktree) = filter.worktree_name {
                if session.worktree_name.as_ref() != Some(worktree) {
                    continue;
                }
            }

            result.push(id.clone());

            if let Some(limit) = filter.limit {
                if result.len() >= limit {
                    break;
                }
            }
        }

        Ok(result)
    }

    async fn delete(&self, id: &SessionId) -> StorageResult<()> {
        self.sessions.write().await.remove(id);
        Ok(())
    }

    async fn update_state(&self, id: &SessionId, state: SessionState) -> StorageResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(id) {
            session.state = state;
            session.updated_at = Utc::now();
            Ok(())
        } else {
            Err(StorageError::not_found(format!(
                "Session not found: {}",
                id.0
            )))
        }
    }

    async fn get_stats(&self, id: &SessionId) -> StorageResult<SessionStats> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(id)
            .ok_or_else(|| StorageError::not_found(format!("Session not found: {}", id.0)))?;

        let duration = (session.updated_at - session.started_at)
            .to_std()
            .unwrap_or_default();

        Ok(SessionStats {
            total_duration: duration,
            commands_executed: session.iterations_completed as usize,
            errors_encountered: 0,
            files_modified: session.files_changed as usize,
        })
    }
}

#[async_trait]
impl EventStorage for MemoryBackend {
    async fn append(&self, events: Vec<EventRecord>) -> StorageResult<()> {
        self.events.write().await.extend(events);
        Ok(())
    }

    async fn query(&self, filter: EventFilter) -> StorageResult<EventStream> {
        let events = self.events.read().await.clone();

        let filtered: Vec<Result<EventRecord, anyhow::Error>> = events
            .into_iter()
            .filter(|event| {
                if let Some(ref job_id) = filter.job_id {
                    if event.job_id != *job_id {
                        return false;
                    }
                }
                if let Some(ref event_type) = filter.event_type {
                    if event.event_type != *event_type {
                        return false;
                    }
                }
                if let Some(after) = filter.after {
                    if event.timestamp < after {
                        return false;
                    }
                }
                if let Some(before) = filter.before {
                    if event.timestamp > before {
                        return false;
                    }
                }
                if let Some(ref correlation_id) = filter.correlation_id {
                    if event.correlation_id.as_ref() != Some(correlation_id) {
                        return false;
                    }
                }
                if let Some(ref agent_id) = filter.agent_id {
                    if event.agent_id.as_ref() != Some(agent_id) {
                        return false;
                    }
                }
                true
            })
            .take(filter.limit.unwrap_or(usize::MAX))
            .map(Ok)
            .collect();

        Ok(Box::pin(stream::iter(filtered)))
    }

    async fn aggregate(&self, job_id: &str) -> StorageResult<EventStats> {
        let events = self.events.read().await;
        let mut stats = EventStats {
            total_events: 0,
            events_by_type: HashMap::new(),
            success_count: 0,
            failure_count: 0,
            average_duration: None,
            first_event: None,
            last_event: None,
        };

        for event in events.iter() {
            if event.job_id == job_id {
                stats.total_events += 1;
                *stats
                    .events_by_type
                    .entry(event.event_type.clone())
                    .or_insert(0) += 1;

                if stats.first_event.is_none() || event.timestamp < stats.first_event.unwrap() {
                    stats.first_event = Some(event.timestamp);
                }
                if stats.last_event.is_none() || event.timestamp > stats.last_event.unwrap() {
                    stats.last_event = Some(event.timestamp);
                }

                if let Some(success) = event.data.get("success").and_then(|v| v.as_bool()) {
                    if success {
                        stats.success_count += 1;
                    } else {
                        stats.failure_count += 1;
                    }
                }
            }
        }

        Ok(stats)
    }

    async fn subscribe(&self, _filter: EventFilter) -> StorageResult<EventSubscription> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx); // Close immediately for now

        Ok(EventSubscription {
            id: Uuid::new_v4().to_string(),
            filter: _filter,
            receiver: rx,
        })
    }

    async fn count(&self, filter: EventFilter) -> StorageResult<usize> {
        let events = self.events.read().await;
        let count = events
            .iter()
            .filter(|event| {
                if let Some(ref job_id) = filter.job_id {
                    if event.job_id != *job_id {
                        return false;
                    }
                }
                if let Some(ref event_type) = filter.event_type {
                    if event.event_type != *event_type {
                        return false;
                    }
                }
                true
            })
            .count();
        Ok(count)
    }

    async fn archive(&self, before: DateTime<Utc>) -> StorageResult<usize> {
        let mut events = self.events.write().await;
        let original_len = events.len();
        events.retain(|event| event.timestamp >= before);
        Ok(original_len - events.len())
    }
}

#[async_trait]
impl CheckpointStorage for MemoryBackend {
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> StorageResult<()> {
        self.checkpoints
            .write()
            .await
            .insert(checkpoint.id.clone(), checkpoint.clone());
        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        Ok(self.checkpoints.read().await.get(id).cloned())
    }

    async fn list(&self, filter: CheckpointFilter) -> StorageResult<Vec<CheckpointInfo>> {
        let checkpoints = self.checkpoints.read().await;
        let mut result = Vec::new();

        for checkpoint in checkpoints.values() {
            if let Some(ref workflow_id) = filter.workflow_id {
                if checkpoint.workflow_id != *workflow_id {
                    continue;
                }
            }
            if let Some(after) = filter.after {
                if checkpoint.created_at < after {
                    continue;
                }
            }
            if let Some(before) = filter.before {
                if checkpoint.created_at > before {
                    continue;
                }
            }

            result.push(CheckpointInfo {
                id: checkpoint.id.clone(),
                workflow_id: checkpoint.workflow_id.clone(),
                created_at: checkpoint.created_at,
                step_index: checkpoint.step_index,
                size_bytes: 0,
            });

            if let Some(limit) = filter.limit {
                if result.len() >= limit {
                    break;
                }
            }
        }

        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        self.checkpoints.write().await.remove(id);
        Ok(())
    }

    async fn get_latest(&self, workflow_id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        let checkpoints = self.checkpoints.read().await;
        let mut latest: Option<&WorkflowCheckpoint> = None;

        for checkpoint in checkpoints.values() {
            if checkpoint.workflow_id == workflow_id {
                if latest.is_none() || checkpoint.created_at > latest.unwrap().created_at {
                    latest = Some(checkpoint);
                }
            }
        }

        Ok(latest.cloned())
    }

    async fn cleanup(&self, keep_last: usize) -> StorageResult<usize> {
        let mut checkpoints = self.checkpoints.write().await;
        let mut sorted: Vec<_> = checkpoints.values().cloned().collect();
        sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let to_delete: Vec<_> = sorted
            .iter()
            .skip(keep_last)
            .map(|c| c.id.clone())
            .collect();
        let deleted = to_delete.len();

        for id in to_delete {
            checkpoints.remove(&id);
        }

        Ok(deleted)
    }
}

#[async_trait]
impl DLQStorage for MemoryBackend {
    async fn enqueue(&self, item: DLQItem) -> StorageResult<()> {
        self.dlq.write().await.insert(item.id.clone(), item);
        Ok(())
    }

    async fn dequeue(&self, limit: usize) -> StorageResult<Vec<DLQItem>> {
        let mut dlq = self.dlq.write().await;
        let items: Vec<DLQItem> = dlq.values().take(limit).cloned().collect();

        for item in &items {
            dlq.remove(&item.id);
        }

        Ok(items)
    }

    async fn list(&self, filter: DLQFilter) -> StorageResult<Vec<DLQItem>> {
        let dlq = self.dlq.read().await;
        let mut result = Vec::new();

        for item in dlq.values() {
            if let Some(ref job_id) = filter.job_id {
                if item.job_id != *job_id {
                    continue;
                }
            }
            if let Some(after) = filter.after {
                if item.enqueued_at < after {
                    continue;
                }
            }
            if let Some(before) = filter.before {
                if item.enqueued_at > before {
                    continue;
                }
            }
            if let Some(min_retry) = filter.min_retry_count {
                if item.retry_count < min_retry {
                    continue;
                }
            }
            if let Some(max_retry) = filter.max_retry_count {
                if item.retry_count > max_retry {
                    continue;
                }
            }

            result.push(item.clone());

            if let Some(limit) = filter.limit {
                if result.len() >= limit {
                    break;
                }
            }
        }

        Ok(result)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        self.dlq.write().await.remove(id);
        Ok(())
    }

    async fn mark_processed(&self, id: &str) -> StorageResult<()> {
        DLQStorage::delete(self, id).await
    }

    async fn get_stats(&self, job_id: &str) -> StorageResult<DLQStats> {
        let dlq = self.dlq.read().await;
        let mut stats = DLQStats {
            total_items: 0,
            items_by_retry_count: HashMap::new(),
            oldest_item: None,
            newest_item: None,
            average_retry_count: 0.0,
        };

        let items: Vec<_> = dlq.values().filter(|item| item.job_id == job_id).collect();

        if !items.is_empty() {
            stats.total_items = items.len();
            let mut total_retries = 0u32;

            for item in &items {
                *stats
                    .items_by_retry_count
                    .entry(item.retry_count)
                    .or_insert(0) += 1;
                total_retries += item.retry_count;

                if stats.oldest_item.is_none() || item.enqueued_at < stats.oldest_item.unwrap() {
                    stats.oldest_item = Some(item.enqueued_at);
                }
                if stats.newest_item.is_none() || item.enqueued_at > stats.newest_item.unwrap() {
                    stats.newest_item = Some(item.enqueued_at);
                }
            }

            stats.average_retry_count = total_retries as f64 / items.len() as f64;
        }

        Ok(stats)
    }

    async fn purge(&self, older_than: Duration) -> StorageResult<usize> {
        let cutoff = Utc::now() - chrono::Duration::from_std(older_than).unwrap();
        let mut dlq = self.dlq.write().await;
        let to_remove: Vec<_> = dlq
            .iter()
            .filter(|(_, item)| item.enqueued_at < cutoff)
            .map(|(id, _)| id.clone())
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            dlq.remove(&id);
        }

        Ok(count)
    }
}

#[async_trait]
impl WorkflowStorage for MemoryBackend {
    async fn save(&self, workflow: &WorkflowDefinition) -> StorageResult<()> {
        self.workflows
            .write()
            .await
            .insert(workflow.id.clone(), workflow.clone());
        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowDefinition>> {
        Ok(self.workflows.read().await.get(id).cloned())
    }

    async fn list(&self, filter: WorkflowFilter) -> StorageResult<Vec<WorkflowInfo>> {
        let workflows = self.workflows.read().await;
        let mut result = Vec::new();

        for workflow in workflows.values() {
            if let Some(ref name) = filter.name {
                if !workflow.name.contains(name) {
                    continue;
                }
            }
            if let Some(ref tag) = filter.tag {
                if !workflow.metadata.tags.contains(tag) {
                    continue;
                }
            }
            if let Some(ref author) = filter.author {
                if workflow.metadata.author.as_ref() != Some(author) {
                    continue;
                }
            }

            result.push(WorkflowInfo {
                id: workflow.id.clone(),
                name: workflow.name.clone(),
                version: workflow.version.clone(),
                created_at: workflow.created_at,
                execution_count: 0,
            });

            if let Some(limit) = filter.limit {
                if result.len() >= limit {
                    break;
                }
            }
        }

        Ok(result)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        self.workflows.write().await.remove(id);
        Ok(())
    }

    async fn update_metadata(&self, id: &str, metadata: WorkflowMetadata) -> StorageResult<()> {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(id) {
            workflow.metadata = metadata;
            workflow.updated_at = Utc::now();
            Ok(())
        } else {
            Err(StorageError::not_found(format!(
                "Workflow not found: {}",
                id
            )))
        }
    }

    async fn get_history(&self, _id: &str) -> StorageResult<Vec<WorkflowExecution>> {
        Ok(vec![])
    }
}
