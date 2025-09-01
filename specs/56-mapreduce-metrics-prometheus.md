---
number: 56
title: MapReduce Prometheus Metrics Export
category: optimization
priority: medium
status: draft
dependencies: [55]
created: 2025-01-29
---

# Specification 56: MapReduce Prometheus Metrics Export

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [55 - OpenTelemetry Integration]

## Context

While OpenTelemetry provides metrics collection, we need a dedicated Prometheus metrics endpoint for integration with existing monitoring infrastructure. Prometheus is the de facto standard for metrics in cloud-native environments, and direct integration enables powerful querying, alerting, and visualization through Grafana.

## Objective

Implement a Prometheus metrics exporter for MapReduce operations that exposes detailed metrics about job execution, agent performance, resource utilization, and system health through a standard `/metrics` endpoint.

## Requirements

### Functional Requirements
- Expose metrics via HTTP `/metrics` endpoint
- Implement standard Prometheus metric types
- Include job-level and agent-level metrics
- Support custom business metrics
- Enable metric aggregation and histograms
- Provide resource utilization metrics
- Include error rate metrics
- Support metric labels for filtering

### Non-Functional Requirements
- Metrics endpoint response < 100ms
- Support 10,000+ metric series
- Efficient metric storage in memory
- Atomic metric updates
- Thread-safe metric collection

## Acceptance Criteria

- [ ] Prometheus metrics endpoint at :9090/metrics
- [ ] All key metrics exposed with proper types
- [ ] Metrics include appropriate labels
- [ ] Histograms with configurable buckets
- [ ] Grafana dashboard template provided
- [ ] Alert rule examples included
- [ ] Metrics documentation generated
- [ ] Performance impact < 1%
- [ ] Cardinality limits enforced
- [ ] Metric retention configured

## Technical Details

### Implementation Approach

1. **Metrics Registry**
```rust
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec,
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec,
    Registry, TextEncoder, Encoder,
};

pub struct MapReduceMetricsRegistry {
    registry: Registry,
    
    // Job metrics
    jobs_total: CounterVec,
    jobs_duration_seconds: HistogramVec,
    jobs_active: GaugeVec,
    
    // Agent metrics
    agents_total: CounterVec,
    agents_duration_seconds: HistogramVec,
    agents_active: Gauge,
    agents_queued: Gauge,
    
    // Performance metrics
    throughput_items_per_second: GaugeVec,
    queue_depth: GaugeVec,
    processing_latency_seconds: HistogramVec,
    
    // Resource metrics
    memory_usage_bytes: GaugeVec,
    cpu_usage_percent: GaugeVec,
    worktrees_active: Gauge,
    disk_usage_bytes: GaugeVec,
    
    // Error metrics
    errors_total: CounterVec,
    retries_total: CounterVec,
    dlq_items: GaugeVec,
    
    // Business metrics
    items_processed_total: CounterVec,
    commits_created_total: CounterVec,
    files_modified_total: CounterVec,
}

impl MapReduceMetricsRegistry {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();
        
        let jobs_total = register_counter_vec!(
            "prodigy_mapreduce_jobs_total",
            "Total number of MapReduce jobs",
            &["status", "workflow"]
        )?;
        
        let jobs_duration_seconds = register_histogram_vec!(
            "prodigy_mapreduce_job_duration_seconds",
            "Job execution duration in seconds",
            &["workflow"],
            vec![30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0]
        )?;
        
        let agents_duration_seconds = register_histogram_vec!(
            "prodigy_mapreduce_agent_duration_seconds",
            "Agent execution duration in seconds",
            &["job_id", "status"],
            vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]
        )?;
        
        let throughput_items_per_second = register_gauge_vec!(
            "prodigy_mapreduce_throughput_items_per_second",
            "Current throughput in items per second",
            &["job_id"]
        )?;
        
        let errors_total = register_counter_vec!(
            "prodigy_mapreduce_errors_total",
            "Total number of errors",
            &["job_id", "error_type", "recoverable"]
        )?;
        
        // Register all metrics with the registry
        registry.register(Box::new(jobs_total.clone()))?;
        registry.register(Box::new(jobs_duration_seconds.clone()))?;
        registry.register(Box::new(agents_duration_seconds.clone()))?;
        // ... register all other metrics
        
        Ok(Self {
            registry,
            jobs_total,
            jobs_duration_seconds,
            jobs_active: register_gauge_vec!("prodigy_mapreduce_jobs_active", "Currently active jobs", &["workflow"])?,
            agents_total: register_counter_vec!("prodigy_mapreduce_agents_total", "Total agents", &["job_id", "status"])?,
            agents_duration_seconds,
            agents_active: register_gauge!("prodigy_mapreduce_agents_active", "Currently active agents")?,
            agents_queued: register_gauge!("prodigy_mapreduce_agents_queued", "Agents waiting in queue")?,
            throughput_items_per_second,
            queue_depth: register_gauge_vec!("prodigy_mapreduce_queue_depth", "Current queue depth", &["job_id", "priority"])?,
            processing_latency_seconds: register_histogram_vec!(
                "prodigy_mapreduce_processing_latency_seconds",
                "Time from queue to processing",
                &["job_id"],
                prometheus::exponential_buckets(0.001, 2.0, 15)?
            )?,
            memory_usage_bytes: register_gauge_vec!("prodigy_mapreduce_memory_bytes", "Memory usage", &["job_id"])?,
            cpu_usage_percent: register_gauge_vec!("prodigy_mapreduce_cpu_percent", "CPU usage", &["job_id"])?,
            worktrees_active: register_gauge!("prodigy_mapreduce_worktrees_active", "Active git worktrees")?,
            disk_usage_bytes: register_gauge_vec!("prodigy_mapreduce_disk_bytes", "Disk usage", &["path"])?,
            errors_total,
            retries_total: register_counter_vec!("prodigy_mapreduce_retries_total", "Total retries", &["job_id", "reason"])?,
            dlq_items: register_gauge_vec!("prodigy_mapreduce_dlq_items", "Items in dead letter queue", &["job_id"])?,
            items_processed_total: register_counter_vec!("prodigy_mapreduce_items_total", "Items processed", &["job_id", "status"])?,
            commits_created_total: register_counter_vec!("prodigy_mapreduce_commits_total", "Git commits created", &["job_id"])?,
            files_modified_total: register_counter_vec!("prodigy_mapreduce_files_modified_total", "Files modified", &["job_id"])?,
        })
    }
    
    pub fn export(&self) -> Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}
```

2. **Metrics HTTP Server**
```rust
use axum::{Router, response::Response, extract::State};
use std::net::SocketAddr;

pub struct MetricsServer {
    registry: Arc<MapReduceMetricsRegistry>,
    port: u16,
}

impl MetricsServer {
    pub async fn start(&self) -> Result<()> {
        let app = Router::new()
            .route("/metrics", get(Self::metrics_handler))
            .route("/health", get(Self::health_handler))
            .with_state(self.registry.clone());
        
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        
        info!("Prometheus metrics server listening on http://{}", addr);
        
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;
        
        Ok(())
    }
    
    async fn metrics_handler(
        State(registry): State<Arc<MapReduceMetricsRegistry>>
    ) -> Result<Response<String>, StatusCode> {
        match registry.export() {
            Ok(metrics) => Ok(Response::builder()
                .status(200)
                .header("Content-Type", "text/plain; version=0.0.4")
                .body(metrics)
                .unwrap()),
            Err(e) => {
                error!("Failed to export metrics: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
```

3. **Metric Collection Integration**
```rust
impl MapReduceExecutor {
    pub fn with_metrics(mut self, registry: Arc<MapReduceMetricsRegistry>) -> Self {
        self.metrics = Some(registry);
        self
    }
    
    async fn record_agent_start(&self, job_id: &str) {
        if let Some(metrics) = &self.metrics {
            metrics.agents_total
                .with_label_values(&[job_id, "started"])
                .inc();
            metrics.agents_active.inc();
            metrics.agents_queued.dec();
        }
    }
    
    async fn record_agent_complete(&self, job_id: &str, result: &AgentResult) {
        if let Some(metrics) = &self.metrics {
            let status = match result.status {
                AgentStatus::Success => "success",
                AgentStatus::Failed(_) => "failed",
                _ => "other",
            };
            
            metrics.agents_total
                .with_label_values(&[job_id, status])
                .inc();
            
            metrics.agents_duration_seconds
                .with_label_values(&[job_id, status])
                .observe(result.duration.as_secs_f64());
            
            metrics.agents_active.dec();
            
            if !result.commits.is_empty() {
                metrics.commits_created_total
                    .with_label_values(&[job_id])
                    .inc_by(result.commits.len() as f64);
            }
        }
    }
}
```

4. **Grafana Dashboard JSON**
```json
{
  "dashboard": {
    "title": "Prodigy MapReduce Metrics",
    "panels": [
      {
        "title": "Active Jobs",
        "targets": [
          {
            "expr": "sum(prodigy_mapreduce_jobs_active)"
          }
        ]
      },
      {
        "title": "Agent Throughput",
        "targets": [
          {
            "expr": "rate(prodigy_mapreduce_agents_total[5m])"
          }
        ]
      },
      {
        "title": "Error Rate",
        "targets": [
          {
            "expr": "rate(prodigy_mapreduce_errors_total[5m])"
          }
        ]
      },
      {
        "title": "P95 Agent Duration",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(prodigy_mapreduce_agent_duration_seconds_bucket[5m]))"
          }
        ]
      }
    ]
  }
}
```

### Architecture Changes
- Add metrics registry to MapReduceExecutor
- Start metrics HTTP server
- Instrument all operations
- Add metric collection points

### Data Structures
```rust
pub struct MetricLabel {
    pub key: String,
    pub value: String,
}

pub struct MetricConfig {
    pub enabled: bool,
    pub port: u16,
    pub path: String,
    pub histogram_buckets: Vec<f64>,
    pub cardinality_limit: usize,
}
```

### APIs and Interfaces
```rust
pub trait MetricsCollector {
    fn record_counter(&self, name: &str, value: f64, labels: &[(&str, &str)]);
    fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);
    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}
```

## Dependencies

- **Prerequisites**: [55 - OpenTelemetry Integration]
- **Affected Components**: 
  - MapReduceExecutor
  - Agent execution paths
  - Job lifecycle management
- **External Dependencies**: 
  - `prometheus` crate
  - `axum` for HTTP server

## Testing Strategy

- **Unit Tests**: 
  - Test metric registration
  - Verify metric updates
  - Test label combinations
  - Validate export format
  
- **Integration Tests**: 
  - Test metrics endpoint
  - Verify metric values
  - Test high cardinality
  - Validate Prometheus scraping
  
- **Performance Tests**: 
  - Measure metric overhead
  - Test with 10k+ series
  - Benchmark export time
  
- **User Acceptance**: 
  - Prometheus scraping works
  - Grafana dashboards functional
  - Alerts trigger correctly

## Documentation Requirements

- **Code Documentation**: 
  - Document each metric
  - Explain label schemes
  - Document bucket choices
  
- **User Documentation**: 
  - Prometheus setup guide
  - Grafana dashboard import
  - Alert rule examples
  
- **Architecture Updates**: 
  - Metrics architecture diagram
  - Data flow documentation

## Implementation Notes

- Use atomic operations for counters
- Implement cardinality limits
- Add metric descriptions
- Use standard metric naming
- Implement graceful shutdown
- Consider metric aggregation
- Add custom business metrics

## Migration and Compatibility

- Metrics are opt-in initially
- No impact on existing functionality
- Gradual metric addition
- Backward compatible configuration
- Standard Prometheus format used