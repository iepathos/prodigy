//! Redis storage backend implementation

use super::super::config::RedisConfig;
use super::super::error::{StorageError, StorageResult};
use super::super::traits::{
    EventStorage, HealthCheck, SessionStorage, StateStorage, UnifiedStorage,
};
use super::super::types::{
    CheckpointData, EventEntry, HealthStatus, JobState, SessionState, WorkflowCheckpoint,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_redis::{Config, Pool, Runtime};
use redis::{AsyncCommands, Script};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

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
        pool_config
            .pool
            .map(|mut p| {
                p.max_size = config.pool_size;
                p.timeouts.wait = Some(Duration::from_secs(10));
                p.timeouts.create = Some(Duration::from_secs(10));
                p.timeouts.recycle = Some(Duration::from_secs(10));
                p
            })
            .unwrap_or_else(|| {
                let mut p = deadpool::managed::PoolConfig::new(config.pool_size);
                p.timeouts.wait = Some(Duration::from_secs(10));
                p.timeouts.create = Some(Duration::from_secs(10));
                p.timeouts.recycle = Some(Duration::from_secs(10));
                pool_config.pool = Some(p);
                p
            });

        // Create pool
        let pool = pool_config
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| StorageError::connection(format!("Failed to create Redis pool: {}", e)))?;

        // Test connection
        let mut conn = pool
            .get()
            .await
            .map_err(|e| StorageError::connection(format!("Failed to connect to Redis: {}", e)))?;

        // Select database
        if config.database > 0 {
            redis::cmd("SELECT")
                .arg(config.database)
                .query_async::<_, ()>(&mut conn)
                .await
                .map_err(|e| {
                    StorageError::connection(format!("Failed to select database: {}", e))
                })?;
        }

        Ok(Self {
            pool: Arc::new(pool),
            config: config.clone(),
            key_prefix: config.key_prefix.clone(),
        })
    }

    /// Generate key with prefix
    fn make_key(&self, key_type: &str, id: &str) -> String {
        format!("{}{}:{}", self.key_prefix, key_type, id)
    }

    /// Generate pattern for key listing
    fn make_pattern(&self, key_type: &str, pattern: &str) -> String {
        format!("{}{}:{}", self.key_prefix, key_type, pattern)
    }

    /// Set key with optional TTL
    async fn set_with_ttl(
        &self,
        key: &str,
        value: &str,
        ttl: Option<Duration>,
    ) -> StorageResult<()> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        if let Some(ttl) = ttl {
            conn.set_ex(key, value, ttl.as_secs() as usize)
                .await
                .map_err(|e| StorageError::io_error(e.to_string()))?;
        } else {
            conn.set(key, value)
                .await
                .map_err(|e| StorageError::io_error(e.to_string()))?;
        }

        Ok(())
    }
}

#[async_trait]
impl SessionStorage for RedisBackend {
    async fn save_session(&self, session: &SessionState) -> StorageResult<()> {
        debug!("Saving session: {}", session.session_id);

        let key = self.make_key("session", &session.session_id);
        let value = serde_json::to_string(session)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.set_with_ttl(&key, &value, Some(self.config.default_ttl))
            .await?;

        // Also add to repository index
        let index_key = self.make_key("session_index", &session.repository);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.sadd(&index_key, &session.session_id)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn load_session(&self, session_id: &str) -> StorageResult<SessionState> {
        debug!("Loading session: {}", session_id);

        let key = self.make_key("session", session_id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let value: String = conn
            .get(&key)
            .await
            .map_err(|e| StorageError::not_found(format!("Session not found: {}", e)))?;

        let session = serde_json::from_str(&value)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(session)
    }

    async fn list_sessions(&self, repository: &str) -> StorageResult<Vec<SessionState>> {
        debug!("Listing sessions for repository: {}", repository);

        let index_key = self.make_key("session_index", repository);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let session_ids: Vec<String> = conn
            .smembers(&index_key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

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

        let key = self.make_key("session", session_id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.del(&key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        // Remove from index
        let index_key = self.make_key("session_index", &session.repository);
        conn.srem(&index_key, session_id)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl EventStorage for RedisBackend {
    async fn append_event(&self, repository: &str, job_id: &str, event: &EventEntry) -> StorageResult<()> {
        debug!("Appending event for job: {}", job_id);

        let key = self.make_key("events", &format!("{}:{}", repository, job_id));
        let value = serde_json::to_string(event)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        // Append to list
        conn.rpush(&key, &value)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        // Set TTL on the list
        conn.expire(&key, self.config.default_ttl.as_secs() as usize)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        // Add to job index
        let index_key = self.make_key("job_index", repository);
        conn.sadd(&index_key, job_id)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn read_events(
        &self,
        repository: &str,
        job_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> StorageResult<Vec<EventEntry>> {
        debug!("Reading events for job: {}", job_id);

        let key = self.make_key("events", &format!("{}:{}", repository, job_id));
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let values: Vec<String> = conn
            .lrange(&key, 0, -1)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        let mut events = Vec::new();
        for value in values {
            match serde_json::from_str::<EventEntry>(&value) {
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

        Ok(events)
    }

    async fn list_job_ids(&self, repository: &str) -> StorageResult<Vec<String>> {
        debug!("Listing job IDs for repository: {}", repository);

        let index_key = self.make_key("job_index", repository);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let job_ids: Vec<String> = conn
            .smembers(&index_key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(job_ids)
    }

    async fn cleanup_old_events(&self, repository: &str, older_than: DateTime<Utc>) -> StorageResult<u64> {
        debug!("Cleaning up old events for repository: {}", repository);

        // Get all job IDs
        let job_ids = self.list_job_ids(repository).await?;
        let mut total_deleted = 0u64;

        for job_id in job_ids {
            let key = self.make_key("events", &format!("{}:{}", repository, job_id));
            let mut conn = self
                .pool
                .get()
                .await
                .map_err(|e| StorageError::connection(e.to_string()))?;

            // Read all events
            let values: Vec<String> = conn
                .lrange(&key, 0, -1)
                .await
                .map_err(|e| StorageError::io_error(e.to_string()))?;

            let mut keep_events = Vec::new();
            let mut deleted_count = 0;

            for value in values {
                match serde_json::from_str::<EventEntry>(&value) {
                    Ok(event) => {
                        if event.timestamp >= older_than {
                            keep_events.push(value);
                        } else {
                            deleted_count += 1;
                        }
                    }
                    Err(_) => keep_events.push(value), // Keep unparseable events
                }
            }

            if deleted_count > 0 {
                // Replace the list with filtered events
                conn.del(&key)
                    .await
                    .map_err(|e| StorageError::io_error(e.to_string()))?;

                if !keep_events.is_empty() {
                    for event_value in keep_events {
                        conn.rpush(&key, &event_value)
                            .await
                            .map_err(|e| StorageError::io_error(e.to_string()))?;
                    }

                    // Reset TTL
                    conn.expire(&key, self.config.default_ttl.as_secs() as usize)
                        .await
                        .map_err(|e| StorageError::io_error(e.to_string()))?;
                }

                total_deleted += deleted_count as u64;
            }
        }

        Ok(total_deleted)
    }
}

#[async_trait]
impl StateStorage for RedisBackend {
    async fn save_job_state(&self, job_id: &str, state: &JobState) -> StorageResult<()> {
        debug!("Saving job state: {}", job_id);

        let key = self.make_key("job_state", job_id);
        let value = serde_json::to_string(state)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.set_with_ttl(&key, &value, Some(self.config.default_ttl))
            .await?;

        // Add to repository index
        let index_key = self.make_key("job_state_index", &state.repository);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.sadd(&index_key, job_id)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn load_job_state(&self, job_id: &str) -> StorageResult<JobState> {
        debug!("Loading job state: {}", job_id);

        let key = self.make_key("job_state", job_id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let value: String = conn
            .get(&key)
            .await
            .map_err(|e| StorageError::not_found(format!("Job state not found: {}", e)))?;

        let state = serde_json::from_str(&value)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(state)
    }

    async fn save_checkpoint(
        &self,
        checkpoint_id: &str,
        checkpoint: &WorkflowCheckpoint,
    ) -> StorageResult<()> {
        debug!("Saving checkpoint: {}", checkpoint_id);

        let key = self.make_key("checkpoint", checkpoint_id);
        let value = serde_json::to_string(checkpoint)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        self.set_with_ttl(&key, &value, Some(self.config.default_ttl))
            .await?;

        // Add to repository index
        let index_key = self.make_key("checkpoint_index", &checkpoint.repository);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        // Store with timestamp for sorting
        let score = checkpoint.created_at.timestamp() as f64;
        conn.zadd(&index_key, checkpoint_id, score)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }

    async fn load_checkpoint(&self, checkpoint_id: &str) -> StorageResult<WorkflowCheckpoint> {
        debug!("Loading checkpoint: {}", checkpoint_id);

        let key = self.make_key("checkpoint", checkpoint_id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        let value: String = conn
            .get(&key)
            .await
            .map_err(|e| StorageError::not_found(format!("Checkpoint not found: {}", e)))?;

        let checkpoint = serde_json::from_str(&value)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(checkpoint)
    }

    async fn list_checkpoints(&self, repository: &str) -> StorageResult<Vec<String>> {
        debug!("Listing checkpoints for repository: {}", repository);

        let index_key = self.make_key("checkpoint_index", repository);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        // Get sorted by timestamp descending
        let checkpoint_ids: Vec<String> = conn
            .zrevrange(&index_key, 0, -1)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(checkpoint_ids)
    }

    async fn delete_checkpoint(&self, checkpoint_id: &str) -> StorageResult<()> {
        debug!("Deleting checkpoint: {}", checkpoint_id);

        // First load checkpoint to get repository
        let checkpoint = self.load_checkpoint(checkpoint_id).await?;

        let key = self.make_key("checkpoint", checkpoint_id);
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| StorageError::connection(e.to_string()))?;

        conn.del(&key)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        // Remove from index
        let index_key = self.make_key("checkpoint_index", &checkpoint.repository);
        conn.zrem(&index_key, checkpoint_id)
            .await
            .map_err(|e| StorageError::io_error(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl HealthCheck for RedisBackend {
    async fn health_check(&self) -> StorageResult<HealthStatus> {
        debug!("Performing Redis health check");

        let start = std::time::Instant::now();

        match self.pool.get().await {
            Ok(mut conn) => {
                // Try a PING command
                match redis::cmd("PING")
                    .query_async::<_, String>(&mut conn)
                    .await
                {
                    Ok(response) if response == "PONG" => {
                        let latency = start.elapsed();

                        // Get additional info
                        let info: String = redis::cmd("INFO")
                            .arg("server")
                            .query_async(&mut conn)
                            .await
                            .unwrap_or_default();

                        let mut details = HashMap::new();
                        details.insert("pool_size".to_string(), self.config.pool_size.to_string());
                        details.insert("database".to_string(), self.config.database.to_string());
                        details.insert("key_prefix".to_string(), self.key_prefix.clone());

                        // Parse some basic info
                        for line in info.lines() {
                            if line.starts_with("redis_version:") {
                                if let Some(version) = line.split(':').nth(1) {
                                    details.insert("redis_version".to_string(), version.to_string());
                                }
                            }
                        }

                        Ok(HealthStatus {
                            healthy: true,
                            backend_type: "redis".to_string(),
                            latency,
                            details,
                        })
                    }
                    Ok(response) => {
                        error!("Unexpected Redis PING response: {}", response);
                        Ok(HealthStatus {
                            healthy: false,
                            backend_type: "redis".to_string(),
                            latency: start.elapsed(),
                            details: HashMap::from([
                                ("error".to_string(), format!("Unexpected PING response: {}", response)),
                            ]),
                        })
                    }
                    Err(e) => {
                        error!("Redis PING failed: {}", e);
                        Ok(HealthStatus {
                            healthy: false,
                            backend_type: "redis".to_string(),
                            latency: start.elapsed(),
                            details: HashMap::from([
                                ("error".to_string(), e.to_string()),
                            ]),
                        })
                    }
                }
            }
            Err(e) => {
                error!("Failed to get Redis connection: {}", e);
                Ok(HealthStatus {
                    healthy: false,
                    backend_type: "redis".to_string(),
                    latency: start.elapsed(),
                    details: HashMap::from([
                        ("error".to_string(), e.to_string()),
                    ]),
                })
            }
        }
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

    fn state_storage(&self) -> &dyn StateStorage {
        self
    }

    async fn health_check(&self) -> StorageResult<HealthStatus> {
        HealthCheck::health_check(self).await
    }
}