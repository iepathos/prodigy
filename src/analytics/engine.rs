//! Analytics engine for Claude session analysis

use anyhow::Result;
use chrono::{Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::models::{Cost, PricingModel, Session, SessionIndex, TimeRange, ToolStat, ToolStats};

/// Analytics engine for processing Claude session data
pub struct AnalyticsEngine {
    index: Arc<RwLock<SessionIndex>>,
    metrics: Arc<MetricsCollector>,
    pricing_model: PricingModel,
}

impl AnalyticsEngine {
    /// Create a new analytics engine
    pub fn new(index: Arc<RwLock<SessionIndex>>) -> Self {
        Self {
            index,
            metrics: Arc::new(MetricsCollector::new()),
            pricing_model: PricingModel::default(),
        }
    }

    /// Calculate the cost of a specific session
    pub async fn calculate_session_cost(&self, session_id: &str) -> Result<Cost> {
        let index = self.index.read().await;
        let session = index.get_session(session_id).await?;

        let cost = Cost {
            input_tokens: session.total_input_tokens(),
            output_tokens: session.total_output_tokens(),
            cache_tokens: session.total_cache_tokens(),
            estimated_cost_usd: self.calculate_cost(session),
        };

        debug!(
            "Session {} cost: ${:.4} ({}+{}+{} tokens)",
            session_id,
            cost.estimated_cost_usd,
            cost.input_tokens,
            cost.output_tokens,
            cost.cache_tokens
        );

        Ok(cost)
    }

    /// Calculate cost based on session token usage
    fn calculate_cost(&self, session: &Session) -> f64 {
        self.pricing_model.calculate_cost(
            session.total_input_tokens(),
            session.total_output_tokens(),
            session.total_cache_tokens(),
        )
    }

    /// Analyze tool usage across sessions in a time range
    pub async fn analyze_tool_usage(&self, time_range: TimeRange) -> Result<ToolStats> {
        let index = self.index.read().await;
        let sessions = index.query_sessions(time_range).await?;

        let mut tool_stats: HashMap<String, ToolStat> = HashMap::new();
        let session_count = sessions.len();

        for session in sessions {
            for tool_use in session.tool_invocations() {
                tool_stats
                    .entry(tool_use.name.clone())
                    .and_modify(|s| s.increment(tool_use))
                    .or_insert_with(|| ToolStat::from(tool_use));
            }
        }

        info!(
            "Analyzed {} tools across {} sessions",
            tool_stats.len(),
            session_count
        );

        Ok(ToolStats { stats: tool_stats })
    }

    /// Get performance bottlenecks by identifying slow tools
    pub async fn identify_bottlenecks(&self, threshold_ms: u64) -> Result<Vec<PerformanceIssue>> {
        let time_range = TimeRange {
            start: Utc::now() - Duration::days(7),
            end: Utc::now(),
        };

        let tool_stats = self.analyze_tool_usage(time_range).await?;
        let mut issues = Vec::new();

        for (tool_name, stat) in tool_stats.stats {
            if stat.average_duration_ms > threshold_ms {
                issues.push(PerformanceIssue {
                    tool_name: tool_name.clone(),
                    issue_type: IssueType::SlowExecution,
                    average_duration_ms: stat.average_duration_ms,
                    occurrence_count: stat.total_invocations,
                    recommendation: format!(
                        "Tool {} averages {}ms per execution. Consider optimization or caching.",
                        tool_name, stat.average_duration_ms
                    ),
                });
            }

            if stat.success_rate < 90.0 {
                issues.push(PerformanceIssue {
                    tool_name: tool_name.clone(),
                    issue_type: IssueType::HighFailureRate,
                    average_duration_ms: stat.average_duration_ms,
                    occurrence_count: stat.failure_count,
                    recommendation: format!(
                        "Tool {} has {:.1}% success rate. Review error patterns.",
                        tool_name, stat.success_rate
                    ),
                });
            }
        }

        Ok(issues)
    }

    /// Generate usage patterns and trends
    pub async fn generate_usage_patterns(&self) -> Result<UsagePatterns> {
        let index = self.index.read().await;
        let last_week = TimeRange {
            start: Utc::now() - Duration::days(7),
            end: Utc::now(),
        };
        let sessions = index.query_sessions(last_week).await?;

        let mut hourly_distribution = vec![0u64; 24];
        let mut daily_sessions = HashMap::new();
        let mut tool_frequency = HashMap::new();

        for session in sessions {
            // Hourly distribution
            let hour = session.started_at.hour() as usize;
            hourly_distribution[hour] += 1;

            // Daily sessions
            let date = session.started_at.date_naive();
            *daily_sessions.entry(date).or_insert(0u64) += 1;

            // Tool frequency
            for tool in &session.tool_invocations {
                *tool_frequency.entry(tool.name.clone()).or_insert(0u64) += 1;
            }
        }

        // Find peak hours
        let peak_hour = hourly_distribution
            .iter()
            .enumerate()
            .max_by_key(|(_, count)| *count)
            .map(|(hour, _)| hour)
            .unwrap_or(0);

        // Find most used tools
        let mut tool_list: Vec<_> = tool_frequency.into_iter().collect();
        tool_list.sort_by(|a, b| b.1.cmp(&a.1));
        let most_used_tools: Vec<String> = tool_list
            .into_iter()
            .take(10)
            .map(|(name, _)| name)
            .collect();

        Ok(UsagePatterns {
            peak_usage_hour: peak_hour,
            hourly_distribution,
            daily_session_counts: daily_sessions,
            most_used_tools,
        })
    }

    /// Get cost projections based on current usage
    pub async fn project_costs(&self, days_ahead: i64) -> Result<CostProjection> {
        let past_period = TimeRange {
            start: Utc::now() - Duration::days(days_ahead),
            end: Utc::now(),
        };

        let index = self.index.read().await;
        let sessions = index.query_sessions(past_period).await?;

        let mut total_cost = 0.0;
        let mut total_tokens = TokenSummary::default();

        for session in sessions {
            total_cost += self.calculate_cost(session);
            total_tokens.input += session.total_input_tokens();
            total_tokens.output += session.total_output_tokens();
            total_tokens.cache += session.total_cache_tokens();
        }

        let days_in_period = days_ahead as f64;
        let daily_average_cost = total_cost / days_in_period;

        Ok(CostProjection {
            daily_average: daily_average_cost,
            weekly_projection: daily_average_cost * 7.0,
            monthly_projection: daily_average_cost * 30.0,
            annual_projection: daily_average_cost * 365.0,
            average_daily_tokens: TokenSummary {
                input: (total_tokens.input as f64 / days_in_period) as u64,
                output: (total_tokens.output as f64 / days_in_period) as u64,
                cache: (total_tokens.cache as f64 / days_in_period) as u64,
            },
        })
    }

    /// Get optimization recommendations based on usage analysis
    pub async fn get_optimization_recommendations(&self) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();

        // Check for performance issues
        let bottlenecks = self.identify_bottlenecks(5000).await?;
        for issue in bottlenecks {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Performance,
                priority: if issue.average_duration_ms > 10000 {
                    Priority::High
                } else {
                    Priority::Medium
                },
                title: format!("Optimize {}", issue.tool_name),
                description: issue.recommendation,
                estimated_savings: None,
            });
        }

        // Check token usage patterns
        let projection = self.project_costs(30).await?;
        if projection.monthly_projection > 1000.0 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Cost,
                priority: Priority::High,
                title: "High monthly costs detected".to_string(),
                description: format!(
                    "Current monthly projection is ${:.2}. Consider implementing caching strategies.",
                    projection.monthly_projection
                ),
                estimated_savings: Some(projection.monthly_projection * 0.2), // Assume 20% savings
            });
        }

        // Check cache utilization
        if projection.average_daily_tokens.cache < projection.average_daily_tokens.input / 10 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Cost,
                priority: Priority::Medium,
                title: "Low cache utilization".to_string(),
                description: "Cache tokens are underutilized. Consider enabling prompt caching for repeated queries.".to_string(),
                estimated_savings: Some(projection.monthly_projection * 0.15),
            });
        }

        Ok(recommendations)
    }
}

/// Metrics collector for tracking analytics
pub struct MetricsCollector {
    metrics: RwLock<HashMap<String, MetricValue>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(HashMap::new()),
        }
    }

    pub async fn record(&self, name: &str, value: MetricValue) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(name.to_string(), value);
    }

    pub async fn get(&self, name: &str) -> Option<MetricValue> {
        let metrics = self.metrics.read().await;
        metrics.get(name).cloned()
    }
}

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
}

/// Performance issue identified by the engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssue {
    pub tool_name: String,
    pub issue_type: IssueType,
    pub average_duration_ms: u64,
    pub occurrence_count: u64,
    pub recommendation: String,
}

/// Types of performance issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueType {
    SlowExecution,
    HighFailureRate,
    ResourceIntensive,
}

/// Usage patterns detected by the engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePatterns {
    pub peak_usage_hour: usize,
    pub hourly_distribution: Vec<u64>,
    pub daily_session_counts: HashMap<chrono::NaiveDate, u64>,
    pub most_used_tools: Vec<String>,
}

/// Cost projection based on historical data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostProjection {
    pub daily_average: f64,
    pub weekly_projection: f64,
    pub monthly_projection: f64,
    pub annual_projection: f64,
    pub average_daily_tokens: TokenSummary,
}

/// Token usage summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenSummary {
    pub input: u64,
    pub output: u64,
    pub cache: u64,
}

/// Optimization recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub category: RecommendationCategory,
    pub priority: Priority,
    pub title: String,
    pub description: String,
    pub estimated_savings: Option<f64>,
}

/// Categories of recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    Performance,
    Cost,
    Reliability,
    Security,
}

/// Priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    High,
    Medium,
    Low,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::new();

        collector
            .record("test_counter", MetricValue::Counter(42))
            .await;
        let value = collector.get("test_counter").await;

        assert!(matches!(value, Some(MetricValue::Counter(42))));
    }

    #[tokio::test]
    async fn test_pricing_model() {
        let model = PricingModel::default();
        let cost = model.calculate_cost(1_000_000, 500_000, 100_000);

        // Verify cost calculation
        assert!(cost > 0.0);
        assert_eq!(cost, 3.0 + 7.5 + 0.0375);
    }
}
