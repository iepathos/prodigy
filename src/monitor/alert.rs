use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::metrics::{AggregationType, MetricsDatabase};
use super::{Alert, AlertCondition, AlertRule, AlertSeverity, Notifier, ThresholdOperator};
use crate::error::Result;

pub struct AlertManager {
    rules: Vec<AlertRule>,
    notifiers: Vec<Box<dyn Notifier>>,
    metrics_db: Arc<MetricsDatabase>,
    alerts_db: AlertsDatabase,
    last_check: Arc<Mutex<HashMap<String, Instant>>>,
}

impl AlertManager {
    pub fn new(metrics_db: Arc<MetricsDatabase>, pool: SqlitePool) -> Self {
        Self {
            rules: vec![],
            notifiers: vec![],
            metrics_db,
            alerts_db: AlertsDatabase::new(pool),
            last_check: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    pub fn add_notifier(&mut self, notifier: Box<dyn Notifier>) {
        self.notifiers.push(notifier);
    }

    pub async fn check_alerts(&self) -> Result<Vec<Alert>> {
        let mut triggered_alerts = Vec::new();
        let now = Instant::now();
        let mut last_check = self.last_check.lock().await;

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            // Check cooldown
            if let Some(last) = last_check.get(&rule.name) {
                if now.duration_since(*last) < rule.cooldown {
                    continue;
                }
            }

            match self.evaluate_condition(&rule.condition).await {
                Ok(true) => {
                    let alert = Alert {
                        id: Uuid::new_v4(),
                        rule_name: rule.name.clone(),
                        severity: rule.severity,
                        message: self.generate_alert_message(rule).await?,
                        timestamp: Utc::now(),
                        acknowledged: false,
                        metadata: HashMap::new(),
                    };

                    // Store alert
                    self.alerts_db.insert_alert(&alert).await?;

                    // Notify
                    for notifier in &self.notifiers {
                        if let Err(e) = notifier.notify(&alert).await {
                            log::error!(
                                "Failed to send notification via {}: {}",
                                notifier.name(),
                                e
                            );
                        }
                    }

                    triggered_alerts.push(alert);
                    last_check.insert(rule.name.clone(), now);
                }
                Ok(false) => {
                    // Condition not met
                }
                Err(e) => {
                    log::error!(
                        "Failed to evaluate alert condition for {}: {}",
                        rule.name,
                        e
                    );
                }
            }
        }

        Ok(triggered_alerts)
    }

    async fn evaluate_condition(&self, condition: &AlertCondition) -> Result<bool> {
        match condition {
            AlertCondition::ThresholdExceeded {
                metric,
                threshold,
                operator,
            } => {
                let timeframe = super::TimeFrame::last_hour();
                let value = self
                    .metrics_db
                    .aggregate_metrics(
                        metric,
                        timeframe.start,
                        timeframe.end,
                        AggregationType::Average,
                    )
                    .await?;

                Ok(match operator {
                    ThresholdOperator::GreaterThan => value > *threshold,
                    ThresholdOperator::LessThan => value < *threshold,
                    ThresholdOperator::GreaterThanOrEqual => value >= *threshold,
                    ThresholdOperator::LessThanOrEqual => value <= *threshold,
                })
            }
            AlertCondition::RateOfChange {
                metric,
                change,
                window,
            } => {
                let end = Utc::now();
                let start = end - chrono::Duration::from_std(*window)?;
                let mid = end - chrono::Duration::from_std(*window / 2)?;

                let first_half = self
                    .metrics_db
                    .aggregate_metrics(metric, start, mid, AggregationType::Average)
                    .await?;

                let second_half = self
                    .metrics_db
                    .aggregate_metrics(metric, mid, end, AggregationType::Average)
                    .await?;

                if first_half == 0.0 {
                    Ok(false)
                } else {
                    let rate = ((second_half - first_half) / first_half) * 100.0;
                    Ok(rate.abs() > *change)
                }
            }
            AlertCondition::Pattern { query: _ } => {
                // For now, pattern matching is simplified
                // In a real implementation, this would use a query language
                Ok(false)
            }
        }
    }

    async fn generate_alert_message(&self, rule: &AlertRule) -> Result<String> {
        match &rule.condition {
            AlertCondition::ThresholdExceeded {
                metric,
                threshold,
                operator,
            } => {
                let timeframe = super::TimeFrame::last_hour();
                let current_value = self
                    .metrics_db
                    .aggregate_metrics(
                        metric,
                        timeframe.start,
                        timeframe.end,
                        AggregationType::Average,
                    )
                    .await?;

                Ok(format!(
                    "Metric '{}' ({:.2}) {} threshold ({:.2})",
                    metric,
                    current_value,
                    match operator {
                        ThresholdOperator::GreaterThan => "exceeded",
                        ThresholdOperator::LessThan => "fell below",
                        ThresholdOperator::GreaterThanOrEqual => "met or exceeded",
                        ThresholdOperator::LessThanOrEqual => "met or fell below",
                    },
                    threshold
                ))
            }
            AlertCondition::RateOfChange {
                metric,
                change,
                window,
            } => Ok(format!(
                "Metric '{metric}' changed by more than {change}% in the last {window:?}"
            )),
            AlertCondition::Pattern { query } => Ok(format!("Pattern match triggered: {query}")),
        }
    }

    pub async fn get_alerts(&self, since: Option<DateTime<Utc>>) -> Result<Vec<Alert>> {
        self.alerts_db.get_alerts(since).await
    }

    pub async fn acknowledge_alert(&self, alert_id: Uuid) -> Result<()> {
        self.alerts_db.acknowledge_alert(alert_id).await
    }
}

pub struct AlertsDatabase {
    pool: SqlitePool,
}

impl AlertsDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS alerts (
                id TEXT PRIMARY KEY,
                rule_name TEXT NOT NULL,
                severity TEXT NOT NULL,
                message TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
                acknowledged_at TEXT,
                metadata_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_alerts_timestamp ON alerts(timestamp);
            CREATE INDEX IF NOT EXISTS idx_alerts_severity ON alerts(severity);
            CREATE INDEX IF NOT EXISTS idx_alerts_acknowledged ON alerts(acknowledged);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_alert(&self, alert: &Alert) -> Result<()> {
        let metadata_json = serde_json::to_string(&alert.metadata)?;

        sqlx::query(
            r#"
            INSERT INTO alerts (
                id, rule_name, severity, message, timestamp, acknowledged, metadata_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(alert.id.to_string())
        .bind(&alert.rule_name)
        .bind(format!("{:?}", alert.severity))
        .bind(&alert.message)
        .bind(alert.timestamp.to_rfc3339())
        .bind(alert.acknowledged)
        .bind(metadata_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_alerts(&self, since: Option<DateTime<Utc>>) -> Result<Vec<Alert>> {
        let query = if let Some(since) = since {
            sqlx::query(
                r#"
                SELECT id, rule_name, severity, message, timestamp, acknowledged, metadata_json
                FROM alerts
                WHERE timestamp > ?
                ORDER BY timestamp DESC
                "#,
            )
            .bind(since.to_rfc3339())
        } else {
            sqlx::query(
                r#"
                SELECT id, rule_name, severity, message, timestamp, acknowledged, metadata_json
                FROM alerts
                ORDER BY timestamp DESC
                LIMIT 100
                "#,
            )
        };

        let rows = query.fetch_all(&self.pool).await?;
        let mut alerts = Vec::new();

        for row in rows {
            let id: String = row.get("id");
            let severity_str: String = row.get("severity");
            let metadata_json: String = row.get("metadata_json");
            let timestamp_str: String = row.get("timestamp");

            let severity = match severity_str.as_str() {
                "Info" => AlertSeverity::Info,
                "Warning" => AlertSeverity::Warning,
                "Critical" => AlertSeverity::Critical,
                _ => AlertSeverity::Info,
            };

            alerts.push(Alert {
                id: Uuid::parse_str(&id)?,
                rule_name: row.get("rule_name"),
                severity,
                message: row.get("message"),
                timestamp: DateTime::parse_from_rfc3339(&timestamp_str)?.with_timezone(&Utc),
                acknowledged: row.get("acknowledged"),
                metadata: serde_json::from_str(&metadata_json)?,
            });
        }

        Ok(alerts)
    }

    pub async fn acknowledge_alert(&self, alert_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE alerts
            SET acknowledged = TRUE, acknowledged_at = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(alert_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// Built-in notifiers

pub struct LogNotifier;

#[async_trait::async_trait]
impl Notifier for LogNotifier {
    async fn notify(&self, alert: &Alert) -> Result<()> {
        match alert.severity {
            AlertSeverity::Info => log::info!("Alert: {} - {}", alert.rule_name, alert.message),
            AlertSeverity::Warning => log::warn!("Alert: {} - {}", alert.rule_name, alert.message),
            AlertSeverity::Critical => {
                log::error!("Alert: {} - {}", alert.rule_name, alert.message)
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "log_notifier"
    }
}

pub struct ConsoleNotifier;

#[async_trait::async_trait]
impl Notifier for ConsoleNotifier {
    async fn notify(&self, alert: &Alert) -> Result<()> {
        let severity_color = match alert.severity {
            AlertSeverity::Info => "\x1b[34m",     // Blue
            AlertSeverity::Warning => "\x1b[33m",  // Yellow
            AlertSeverity::Critical => "\x1b[31m", // Red
        };
        let reset = "\x1b[0m";

        println!(
            "{}[ALERT] {} - {}: {}{}",
            severity_color,
            alert.timestamp.format("%Y-%m-%d %H:%M:%S"),
            alert.rule_name,
            alert.message,
            reset
        );

        Ok(())
    }

    fn name(&self) -> &str {
        "console_notifier"
    }
}

// Default alert rules
pub fn default_alert_rules() -> Vec<AlertRule> {
    vec![
        AlertRule {
            name: "High Token Usage".to_string(),
            condition: AlertCondition::ThresholdExceeded {
                metric: "claude.tokens.total".to_string(),
                threshold: 100_000.0,
                operator: ThresholdOperator::GreaterThan,
            },
            severity: AlertSeverity::Warning,
            cooldown: Duration::from_secs(3600), // 1 hour
            enabled: true,
        },
        AlertRule {
            name: "High Error Rate".to_string(),
            condition: AlertCondition::ThresholdExceeded {
                metric: "claude.errors".to_string(),
                threshold: 10.0,
                operator: ThresholdOperator::GreaterThan,
            },
            severity: AlertSeverity::Critical,
            cooldown: Duration::from_secs(1800), // 30 minutes
            enabled: true,
        },
        AlertRule {
            name: "Slow Response Time".to_string(),
            condition: AlertCondition::ThresholdExceeded {
                metric: "claude.response_time_ms".to_string(),
                threshold: 5000.0,
                operator: ThresholdOperator::GreaterThan,
            },
            severity: AlertSeverity::Warning,
            cooldown: Duration::from_secs(1800), // 30 minutes
            enabled: true,
        },
        AlertRule {
            name: "Low Completion Rate".to_string(),
            condition: AlertCondition::ThresholdExceeded {
                metric: "specs.completion_percentage".to_string(),
                threshold: 10.0,
                operator: ThresholdOperator::LessThan,
            },
            severity: AlertSeverity::Info,
            cooldown: Duration::from_secs(86400), // 24 hours
            enabled: true,
        },
        AlertRule {
            name: "High Memory Usage".to_string(),
            condition: AlertCondition::ThresholdExceeded {
                metric: "system.memory.used_percentage".to_string(),
                threshold: 90.0,
                operator: ThresholdOperator::GreaterThan,
            },
            severity: AlertSeverity::Warning,
            cooldown: Duration::from_secs(600), // 10 minutes
            enabled: true,
        },
        AlertRule {
            name: "Cost Spike".to_string(),
            condition: AlertCondition::RateOfChange {
                metric: "claude.cost.usd".to_string(),
                change: 50.0,                      // 50% change
                window: Duration::from_secs(3600), // 1 hour window
            },
            severity: AlertSeverity::Warning,
            cooldown: Duration::from_secs(3600), // 1 hour
            enabled: true,
        },
    ]
}
