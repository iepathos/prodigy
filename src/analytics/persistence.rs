//! SQLite persistence layer for analytics data

#[cfg(feature = "postgres")]
use anyhow::{Context, Result};
#[cfg(feature = "postgres")]
use chrono::{DateTime, Utc};
#[cfg(feature = "postgres")]
use serde_json::json;
#[cfg(feature = "postgres")]
use sqlx::{sqlite::SqlitePool, Row};
#[cfg(feature = "postgres")]
use std::path::Path;
#[cfg(feature = "postgres")]
use tracing::{debug, info};

#[cfg(feature = "postgres")]
use super::models::{Session, SessionEvent, ToolInvocation};

#[cfg(feature = "postgres")]
/// Database connection pool for analytics persistence
pub struct AnalyticsDatabase {
    pool: SqlitePool,
}

#[cfg(not(feature = "postgres"))]
/// Database connection pool for analytics persistence (disabled)
pub struct AnalyticsDatabase;

impl AnalyticsDatabase {
    /// Create a new database connection
    pub async fn new(database_path: impl AsRef<Path>) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = database_path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let database_url = format!("sqlite://{}?mode=rwc", database_path.as_ref().display());
        let pool = SqlitePool::connect(&database_url)
            .await
            .with_context(|| format!("Failed to connect to analytics database at {}", database_path.as_ref().display()))?;

        let db = Self { pool };
        db.initialize_schema()
            .await
            .with_context(|| "Failed to initialize analytics database schema")?;

        info!(
            "Analytics database initialized at {}",
            database_path.as_ref().display()
        );
        Ok(db)
    }

    /// Create database schema if it doesn't exist
    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                jsonl_path TEXT NOT NULL,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                model TEXT,
                total_input_tokens INTEGER NOT NULL DEFAULT 0,
                total_output_tokens INTEGER NOT NULL DEFAULT 0,
                total_cache_tokens INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS session_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS tool_invocations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                invoked_at TEXT NOT NULL,
                duration_ms INTEGER,
                parameters TEXT NOT NULL,
                result_size INTEGER,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions(started_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_project_path ON sessions(project_path);
            CREATE INDEX IF NOT EXISTS idx_events_session_id ON session_events(session_id);
            CREATE INDEX IF NOT EXISTS idx_events_timestamp ON session_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_tools_session_id ON tool_invocations(session_id);
            CREATE INDEX IF NOT EXISTS idx_tools_name ON tool_invocations(tool_name);
            "#,
        )
        .execute(&self.pool)
        .await?;

        debug!("Database schema initialized");
        Ok(())
    }

    /// Store or update a session
    pub async fn upsert_session(&self, session: &Session) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (
                session_id, project_path, jsonl_path, started_at, completed_at,
                model, total_input_tokens, total_output_tokens, total_cache_tokens
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(session_id) DO UPDATE SET
                completed_at = excluded.completed_at,
                model = excluded.model,
                total_input_tokens = excluded.total_input_tokens,
                total_output_tokens = excluded.total_output_tokens,
                total_cache_tokens = excluded.total_cache_tokens,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&session.session_id)
        .bind(&session.project_path)
        .bind(&session.jsonl_path)
        .bind(session.started_at.to_rfc3339())
        .bind(session.completed_at.map(|dt| dt.to_rfc3339()))
        .bind(&session.model)
        .bind(session.total_input_tokens as i64)
        .bind(session.total_output_tokens as i64)
        .bind(session.total_cache_tokens as i64)
        .execute(&self.pool)
        .await?;

        // Store events
        for event in &session.events {
            self.store_event(&session.session_id, event).await?;
        }

        // Store tool invocations
        for tool in &session.tool_invocations {
            self.store_tool_invocation(&session.session_id, tool)
                .await?;
        }

        debug!("Session {} persisted", session.session_id);
        Ok(())
    }

    /// Store a session event
    async fn store_event(&self, session_id: &str, event: &SessionEvent) -> Result<()> {
        let (event_type, content) = match event {
            SessionEvent::System { message, .. } => ("system", json!({"message": message})),
            SessionEvent::Assistant { content, model, .. } => {
                ("assistant", json!({"content": content, "model": model}))
            }
            SessionEvent::ToolUse {
                tool_name,
                parameters,
                ..
            } => (
                "tool_use",
                json!({"tool_name": tool_name, "parameters": parameters}),
            ),
            SessionEvent::ToolResult {
                tool_name,
                result,
                duration_ms,
                ..
            } => (
                "tool_result",
                json!({"tool_name": tool_name, "result": result, "duration_ms": duration_ms}),
            ),
            SessionEvent::Error {
                error_type,
                message,
                ..
            } => (
                "error",
                json!({"error_type": error_type, "message": message}),
            ),
        };

        sqlx::query(
            r#"
            INSERT INTO session_events (session_id, event_type, timestamp, content)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(session_id)
        .bind(event_type)
        .bind(event.timestamp().to_rfc3339())
        .bind(content.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Store a tool invocation
    async fn store_tool_invocation(&self, session_id: &str, tool: &ToolInvocation) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tool_invocations (
                session_id, tool_name, invoked_at, duration_ms, parameters, result_size
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(session_id)
        .bind(&tool.name)
        .bind(tool.invoked_at.to_rfc3339())
        .bind(tool.duration_ms.map(|d| d as i64))
        .bind(tool.parameters.to_string())
        .bind(tool.result_size.map(|s| s as i64))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load a session by ID
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let session_row = sqlx::query(
            r#"
            SELECT session_id, project_path, jsonl_path, started_at, completed_at,
                   model, total_input_tokens, total_output_tokens, total_cache_tokens
            FROM sessions WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = session_row else {
            return Ok(None);
        };

        let events = self.load_events(session_id).await?;
        let tools = self.load_tool_invocations(session_id).await?;

        Ok(Some(Session {
            session_id: row.get("session_id"),
            project_path: row.get("project_path"),
            jsonl_path: row.get("jsonl_path"),
            started_at: DateTime::parse_from_rfc3339(row.get("started_at"))?.with_timezone(&Utc),
            completed_at: row
                .get::<Option<String>, _>("completed_at")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            model: row.get("model"),
            events,
            total_input_tokens: row.get::<i64, _>("total_input_tokens") as u64,
            total_output_tokens: row.get::<i64, _>("total_output_tokens") as u64,
            total_cache_tokens: row.get::<i64, _>("total_cache_tokens") as u64,
            tool_invocations: tools,
        }))
    }

    /// Load events for a session
    async fn load_events(&self, session_id: &str) -> Result<Vec<SessionEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT event_type, timestamp, content
            FROM session_events
            WHERE session_id = ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            let event_type: String = row.get("event_type");
            let timestamp = DateTime::parse_from_rfc3339(row.get("timestamp"))?.with_timezone(&Utc);
            let content: serde_json::Value = serde_json::from_str(row.get("content"))?;

            let event = match event_type.as_str() {
                "system" => SessionEvent::System {
                    timestamp,
                    message: content["message"].as_str().unwrap_or("").to_string(),
                },
                "assistant" => SessionEvent::Assistant {
                    timestamp,
                    content: content["content"].as_str().unwrap_or("").to_string(),
                    model: content["model"].as_str().map(String::from),
                },
                "tool_use" => SessionEvent::ToolUse {
                    timestamp,
                    tool_name: content["tool_name"].as_str().unwrap_or("").to_string(),
                    parameters: content["parameters"].clone(),
                },
                "tool_result" => SessionEvent::ToolResult {
                    timestamp,
                    tool_name: content["tool_name"].as_str().unwrap_or("").to_string(),
                    result: content["result"].clone(),
                    duration_ms: content["duration_ms"].as_u64(),
                },
                "error" => SessionEvent::Error {
                    timestamp,
                    error_type: content["error_type"].as_str().unwrap_or("").to_string(),
                    message: content["message"].as_str().unwrap_or("").to_string(),
                },
                _ => continue,
            };
            events.push(event);
        }

        Ok(events)
    }

    /// Load tool invocations for a session
    async fn load_tool_invocations(&self, session_id: &str) -> Result<Vec<ToolInvocation>> {
        let rows = sqlx::query(
            r#"
            SELECT tool_name, invoked_at, duration_ms, parameters, result_size
            FROM tool_invocations
            WHERE session_id = ?
            ORDER BY invoked_at ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        let mut tools = Vec::new();
        for row in rows {
            tools.push(ToolInvocation {
                name: row.get("tool_name"),
                invoked_at: DateTime::parse_from_rfc3339(row.get("invoked_at"))?
                    .with_timezone(&Utc),
                duration_ms: row.get::<Option<i64>, _>("duration_ms").map(|d| d as u64),
                parameters: serde_json::from_str(row.get("parameters"))?,
                result_size: row.get::<Option<i64>, _>("result_size").map(|s| s as usize),
            });
        }

        Ok(tools)
    }

    /// Query sessions within a time range
    pub async fn query_sessions(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Session>> {
        let rows = sqlx::query(
            r#"
            SELECT session_id FROM sessions
            WHERE started_at >= ? AND started_at <= ?
            ORDER BY started_at DESC
            "#,
        )
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::new();
        for row in rows {
            let session_id: String = row.get("session_id");
            if let Some(session) = self.get_session(&session_id).await? {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    /// Delete sessions older than retention period
    pub async fn cleanup_old_sessions(&self, retention_days: i64) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(retention_days);

        let result = sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE started_at < ?
            "#,
        )
        .bind(cutoff.to_rfc3339())
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();
        if deleted > 0 {
            info!("Cleaned up {} old sessions", deleted);
        }

        Ok(deleted)
    }

    /// Archive sessions to a backup location
    pub async fn archive_sessions(
        &self,
        before: DateTime<Utc>,
        archive_path: impl AsRef<Path>,
    ) -> Result<u64> {
        let sessions = self
            .query_sessions(
                DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")?.with_timezone(&Utc),
                before,
            )
            .await?;

        // Create archive file
        let archive_file = archive_path.as_ref().join(format!(
            "analytics_archive_{}.json",
            before.format("%Y%m%d")
        ));
        let archive_data = serde_json::to_string_pretty(&sessions)?;
        tokio::fs::write(&archive_file, archive_data).await?;

        // Delete archived sessions
        let result = sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE started_at < ?
            "#,
        )
        .bind(before.to_rfc3339())
        .execute(&self.pool)
        .await?;

        let archived = result.rows_affected();
        info!(
            "Archived {} sessions to {}",
            archived,
            archive_file.display()
        );

        Ok(archived)
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<DatabaseStats> {
        let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(&self.pool)
            .await?;

        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session_events")
            .fetch_one(&self.pool)
            .await?;

        let tool_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tool_invocations")
            .fetch_one(&self.pool)
            .await?;

        Ok(DatabaseStats {
            total_sessions: session_count as u64,
            total_events: event_count as u64,
            total_tool_invocations: tool_count as u64,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseStats {
    pub total_sessions: u64,
    pub total_events: u64,
    pub total_tool_invocations: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_initialization() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = AnalyticsDatabase::new(&db_path).await.unwrap();
        let stats = db.get_stats().await.unwrap();

        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.total_events, 0);
        assert_eq!(stats.total_tool_invocations, 0);
    }

    #[tokio::test]
    async fn test_session_persistence() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = AnalyticsDatabase::new(&db_path).await.unwrap();

        let session = Session {
            session_id: "test-123".to_string(),
            project_path: "/test/project".to_string(),
            jsonl_path: "/test/log.jsonl".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            model: Some("claude-3-opus".to_string()),
            events: vec![],
            total_input_tokens: 100,
            total_output_tokens: 200,
            total_cache_tokens: 50,
            tool_invocations: vec![],
        };

        db.upsert_session(&session).await.unwrap();

        let loaded = db.get_session("test-123").await.unwrap();
        assert!(loaded.is_some());

        let loaded_session = loaded.unwrap();
        assert_eq!(loaded_session.session_id, "test-123");
        assert_eq!(loaded_session.total_input_tokens, 100);
    }

    #[tokio::test]
    async fn test_data_retention() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = AnalyticsDatabase::new(&db_path).await.unwrap();

        // Create an old session
        let old_session = Session {
            session_id: "old-session".to_string(),
            project_path: "/test/project".to_string(),
            jsonl_path: "/test/log.jsonl".to_string(),
            started_at: Utc::now() - chrono::Duration::days(100),
            completed_at: Some(Utc::now() - chrono::Duration::days(100)),
            model: None,
            events: vec![],
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_tokens: 0,
            tool_invocations: vec![],
        };

        db.upsert_session(&old_session).await.unwrap();

        // Clean up sessions older than 30 days
        let deleted = db.cleanup_old_sessions(30).await.unwrap();
        assert_eq!(deleted, 1);

        // Verify old session is gone
        let loaded = db.get_session("old-session").await.unwrap();
        assert!(loaded.is_none());
    }
}
