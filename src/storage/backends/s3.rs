//! S3 storage backend implementation

use super::super::config::S3Config;
use super::super::error::{StorageError, StorageResult};
use super::super::traits::{
    EventStorage, HealthCheck, SessionStorage, StateStorage, UnifiedStorage,
};
use super::super::types::{
    CheckpointData, EventEntry, HealthStatus, JobState, SessionState, WorkflowCheckpoint,
};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    types::{Delete, ObjectIdentifier, ServerSideEncryption, StorageClass},
    Client,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// S3 storage backend
pub struct S3Backend {
    client: Arc<Client>,
    config: S3Config,
    bucket: String,
    prefix: String,
}

impl S3Backend {
    /// Create new S3 backend
    pub async fn new(config: &S3Config) -> StorageResult<Self> {
        info!("Initializing S3 backend for bucket: {}", config.bucket);

        // Configure AWS SDK
        let mut aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(config.region.clone()));

        // Set endpoint if provided (for S3-compatible services)
        if let Some(endpoint) = &config.endpoint {
            aws_config = aws_config.endpoint_url(endpoint);
        }

        // Set credentials if provided
        if let (Some(access_key), Some(secret_key)) = (&config.access_key_id, &config.secret_access_key) {
            let creds = aws_config::Credentials::new(
                access_key,
                secret_key,
                None, // session token
                None, // expiration
                "s3_backend",
            );
            aws_config = aws_config.credentials_provider(creds);
        }

        let aws_config = aws_config.load().await;
        let client = Client::new(&aws_config);

        // Test bucket access
        match client.head_bucket().bucket(&config.bucket).send().await {
            Ok(_) => info!("Successfully connected to S3 bucket: {}", config.bucket),
            Err(e) => {
                return Err(StorageError::connection(format!(
                    "Failed to access S3 bucket {}: {}",
                    config.bucket, e
                )))
            }
        }

        Ok(Self {
            client: Arc::new(client),
            config: config.clone(),
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
        })
    }

    /// Generate S3 key with prefix
    fn make_key(&self, key_type: &str, id: &str) -> String {
        format!("{}{}/{}", self.prefix, key_type, id)
    }

    /// List keys with prefix
    async fn list_keys(&self, prefix: &str) -> StorageResult<Vec<String>> {
        let full_prefix = format!("{}{}", self.prefix, prefix);

        let mut keys = Vec::new();
        let mut continuation_token = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&full_prefix)
                .max_keys(1000);

            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|e| StorageError::io_error(format!("Failed to list S3 objects: {}", e)))?;

            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        // Remove prefix to get just the ID
                        if let Some(id) = key.strip_prefix(&full_prefix) {
                            keys.push(id.to_string());
                        }
                    }
                }
            }

            if response.is_truncated.unwrap_or(false) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(keys)
    }

    /// Put object with optional encryption
    async fn put_object(&self, key: &str, content: Vec<u8>) -> StorageResult<()> {
        let mut request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(content.into())
            .storage_class(match self.config.storage_class {
                super::super::config::S3StorageClass::Standard => StorageClass::Standard,
                super::super::config::S3StorageClass::StandardIa => StorageClass::StandardIa,
                super::super::config::S3StorageClass::IntelligentTiering => {
                    StorageClass::IntelligentTiering
                }
                super::super::config::S3StorageClass::GlacierFlexibleRetrieval => {
                    StorageClass::Glacier
                }
                super::super::config::S3StorageClass::GlacierInstantRetrieval => {
                    StorageClass::GlacierIr
                }
            });

        if self.config.enable_encryption {
            request = request.server_side_encryption(ServerSideEncryption::Aes256);
        }

        request
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to put S3 object: {}", e)))?;

        Ok(())
    }

    /// Get object content
    async fn get_object(&self, key: &str) -> StorageResult<Vec<u8>> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::not_found(format!("Failed to get S3 object: {}", e)))?;

        let bytes = response
            .body
            .collect()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to read S3 object body: {}", e)))?;

        Ok(bytes.to_vec())
    }

    /// Delete object
    async fn delete_object(&self, key: &str) -> StorageResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to delete S3 object: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl SessionStorage for S3Backend {
    async fn save_session(&self, session: &SessionState) -> StorageResult<()> {
        debug!("Saving session: {}", session.session_id);

        let key = self.make_key("sessions", &session.session_id);
        let content = serde_json::to_vec(session)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.put_object(&key, content).await?;

        // Also update repository index
        let index_key = self.make_key("session_index", &session.repository);
        let mut index = match self.get_object(&index_key).await {
            Ok(data) => {
                serde_json::from_slice::<Vec<String>>(&data).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        };

        if !index.contains(&session.session_id) {
            index.push(session.session_id.clone());
            let index_content = serde_json::to_vec(&index)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            self.put_object(&index_key, index_content).await?;
        }

        Ok(())
    }

    async fn load_session(&self, session_id: &str) -> StorageResult<SessionState> {
        debug!("Loading session: {}", session_id);

        let key = self.make_key("sessions", session_id);
        let data = self.get_object(&key).await?;

        let session = serde_json::from_slice(&data)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(session)
    }

    async fn list_sessions(&self, repository: &str) -> StorageResult<Vec<SessionState>> {
        debug!("Listing sessions for repository: {}", repository);

        let index_key = self.make_key("session_index", repository);
        let session_ids = match self.get_object(&index_key).await {
            Ok(data) => {
                serde_json::from_slice::<Vec<String>>(&data)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?
            }
            Err(_) => Vec::new(),
        };

        let mut sessions = Vec::new();
        for session_id in session_ids {
            match self.load_session(&session_id).await {
                Ok(session) => sessions.push(session),
                Err(e) => warn!("Failed to load session {}: {}", session_id, e),
            }
        }

        // Sort by started_at descending
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        Ok(sessions)
    }

    async fn delete_session(&self, session_id: &str) -> StorageResult<()> {
        debug!("Deleting session: {}", session_id);

        // First load session to get repository
        let session = self.load_session(session_id).await?;

        let key = self.make_key("sessions", session_id);
        self.delete_object(&key).await?;

        // Update repository index
        let index_key = self.make_key("session_index", &session.repository);
        if let Ok(data) = self.get_object(&index_key).await {
            let mut index: Vec<String> = serde_json::from_slice(&data)
                .unwrap_or_default();
            index.retain(|id| id != session_id);

            let index_content = serde_json::to_vec(&index)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            self.put_object(&index_key, index_content).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl EventStorage for S3Backend {
    async fn append_event(&self, repository: &str, job_id: &str, event: &EventEntry) -> StorageResult<()> {
        debug!("Appending event for job: {}", job_id);

        // Events are stored as individual objects with timestamp in key
        let timestamp = event.timestamp.timestamp_nanos_opt().unwrap_or(0);
        let key = self.make_key(
            &format!("events/{}/{}", repository, job_id),
            &format!("{:020}_{}", timestamp, uuid::Uuid::new_v4()),
        );

        let content = serde_json::to_vec(event)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.put_object(&key, content).await?;

        // Update job index
        let index_key = self.make_key("job_index", repository);
        let mut index = match self.get_object(&index_key).await {
            Ok(data) => {
                serde_json::from_slice::<Vec<String>>(&data).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        };

        if !index.contains(&job_id.to_string()) {
            index.push(job_id.to_string());
            let index_content = serde_json::to_vec(&index)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            self.put_object(&index_key, index_content).await?;
        }

        Ok(())
    }

    async fn read_events(
        &self,
        repository: &str,
        job_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> StorageResult<Vec<EventEntry>> {
        debug!("Reading events for job: {}", job_id);

        let prefix = format!("events/{}/{}/", repository, job_id);
        let keys = self.list_keys(&prefix).await?;

        let mut events = Vec::new();
        for key in keys {
            let full_key = self.make_key(&format!("events/{}/{}", repository, job_id), &key);
            match self.get_object(&full_key).await {
                Ok(data) => {
                    match serde_json::from_slice::<EventEntry>(&data) {
                        Ok(event) => {
                            if let Some(since_ts) = since {
                                if event.timestamp > since_ts {
                                    events.push(event);
                                }
                            } else {
                                events.push(event);
                            }
                        }
                        Err(e) => warn!("Failed to deserialize event: {}", e),
                    }
                }
                Err(e) => warn!("Failed to read event {}: {}", key, e),
            }
        }

        // Sort by timestamp
        events.sort_by_key(|e| e.timestamp);

        Ok(events)
    }

    async fn list_job_ids(&self, repository: &str) -> StorageResult<Vec<String>> {
        debug!("Listing job IDs for repository: {}", repository);

        let index_key = self.make_key("job_index", repository);
        match self.get_object(&index_key).await {
            Ok(data) => {
                let job_ids = serde_json::from_slice(&data)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;
                Ok(job_ids)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    async fn cleanup_old_events(&self, repository: &str, older_than: DateTime<Utc>) -> StorageResult<u64> {
        debug!("Cleaning up old events for repository: {}", repository);

        let job_ids = self.list_job_ids(repository).await?;
        let mut total_deleted = 0u64;

        for job_id in job_ids {
            let prefix = format!("events/{}/{}/", repository, job_id);
            let keys = self.list_keys(&prefix).await?;

            let mut delete_objects = Vec::new();

            for key in keys {
                let full_key = self.make_key(&format!("events/{}/{}", repository, job_id), &key);

                // Parse timestamp from key name (first 20 digits)
                if let Some(timestamp_str) = key.split('_').next() {
                    if let Ok(timestamp_nanos) = timestamp_str.parse::<i64>() {
                        let timestamp = DateTime::from_timestamp(
                            timestamp_nanos / 1_000_000_000,
                            (timestamp_nanos % 1_000_000_000) as u32,
                        );

                        if let Some(ts) = timestamp {
                            if ts < older_than {
                                delete_objects.push(
                                    ObjectIdentifier::builder()
                                        .key(full_key)
                                        .build()
                                        .map_err(|e| StorageError::io_error(e.to_string()))?
                                );
                            }
                        }
                    }
                }
            }

            if !delete_objects.is_empty() {
                let delete_count = delete_objects.len() as u64;

                // Delete in batches of 1000 (S3 limit)
                for chunk in delete_objects.chunks(1000) {
                    let delete = Delete::builder()
                        .set_objects(Some(chunk.to_vec()))
                        .build()
                        .map_err(|e| StorageError::io_error(e.to_string()))?;

                    self.client
                        .delete_objects()
                        .bucket(&self.bucket)
                        .delete(delete)
                        .send()
                        .await
                        .map_err(|e| StorageError::io_error(format!("Failed to delete S3 objects: {}", e)))?;
                }

                total_deleted += delete_count;
            }
        }

        Ok(total_deleted)
    }
}

#[async_trait]
impl StateStorage for S3Backend {
    async fn save_job_state(&self, job_id: &str, state: &JobState) -> StorageResult<()> {
        debug!("Saving job state: {}", job_id);

        let key = self.make_key("job_states", job_id);
        let content = serde_json::to_vec(state)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.put_object(&key, content).await?;

        // Update repository index
        let index_key = self.make_key("job_state_index", &state.repository);
        let mut index = match self.get_object(&index_key).await {
            Ok(data) => {
                serde_json::from_slice::<Vec<String>>(&data).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        };

        if !index.contains(&job_id.to_string()) {
            index.push(job_id.to_string());
            let index_content = serde_json::to_vec(&index)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            self.put_object(&index_key, index_content).await?;
        }

        Ok(())
    }

    async fn load_job_state(&self, job_id: &str) -> StorageResult<JobState> {
        debug!("Loading job state: {}", job_id);

        let key = self.make_key("job_states", job_id);
        let data = self.get_object(&key).await?;

        let state = serde_json::from_slice(&data)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(state)
    }

    async fn save_checkpoint(
        &self,
        checkpoint_id: &str,
        checkpoint: &WorkflowCheckpoint,
    ) -> StorageResult<()> {
        debug!("Saving checkpoint: {}", checkpoint_id);

        let key = self.make_key("checkpoints", checkpoint_id);
        let content = serde_json::to_vec(checkpoint)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.put_object(&key, content).await?;

        // Update repository index with timestamp for sorting
        let index_key = self.make_key("checkpoint_index", &checkpoint.repository);
        let mut index = match self.get_object(&index_key).await {
            Ok(data) => {
                serde_json::from_slice::<Vec<(String, i64)>>(&data).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        };

        // Remove existing entry if present
        index.retain(|(id, _)| id != checkpoint_id);
        // Add new entry with timestamp
        index.push((checkpoint_id.to_string(), checkpoint.created_at.timestamp()));

        // Sort by timestamp descending
        index.sort_by(|a, b| b.1.cmp(&a.1));

        let index_content = serde_json::to_vec(&index)
            .map_err(|e| StorageError::serialization(e.to_string()))?;
        self.put_object(&index_key, index_content).await?;

        Ok(())
    }

    async fn load_checkpoint(&self, checkpoint_id: &str) -> StorageResult<WorkflowCheckpoint> {
        debug!("Loading checkpoint: {}", checkpoint_id);

        let key = self.make_key("checkpoints", checkpoint_id);
        let data = self.get_object(&key).await?;

        let checkpoint = serde_json::from_slice(&data)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(checkpoint)
    }

    async fn list_checkpoints(&self, repository: &str) -> StorageResult<Vec<String>> {
        debug!("Listing checkpoints for repository: {}", repository);

        let index_key = self.make_key("checkpoint_index", repository);
        match self.get_object(&index_key).await {
            Ok(data) => {
                let index: Vec<(String, i64)> = serde_json::from_slice(&data)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;

                // Return just the IDs (already sorted by timestamp)
                Ok(index.into_iter().map(|(id, _)| id).collect())
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    async fn delete_checkpoint(&self, checkpoint_id: &str) -> StorageResult<()> {
        debug!("Deleting checkpoint: {}", checkpoint_id);

        // First load checkpoint to get repository
        let checkpoint = self.load_checkpoint(checkpoint_id).await?;

        let key = self.make_key("checkpoints", checkpoint_id);
        self.delete_object(&key).await?;

        // Update repository index
        let index_key = self.make_key("checkpoint_index", &checkpoint.repository);
        if let Ok(data) = self.get_object(&index_key).await {
            let mut index: Vec<(String, i64)> = serde_json::from_slice(&data)
                .unwrap_or_default();
            index.retain(|(id, _)| id != checkpoint_id);

            let index_content = serde_json::to_vec(&index)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            self.put_object(&index_key, index_content).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl HealthCheck for S3Backend {
    async fn health_check(&self) -> StorageResult<HealthStatus> {
        debug!("Performing S3 health check");

        let start = std::time::Instant::now();

        // Try to list objects with a small limit to test connectivity
        match self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&self.prefix)
            .max_keys(1)
            .send()
            .await
        {
            Ok(_) => {
                let latency = start.elapsed();

                let mut details = HashMap::new();
                details.insert("bucket".to_string(), self.bucket.clone());
                details.insert("prefix".to_string(), self.prefix.clone());
                details.insert("region".to_string(), self.config.region.clone());

                if self.config.endpoint.is_some() {
                    details.insert("endpoint".to_string(), self.config.endpoint.clone().unwrap());
                }

                details.insert(
                    "encryption".to_string(),
                    self.config.enable_encryption.to_string(),
                );
                details.insert(
                    "storage_class".to_string(),
                    format!("{:?}", self.config.storage_class),
                );

                Ok(HealthStatus {
                    healthy: true,
                    backend_type: "s3".to_string(),
                    latency,
                    details,
                })
            }
            Err(e) => {
                error!("S3 health check failed: {}", e);
                Ok(HealthStatus {
                    healthy: false,
                    backend_type: "s3".to_string(),
                    latency: start.elapsed(),
                    details: HashMap::from([
                        ("error".to_string(), e.to_string()),
                        ("bucket".to_string(), self.bucket.clone()),
                    ]),
                })
            }
        }
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

    fn state_storage(&self) -> &dyn StateStorage {
        self
    }

    async fn health_check(&self) -> StorageResult<HealthStatus> {
        HealthCheck::health_check(self).await
    }
}