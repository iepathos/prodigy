//! PostgreSQL storage backend implementation

use super::super::config::PostgresConfig;
use super::super::error::{StorageError, StorageResult};
use super::super::traits::{
    EventStorage, HealthCheck, SessionStorage, StateStorage, UnifiedStorage,
};
use super::super::types::{
    CheckpointData, EventEntry, HealthStatus, JobState, SessionState, WorkflowCheckpoint,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::{ConnectOptions, Row};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// PostgreSQL storage backend
pub struct PostgresBackend {
    pool: Arc<PgPool>,
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
            .map_err(|e| StorageError::connection(format!("Failed to connect to database: {}", e)))?;

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
            .map_err(|e| StorageError::io_error(format!("Failed to create sessions table: {}", e)))?;

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
                INDEX idx_events_job (repository, job_id, timestamp),
                INDEX idx_events_correlation (correlation_id),
                INDEX idx_events_type (event_type)
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to create events table: {}", e)))?;

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
                INDEX idx_job_states_repo (repository, job_id)
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to create job_states table: {}", e)))?;

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
                INDEX idx_checkpoints_session (session_id),
                INDEX idx_checkpoints_job (job_id)
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| StorageError::io_error(format!("Failed to create checkpoints table: {}", e)))?;

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
    async fn save_session(&self, session: &SessionState) -> StorageResult<()> {
        debug!("Saving session: {}", session.session_id);

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
            .bind(&session.session_id)
            .bind(&session.repository)
            .bind(format!("{:?}", session.status))
            .bind(&session.started_at)
            .bind(&session.completed_at)
            .bind(&session.workflow_path)
            .bind(&session.git_branch)
            .bind(&data)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn load_session(&self, session_id: &str) -> StorageResult<SessionState> {
        debug!("Loading session: {}", session_id);

        let query = format!(
            "SELECT data FROM {}.sessions WHERE session_id = $1",
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(session_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let data: JsonValue = row.get("data");
        let session = serde_json::from_value(data)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(session)
    }

    async fn list_sessions(&self, repository: &str) -> StorageResult<Vec<SessionState>> {
        debug!("Listing sessions for repository: {}", repository);

        let query = format!(
            "SELECT data FROM {}.sessions WHERE repository = $1 ORDER BY started_at DESC",
            self.schema
        );

        let rows = sqlx::query(&query)
            .bind(repository)
            .fetch_all(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let mut sessions = Vec::new();
        for row in rows {
            let data: JsonValue = row.get("data");
            let session = serde_json::from_value(data)
                .map_err(|e| StorageError::deserialization(e.to_string()))?;
            sessions.push(session);
        }

        Ok(sessions)
    }

    async fn delete_session(&self, session_id: &str) -> StorageResult<()> {
        debug!("Deleting session: {}", session_id);

        let query = format!(
            "DELETE FROM {}.sessions WHERE session_id = $1",
            self.schema
        );

        sqlx::query(&query)
            .bind(session_id)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }
}

#[async_trait]
impl EventStorage for PostgresBackend {
    async fn append_event(&self, repository: &str, job_id: &str, event: &EventEntry) -> StorageResult<()> {
        debug!("Appending event for job: {}", job_id);

        let data = serde_json::to_value(event)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let query = format!(
            r#"
            INSERT INTO {}.events (repository, job_id, timestamp, event_type, work_item_id, agent_id, correlation_id, data)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            self.schema
        );

        sqlx::query(&query)
            .bind(repository)
            .bind(job_id)
            .bind(&event.timestamp)
            .bind(&event.event_type)
            .bind(&event.work_item_id)
            .bind(&event.agent_id)
            .bind(&event.correlation_id)
            .bind(&data)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn read_events(
        &self,
        repository: &str,
        job_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> StorageResult<Vec<EventEntry>> {
        debug!("Reading events for job: {}", job_id);

        let query = if let Some(since_ts) = since {
            format!(
                "SELECT data FROM {}.events WHERE repository = $1 AND job_id = $2 AND timestamp > $3 ORDER BY timestamp",
                self.schema
            )
        } else {
            format!(
                "SELECT data FROM {}.events WHERE repository = $1 AND job_id = $2 ORDER BY timestamp",
                self.schema
            )
        };

        let rows = if let Some(since_ts) = since {
            sqlx::query(&query)
                .bind(repository)
                .bind(job_id)
                .bind(since_ts)
                .fetch_all(&*self.pool)
                .await
                .map_err(Self::sql_error)?
        } else {
            sqlx::query(&query)
                .bind(repository)
                .bind(job_id)
                .fetch_all(&*self.pool)
                .await
                .map_err(Self::sql_error)?
        };

        let mut events = Vec::new();
        for row in rows {
            let data: JsonValue = row.get("data");
            let event = serde_json::from_value(data)
                .map_err(|e| StorageError::deserialization(e.to_string()))?;
            events.push(event);
        }

        Ok(events)
    }

    async fn list_job_ids(&self, repository: &str) -> StorageResult<Vec<String>> {
        debug!("Listing job IDs for repository: {}", repository);

        let query = format!(
            "SELECT DISTINCT job_id FROM {}.events WHERE repository = $1 ORDER BY job_id DESC",
            self.schema
        );

        let rows = sqlx::query(&query)
            .bind(repository)
            .fetch_all(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let mut job_ids = Vec::new();
        for row in rows {
            let job_id: String = row.get("job_id");
            job_ids.push(job_id);
        }

        Ok(job_ids)
    }

    async fn cleanup_old_events(&self, repository: &str, older_than: DateTime<Utc>) -> StorageResult<u64> {
        debug!("Cleaning up old events for repository: {}", repository);

        let query = format!(
            "DELETE FROM {}.events WHERE repository = $1 AND timestamp < $2",
            self.schema
        );

        let result = sqlx::query(&query)
            .bind(repository)
            .bind(older_than)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(result.rows_affected())
    }
}

#[async_trait]
impl StateStorage for PostgresBackend {
    async fn save_job_state(&self, job_id: &str, state: &JobState) -> StorageResult<()> {
        debug!("Saving job state: {}", job_id);

        let data = serde_json::to_value(state)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let query = format!(
            r#"
            INSERT INTO {}.job_states (job_id, repository, workflow_name, status, started_at, completed_at, data)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (job_id) DO UPDATE
            SET status = $4, completed_at = $6, data = $7, updated_at = NOW()
            "#,
            self.schema
        );

        sqlx::query(&query)
            .bind(job_id)
            .bind(&state.repository)
            .bind(&state.workflow_name)
            .bind(format!("{:?}", state.status))
            .bind(&state.started_at)
            .bind(&state.completed_at)
            .bind(&data)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn load_job_state(&self, job_id: &str) -> StorageResult<JobState> {
        debug!("Loading job state: {}", job_id);

        let query = format!(
            "SELECT data FROM {}.job_states WHERE job_id = $1",
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(job_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let data: JsonValue = row.get("data");
        let state = serde_json::from_value(data)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(state)
    }

    async fn save_checkpoint(
        &self,
        checkpoint_id: &str,
        checkpoint: &WorkflowCheckpoint,
    ) -> StorageResult<()> {
        debug!("Saving checkpoint: {}", checkpoint_id);

        let data = serde_json::to_value(&checkpoint.data)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let metadata = serde_json::to_value(&checkpoint.metadata)
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let query = format!(
            r#"
            INSERT INTO {}.checkpoints (checkpoint_id, repository, session_id, job_id, created_at, data, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (checkpoint_id) DO UPDATE
            SET data = $6, metadata = $7
            "#,
            self.schema
        );

        sqlx::query(&query)
            .bind(checkpoint_id)
            .bind(&checkpoint.repository)
            .bind(&checkpoint.session_id)
            .bind(&checkpoint.job_id)
            .bind(&checkpoint.created_at)
            .bind(&data)
            .bind(&metadata)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }

    async fn load_checkpoint(&self, checkpoint_id: &str) -> StorageResult<WorkflowCheckpoint> {
        debug!("Loading checkpoint: {}", checkpoint_id);

        let query = format!(
            "SELECT repository, session_id, job_id, created_at, data, metadata FROM {}.checkpoints WHERE checkpoint_id = $1",
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(checkpoint_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let data_json: JsonValue = row.get("data");
        let metadata_json: JsonValue = row.get("metadata");

        let data = serde_json::from_value(data_json)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        let metadata = serde_json::from_value(metadata_json)
            .map_err(|e| StorageError::deserialization(e.to_string()))?;

        Ok(WorkflowCheckpoint {
            repository: row.get("repository"),
            session_id: row.get("session_id"),
            job_id: row.get("job_id"),
            created_at: row.get("created_at"),
            data,
            metadata,
        })
    }

    async fn list_checkpoints(&self, repository: &str) -> StorageResult<Vec<String>> {
        debug!("Listing checkpoints for repository: {}", repository);

        let query = format!(
            "SELECT checkpoint_id FROM {}.checkpoints WHERE repository = $1 ORDER BY created_at DESC",
            self.schema
        );

        let rows = sqlx::query(&query)
            .bind(repository)
            .fetch_all(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        let mut checkpoint_ids = Vec::new();
        for row in rows {
            let checkpoint_id: String = row.get("checkpoint_id");
            checkpoint_ids.push(checkpoint_id);
        }

        Ok(checkpoint_ids)
    }

    async fn delete_checkpoint(&self, checkpoint_id: &str) -> StorageResult<()> {
        debug!("Deleting checkpoint: {}", checkpoint_id);

        let query = format!(
            "DELETE FROM {}.checkpoints WHERE checkpoint_id = $1",
            self.schema
        );

        sqlx::query(&query)
            .bind(checkpoint_id)
            .execute(&*self.pool)
            .await
            .map_err(Self::sql_error)?;

        Ok(())
    }
}

#[async_trait]
impl HealthCheck for PostgresBackend {
    async fn health_check(&self) -> StorageResult<HealthStatus> {
        debug!("Performing PostgreSQL health check");

        // Try to acquire a connection and run a simple query
        let start = std::time::Instant::now();

        match sqlx::query("SELECT 1")
            .fetch_one(&*self.pool)
            .await
        {
            Ok(_) => {
                let latency = start.elapsed();
                Ok(HealthStatus {
                    healthy: true,
                    backend_type: "postgres".to_string(),
                    latency,
                    details: HashMap::from([
                        ("pool_size".to_string(), self.pool.size().to_string()),
                        ("idle_connections".to_string(), self.pool.num_idle().to_string()),
                        ("schema".to_string(), self.schema.clone()),
                    ]),
                })
            }
            Err(e) => {
                error!("PostgreSQL health check failed: {}", e);
                Ok(HealthStatus {
                    healthy: false,
                    backend_type: "postgres".to_string(),
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
impl UnifiedStorage for PostgresBackend {
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