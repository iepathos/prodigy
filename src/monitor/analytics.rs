use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::metrics::{AggregationType, MetricsDatabase};
use super::{Metric, TimeFrame};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub name: String,
    pub timeframe: TimeFrame,
    pub findings: Vec<Finding>,
    pub recommendations: Vec<String>,
    pub metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: FindingSeverity,
    pub title: String,
    pub description: String,
    pub evidence: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[async_trait]
pub trait Analyzer: Send + Sync {
    async fn analyze(&self, timeframe: TimeFrame) -> Result<Analysis>;
    fn name(&self) -> &str;
}

pub struct AnalyticsEngine {
    metrics_db: Arc<MetricsDatabase>,
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalyticsEngine {
    pub fn new(metrics_db: Arc<MetricsDatabase>) -> Self {
        Self {
            metrics_db,
            analyzers: vec![],
        }
    }

    pub fn register(&mut self, analyzer: Box<dyn Analyzer>) {
        self.analyzers.push(analyzer);
    }

    pub async fn run_analysis(&self, timeframe: TimeFrame) -> Result<Vec<Analysis>> {
        let mut results = Vec::new();

        for analyzer in &self.analyzers {
            match analyzer.analyze(timeframe.clone()).await {
                Ok(analysis) => results.push(analysis),
                Err(e) => {
                    log::error!("Analysis failed for {}: {}", analyzer.name(), e);
                }
            }
        }

        Ok(results)
    }
}

pub struct BottleneckAnalyzer {
    metrics_db: Arc<MetricsDatabase>,
}

impl BottleneckAnalyzer {
    pub fn new(metrics_db: Arc<MetricsDatabase>) -> Self {
        Self { metrics_db }
    }
}

#[async_trait]
impl Analyzer for BottleneckAnalyzer {
    async fn analyze(&self, timeframe: TimeFrame) -> Result<Analysis> {
        let mut findings = Vec::new();
        let mut metrics = HashMap::new();

        // Analyze spec completion times
        let spec_metrics = self
            .metrics_db
            .query_metrics(
                "specs.completion_time_hours",
                timeframe.start,
                timeframe.end,
                None,
            )
            .await?;

        if !spec_metrics.is_empty() {
            // Calculate average completion time
            let times: Vec<f64> = spec_metrics
                .iter()
                .filter_map(|m| match &m.value {
                    super::MetricValue::Gauge(v) => Some(*v),
                    _ => None,
                })
                .collect();

            if !times.is_empty() {
                let avg_time = times.iter().sum::<f64>() / times.len() as f64;
                let max_time = times.iter().fold(0.0, |a, &b| a.max(b));

                metrics.insert("avg_completion_hours".to_string(), avg_time);
                metrics.insert("max_completion_hours".to_string(), max_time);

                // Find specs taking longer than average
                let threshold = avg_time * 1.5;
                let slow_specs = spec_metrics
                    .iter()
                    .filter(|m| matches!(&m.value, super::MetricValue::Gauge(v) if *v > threshold))
                    .count();

                if slow_specs > 0 {
                    findings.push(Finding {
                        severity: FindingSeverity::Medium,
                        title: "Slow Spec Completion Detected".to_string(),
                        description: format!(
                            "{} specs took more than 50% longer than average ({:.1} hours)",
                            slow_specs, avg_time
                        ),
                        evidence: {
                            let mut evidence = HashMap::new();
                            evidence.insert(
                                "slow_spec_count".to_string(),
                                serde_json::json!(slow_specs),
                            );
                            evidence.insert(
                                "threshold_hours".to_string(),
                                serde_json::json!(threshold),
                            );
                            evidence
                        },
                    });
                }
            }
        }

        // Analyze Claude response times
        let response_times = self
            .metrics_db
            .query_metrics(
                "claude.response_time_ms",
                timeframe.start,
                timeframe.end,
                None,
            )
            .await?;

        if !response_times.is_empty() {
            let mut all_times = Vec::new();
            for metric in &response_times {
                if let super::MetricValue::Histogram(times) = &metric.value {
                    all_times.extend(times);
                }
            }

            if !all_times.is_empty() {
                let avg_response = all_times.iter().sum::<f64>() / all_times.len() as f64;
                let p95_response = calculate_percentile(&mut all_times, 0.95);

                metrics.insert("avg_response_ms".to_string(), avg_response);
                metrics.insert("p95_response_ms".to_string(), p95_response);

                if p95_response > 5000.0 {
                    findings.push(Finding {
                        severity: FindingSeverity::High,
                        title: "High Claude Response Times".to_string(),
                        description: format!(
                            "95th percentile response time is {:.0}ms, which may impact productivity",
                            p95_response
                        ),
                        evidence: {
                            let mut evidence = HashMap::new();
                            evidence.insert("p95_ms".to_string(), serde_json::json!(p95_response));
                            evidence.insert("avg_ms".to_string(), serde_json::json!(avg_response));
                            evidence
                        },
                    });
                }
            }
        }

        // Analyze error rates
        let error_count = self
            .metrics_db
            .aggregate_metrics(
                "claude.errors",
                timeframe.start,
                timeframe.end,
                AggregationType::Sum,
            )
            .await?;

        let total_requests = self
            .metrics_db
            .aggregate_metrics(
                "claude.requests",
                timeframe.start,
                timeframe.end,
                AggregationType::Count,
            )
            .await?;

        if total_requests > 0.0 {
            let error_rate = (error_count / total_requests) * 100.0;
            metrics.insert("error_rate_pct".to_string(), error_rate);

            if error_rate > 5.0 {
                findings.push(Finding {
                    severity: FindingSeverity::High,
                    title: "High Error Rate".to_string(),
                    description: format!(
                        "Error rate is {:.1}%, indicating potential reliability issues",
                        error_rate
                    ),
                    evidence: {
                        let mut evidence = HashMap::new();
                        evidence.insert("error_count".to_string(), serde_json::json!(error_count));
                        evidence.insert(
                            "total_requests".to_string(),
                            serde_json::json!(total_requests),
                        );
                        evidence
                    },
                });
            }
        }

        let mut recommendations = Vec::new();

        if findings.iter().any(|f| {
            matches!(
                f.severity,
                FindingSeverity::High | FindingSeverity::Critical
            )
        }) {
            recommendations
                .push("Consider implementing retry logic with exponential backoff".to_string());
            recommendations
                .push("Review Claude API rate limits and adjust request frequency".to_string());
        }

        if metrics
            .get("avg_completion_hours")
            .map(|&v| v > 24.0)
            .unwrap_or(false)
        {
            recommendations
                .push("Break down large specs into smaller, more manageable tasks".to_string());
            recommendations.push("Consider parallel execution of independent specs".to_string());
        }

        Ok(Analysis {
            name: self.name().to_string(),
            timeframe,
            findings,
            recommendations,
            metrics,
        })
    }

    fn name(&self) -> &str {
        "bottleneck_analyzer"
    }
}

pub struct CostOptimizer {
    metrics_db: Arc<MetricsDatabase>,
}

impl CostOptimizer {
    pub fn new(metrics_db: Arc<MetricsDatabase>) -> Self {
        Self { metrics_db }
    }
}

#[async_trait]
impl Analyzer for CostOptimizer {
    async fn analyze(&self, timeframe: TimeFrame) -> Result<Analysis> {
        let mut findings = Vec::new();
        let mut metrics = HashMap::new();

        // Analyze token usage and costs
        let total_cost = self
            .metrics_db
            .aggregate_metrics(
                "claude.cost.usd",
                timeframe.start,
                timeframe.end,
                AggregationType::Sum,
            )
            .await?;

        let input_cost = self
            .metrics_db
            .aggregate_metrics(
                "claude.cost.input_usd",
                timeframe.start,
                timeframe.end,
                AggregationType::Sum,
            )
            .await?;

        let output_cost = self
            .metrics_db
            .aggregate_metrics(
                "claude.cost.output_usd",
                timeframe.start,
                timeframe.end,
                AggregationType::Sum,
            )
            .await?;

        metrics.insert("total_cost_usd".to_string(), total_cost);
        metrics.insert("input_cost_usd".to_string(), input_cost);
        metrics.insert("output_cost_usd".to_string(), output_cost);

        // Calculate daily average
        let days = (timeframe.end - timeframe.start).num_days() as f64;
        let daily_avg = if days > 0.0 {
            total_cost / days
        } else {
            total_cost
        };
        metrics.insert("daily_avg_cost_usd".to_string(), daily_avg);

        // Check for cost spikes
        let mut daily_costs = Vec::new();
        let mut current = timeframe.start;
        while current < timeframe.end {
            let next = current + ChronoDuration::days(1);
            let daily_cost = self
                .metrics_db
                .aggregate_metrics("claude.cost.usd", current, next, AggregationType::Sum)
                .await?;
            daily_costs.push(daily_cost);
            current = next;
        }

        if !daily_costs.is_empty() {
            let max_daily = daily_costs.iter().fold(0.0, |a, &b| a.max(b));

            if max_daily > daily_avg * 2.0 {
                findings.push(Finding {
                    severity: FindingSeverity::Medium,
                    title: "Cost Spike Detected".to_string(),
                    description: format!(
                        "Maximum daily cost (${:.2}) is more than 2x the average (${:.2})",
                        max_daily, daily_avg
                    ),
                    evidence: {
                        let mut evidence = HashMap::new();
                        evidence.insert("max_daily_usd".to_string(), serde_json::json!(max_daily));
                        evidence.insert("avg_daily_usd".to_string(), serde_json::json!(daily_avg));
                        evidence
                    },
                });
            }
        }

        // Analyze token efficiency
        let input_tokens = self
            .metrics_db
            .aggregate_metrics(
                "claude.tokens.input",
                timeframe.start,
                timeframe.end,
                AggregationType::Sum,
            )
            .await?;

        let output_tokens = self
            .metrics_db
            .aggregate_metrics(
                "claude.tokens.output",
                timeframe.start,
                timeframe.end,
                AggregationType::Sum,
            )
            .await?;

        if input_tokens > 0.0 {
            let input_output_ratio = output_tokens / input_tokens;
            metrics.insert("input_output_ratio".to_string(), input_output_ratio);

            if input_output_ratio < 0.5 {
                findings.push(Finding {
                    severity: FindingSeverity::Low,
                    title: "Low Output Generation".to_string(),
                    description: format!(
                        "Output tokens are only {:.1}x input tokens, suggesting inefficient prompts",
                        input_output_ratio
                    ),
                    evidence: {
                        let mut evidence = HashMap::new();
                        evidence.insert("input_tokens".to_string(), serde_json::json!(input_tokens));
                        evidence.insert("output_tokens".to_string(), serde_json::json!(output_tokens));
                        evidence
                    },
                });
            }
        }

        let mut recommendations = Vec::new();

        if total_cost > 100.0 {
            recommendations
                .push("Implement response caching to reduce redundant API calls".to_string());
        }

        if input_cost > output_cost * 0.5 {
            recommendations.push("Optimize prompts to reduce input token count".to_string());
            recommendations.push("Consider using context compression techniques".to_string());
        }

        if daily_avg > 50.0 {
            recommendations.push(
                "Review model selection - consider using Claude Haiku for simple tasks".to_string(),
            );
            recommendations.push("Implement budget alerts to prevent unexpected costs".to_string());
        }

        Ok(Analysis {
            name: self.name().to_string(),
            timeframe,
            findings,
            recommendations,
            metrics,
        })
    }

    fn name(&self) -> &str {
        "cost_optimizer"
    }
}

pub struct VelocityTracker {
    metrics_db: Arc<MetricsDatabase>,
}

impl VelocityTracker {
    pub fn new(metrics_db: Arc<MetricsDatabase>) -> Self {
        Self { metrics_db }
    }
}

#[async_trait]
impl Analyzer for VelocityTracker {
    async fn analyze(&self, timeframe: TimeFrame) -> Result<Analysis> {
        let mut findings = Vec::new();
        let mut metrics = HashMap::new();

        // Calculate completion velocity
        let completed_start = self
            .metrics_db
            .aggregate_metrics(
                "specs.completed",
                timeframe.start,
                timeframe.start,
                AggregationType::Sum,
            )
            .await?;

        let completed_end = self
            .metrics_db
            .aggregate_metrics(
                "specs.completed",
                timeframe.end,
                timeframe.end,
                AggregationType::Sum,
            )
            .await?;

        let specs_completed = completed_end - completed_start;
        let days = (timeframe.end - timeframe.start).num_days() as f64;
        let velocity = if days > 0.0 {
            specs_completed / days
        } else {
            0.0
        };

        metrics.insert("specs_completed".to_string(), specs_completed);
        metrics.insert("velocity_per_day".to_string(), velocity);

        // Calculate remaining work
        let total_specs = self
            .metrics_db
            .aggregate_metrics(
                "specs.total",
                timeframe.end,
                timeframe.end,
                AggregationType::Max,
            )
            .await?;

        let remaining = total_specs - completed_end;
        metrics.insert("specs_remaining".to_string(), remaining);

        // Project completion date
        if velocity > 0.0 && remaining > 0.0 {
            let days_to_complete = remaining / velocity;
            let estimated_completion = Utc::now() + ChronoDuration::days(days_to_complete as i64);

            metrics.insert("days_to_complete".to_string(), days_to_complete);

            findings.push(Finding {
                severity: FindingSeverity::Info,
                title: "Project Completion Estimate".to_string(),
                description: format!(
                    "At current velocity ({:.1} specs/day), project will complete around {}",
                    velocity,
                    estimated_completion.format("%Y-%m-%d")
                ),
                evidence: {
                    let mut evidence = HashMap::new();
                    evidence.insert("velocity".to_string(), serde_json::json!(velocity));
                    evidence.insert("remaining_specs".to_string(), serde_json::json!(remaining));
                    evidence.insert(
                        "estimated_date".to_string(),
                        serde_json::json!(estimated_completion.to_rfc3339()),
                    );
                    evidence
                },
            });
        }

        // Check for velocity trends
        let week_ago = timeframe.end - ChronoDuration::weeks(1);
        let last_week_velocity = if week_ago >= timeframe.start {
            let completed_week_ago = self
                .metrics_db
                .aggregate_metrics("specs.completed", week_ago, week_ago, AggregationType::Sum)
                .await?;
            (completed_end - completed_week_ago) / 7.0
        } else {
            velocity
        };

        if last_week_velocity > 0.0 {
            let velocity_change = ((velocity - last_week_velocity) / last_week_velocity) * 100.0;
            metrics.insert("velocity_change_pct".to_string(), velocity_change);

            if velocity_change < -20.0 {
                findings.push(Finding {
                    severity: FindingSeverity::Medium,
                    title: "Declining Velocity".to_string(),
                    description: format!(
                        "Velocity has decreased by {:.0}% compared to last week",
                        velocity_change.abs()
                    ),
                    evidence: {
                        let mut evidence = HashMap::new();
                        evidence
                            .insert("current_velocity".to_string(), serde_json::json!(velocity));
                        evidence.insert(
                            "last_week_velocity".to_string(),
                            serde_json::json!(last_week_velocity),
                        );
                        evidence
                    },
                });
            }
        }

        let mut recommendations = Vec::new();

        if velocity < 1.0 {
            recommendations
                .push("Consider breaking down complex specs into smaller tasks".to_string());
            recommendations.push("Review and remove any blockers in the workflow".to_string());
        }

        if remaining > 50.0 && velocity < 5.0 {
            recommendations.push("Consider parallelizing independent specs".to_string());
            recommendations.push("Allocate more resources to increase throughput".to_string());
        }

        Ok(Analysis {
            name: self.name().to_string(),
            timeframe,
            findings,
            recommendations,
            metrics,
        })
    }

    fn name(&self) -> &str {
        "velocity_tracker"
    }
}

fn calculate_percentile(data: &mut Vec<f64>, percentile: f64) -> f64 {
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let index = ((percentile * (data.len() - 1) as f64) as usize).min(data.len() - 1);
    data[index]
}
