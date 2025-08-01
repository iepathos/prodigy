//! Metrics events and core data types

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Tags for metrics events
pub type Tags = HashMap<String, String>;

/// Core metrics event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricEvent {
    /// Counter metric (monotonically increasing)
    Counter {
        name: String,
        value: i64,
        tags: Tags,
        timestamp: DateTime<Utc>,
    },
    /// Gauge metric (current value)
    Gauge {
        name: String,
        value: f64,
        tags: Tags,
        timestamp: DateTime<Utc>,
    },
    /// Timer metric (duration measurement)
    Timer {
        name: String,
        duration: Duration,
        tags: Tags,
        timestamp: DateTime<Utc>,
    },
    /// Custom metric with arbitrary data
    Custom {
        name: String,
        data: serde_json::Value,
        tags: Tags,
        timestamp: DateTime<Utc>,
    },
}

impl MetricEvent {
    /// Create a new counter event
    pub fn counter(name: impl Into<String>, value: i64, tags: Tags) -> Self {
        Self::Counter {
            name: name.into(),
            value,
            tags,
            timestamp: Utc::now(),
        }
    }

    /// Create a new gauge event
    pub fn gauge(name: impl Into<String>, value: f64, tags: Tags) -> Self {
        Self::Gauge {
            name: name.into(),
            value,
            tags,
            timestamp: Utc::now(),
        }
    }

    /// Create a new timer event
    pub fn timer(name: impl Into<String>, duration: Duration, tags: Tags) -> Self {
        Self::Timer {
            name: name.into(),
            duration,
            tags,
            timestamp: Utc::now(),
        }
    }

    /// Create a new custom event
    pub fn custom(name: impl Into<String>, data: serde_json::Value, tags: Tags) -> Self {
        Self::Custom {
            name: name.into(),
            data,
            tags,
            timestamp: Utc::now(),
        }
    }

    /// Get the metric name
    pub fn name(&self) -> &str {
        match self {
            Self::Counter { name, .. } => name,
            Self::Gauge { name, .. } => name,
            Self::Timer { name, .. } => name,
            Self::Custom { name, .. } => name,
        }
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Counter { timestamp, .. } => *timestamp,
            Self::Gauge { timestamp, .. } => *timestamp,
            Self::Timer { timestamp, .. } => *timestamp,
            Self::Custom { timestamp, .. } => *timestamp,
        }
    }

    /// Get the tags
    pub fn tags(&self) -> &Tags {
        match self {
            Self::Counter { tags, .. } => tags,
            Self::Gauge { tags, .. } => tags,
            Self::Timer { tags, .. } => tags,
            Self::Custom { tags, .. } => tags,
        }
    }
}

/// Core trait for metrics collection
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record a metrics event
    async fn record(&self, event: MetricEvent) -> Result<()>;

    /// Flush buffered events
    async fn flush(&self) -> Result<()>;

    /// Get collector name for identification
    fn name(&self) -> &str;
}

/// Time range for queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Aggregation types for metrics queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Aggregation {
    Sum,
    Average,
    Min,
    Max,
    Count,
    Percentile(f64),
}

/// Query for metrics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQuery {
    pub metric_names: Vec<String>,
    pub time_range: Option<TimeRange>,
    pub tags: Option<Tags>,
    pub aggregation: Option<Aggregation>,
}

/// Result of metrics query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResult {
    pub events: Vec<MetricEvent>,
    pub count: usize,
}

/// Result of aggregation query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateResult {
    pub value: f64,
    pub count: usize,
    pub aggregation: Aggregation,
}

/// Trait for metrics querying
#[async_trait]
pub trait MetricsReader: Send + Sync {
    /// Query metrics events
    async fn query(&self, query: MetricsQuery) -> Result<MetricsResult>;

    /// Aggregate metrics data
    async fn aggregate(
        &self,
        query: MetricsQuery,
        aggregation: Aggregation,
    ) -> Result<AggregateResult>;
}
