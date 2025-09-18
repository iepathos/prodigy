//! PostgreSQL storage backend implementation

use super::super::config::PostgresConfig;
use super::super::error::{StorageError, StorageResult};
use super::super::lock::StorageLockGuard;
use super::super::traits::{
    CheckpointStorage, DLQStorage, EventStorage, SessionStorage, UnifiedStorage, WorkflowStorage,
};
use super::super::types::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::{ConnectOptions, Row};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// PostgreSQL storage backend
pub struct PostgresBackend {
    pool: Arc<PgPool>,
    #[allow(dead_code)]
    config: PostgresConfig,
    schema: String,
}

impl PostgresBackend {
    /// Create new PostgreSQL backend
    pub async fn new(config: &PostgresConfig) -> StorageResult<Self> {
        info!("Initializing PostgreSQL backend");

        // Parse connection options
        let mut connect_options = PgConnectOptions::from_str(&config.connection_string)
            .map_err(|e| StorageError::connection(format!("Invalid connection string: {}", e)))?;

        // Configure SSL mode
        connect_options = match config.ssl_mode {
            super::super::config::SslMode::Disable => {
                connect_options.ssl_mode(sqlx::postgres::PgSslMode::Disable)
            }
            super::super::config::SslMode::Prefer => {
                connect_options.ssl_mode(sqlx::postgres::PgSslMode::Prefer)
            }
            super::super::config::SslMode::Require => {
                connect_options.ssl_mode(sqlx::postgres::PgSslMode::Require)
            }
            super::super::config::SslMode::VerifyCa => {
                connect_options.ssl_mode(sqlx::postgres::PgSslMode::VerifyCa)
            }
            super::super::config::SslMode::VerifyFull => {
                connect_options.ssl_mode(sqlx::postgres::PgSslMode::VerifyFull)
            }
        };

        // Set timeouts
        connect_options = connect_options
            .statement_cache_capacity(100)
            .log_statements(tracing::log::LevelFilter::Debug)
            .log_slow_statements(tracing::log::LevelFilter::Warn, Duration::from_secs(1));

        // Create connection pool
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.connection_timeout)
            .idle_timeout(Some(Duration::from_secs(600)))
            .max_lifetime(Some(Duration::from_secs(3600)))
            .test_before_acquire(true)
            .connect_with(connect_options)
            .await
            .map_err(|e| {
                StorageError::connection(format!("Failed to connect to database: {}", e))
            })?;

        let backend = Self {
            pool: Arc::new(pool),
            config: config.clone(),
            schema: config.schema.clone(),
        };

        // Initialize schema
        backend.initialize_schema().await?;

        Ok(backend)
    }

    /// Initialize database schema
    async fn initialize_schema(&self) -> StorageResult<()> {
        info!("Initializing PostgreSQL schema: {}", self.schema);

        // Create schema if not exists
        let query = format!("CREATE SCHEMA IF NOT EXISTS {}", self.schema);
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to create schema: {}", e)))?;

        // Create sessions table
        let query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.sessions (
                session_id VARCHAR(255) PRIMARY KEY,
                repository VARCHAR(255) NOT NULL,
                status VARCHAR(50) NOT NULL,
                started_at TIMESTAMPTZ NOT NULL,
                completed_at TIMESTAMPTZ,
                workflow_path VARCHAR(512),
                git_branch VARCHAR(255),
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                StorageError::io_error(format!("Failed to create sessions table: {}", e))
            })?;

        // Create events table
        let query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.events (
                id BIGSERIAL PRIMARY KEY,
                repository VARCHAR(255) NOT NULL,
                job_id VARCHAR(255) NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                event_type VARCHAR(100) NOT NULL,
                work_item_id VARCHAR(255),
                agent_id VARCHAR(255),
                correlation_id UUID,
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to create events table: {}", e)))?;

        // Create indexes for events table
        let index_queries = vec![
            format!("CREATE INDEX IF NOT EXISTS idx_events_job ON {}.events (repository, job_id, timestamp)", self.schema),
            format!("CREATE INDEX IF NOT EXISTS idx_events_correlation ON {}.events (correlation_id)", self.schema),
            format!("CREATE INDEX IF NOT EXISTS idx_events_type ON {}.events (event_type)", self.schema),
        ];

        for query in index_queries {
            sqlx::query(&query)
                .execute(&*self.pool)
                .await
                .map_err(|e| {
                    StorageError::io_error(format!("Failed to create events index: {}", e))
                })?;
        }

        // Create job_states table
        let query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.job_states (
                job_id VARCHAR(255) PRIMARY KEY,
                repository VARCHAR(255) NOT NULL,
                workflow_name VARCHAR(255) NOT NULL,
                status VARCHAR(50) NOT NULL,
                started_at TIMESTAMPTZ NOT NULL,
                completed_at TIMESTAMPTZ,
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                StorageError::io_error(format!("Failed to create job_states table: {}", e))
            })?;

        // Create indexes for job_states table
        let query = format!(
            "CREATE INDEX IF NOT EXISTS idx_job_states_repo ON {}.job_states (repository, job_id)",
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                StorageError::io_error(format!("Failed to create job_states index: {}", e))
            })?;

        // Create checkpoints table
        let query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.checkpoints (
                checkpoint_id VARCHAR(255) PRIMARY KEY,
                repository VARCHAR(255) NOT NULL,
                session_id VARCHAR(255),
                job_id VARCHAR(255),
                created_at TIMESTAMPTZ NOT NULL,
                data JSONB NOT NULL,
                metadata JSONB,
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                StorageError::io_error(format!("Failed to create checkpoints table: {}", e))
            })?;

        // Create indexes for checkpoints table
        let index_queries = vec![
            format!(
                "CREATE INDEX IF NOT EXISTS idx_checkpoints_session ON {}.checkpoints (session_id)",
                self.schema
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_checkpoints_job ON {}.checkpoints (job_id)",
                self.schema
            ),
        ];

        for query in index_queries {
            sqlx::query(&query)
                .execute(&*self.pool)
                .await
                .map_err(|e| {
                    StorageError::io_error(format!("Failed to create checkpoints index: {}", e))
                })?;
        }

        info!("PostgreSQL schema initialized successfully");
        Ok(())
    }

    /// Convert SQL error to storage error
    fn sql_error(e: sqlx::Error) -> StorageError {
        match e {
            sqlx::Error::RowNotFound => StorageError::not_found("Record not found"),
            sqlx::Error::Database(db_err) => StorageError::io_error(db_err.to_string()),
            _ => StorageError::io_error(e.to_string()),
        }
    }
}

#[async_trait]
impl SessionStorage for PostgresBackend {
    async fn save(&self, session: &PersistedSession) -> StorageResult<()> {
        debug!("Saving session: {}", session.id.0);

        let data = serde_json::to_value(session)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let query = format!(
            r#"
            INSERT INTO {}.sessions (session_id, repository, status, started_at, completed_at, workflow_path, git_branch, data)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (session_id) DO UPDATE
            SET status = $3, completed_at = $5, data = $8, updated_at = NOW()
            "#,
            self.schema
        );

        sqlx::query(&query)
            .bind(&session.id.0)
            .bind(session.worktree_name.as_deref().unwrap_or(""))
            .bind(format!("{:?}", session.state))
            .bind(session.started_at)
            .bind(session.updated_at)
            .bind("")
            .bind("")
            .bind(&data)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn load(&self, id: &SessionId) -> StorageResult<Option<PersistedSession>> {
        debug!("Loading session: {}", id.0);

        let query = format!(
            "SELECT data FROM {}.sessions WHERE session_id = $1",
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(&id.0)
            .fetch_optional(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        match row {
            Some(r) => {
                let data: JsonValue = r.get("data");
                let session = serde_json::from_value(data)
                    .map_err(|e| StorageError::deserialization(e.to_string()))?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, filter: SessionFilter) -> StorageResult<Vec<SessionId>> {
        debug!("Listing sessions with filter: {:?}", filter);

        let query = format!(
            "SELECT session_id FROM {}.sessions ORDER BY started_at DESC",
            self.schema
        );

        let rows = sqlx::query(&query)
            .fetch_all(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let mut ids = Vec::new();
        for row in rows {
            let id: String = row.get("session_id");
            ids.push(SessionId(id));
        }

        Ok(ids)
    }

    async fn delete(&self, id: &SessionId) -> StorageResult<()> {
        debug!("Deleting session: {}", id.0);

        let query = format!("DELETE FROM {}.sessions WHERE session_id = $1", self.schema);

        sqlx::query(&query)
            .bind(&id.0)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn update_state(&self, id: &SessionId, state: SessionState) -> StorageResult<()> {
        debug!("Updating session state: {} to {:?}", id.0, state);

        let query = format!(
            "UPDATE {}.sessions SET status = $2, updated_at = NOW() WHERE session_id = $1",
            self.schema
        );

        sqlx::query(&query)
            .bind(&id.0)
            .bind(format!("{:?}", state))
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn get_stats(&self, id: &SessionId) -> StorageResult<SessionStats> {
        debug!("Getting session stats: {}", id.0);

        // For now, return a simple implementation
        Ok(SessionStats {
            total_duration: Duration::from_secs(0),
            commands_executed: 0,
            errors_encountered: 0,
            files_modified: 0,
        })
    }
}

#[async_trait]
impl EventStorage for PostgresBackend {
    async fn append(&self, events: Vec<EventRecord>) -> StorageResult<()> {
        debug!("Appending {} events", events.len());

        for event in events {
            let data = serde_json::to_value(&event)
                .map_err(|e| StorageError::serialization(e.to_string()))?;

            let query = format!(
                r#"
                INSERT INTO {}.events (repository, job_id, timestamp, event_type, work_item_id, agent_id, correlation_id, data)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                self.schema
            );

            sqlx::query(&query)
                .bind("default")
                .bind(&event.job_id)
                .bind(event.timestamp)
                .bind(&event.event_type)
                .bind(&event.id)
                .bind(&event.agent_id)
                .bind(&event.correlation_id)
                .bind(&data)
                .execute(&*self.pool)
                .await
                .map_err(Self::sql_error)?;
        }

        Ok(())
    }

    async fn query(&self, filter: EventFilter) -> StorageResult<EventStream> {
        use futures::stream;
        debug!("Querying events with filter: {:?}", filter);

        // For now, return an empty stream
        Ok(Box::pin(stream::empty()))
    }

    async fn aggregate(&self, job_id: &str) -> StorageResult<EventStats> {
        debug!("Aggregating events for job: {}", job_id);

        // Return empty stats for now
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
        debug!("Subscribing to events with filter: {:?}", filter);

        // Create a channel for subscription
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();

        Ok(EventSubscription {
            id: uuid::Uuid::new_v4().to_string(),
            filter,
            receiver: rx,
        })
    }

    async fn count(&self, filter: EventFilter) -> StorageResult<usize> {
        debug!("Counting events with filter: {:?}", filter);
        Ok(0)
    }

    async fn archive(&self, before: DateTime<Utc>) -> StorageResult<usize> {
        debug!("Archiving events before: {}", before);
        Ok(0)
    }
}

#[async_trait]
impl CheckpointStorage for PostgresBackend {
    async fn save(&self, checkpoint: &WorkflowCheckpoint) -> StorageResult<()> {
        debug!("Saving checkpoint: {}", checkpoint.id);

        let data = serde_json::to_value(&checkpoint.state)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let query = format!(
            r#"
            INSERT INTO {}.checkpoints (checkpoint_id, repository, job_id, created_at, data, metadata)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (checkpoint_id) DO UPDATE
            SET data = $5, metadata = $6
            "#,
            self.schema
        );

        // Convert string values to JSON values for storage
        let json_variables: HashMap<String, JsonValue> = checkpoint
            .variables
            .iter()
            .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
            .collect();
        let metadata = serde_json::to_value(&json_variables)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        sqlx::query(&query)
            .bind(&checkpoint.id)
            .bind("default")
            .bind(&checkpoint.workflow_id)
            .bind(checkpoint.created_at)
            .bind(&data)
            .bind(&metadata)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        debug!("Loading checkpoint: {}", id);

        let query = format!(
            "SELECT repository, job_id, created_at, data, metadata FROM {}.checkpoints WHERE checkpoint_id = $1",
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(id)
            .fetch_optional(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        match row {
            Some(r) => {
                let data: JsonValue = r.get("data");
                let metadata: JsonValue = r.get("metadata");
                let json_variables: HashMap<String, JsonValue> =
                    serde_json::from_value(metadata).unwrap_or_default();
                // Convert JSON values to strings
                let variables: HashMap<String, String> = json_variables
                    .into_iter()
                    .map(|(k, v)| {
                        let value_str = match v {
                            JsonValue::String(s) => s,
                            _ => v.to_string(),
                        };
                        (k, value_str)
                    })
                    .collect();
                let checkpoint = WorkflowCheckpoint {
                    id: id.to_string(),
                    workflow_id: r.get("job_id"),
                    created_at: r.get("created_at"),
                    step_index: 0,
                    completed_steps: Vec::new(),
                    variables,
                    state: data,
                };
                Ok(Some(checkpoint))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, filter: CheckpointFilter) -> StorageResult<Vec<CheckpointInfo>> {
        debug!("Listing checkpoints with filter: {:?}", filter);

        let query = format!(
            "SELECT checkpoint_id, job_id, created_at FROM {}.checkpoints ORDER BY created_at DESC",
            self.schema
        );

        let rows = sqlx::query(&query)
            .fetch_all(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let mut infos = Vec::new();
        for row in rows {
            infos.push(CheckpointInfo {
                id: row.get("checkpoint_id"),
                workflow_id: row.get("job_id"),
                created_at: row.get("created_at"),
                step_index: 0,
                size_bytes: 0,
            });
        }

        Ok(infos)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting checkpoint: {}", id);

        let query = format!(
            "DELETE FROM {}.checkpoints WHERE checkpoint_id = $1",
            self.schema
        );

        sqlx::query(&query)
            .bind(id)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn get_latest(&self, workflow_id: &str) -> StorageResult<Option<WorkflowCheckpoint>> {
        debug!("Getting latest checkpoint for workflow: {}", workflow_id);

        let query = format!(
            "SELECT checkpoint_id, created_at, data, metadata FROM {}.checkpoints WHERE job_id = $1 ORDER BY created_at DESC LIMIT 1",
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(workflow_id)
            .fetch_optional(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        match row {
            Some(r) => {
                let data: JsonValue = r.get("data");
                let metadata: JsonValue = r.get("metadata");
                let json_variables: HashMap<String, JsonValue> =
                    serde_json::from_value(metadata).unwrap_or_default();
                // Convert JSON values to strings
                let variables: HashMap<String, String> = json_variables
                    .into_iter()
                    .map(|(k, v)| {
                        let value_str = match v {
                            JsonValue::String(s) => s,
                            _ => v.to_string(),
                        };
                        (k, value_str)
                    })
                    .collect();
                let checkpoint = WorkflowCheckpoint {
                    id: r.get("checkpoint_id"),
                    workflow_id: workflow_id.to_string(),
                    created_at: r.get("created_at"),
                    step_index: 0,
                    completed_steps: Vec::new(),
                    variables,
                    state: data,
                };
                Ok(Some(checkpoint))
            }
            None => Ok(None),
        }
    }

    async fn cleanup(&self, keep_last: usize) -> StorageResult<usize> {
        debug!("Cleaning up checkpoints, keeping last {}", keep_last);
        Ok(0)
    }
}

#[async_trait]
impl DLQStorage for PostgresBackend {
    async fn enqueue(&self, item: DLQItem) -> StorageResult<()> {
        debug!("Enqueueing DLQ item: {}", item.id);
        Ok(())
    }

    async fn dequeue(&self, limit: usize) -> StorageResult<Vec<DLQItem>> {
        debug!("Dequeueing {} DLQ items", limit);
        Ok(Vec::new())
    }

    async fn list(&self, filter: DLQFilter) -> StorageResult<Vec<DLQItem>> {
        debug!("Listing DLQ items with filter: {:?}", filter);
        Ok(Vec::new())
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting DLQ item: {}", id);
        Ok(())
    }

    async fn mark_processed(&self, id: &str) -> StorageResult<()> {
        debug!("Marking DLQ item as processed: {}", id);
        Ok(())
    }

    async fn get_stats(&self, job_id: &str) -> StorageResult<DLQStats> {
        debug!("Getting DLQ stats for job: {}", job_id);
        Ok(DLQStats {
            total_items: 0,
            items_by_retry_count: HashMap::new(),
            oldest_item: None,
            newest_item: None,
            average_retry_count: 0.0,
        })
    }

    async fn purge(&self, older_than: Duration) -> StorageResult<usize> {
        debug!("Purging DLQ items older than: {:?}", older_than);
        Ok(0)
    }
}

#[async_trait]
impl WorkflowStorage for PostgresBackend {
    async fn save(&self, workflow: &WorkflowDefinition) -> StorageResult<()> {
        debug!("Saving workflow: {}", workflow.id);
        Ok(())
    }

    async fn load(&self, id: &str) -> StorageResult<Option<WorkflowDefinition>> {
        debug!("Loading workflow: {}", id);
        Ok(None)
    }

    async fn list(&self, filter: WorkflowFilter) -> StorageResult<Vec<WorkflowInfo>> {
        debug!("Listing workflows with filter: {:?}", filter);
        Ok(Vec::new())
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        debug!("Deleting workflow: {}", id);
        Ok(())
    }

    async fn update_metadata(&self, id: &str, _metadata: WorkflowMetadata) -> StorageResult<()> {
        debug!("Updating workflow metadata: {}", id);
        Ok(())
    }

    async fn get_history(&self, id: &str) -> StorageResult<Vec<WorkflowExecution>> {
        debug!("Getting workflow history: {}", id);
        Ok(Vec::new())
    }
}

#[async_trait]
impl UnifiedStorage for PostgresBackend {
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
        debug!("Acquiring lock for key: {} with TTL: {:?}", key, ttl);
        Err(StorageError::operation("Lock acquisition not implemented"))
    }

    async fn health_check(&self) -> StorageResult<HealthStatus> {
        debug!("Performing health check");

        let start = std::time::Instant::now();
        match sqlx::query("SELECT 1").fetch_one(&*self.pool).await {
            Ok(_) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                Ok(HealthStatus {
                    healthy: true,
                    backend_type: "postgres".to_string(),
                    connection_status: ConnectionStatus::Connected,
                    latency_ms,
                    errors: Vec::new(),
                })
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                Ok(HealthStatus {
                    healthy: false,
                    backend_type: "postgres".to_string(),
                    connection_status: ConnectionStatus::Disconnected,
                    latency_ms,
                    errors: vec![e.to_string()],
                })
            }
        }
    }

    async fn get_metrics(&self) -> StorageResult<StorageMetrics> {
        debug!("Getting storage metrics");
        Ok(StorageMetrics {
            operations_total: 0,
            operations_failed: 0,
            average_latency_ms: 0.0,
            storage_size_bytes: 0,
            active_connections: self.pool.size(),
        })
    }
}
