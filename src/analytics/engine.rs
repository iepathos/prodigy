//! Analytics engine for Claude session analysis

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Timelike, Utc};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::models::{
    Cost, PricingModel, Session, SessionEvent, SessionIndex, TimeRange, ToolStat, ToolStats,
};

/// Analytics engine for processing Claude session data
pub struct AnalyticsEngine {
    index: Arc<RwLock<SessionIndex>>,
    _metrics: Arc<MetricsCollector>,
    pricing_model: PricingModel,
}

impl AnalyticsEngine {
    /// Create a new analytics engine
    pub fn new(index: Arc<RwLock<SessionIndex>>) -> Self {
        Self {
            index,
            _metrics: Arc::new(MetricsCollector::new()),
            pricing_model: PricingModel::default(),
        }
    }

    /// Calculate the cost of a specific session
    pub async fn calculate_session_cost(&self, session_id: &str) -> Result<Cost> {
        let index = self.index.read().await;
        let session = index.get_session(session_id)
            .await
            .with_context(|| format!("Failed to retrieve session {} for cost calculation", session_id))?;

        let cost = Cost {
            input_tokens: session.total_input_tokens(),
            output_tokens: session.total_output_tokens(),
            cache_tokens: session.total_cache_tokens(),
            estimated_cost_usd: self.calculate_cost(&session),
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
        let sessions = index.query_sessions(time_range)
            .await
            .with_context(|| format!("Failed to query sessions in time range {} to {}", time_range.start, time_range.end))?;

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

        // Generate hourly distribution using functional approach
        let mut hourly_distribution = vec![0u64; 24];
        sessions.iter().for_each(|session| {
            hourly_distribution[session.started_at.hour() as usize] += 1;
        });

        // Generate daily sessions map using fold
        let daily_sessions = sessions.iter()
            .fold(HashMap::new(), |mut acc, session| {
                *acc.entry(session.started_at.date_naive()).or_insert(0u64) += 1;
                acc
            });

        // Generate tool frequency map using fold
        let tool_frequency = sessions.iter()
            .flat_map(|session| &session.tool_invocations)
            .fold(HashMap::new(), |mut acc, tool| {
                *acc.entry(tool.name.clone()).or_insert(0u64) += 1;
                acc
            })

        // Find peak hours
        let peak_hour = hourly_distribution
            .iter()
            .enumerate()
            .max_by_key(|(_, count)| *count)
            .map(|(hour, _)| hour)
            .unwrap_or(0); // Safe: hourly_distribution is always 24 elements, so max() will always find a value

        // Find most used tools using functional approach
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

        // Calculate total cost and tokens using fold
        let (total_cost, total_tokens) = sessions.iter()
            .fold((0.0, TokenSummary::default()), |(cost, mut tokens), session| {
                tokens.input += session.total_input_tokens();
                tokens.output += session.total_output_tokens();
                tokens.cache += session.total_cache_tokens();
                (cost + self.calculate_cost(session), tokens)
            })

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

    /// Analyze patterns across multiple sessions
    pub async fn analyze_cross_session_patterns(
        &self,
        session_ids: Vec<String>,
    ) -> Result<CrossSessionAnalysis> {
        let index = self.index.read().await;

        // Collect sessions using filter_map
        let sessions: Vec<_> = futures::stream::iter(session_ids.iter())
            .then(|id| async {
                index.get_session(id).await.ok()
            })
            .filter_map(|session| async move { session })
            .collect::<Vec<_>>()
            .await

        if sessions.is_empty() {
            return Ok(CrossSessionAnalysis::default());
        }

        // Analyze sessions using functional approach
        let tool_usage = sessions.iter()
            .flat_map(|session| &session.tool_invocations)
            .fold(HashMap::new(), |mut acc, tool| {
                *acc.entry(tool.name.clone()).or_insert(0u64) += 1;
                acc
            });

        let total_cost = sessions.iter()
            .map(|session| self.calculate_cost(session))
            .sum::<f64>();

        let total_duration_ms = sessions.iter()
            .filter_map(|session| session.completed_at.map(|completed| {
                (completed - session.started_at).num_milliseconds().max(0) as u64
            }))
            .sum::<u64>();

        let error_patterns = sessions.iter()
            .flat_map(|session| &session.events)
            .filter_map(|event| {
                if let SessionEvent::Error { error_type, .. } = event {
                    Some(error_type.clone())
                } else {
                    None
                }
            })
            .fold(HashMap::new(), |mut acc, error_type| {
                *acc.entry(error_type).or_insert(0u64) += 1;
                acc
            })

        // Find most common tools using functional approach
        let mut tool_list: Vec<_> = tool_usage.into_iter().collect();
        tool_list.sort_by(|a, b| b.1.cmp(&a.1));
        let common_tools: Vec<String> = tool_list
            .into_iter()
            .take(10)
            .map(|(name, _)| name)
            .collect();

        // Find most common errors using functional approach
        let mut error_list: Vec<_> = error_patterns.into_iter().collect();
        error_list.sort_by(|a, b| b.1.cmp(&a.1));
        let common_errors: Vec<String> = error_list
            .into_iter()
            .take(5)
            .map(|(error, _)| error)
            .collect();

        Ok(CrossSessionAnalysis {
            session_count: sessions.len(),
            total_cost,
            average_cost: total_cost / sessions.len() as f64,
            total_duration_ms,
            average_duration_ms: total_duration_ms / sessions.len() as u64,
            common_tools,
            common_errors,
            earliest_session: sessions.iter().map(|s| s.started_at).min(),
            latest_session: sessions.iter().map(|s| s.started_at).max(),
        })
    }

    /// Compare two sessions to identify differences
    pub async fn compare_sessions(
        &self,
        session_id_1: &str,
        session_id_2: &str,
    ) -> Result<SessionComparison> {
        let index = self.index.read().await;
        let session1 = index.get_session(session_id_1).await?;
        let session2 = index.get_session(session_id_2).await?;

        // Compare token usage
        let token_diff = TokenComparison {
            input_diff: session2.total_input_tokens as i64 - session1.total_input_tokens as i64,
            output_diff: session2.total_output_tokens as i64 - session1.total_output_tokens as i64,
            cache_diff: session2.total_cache_tokens as i64 - session1.total_cache_tokens as i64,
        };

        // Compare costs
        let cost1 = self.calculate_cost(&session1);
        let cost2 = self.calculate_cost(&session2);
        let cost_diff = cost2 - cost1;

        // Compare tools used
        let tools1: HashSet<String> = session1
            .tool_invocations
            .iter()
            .map(|t| t.name.clone())
            .collect();
        let tools2: HashSet<String> = session2
            .tool_invocations
            .iter()
            .map(|t| t.name.clone())
            .collect();

        let tools_added: Vec<String> = tools2.difference(&tools1).cloned().collect();
        let tools_removed: Vec<String> = tools1.difference(&tools2).cloned().collect();
        let tools_common: Vec<String> = tools1.intersection(&tools2).cloned().collect();

        // Calculate duration difference
        let duration1 = session1.completed_at.unwrap_or(session1.started_at) - session1.started_at;
        let duration2 = session2.completed_at.unwrap_or(session2.started_at) - session2.started_at;
        let duration_diff_ms = (duration2 - duration1).num_milliseconds();

        Ok(SessionComparison {
            session_id_1: session_id_1.to_string(),
            session_id_2: session_id_2.to_string(),
            token_comparison: token_diff,
            cost_diff,
            cost_percentage_change: if cost1 > 0.0 {
                (cost_diff / cost1) * 100.0
            } else {
                0.0
            },
            duration_diff_ms,
            tools_added,
            tools_removed,
            tools_common,
        })
    }

    /// Get optimization recommendations based on usage analysis
    pub async fn get_optimization_recommendations(&self) -> Result<Vec<Recommendation>> {
        // Check for performance issues and convert to recommendations
        let bottlenecks = self.identify_bottlenecks(5000).await?;
        let mut recommendations: Vec<Recommendation> = bottlenecks
            .into_iter()
            .map(|issue| Recommendation {
                category: RecommendationCategory::Performance,
                priority: if issue.average_duration_ms > 10000 {
                    Priority::High
                } else {
                    Priority::Medium
                },
                title: format!("Optimize {}", issue.tool_name),
                description: issue.recommendation,
                estimated_savings: None,
            })
            .collect()

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

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
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

/// Cross-session analysis results
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrossSessionAnalysis {
    pub session_count: usize,
    pub total_cost: f64,
    pub average_cost: f64,
    pub total_duration_ms: u64,
    pub average_duration_ms: u64,
    pub common_tools: Vec<String>,
    pub common_errors: Vec<String>,
    pub earliest_session: Option<DateTime<Utc>>,
    pub latest_session: Option<DateTime<Utc>>,
}

/// Session comparison results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionComparison {
    pub session_id_1: String,
    pub session_id_2: String,
    pub token_comparison: TokenComparison,
    pub cost_diff: f64,
    pub cost_percentage_change: f64,
    pub duration_diff_ms: i64,
    pub tools_added: Vec<String>,
    pub tools_removed: Vec<String>,
    pub tools_common: Vec<String>,
}

/// Token usage comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenComparison {
    pub input_diff: i64,
    pub output_diff: i64,
    pub cache_diff: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::models::{Session, SessionEvent, ToolInvocation};

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

    #[tokio::test]
    async fn test_analytics_engine_cost_calculation() {
        let index = Arc::new(RwLock::new(SessionIndex::new()));
        let engine = AnalyticsEngine::new(index.clone());

        // Add a test session
        let session = Session {
            session_id: "test-session".to_string(),
            project_path: "/test".to_string(),
            jsonl_path: "/test.jsonl".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            model: Some("claude-3".to_string()),
            events: vec![],
            total_input_tokens: 1000,
            total_output_tokens: 2000,
            total_cache_tokens: 500,
            tool_invocations: vec![],
        };

        let mut idx = index.write().await;
        idx.insert_test_session("test-session".to_string(), session);
        drop(idx);

        let cost = engine.calculate_session_cost("test-session").await.unwrap();
        assert_eq!(cost.input_tokens, 1000);
        assert_eq!(cost.output_tokens, 2000);
        assert_eq!(cost.cache_tokens, 500);
        assert!(cost.estimated_cost_usd > 0.0);
    }

    #[tokio::test]
    async fn test_tool_usage_analysis() {
        let index = Arc::new(RwLock::new(SessionIndex::new()));
        let engine = AnalyticsEngine::new(index.clone());

        // Create session with tool invocations
        let session = Session {
            session_id: "test-tools".to_string(),
            project_path: "/test".to_string(),
            jsonl_path: "/test.jsonl".to_string(),
            started_at: Utc::now() - Duration::hours(1),
            completed_at: Some(Utc::now()),
            model: None,
            events: vec![],
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_tokens: 0,
            tool_invocations: vec![
                ToolInvocation {
                    name: "Read".to_string(),
                    invoked_at: Utc::now(),
                    duration_ms: Some(100),
                    parameters: serde_json::json!({}),
                    result_size: Some(1024),
                },
                ToolInvocation {
                    name: "Write".to_string(),
                    invoked_at: Utc::now(),
                    duration_ms: Some(200),
                    parameters: serde_json::json!({}),
                    result_size: Some(2048),
                },
                ToolInvocation {
                    name: "Read".to_string(),
                    invoked_at: Utc::now(),
                    duration_ms: Some(150),
                    parameters: serde_json::json!({}),
                    result_size: Some(512),
                },
            ],
        };

        let mut idx = index.write().await;
        idx.insert_test_session("test-tools".to_string(), session);
        drop(idx);

        let time_range = TimeRange {
            start: Utc::now() - Duration::days(1),
            end: Utc::now(),
        };

        let tool_stats = engine.analyze_tool_usage(time_range).await.unwrap();
        assert_eq!(tool_stats.stats.len(), 2); // Read and Write

        let read_stat = tool_stats.stats.get("Read").unwrap();
        assert_eq!(read_stat.total_invocations, 2);
        assert_eq!(read_stat.average_duration_ms, 125); // (100 + 150) / 2

        let write_stat = tool_stats.stats.get("Write").unwrap();
        assert_eq!(write_stat.total_invocations, 1);
        assert_eq!(write_stat.average_duration_ms, 200);
    }

    #[tokio::test]
    async fn test_cross_session_analysis() {
        let index = Arc::new(RwLock::new(SessionIndex::new()));
        let engine = AnalyticsEngine::new(index.clone());

        // Create multiple sessions
        let session1 = Session {
            session_id: "session1".to_string(),
            project_path: "/test".to_string(),
            jsonl_path: "/test1.jsonl".to_string(),
            started_at: Utc::now() - Duration::hours(2),
            completed_at: Some(Utc::now() - Duration::hours(1)),
            model: None,
            events: vec![SessionEvent::Error {
                timestamp: Utc::now(),
                error_type: "NetworkError".to_string(),
                message: "Connection failed".to_string(),
            }],
            total_input_tokens: 1000,
            total_output_tokens: 2000,
            total_cache_tokens: 500,
            tool_invocations: vec![ToolInvocation {
                name: "Read".to_string(),
                invoked_at: Utc::now(),
                duration_ms: Some(100),
                parameters: serde_json::json!({}),
                result_size: None,
            }],
        };

        let session2 = Session {
            session_id: "session2".to_string(),
            project_path: "/test".to_string(),
            jsonl_path: "/test2.jsonl".to_string(),
            started_at: Utc::now() - Duration::hours(1),
            completed_at: Some(Utc::now()),
            model: None,
            events: vec![SessionEvent::Error {
                timestamp: Utc::now(),
                error_type: "NetworkError".to_string(),
                message: "Timeout".to_string(),
            }],
            total_input_tokens: 1500,
            total_output_tokens: 2500,
            total_cache_tokens: 600,
            tool_invocations: vec![
                ToolInvocation {
                    name: "Write".to_string(),
                    invoked_at: Utc::now(),
                    duration_ms: Some(200),
                    parameters: serde_json::json!({}),
                    result_size: None,
                },
                ToolInvocation {
                    name: "Read".to_string(),
                    invoked_at: Utc::now(),
                    duration_ms: Some(150),
                    parameters: serde_json::json!({}),
                    result_size: None,
                },
            ],
        };

        let mut idx = index.write().await;
        idx.insert_test_session("session1".to_string(), session1);
        idx.insert_test_session("session2".to_string(), session2);
        drop(idx);

        let analysis = engine
            .analyze_cross_session_patterns(vec!["session1".to_string(), "session2".to_string()])
            .await
            .unwrap();

        assert_eq!(analysis.session_count, 2);
        assert!(analysis.total_cost > 0.0);
        assert!(analysis.common_tools.contains(&"Read".to_string()));
        assert!(analysis.common_errors.contains(&"NetworkError".to_string()));
    }

    #[tokio::test]
    async fn test_session_comparison() {
        let index = Arc::new(RwLock::new(SessionIndex::new()));
        let engine = AnalyticsEngine::new(index.clone());

        let session1 = Session {
            session_id: "compare1".to_string(),
            project_path: "/test".to_string(),
            jsonl_path: "/test1.jsonl".to_string(),
            started_at: Utc::now() - Duration::hours(2),
            completed_at: Some(Utc::now() - Duration::hours(1)),
            model: None,
            events: vec![],
            total_input_tokens: 1000,
            total_output_tokens: 2000,
            total_cache_tokens: 500,
            tool_invocations: vec![ToolInvocation {
                name: "Read".to_string(),
                invoked_at: Utc::now(),
                duration_ms: Some(100),
                parameters: serde_json::json!({}),
                result_size: None,
            }],
        };

        let session2 = Session {
            session_id: "compare2".to_string(),
            project_path: "/test".to_string(),
            jsonl_path: "/test2.jsonl".to_string(),
            started_at: Utc::now() - Duration::hours(1),
            completed_at: Some(Utc::now()),
            model: None,
            events: vec![],
            total_input_tokens: 1500,
            total_output_tokens: 2500,
            total_cache_tokens: 600,
            tool_invocations: vec![
                ToolInvocation {
                    name: "Write".to_string(),
                    invoked_at: Utc::now(),
                    duration_ms: Some(200),
                    parameters: serde_json::json!({}),
                    result_size: None,
                },
                ToolInvocation {
                    name: "Read".to_string(),
                    invoked_at: Utc::now(),
                    duration_ms: Some(150),
                    parameters: serde_json::json!({}),
                    result_size: None,
                },
            ],
        };

        let mut idx = index.write().await;
        idx.insert_test_session("compare1".to_string(), session1);
        idx.insert_test_session("compare2".to_string(), session2);
        drop(idx);

        let comparison = engine
            .compare_sessions("compare1", "compare2")
            .await
            .unwrap();

        assert_eq!(comparison.token_comparison.input_diff, 500);
        assert_eq!(comparison.token_comparison.output_diff, 500);
        assert_eq!(comparison.token_comparison.cache_diff, 100);
        assert!(comparison.cost_diff > 0.0);
        assert!(comparison.tools_added.contains(&"Write".to_string()));
        assert!(comparison.tools_common.contains(&"Read".to_string()));
    }

    #[tokio::test]
    async fn test_bottleneck_identification() {
        let index = Arc::new(RwLock::new(SessionIndex::new()));
        let engine = AnalyticsEngine::new(index.clone());

        let session = Session {
            session_id: "bottleneck-test".to_string(),
            project_path: "/test".to_string(),
            jsonl_path: "/test.jsonl".to_string(),
            started_at: Utc::now() - Duration::hours(1),
            completed_at: Some(Utc::now()),
            model: None,
            events: vec![],
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_tokens: 0,
            tool_invocations: vec![ToolInvocation {
                name: "SlowTool".to_string(),
                invoked_at: Utc::now(),
                duration_ms: Some(10000), // 10 seconds - slow!
                parameters: serde_json::json!({}),
                result_size: None,
            }],
        };

        let mut idx = index.write().await;
        idx.insert_test_session("bottleneck-test".to_string(), session);
        drop(idx);

        let bottlenecks = engine.identify_bottlenecks(5000).await.unwrap();
        assert!(!bottlenecks.is_empty());

        let slow_tool = &bottlenecks[0];
        assert_eq!(slow_tool.tool_name, "SlowTool");
        assert!(matches!(slow_tool.issue_type, IssueType::SlowExecution));
    }

    #[tokio::test]
    async fn test_cost_projection() {
        let index = Arc::new(RwLock::new(SessionIndex::new()));
        let engine = AnalyticsEngine::new(index.clone());

        // Create sessions over past week
        for i in 0..7 {
            let session = Session {
                session_id: format!("proj-session-{}", i),
                project_path: "/test".to_string(),
                jsonl_path: format!("/test{}.jsonl", i),
                started_at: Utc::now() - Duration::days(i),
                completed_at: Some(Utc::now() - Duration::days(i) + Duration::hours(1)),
                model: None,
                events: vec![],
                total_input_tokens: 1000,
                total_output_tokens: 2000,
                total_cache_tokens: 500,
                tool_invocations: vec![],
            };

            let mut idx = index.write().await;
            idx.insert_test_session(format!("proj-session-{}", i), session);
        }

        let projection = engine.project_costs(7).await.unwrap();
        assert!(projection.daily_average > 0.0);
        assert!(projection.weekly_projection > 0.0);
        assert!(projection.monthly_projection > 0.0);
        assert!(projection.annual_projection > 0.0);
    }
}
