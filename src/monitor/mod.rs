use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

pub mod alert;
pub mod analytics;
pub mod collector;
pub mod dashboard;
pub mod export;
pub mod metrics;
pub mod performance;
pub mod report;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub id: Uuid,
    pub name: String,
    pub value: MetricValue,
    pub timestamp: DateTime<Utc>,
    pub labels: HashMap<String, String>,
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Summary {
        sum: f64,
        count: u64,
        quantiles: Vec<(f64, f64)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub rule_name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub cooldown: Duration,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AlertCondition {
    ThresholdExceeded {
        metric: String,
        threshold: f64,
        operator: ThresholdOperator,
    },
    RateOfChange {
        metric: String,
        change: f64,
        window: Duration,
    },
    Pattern {
        query: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ThresholdOperator {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeFrame {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeFrame {
    pub fn last_hour() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(1);
        Self { start, end }
    }

    pub fn last_day() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(1);
        Self { start, end }
    }

    pub fn last_week() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::weeks(1);
        Self { start, end }
    }

    pub fn last_month() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(30);
        Self { start, end }
    }
}

use async_trait::async_trait;

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, alert: &Alert) -> Result<()>;
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub metrics_retention_days: u32,
    pub alert_check_interval: Duration,
    pub dashboard_port: u16,
    pub enable_prometheus_export: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            metrics_retention_days: 30,
            alert_check_interval: Duration::from_secs(60),
            dashboard_port: 8080,
            enable_prometheus_export: true,
        }
    }
}
