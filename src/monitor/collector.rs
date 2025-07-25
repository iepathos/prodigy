use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{Metric, MetricValue};
use crate::claude::ClaudeManager;
use crate::error::Result;
use crate::spec::SpecEngine;
use crate::state::StateManager;

#[async_trait]
pub trait MetricCollector: Send + Sync {
    async fn collect(&self) -> Result<Vec<Metric>>;
    fn name(&self) -> &str;
    fn interval(&self) -> Duration;
}

pub struct MetricsCollector {
    collectors: Vec<Box<dyn MetricCollector>>,
    last_collection: Arc<Mutex<HashMap<String, Instant>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            collectors: vec![],
            last_collection: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&mut self, collector: Box<dyn MetricCollector>) {
        self.collectors.push(collector);
    }

    pub async fn collect_all(&self) -> Result<Vec<Metric>> {
        let mut all_metrics = Vec::new();
        let mut last_collection = self.last_collection.lock().await;

        for collector in &self.collectors {
            let name = collector.name();
            let interval = collector.interval();

            // Check if enough time has passed since last collection
            let should_collect = match last_collection.get(name) {
                Some(last) => last.elapsed() >= interval,
                None => true,
            };

            if should_collect {
                match collector.collect().await {
                    Ok(metrics) => {
                        all_metrics.extend(metrics);
                        last_collection.insert(name.to_string(), Instant::now());
                    }
                    Err(e) => {
                        log::error!("Failed to collect metrics from {}: {}", name, e);
                    }
                }
            }
        }

        Ok(all_metrics)
    }
}

pub struct ClaudeMetricsCollector {
    claude_manager: Arc<ClaudeManager>,
}

impl ClaudeMetricsCollector {
    pub fn new(claude_manager: Arc<ClaudeManager>) -> Self {
        Self { claude_manager }
    }
}

#[async_trait]
impl MetricCollector for ClaudeMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = Utc::now();

        // Get token usage from Claude manager
        if let Ok(usage) = self.claude_manager.get_token_usage().await {
            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "claude.tokens.input".to_string(),
                value: MetricValue::Counter(usage.input_tokens),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "claude.tokens.output".to_string(),
                value: MetricValue::Counter(usage.output_tokens),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "claude.tokens.total".to_string(),
                value: MetricValue::Counter(usage.total_tokens),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });
        }

        // Get response times
        if let Ok(response_times) = self.claude_manager.get_response_times().await {
            if !response_times.is_empty() {
                metrics.push(Metric {
                    id: Uuid::new_v4(),
                    name: "claude.response_time_ms".to_string(),
                    value: MetricValue::Histogram(response_times),
                    timestamp,
                    labels: HashMap::new(),
                    project_id: None,
                });
            }
        }

        // Get error rate
        if let Ok(error_count) = self.claude_manager.get_error_count().await {
            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "claude.errors".to_string(),
                value: MetricValue::Counter(error_count),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });
        }

        Ok(metrics)
    }

    fn name(&self) -> &str {
        "claude_metrics"
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(60) // Collect every minute
    }
}

pub struct SpecMetricsCollector {
    spec_engine: Arc<SpecEngine>,
    state_manager: Arc<StateManager>,
}

impl SpecMetricsCollector {
    pub fn new(spec_engine: Arc<SpecEngine>, state_manager: Arc<StateManager>) -> Self {
        Self {
            spec_engine,
            state_manager,
        }
    }
}

#[async_trait]
impl MetricCollector for SpecMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = Utc::now();

        // Get spec completion metrics
        if let Ok(projects) = self.state_manager.list_projects().await {
            for project in projects {
                let mut labels = HashMap::new();
                labels.insert("project_id".to_string(), project.id.to_string());
                labels.insert("project_name".to_string(), project.name.clone());

                // Count specs by status
                if let Ok(specs) = self.spec_engine.list_specs(&project.path).await {
                    let total = specs.len() as u64;
                    let completed = specs.iter().filter(|s| s.status == "completed").count() as u64;
                    let in_progress =
                        specs.iter().filter(|s| s.status == "in_progress").count() as u64;
                    let pending = specs.iter().filter(|s| s.status == "pending").count() as u64;

                    metrics.push(Metric {
                        id: Uuid::new_v4(),
                        name: "specs.total".to_string(),
                        value: MetricValue::Gauge(total as f64),
                        timestamp,
                        labels: labels.clone(),
                        project_id: Some(project.id),
                    });

                    metrics.push(Metric {
                        id: Uuid::new_v4(),
                        name: "specs.completed".to_string(),
                        value: MetricValue::Gauge(completed as f64),
                        timestamp,
                        labels: labels.clone(),
                        project_id: Some(project.id),
                    });

                    metrics.push(Metric {
                        id: Uuid::new_v4(),
                        name: "specs.in_progress".to_string(),
                        value: MetricValue::Gauge(in_progress as f64),
                        timestamp,
                        labels: labels.clone(),
                        project_id: Some(project.id),
                    });

                    metrics.push(Metric {
                        id: Uuid::new_v4(),
                        name: "specs.pending".to_string(),
                        value: MetricValue::Gauge(pending as f64),
                        timestamp,
                        labels: labels.clone(),
                        project_id: Some(project.id),
                    });

                    // Completion percentage
                    let completion_pct = if total > 0 {
                        (completed as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };

                    metrics.push(Metric {
                        id: Uuid::new_v4(),
                        name: "specs.completion_percentage".to_string(),
                        value: MetricValue::Gauge(completion_pct),
                        timestamp,
                        labels: labels.clone(),
                        project_id: Some(project.id),
                    });
                }
            }
        }

        Ok(metrics)
    }

    fn name(&self) -> &str {
        "spec_metrics"
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(300) // Collect every 5 minutes
    }
}

pub struct SystemMetricsCollector;

impl SystemMetricsCollector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MetricCollector for SystemMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = Utc::now();

        // Memory usage
        if let Ok(mem_info) = sys_info::mem_info() {
            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "system.memory.total_kb".to_string(),
                value: MetricValue::Gauge(mem_info.total as f64),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "system.memory.free_kb".to_string(),
                value: MetricValue::Gauge(mem_info.free as f64),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });

            let used_pct =
                ((mem_info.total - mem_info.free) as f64 / mem_info.total as f64) * 100.0;
            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "system.memory.used_percentage".to_string(),
                value: MetricValue::Gauge(used_pct),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });
        }

        // CPU load
        if let Ok(loadavg) = sys_info::loadavg() {
            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "system.cpu.load_1m".to_string(),
                value: MetricValue::Gauge(loadavg.one),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "system.cpu.load_5m".to_string(),
                value: MetricValue::Gauge(loadavg.five),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "system.cpu.load_15m".to_string(),
                value: MetricValue::Gauge(loadavg.fifteen),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });
        }

        Ok(metrics)
    }

    fn name(&self) -> &str {
        "system_metrics"
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(60) // Collect every minute
    }
}

pub struct CostMetricsCollector {
    claude_manager: Arc<ClaudeManager>,
}

impl CostMetricsCollector {
    pub fn new(claude_manager: Arc<ClaudeManager>) -> Self {
        Self { claude_manager }
    }

    fn calculate_cost(input_tokens: u64, output_tokens: u64, model: &str) -> f64 {
        // Pricing per 1K tokens (example rates, adjust as needed)
        let (input_rate, output_rate) = match model {
            "claude-3-opus-20240229" => (0.015, 0.075),
            "claude-3-sonnet-20240229" => (0.003, 0.015),
            "claude-3-haiku-20240307" => (0.00025, 0.00125),
            _ => (0.003, 0.015), // Default to Sonnet pricing
        };

        let input_cost = (input_tokens as f64 / 1000.0) * input_rate;
        let output_cost = (output_tokens as f64 / 1000.0) * output_rate;

        input_cost + output_cost
    }
}

#[async_trait]
impl MetricCollector for CostMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = Utc::now();

        // Calculate costs based on token usage
        if let Ok(usage) = self.claude_manager.get_token_usage().await {
            let model = self
                .claude_manager
                .get_current_model()
                .await
                .unwrap_or_else(|_| "claude-3-sonnet-20240229".to_string());
            let cost = Self::calculate_cost(usage.input_tokens, usage.output_tokens, &model);

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "claude.cost.usd".to_string(),
                value: MetricValue::Gauge(cost),
                timestamp,
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("model".to_string(), model.clone());
                    labels
                },
                project_id: None,
            });

            // Cost breakdown
            let input_cost = Self::calculate_cost(usage.input_tokens, 0, &model);
            let output_cost = Self::calculate_cost(0, usage.output_tokens, &model);

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "claude.cost.input_usd".to_string(),
                value: MetricValue::Gauge(input_cost),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });

            metrics.push(Metric {
                id: Uuid::new_v4(),
                name: "claude.cost.output_usd".to_string(),
                value: MetricValue::Gauge(output_cost),
                timestamp,
                labels: HashMap::new(),
                project_id: None,
            });
        }

        Ok(metrics)
    }

    fn name(&self) -> &str {
        "cost_metrics"
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(300) // Collect every 5 minutes
    }
}
