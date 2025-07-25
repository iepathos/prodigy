use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct PerformanceTracker {
    traces: Arc<Mutex<HashMap<Uuid, Trace>>>,
    storage: TraceStorage,
}

impl PerformanceTracker {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            traces: Arc::new(Mutex::new(HashMap::new())),
            storage: TraceStorage::new(pool),
        }
    }

    pub async fn start_trace(&self, operation: String) -> TraceHandle {
        let trace = Trace {
            id: Uuid::new_v4(),
            operation,
            start_time: Instant::now(),
            start_timestamp: Utc::now(),
            end_time: None,
            end_timestamp: None,
            metadata: HashMap::new(),
            spans: Vec::new(),
        };

        let trace_id = trace.id;
        self.traces.lock().await.insert(trace_id, trace);

        TraceHandle {
            id: trace_id,
            tracker: self.traces.clone(),
        }
    }

    pub async fn get_trace(&self, id: Uuid) -> Option<Trace> {
        self.traces.lock().await.get(&id).cloned()
    }

    pub async fn save_completed_traces(&self) -> Result<()> {
        let mut traces = self.traces.lock().await;
        let completed: Vec<Trace> = traces
            .values()
            .filter(|t| t.end_time.is_some())
            .cloned()
            .collect();

        for trace in &completed {
            self.storage.save_trace(trace).await?;
            traces.remove(&trace.id);
        }

        Ok(())
    }

    pub async fn query_traces(
        &self,
        operation: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TraceSummary>> {
        self.storage.query_traces(operation, start, end).await
    }

    pub async fn get_operation_stats(
        &self,
        operation: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OperationStats> {
        self.storage
            .get_operation_stats(operation, start, end)
            .await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub id: Uuid,
    pub operation: String,
    #[serde(skip, default = "Instant::now")]
    pub start_time: Instant,
    pub start_timestamp: DateTime<Utc>,
    #[serde(skip)]
    pub end_time: Option<Instant>,
    pub end_timestamp: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
    pub spans: Vec<Span>,
}

impl Trace {
    pub fn duration(&self) -> Option<Duration> {
        self.end_time.map(|end| end.duration_since(self.start_time))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub id: Uuid,
    pub name: String,
    #[serde(skip, default = "Instant::now")]
    pub start_time: Instant,
    pub start_timestamp: DateTime<Utc>,
    #[serde(skip)]
    pub end_time: Option<Instant>,
    pub end_timestamp: Option<DateTime<Utc>>,
    pub tags: HashMap<String, String>,
}

impl Span {
    pub fn duration(&self) -> Option<Duration> {
        self.end_time.map(|end| end.duration_since(self.start_time))
    }
}

pub struct TraceHandle {
    id: Uuid,
    tracker: Arc<Mutex<HashMap<Uuid, Trace>>>,
}

impl TraceHandle {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub async fn add_metadata(&self, key: String, value: String) {
        if let Some(trace) = self.tracker.lock().await.get_mut(&self.id) {
            trace.metadata.insert(key, value);
        }
    }

    pub async fn start_span(&self, name: String) -> SpanHandle {
        let span = Span {
            id: Uuid::new_v4(),
            name,
            start_time: Instant::now(),
            start_timestamp: Utc::now(),
            end_time: None,
            end_timestamp: None,
            tags: HashMap::new(),
        };

        let span_id = span.id;
        if let Some(trace) = self.tracker.lock().await.get_mut(&self.id) {
            trace.spans.push(span);
        }

        SpanHandle {
            trace_id: self.id,
            span_id,
            tracker: self.tracker.clone(),
        }
    }

    pub async fn end(self) {
        if let Some(trace) = self.tracker.lock().await.get_mut(&self.id) {
            trace.end_time = Some(Instant::now());
            trace.end_timestamp = Some(Utc::now());
        }
    }
}

pub struct SpanHandle {
    trace_id: Uuid,
    span_id: Uuid,
    tracker: Arc<Mutex<HashMap<Uuid, Trace>>>,
}

impl SpanHandle {
    pub async fn add_tag(&self, key: String, value: String) {
        if let Some(trace) = self.tracker.lock().await.get_mut(&self.trace_id) {
            if let Some(span) = trace.spans.iter_mut().find(|s| s.id == self.span_id) {
                span.tags.insert(key, value);
            }
        }
    }

    pub async fn end(self) {
        if let Some(trace) = self.tracker.lock().await.get_mut(&self.trace_id) {
            if let Some(span) = trace.spans.iter_mut().find(|s| s.id == self.span_id) {
                span.end_time = Some(Instant::now());
                span.end_timestamp = Some(Utc::now());
            }
        }
    }
}

#[derive(Debug, Clone)]
struct TraceStorage {
    pool: SqlitePool,
}

impl TraceStorage {
    fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS traces (
                id TEXT PRIMARY KEY,
                operation TEXT NOT NULL,
                start_timestamp TEXT NOT NULL,
                end_timestamp TEXT,
                duration_ms INTEGER,
                metadata_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_traces_operation ON traces(operation);
            CREATE INDEX IF NOT EXISTS idx_traces_timestamp ON traces(start_timestamp);
            CREATE INDEX IF NOT EXISTS idx_traces_duration ON traces(duration_ms);

            CREATE TABLE IF NOT EXISTS spans (
                id TEXT PRIMARY KEY,
                trace_id TEXT NOT NULL,
                name TEXT NOT NULL,
                start_timestamp TEXT NOT NULL,
                end_timestamp TEXT,
                duration_ms INTEGER,
                tags_json TEXT NOT NULL,
                FOREIGN KEY (trace_id) REFERENCES traces(id)
            );

            CREATE INDEX IF NOT EXISTS idx_spans_trace ON spans(trace_id);
            CREATE INDEX IF NOT EXISTS idx_spans_name ON spans(name);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn save_trace(&self, trace: &Trace) -> Result<()> {
        let duration_ms = trace.duration().map(|d| d.as_millis() as i64);
        let metadata_json = serde_json::to_string(&trace.metadata)?;

        let tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO traces (
                id, operation, start_timestamp, end_timestamp, duration_ms, metadata_json
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(trace.id.to_string())
        .bind(&trace.operation)
        .bind(trace.start_timestamp.to_rfc3339())
        .bind(trace.end_timestamp.map(|t| t.to_rfc3339()))
        .bind(duration_ms)
        .bind(metadata_json)
        .execute(&self.pool)
        .await?;

        for span in &trace.spans {
            let span_duration_ms = span.duration().map(|d| d.as_millis() as i64);
            let tags_json = serde_json::to_string(&span.tags)?;

            sqlx::query(
                r#"
                INSERT INTO spans (
                    id, trace_id, name, start_timestamp, end_timestamp, duration_ms, tags_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(span.id.to_string())
            .bind(trace.id.to_string())
            .bind(&span.name)
            .bind(span.start_timestamp.to_rfc3339())
            .bind(span.end_timestamp.map(|t| t.to_rfc3339()))
            .bind(span_duration_ms)
            .bind(tags_json)
            .execute(&self.pool)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn query_traces(
        &self,
        operation: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TraceSummary>> {
        let query = if let Some(op) = operation {
            sqlx::query(
                r#"
                SELECT id, operation, start_timestamp, end_timestamp, duration_ms, metadata_json
                FROM traces
                WHERE operation = ? AND start_timestamp >= ? AND start_timestamp <= ?
                ORDER BY start_timestamp DESC
                LIMIT 1000
                "#,
            )
            .bind(op)
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
        } else {
            sqlx::query(
                r#"
                SELECT id, operation, start_timestamp, end_timestamp, duration_ms, metadata_json
                FROM traces
                WHERE start_timestamp >= ? AND start_timestamp <= ?
                ORDER BY start_timestamp DESC
                LIMIT 1000
                "#,
            )
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
        };

        let rows = query.fetch_all(&self.pool).await?;
        let mut summaries = Vec::new();

        for row in rows {
            let id: String = row.get("id");
            let operation: String = row.get("operation");
            let start_timestamp_str: String = row.get("start_timestamp");
            let end_timestamp_str: Option<String> = row.get("end_timestamp");
            let duration_ms: Option<i64> = row.get("duration_ms");
            let metadata_json: String = row.get("metadata_json");

            summaries.push(TraceSummary {
                id: Uuid::parse_str(&id)?,
                operation,
                start_timestamp: DateTime::parse_from_rfc3339(&start_timestamp_str)?
                    .with_timezone(&Utc),
                end_timestamp: end_timestamp_str
                    .map(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .flatten()
                    .map(|dt| dt.with_timezone(&Utc)),
                duration_ms,
                metadata: serde_json::from_str(&metadata_json)?,
                span_count: self.get_span_count(&id).await?,
            });
        }

        Ok(summaries)
    }

    async fn get_span_count(&self, trace_id: &str) -> Result<usize> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM spans
            WHERE trace_id = ?
            "#,
        )
        .bind(trace_id)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = row.get("count");
        Ok(count as usize)
    }

    async fn get_operation_stats(
        &self,
        operation: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<OperationStats> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as count,
                AVG(duration_ms) as avg_duration,
                MIN(duration_ms) as min_duration,
                MAX(duration_ms) as max_duration,
                SUM(CASE WHEN end_timestamp IS NULL THEN 1 ELSE 0 END) as incomplete_count
            FROM traces
            WHERE operation = ? AND start_timestamp >= ? AND start_timestamp <= ?
            "#,
        )
        .bind(operation)
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = row.get("count");
        let avg_duration: Option<f64> = row.get("avg_duration");
        let min_duration: Option<i64> = row.get("min_duration");
        let max_duration: Option<i64> = row.get("max_duration");
        let incomplete_count: i64 = row.get("incomplete_count");

        // Calculate percentiles
        let durations = self.get_operation_durations(operation, start, end).await?;
        let p50 = calculate_percentile(&durations, 0.5);
        let p95 = calculate_percentile(&durations, 0.95);
        let p99 = calculate_percentile(&durations, 0.99);

        Ok(OperationStats {
            operation: operation.to_string(),
            count: count as u64,
            avg_duration_ms: avg_duration.unwrap_or(0.0),
            min_duration_ms: min_duration.unwrap_or(0),
            max_duration_ms: max_duration.unwrap_or(0),
            p50_duration_ms: p50,
            p95_duration_ms: p95,
            p99_duration_ms: p99,
            incomplete_count: incomplete_count as u64,
            error_rate: 0.0, // TODO: Track errors
        })
    }

    async fn get_operation_durations(
        &self,
        operation: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<i64>> {
        let rows = sqlx::query(
            r#"
            SELECT duration_ms
            FROM traces
            WHERE operation = ? AND start_timestamp >= ? AND start_timestamp <= ?
                AND duration_ms IS NOT NULL
            "#,
        )
        .bind(operation)
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        let durations: Vec<i64> = rows
            .into_iter()
            .filter_map(|row| row.get("duration_ms"))
            .collect();

        Ok(durations)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    pub id: Uuid,
    pub operation: String,
    pub start_timestamp: DateTime<Utc>,
    pub end_timestamp: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub metadata: HashMap<String, String>,
    pub span_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    pub operation: String,
    pub count: u64,
    pub avg_duration_ms: f64,
    pub min_duration_ms: i64,
    pub max_duration_ms: i64,
    pub p50_duration_ms: i64,
    pub p95_duration_ms: i64,
    pub p99_duration_ms: i64,
    pub incomplete_count: u64,
    pub error_rate: f64,
}

fn calculate_percentile(data: &[i64], percentile: f64) -> i64 {
    if data.is_empty() {
        return 0;
    }

    let mut sorted = data.to_vec();
    sorted.sort();

    let index = ((percentile * (sorted.len() - 1) as f64) as usize).min(sorted.len() - 1);
    sorted[index]
}

// Convenience macros for performance tracking
#[macro_export]
macro_rules! trace_operation {
    ($tracker:expr, $operation:expr, $block:expr) => {{
        let trace = $tracker.start_trace($operation.to_string()).await;
        let result = $block;
        trace.end().await;
        result
    }};
}
