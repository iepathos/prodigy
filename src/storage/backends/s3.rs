//! S3 storage backend implementation

use super::super::config::S3Config;
use super::super::error::{StorageError, StorageResult};
use super::super::lock::StorageLockGuard;
use super::super::traits::{
    CheckpointStorage, DLQStorage, EventStorage, SessionStorage, UnifiedStorage, WorkflowStorage,
};
use super::super::types::*;
use async_trait::async_trait;
use aws_sdk_s3::Client;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// S3 storage backend
pub struct S3Backend {
    client: Arc<Client>,
    config: S3Config,
}

impl S3Backend {
    /// Create new S3 backend
    pub async fn new(config: &S3Config) -> StorageResult<Self> {
        info!("Initializing S3 backend");

        // Create AWS config
        let aws_config = if let Some(ref endpoint) = config.endpoint {
            aws_config::from_env()
                .endpoint_url(endpoint)
                .load()
                .await
        } else {
            aws_config::load_from_env().await
        };

        // Create S3 client
        let client = Client::new(&aws_config);

        // Test connection
        client
            .head_bucket()
            .bucket(&config.bucket)
            .send()
            .await
            .map_err(|e| StorageError::connection(format!("Failed to access S3 bucket: {}", e)))?;

        Ok(Self {
            client: Arc::new(client),
            config: config.clone(),
        })
    }

    /// Make an S3 key
    fn make_key(&self, namespace: &str, key: &str) -> String {
        format!("{}/{}/{}", self.config.prefix, namespace, key)
    }
}

#[async_trait]
impl SessionStorage for S3Backend {
    async fn save(&self, session: &PersistedSession) -> StorageResult<()> {
        debug!("Saving session: {}", session.id.0);

        let key = self.make_key("sessions", &session.id.0);
        let body = serde_json::to_vec(session)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(body.into())
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to save session: {}", e)))?;

        Ok(())
    }

    async fn load(&self, id: &SessionId) -> StorageResult<Option<PersistedSession>> {
        debug!("Loading session: {}", id.0);

        let key = self.make_key("sessions", &id.0);

        match self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(result) => {
                let bytes = result
                    .body
                    .collect()
                    .await
                    .map_err(|e| StorageError::io_error(format!("Failed to read session: {}", e)))?
                    .into_bytes();

                let session = serde_json::from_slice(&bytes)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;

                Ok(Some(session))
            }
            Err(_) => Ok(None),
        }
    }

    async fn list(&self, _filter: SessionFilter) -> StorageResult<Vec<SessionId>> {
        debug!("Listing sessions");
        // Simplified implementation
        Ok(Vec::new())
    }

    async fn delete(&self, id: &SessionId) -> StorageResult<()> {
        debug!("Deleting session: {}", id.0);

        let key = self.make_key("sessions", &id.0);

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to delete session: {}", e)))?;

        Ok(())
    }

    async fn update_state(&self, id: &SessionId, state: SessionState) -> StorageResult<()> {
        debug!("Updating session state: {} to {:?}", id.0, state);

        // Load, update, save
        if let Some(mut session) = self.load(id).await? {
            session.state = state;
            self.save(&session).await?;
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
impl EventStorage for S3Backend {
    async fn append(&self, events: Vec<EventRecord>) -> StorageResult<()> {
        debug!("Appending {} events", events.len());

        for event in events {
            let key = self.make_key("events", &format!("{}/{}", event.job_id, event.id));
            let body = serde_json::to_vec(&event)
                .map_err(|e| StorageError::serialization(e.to_string()))?;

            self.client
                .put_object()
                .bucket(&self.config.bucket)
                .key(&key)
                .body(body.into())
                .send()
                .await
                .map_err(|e| StorageError::io_error(format!("Failed to save event: {}", e)))?;
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
impl CheckpointStorage for S3Backend {
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> StorageResult<()> {
        debug!("Saving checkpoint: {}", checkpoint.id);

        let key = self.make_key("checkpoints", &checkpoint.id);
        let body = serde_json::to_vec(checkpoint)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(body.into())
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to save checkpoint: {}", e)))?;

        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        debug!("Loading checkpoint: {}", id);

        let key = self.make_key("checkpoints", id);

        match self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(result) => {
                let bytes = result
                    .body
                    .collect()
                    .await
                    .map_err(|e| {
                        StorageError::io_error(format!("Failed to read checkpoint: {}", e))
                    })?
                    .into_bytes();

                let checkpoint = serde_json::from_slice(&bytes)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;

                Ok(Some(checkpoint))
            }
            Err(_) => Ok(None),
        }
    }

    async fn list(&self, _filter: CheckpointFilter) -> StorageResult<Vec<CheckpointInfo>> {
        Ok(Vec::new())
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting checkpoint: {}", id);

        let key = self.make_key("checkpoints", id);

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to delete checkpoint: {}", e)))?;

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
impl DLQStorage for S3Backend {
    async fn enqueue(&self, item: DLQItem) -> StorageResult<()> {
        debug!("Enqueueing DLQ item: {}", item.id);

        let key = self.make_key("dlq", &item.id);
        let body = serde_json::to_vec(&item)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(body.into())
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to enqueue DLQ item: {}", e)))?;

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

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to delete DLQ item: {}", e)))?;

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
impl WorkflowStorage for S3Backend {
    async fn save(&self, workflow: &WorkflowDefinition) -> StorageResult<()> {
        debug!("Saving workflow: {}", workflow.id);

        let key = self.make_key("workflows", &workflow.id);
        let body = serde_json::to_vec(workflow)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(body.into())
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to save workflow: {}", e)))?;

        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowDefinition>> {
        debug!("Loading workflow: {}", id);

        let key = self.make_key("workflows", id);

        match self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(result) => {
                let bytes = result
                    .body
                    .collect()
                    .await
                    .map_err(|e| {
                        StorageError::io_error(format!("Failed to read workflow: {}", e))
                    })?
                    .into_bytes();

                let workflow = serde_json::from_slice(&bytes)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;

                Ok(Some(workflow))
            }
            Err(_) => Ok(None),
        }
    }

    async fn list(&self, _filter: WorkflowFilter) -> StorageResult<Vec<WorkflowInfo>> {
        Ok(Vec::new())
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting workflow: {}", id);

        let key = self.make_key("workflows", id);

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to delete workflow: {}", e)))?;

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
impl UnifiedStorage for S3Backend {
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

        match self
            .client
            .head_bucket()
            .bucket(&self.config.bucket)
            .send()
            .await
        {
            Ok(_) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                Ok(HealthStatus {
                    healthy: true,
                    backend_type: "s3".to_string(),
                    connection_status: ConnectionStatus::Connected,
                    latency_ms,
                    errors: Vec::new(),
                })
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                Ok(HealthStatus {
                    healthy: false,
                    backend_type: "s3".to_string(),
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
            active_connections: 0,
        })
    }
}