use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::Result;
use super::{Metric, MetricValue};

pub struct MetricsDatabase {
    pool: SqlitePool,
}

impl MetricsDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metrics (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_json TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                labels_json TEXT NOT NULL,
                project_id TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_metrics_name ON metrics(name);
            CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics(timestamp);
            CREATE INDEX IF NOT EXISTS idx_metrics_project ON metrics(project_id);
            CREATE INDEX IF NOT EXISTS idx_metrics_name_timestamp ON metrics(name, timestamp);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_metric(&self, metric: &Metric) -> Result<()> {
        let value_json = serde_json::to_string(&metric.value)?;
        let labels_json = serde_json::to_string(&metric.labels)?;

        sqlx::query(
            r#"
            INSERT INTO metrics (
                id, name, value_type, value_json, timestamp, labels_json, project_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(metric.id.to_string())
        .bind(&metric.name)
        .bind(metric.value.type_name())
        .bind(value_json)
        .bind(metric.timestamp.to_rfc3339())
        .bind(labels_json)
        .bind(metric.project_id.map(|id| id.to_string()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn query_metrics(
        &self,
        name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        labels: Option<HashMap<String, String>>,
    ) -> Result<Vec<Metric>> {
        let mut query = format!(
            r#"
            SELECT id, name, value_type, value_json, timestamp, labels_json, project_id
            FROM metrics
            WHERE name = ? AND timestamp >= ? AND timestamp <= ?
            "#
        );

        let rows = sqlx::query(&query)
            .bind(name)
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .fetch_all(&self.pool)
            .await?;

        let mut metrics = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            let name: String = row.get("name");
            let value_json: String = row.get("value_json");
            let timestamp_str: String = row.get("timestamp");
            let labels_json: String = row.get("labels_json");
            let project_id: Option<String> = row.get("project_id");

            let value: MetricValue = serde_json::from_str(&value_json)?;
            let metric_labels: HashMap<String, String> = serde_json::from_str(&labels_json)?;

            // Filter by labels if provided
            if let Some(ref filter_labels) = labels {
                let matches = filter_labels.iter().all(|(k, v)| {
                    metric_labels.get(k).map(|mv| mv == v).unwrap_or(false)
                });
                if !matches {
                    continue;
                }
            }

            metrics.push(Metric {
                id: Uuid::parse_str(&id)?,
                name,
                value,
                timestamp: DateTime::parse_from_rfc3339(&timestamp_str)?.with_timezone(&Utc),
                labels: metric_labels,
                project_id: project_id.map(|id| Uuid::parse_str(&id).ok()).flatten(),
            });
        }

        Ok(metrics)
    }

    pub async fn aggregate_metrics(
        &self,
        name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        aggregation: AggregationType,
    ) -> Result<f64> {
        let query = match aggregation {
            AggregationType::Sum => format!(
                r#"
                SELECT SUM(CAST(json_extract(value_json, '$.Counter') AS REAL)) as result
                FROM metrics
                WHERE name = ? AND timestamp >= ? AND timestamp <= ?
                    AND value_type = 'Counter'
                "#
            ),
            AggregationType::Average => format!(
                r#"
                SELECT AVG(CAST(json_extract(value_json, '$.Gauge') AS REAL)) as result
                FROM metrics
                WHERE name = ? AND timestamp >= ? AND timestamp <= ?
                    AND value_type = 'Gauge'
                "#
            ),
            AggregationType::Max => format!(
                r#"
                SELECT MAX(CAST(json_extract(value_json, '$.Gauge') AS REAL)) as result
                FROM metrics
                WHERE name = ? AND timestamp >= ? AND timestamp <= ?
                    AND value_type = 'Gauge'
                "#
            ),
            AggregationType::Min => format!(
                r#"
                SELECT MIN(CAST(json_extract(value_json, '$.Gauge') AS REAL)) as result
                FROM metrics
                WHERE name = ? AND timestamp >= ? AND timestamp <= ?
                    AND value_type = 'Gauge'
                "#
            ),
            AggregationType::Count => format!(
                r#"
                SELECT COUNT(*) as result
                FROM metrics
                WHERE name = ? AND timestamp >= ? AND timestamp <= ?
                "#
            ),
        };

        let row = sqlx::query(&query)
            .bind(name)
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .fetch_one(&self.pool)
            .await?;

        let result: Option<f64> = row.get("result");
        Ok(result.unwrap_or(0.0))
    }

    pub async fn cleanup_old_metrics(&self, retention_days: u32) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        
        let result = sqlx::query(
            r#"
            DELETE FROM metrics
            WHERE timestamp < ?
            "#,
        )
        .bind(cutoff.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AggregationType {
    Sum,
    Average,
    Max,
    Min,
    Count,
}

impl MetricValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            MetricValue::Counter(_) => "Counter",
            MetricValue::Gauge(_) => "Gauge",
            MetricValue::Histogram(_) => "Histogram",
            MetricValue::Summary { .. } => "Summary",
        }
    }
}