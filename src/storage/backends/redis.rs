//! Redis storage backend implementation

use super::super::config::RedisConfig;
use super::super::error::{StorageError, StorageResult};
use super::super::lock::StorageLockGuard;
use super::super::traits::{
    CheckpointStorage, DLQStorage, EventStorage, SessionStorage, UnifiedStorage, WorkflowStorage,
};
use super::super::types::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_redis::{Config, Pool, Runtime};
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Redis storage backend
pub struct RedisBackend {
    pool: Arc<Pool>,
    config: RedisConfig,
    key_prefix: String,
}

impl RedisBackend {
    /// Create new Redis backend
    pub async fn new(config: &RedisConfig) -> StorageResult<Self> {
        info!("Initializing Redis backend");

        // Create connection pool configuration
        let mut pool_config = Config::from_url(&config.url);
        if let Some(ref mut p) = pool_config.pool {
            p.max_size = config.pool_size;
            p.timeouts.wait = Some(Duration::from_secs(10));
            p.timeouts.create = Some(Duration::from_secs(10));
            p.timeouts.recycle = Some(Duration::from_secs(10));
        }

        // Create pool
        let pool = pool_config
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| StorageError::connection(format!("Failed to create Redis pool: {}", e)))?;

        // Test connection
        let mut conn = pool
            .get()
            .await
            .map_err(|e| StorageError::connection(format!("Failed to connect to Redis: {}", e)))?;

        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| StorageError::connection(format!("Redis ping failed: {}", e)))?;

        Ok(Self {
            pool: Arc::new(pool),
            config: config.clone(),
            key_prefix: config.key_prefix.clone(),
        })
    }

    /// Make a prefixed key
    fn make_key(&self, namespace: &str, key: &str) -> String {
        format!("{}:{}:{}", self.key_prefix, namespace, key)
    }
}

#[async_trait]
impl SessionStorage for RedisBackend {
    async fn save(&self, session: &PersistedSession) -> StorageResult<()> {
        debug!("Saving session: {}", session.id.0);

        let key = self.make_key("session", &session.id.0);
        let value = serde_json::to_string(session)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.set_ex::<_, _, ()>(&key, value, self.config.default_ttl.as_secs())
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn load(&self, id: &SessionId) -> StorageResult<Option<PersistedSession>> {
        debug!("Loading session: {}", id.0);

        let key = self.make_key("session", &id.0);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let value: Option<String> = conn.get(&key).await.ok();

        match value {
            Some(v) => {
                let session = serde_json::from_str(&v)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, _filter: SessionFilter) -> StorageResult<Vec<SessionId>> {
        debug!("Listing sessions");
        // Simplified implementation
        Ok(Vec::new())
    }

    async fn delete(&self, id: &SessionId) -> StorageResult<()> {
        debug!("Deleting session: {}", id.0);

        let key = self.make_key("session", &id.0);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn update_state(&self, id: &SessionId, state: SessionState) -> StorageResult<()> {
        debug!("Updating session state: {} to {:?}", id.0, state);

        // Load, update, save
        if let Some(mut session) = <Self as SessionStorage>::load(self, id).await? {
            session.state = state;
            <Self as SessionStorage>::save(self, &session).await?;
        }

        Ok(())
    }

    async fn get_stats(&self, _id: &SessionId) -> StorageResult<SessionStats> {
        Ok(SessionStats {
            total_duration: Duration::from_secs(0),
            commands_executed: 0,
            errors_encountered: 0,
            files_modified: 0,
        })
    }
}

#[async_trait]
impl EventStorage for RedisBackend {
    async fn append(&self, events: Vec<EventRecord>) -> StorageResult<()> {
        debug!("Appending {} events", events.len());

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        for event in events {
            let key = self.make_key("events", &event.job_id);
            let value = serde_json::to_string(&event)
                .map_err(|e| StorageError::serialization(e.to_string()))?;

            conn.rpush::<_, _, ()>(&key, &value)
                .await
                .map_err(|e| StorageError::io_error(e.to_string()))?;

            // Set TTL
            conn.expire::<_, ()>(&key, self.config.default_ttl.as_secs() as i64)
                .await
                .map_err(|e| StorageError::io_error(e.to_string()))?;
        }

        Ok(())
    }

    async fn query(&self, _filter: EventFilter) -> StorageResult<EventStream> {
        use futures::stream;
        Ok(Box::pin(stream::empty()))
    }

    async fn aggregate(&self, _job_id: &str) -> StorageResult<EventStats> {
        Ok(EventStats {
            total_events: 0,
            events_by_type: HashMap::new(),
            success_count: 0,
            failure_count: 0,
            average_duration: None,
            first_event: None,
            last_event: None,
        })
    }

    async fn subscribe(&self, filter: EventFilter) -> StorageResult<EventSubscription> {
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok(EventSubscription {
            id: uuid::Uuid::new_v4().to_string(),
            filter,
            receiver: rx,
        })
    }

    async fn count(&self, _filter: EventFilter) -> StorageResult<usize> {
        Ok(0)
    }

    async fn archive(&self, _before: DateTime<Utc>) -> StorageResult<usize> {
        Ok(0)
    }
}

#[async_trait]
impl CheckpointStorage for RedisBackend {
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> StorageResult<()> {
        debug!("Saving checkpoint: {}", checkpoint.id);

        let key = self.make_key("checkpoint", &checkpoint.id);
        let value = serde_json::to_string(checkpoint)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.set_ex::<_, _, ()>(&key, value, self.config.default_ttl.as_secs())
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        debug!("Loading checkpoint: {}", id);

        let key = self.make_key("checkpoint", id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let value: Option<String> = conn.get(&key).await.ok();

        match value {
            Some(v) => {
                let checkpoint = serde_json::from_str(&v)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;
                Ok(Some(checkpoint))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, _filter: CheckpointFilter) -> StorageResult<Vec<CheckpointInfo>> {
        Ok(Vec::new())
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting checkpoint: {}", id);

        let key = self.make_key("checkpoint", id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn get_latest(&self, _workflow_id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        Ok(None)
    }

    async fn cleanup(&self, _keep_last: usize) -> StorageResult<usize> {
        Ok(0)
    }
}

#[async_trait]
impl DLQStorage for RedisBackend {
    async fn enqueue(&self, item: DLQItem) -> StorageResult<()> {
        debug!("Enqueueing DLQ item: {}", item.id);

        let key = self.make_key("dlq", &item.id);
        let value =
            serde_json::to_string(&item).map_err(|e| StorageError::serialization(e.to_string()))?;

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.set_ex::<_, _, ()>(&key, value, self.config.default_ttl.as_secs())
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn dequeue(&self, _limit: usize) -> StorageResult<Vec<DLQItem>> {
        Ok(Vec::new())
    }

    async fn list(&self, _filter: DLQFilter) -> StorageResult<Vec<DLQItem>> {
        Ok(Vec::new())
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting DLQ item: {}", id);

        let key = self.make_key("dlq", id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn mark_processed(&self, _id: &str) -> StorageResult<()> {
        Ok(())
    }

    async fn get_stats(&self, _job_id: &str) -> StorageResult<DLQStats> {
        Ok(DLQStats {
            total_items: 0,
            items_by_retry_count: HashMap::new(),
            oldest_item: None,
            newest_item: None,
            average_retry_count: 0.0,
        })
    }

    async fn purge(&self, _older_than: Duration) -> StorageResult<usize> {
        Ok(0)
    }
}

#[async_trait]
impl WorkflowStorage for RedisBackend {
    async fn save(&self, workflow: &WorkflowDefinition) -> StorageResult<()> {
        debug!("Saving workflow: {}", workflow.id);

        let key = self.make_key("workflow", &workflow.id);
        let value = serde_json::to_string(workflow)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.set::<_, _, ()>(&key, value)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowDefinition>> {
        debug!("Loading workflow: {}", id);

        let key = self.make_key("workflow", id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let value: Option<String> = conn.get(&key).await.ok();

        match value {
            Some(v) => {
                let workflow = serde_json::from_str(&v)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;
                Ok(Some(workflow))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, _filter: WorkflowFilter) -> StorageResult<Vec<WorkflowInfo>> {
        Ok(Vec::new())
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting workflow: {}", id);

        let key = self.make_key("workflow", id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn update_metadata(&self, _id: &str, _metadata: WorkflowMetadata) -> StorageResult<()> {
        Ok(())
    }

    async fn get_history(&self, _id: &str) -> StorageResult<Vec<WorkflowExecution>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl UnifiedStorage for RedisBackend {
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
        _key: &str,
        _ttl: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>> {
        Err(StorageError::operation("Lock acquisition not implemented"))
    }

    async fn health_check(&self) -> StorageResult<HealthStatus> {
        debug!("Performing health check");

        let start = std::time::Instant::now();
        let mut conn = self.pool.get().await.map_err(|e| {
            StorageError::connection(format!("Failed to get connection for health check: {}", e))
        })?;

        match redis::cmd("PING").query_async::<String>(&mut conn).await {
            Ok(_) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                Ok(HealthStatus {
                    healthy: true,
                    backend_type: "redis".to_string(),
                    connection_status: ConnectionStatus::Connected,
                    latency_ms,
                    errors: Vec::new(),
                })
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                Ok(HealthStatus {
                    healthy: false,
                    backend_type: "redis".to_string(),
                    connection_status: ConnectionStatus::Disconnected,
                    latency_ms,
                    errors: vec![e.to_string()],
                })
            }
        }
    }

    async fn get_metrics(&self) -> StorageResult<StorageMetrics> {
        Ok(StorageMetrics {
            operations_total: 0,
            operations_failed: 0,
            average_latency_ms: 0.0,
            storage_size_bytes: 0,
            active_connections: self.pool.status().size as u32,
        })
    }
}
