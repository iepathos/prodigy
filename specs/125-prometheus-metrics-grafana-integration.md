---
number: 125
title: Prometheus Metrics and Grafana Integration
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-10-11
---

# Specification 125: Prometheus Metrics and Grafana Integration

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently has comprehensive event tracking for workflows, MapReduce jobs, and Claude sessions through JSONL event logs stored in `~/.prodigy/events/`. While these events provide detailed forensic analysis capabilities, they lack:

❌ **Real-time observability**: Events are append-only logs, not queryable metrics
❌ **Dashboard visualization**: No built-in way to visualize workflow performance trends
❌ **Time-series analytics**: Difficult to analyze success rates, durations, and patterns over time
❌ **Alerting infrastructure**: No proactive alerts for failures, DLQ buildup, or performance degradation
❌ **Multi-instance aggregation**: Cannot aggregate metrics across multiple Prodigy instances

Users want to answer questions like:
- "What is my workflow success rate over the last 7 days?"
- "Which workflows are slowest and consuming the most Claude tokens?"
- "How many items are currently in the DLQ across all jobs?"
- "What percentage of steps require on_failure recovery?"
- "What is the P95 latency for MapReduce agents?"

The existing analytics module (`src/analytics/`) focuses on Claude session analysis but doesn't provide workflow-level metrics suitable for dashboards.

### Current State

**Event Infrastructure** (`src/cook/execution/events/`):
- Rich event types: `MapReduceEvent`, workflow events
- JSONL storage in `~/.prodigy/events/{repo_name}/{job_id}/`
- Event streaming for real-time consumption
- No aggregation or metrics export

**Analytics Module** (`src/analytics/`):
- SQLite persistence for Claude sessions
- Token tracking and tool invocation analysis
- Session replay capabilities
- Not integrated with workflow metrics

**Storage Abstraction** (`src/storage/types.rs`):
- Well-defined types for sessions, events, workflows
- No metrics aggregation layer
- No time-series query support

### Industry Standard: Prometheus + Grafana

The Prometheus metrics format and Grafana dashboards are the industry standard for observability:
- **Prometheus**: Time-series metrics with flexible labels and efficient storage
- **Grafana**: Rich visualization with built-in support for Prometheus datasources
- **Push vs Pull**: Prometheus scrapes `/metrics` endpoints (standard practice)
- **Minimal overhead**: In-memory aggregation with configurable retention

## Objective

Implement Prometheus metrics collection and HTTP export endpoint in Prodigy to enable Grafana dashboard visualization of workflow performance, MapReduce execution, and Claude usage metrics. Provide optional PostgreSQL storage for detailed event queries alongside time-series metrics.

**Key Goals**:
1. **Expose `/metrics` endpoint** in Prometheus text format with workflow, step, agent, and Claude metrics
2. **Integrate with event system** to automatically update metrics as events are emitted
3. **Provide Docker Compose setup** for local Prometheus + Grafana stack with pre-configured dashboards
4. **Support optional PostgreSQL** for detailed event storage and complex queries
5. **Enable multi-instance deployments** where each Prodigy instance exposes metrics independently

## Requirements

### Functional Requirements

#### 1. Prometheus Metrics Collection

**Workflow-Level Metrics**:
- `prodigy_workflow_executions_total{workflow_name, status}` (counter) - Total executions by status (success, failed, cancelled)
- `prodigy_workflow_duration_seconds{workflow_name}` (histogram) - Execution duration with quantiles
- `prodigy_workflow_steps_total{workflow_name}` (counter) - Total steps executed per workflow
- `prodigy_workflow_active{workflow_name}` (gauge) - Currently running workflows

**Step-Level Metrics**:
- `prodigy_step_executions_total{step_type, status}` (counter) - Steps by type (claude, shell, goal_seek) and outcome
- `prodigy_step_duration_seconds{step_type}` (histogram) - Step execution latency
- `prodigy_step_on_failure_recoveries_total{step_type}` (counter) - Steps that triggered on_failure
- `prodigy_step_validations_total{step_type, result}` (counter) - Validation pass/fail counts

**MapReduce Metrics**:
- `prodigy_mapreduce_jobs_active{job_id}` (gauge) - Active MapReduce jobs
- `prodigy_mapreduce_agents_active{job_id}` (gauge) - Currently running agents per job
- `prodigy_mapreduce_items_total{job_id, status}` (counter) - Items processed (success, failed, dlq)
- `prodigy_mapreduce_dlq_items{job_id}` (gauge) - Current DLQ size per job
- `prodigy_agent_duration_seconds{job_id}` (histogram) - Agent execution time distribution
- `prodigy_mapreduce_worktrees_active{job_id}` (gauge) - Active worktrees per job

**Claude Usage Metrics**:
- `prodigy_claude_tokens_total{token_type}` (counter) - Token usage by type (input, output, cache)
- `prodigy_claude_tool_invocations_total{tool_name}` (counter) - Tool usage by name
- `prodigy_claude_sessions_total{status}` (counter) - Claude session outcomes
- `prodigy_claude_errors_total{error_type}` (counter) - Claude error classification

**System Metrics**:
- `prodigy_worktree_pool_size` (gauge) - Available worktrees in pool
- `prodigy_storage_events_written_total` (counter) - Events persisted to storage
- `prodigy_checkpoints_created_total` (counter) - Checkpoint operations

#### 2. HTTP Metrics Server

**Endpoint Specification**:
- `GET /metrics` - Prometheus text format metrics export
- `GET /health` - Health check endpoint (returns 200 OK)
- `GET /metrics/json` - JSON format for programmatic access (optional)

**Server Configuration**:
- Configurable port (default: 9090)
- Enable/disable via CLI flag: `--metrics-port <PORT>` or `--no-metrics`
- Environment variable: `PRODIGY_METRICS_PORT`
- Bind to `0.0.0.0` for container deployments

**Performance Requirements**:
- `/metrics` endpoint response time: <50ms
- Minimal memory overhead: <10MB for metrics storage
- Non-blocking: Metrics server runs in background tokio task

#### 3. Event System Integration

**Hook into Event Emitters**:
- Extend `src/cook/execution/events/streaming.rs` to update Prometheus metrics
- Subscribe to `MapReduceEvent` variants and update corresponding counters/gauges
- Track workflow start/completion events for duration histograms
- Record step-level events for type-specific metrics

**Event → Metric Mapping**:
```rust
MapReduceEvent::AgentCompleted { duration, .. } => {
    AGENT_DURATION.observe(duration.num_seconds());
    MAPREDUCE_ITEMS.with_label_values(&[job_id, "success"]).inc();
    MAPREDUCE_AGENTS_ACTIVE.with_label_values(&[job_id]).dec();
}

MapReduceEvent::DLQItemAdded { job_id, .. } => {
    MAPREDUCE_DLQ_SIZE.with_label_values(&[job_id]).inc();
}

MapReduceEvent::ClaudeTokenUsage { input, output, cache, .. } => {
    CLAUDE_TOKENS.with_label_values(&["input"]).inc_by(input);
    CLAUDE_TOKENS.with_label_values(&["output"]).inc_by(output);
    CLAUDE_TOKENS.with_label_values(&["cache"]).inc_by(cache);
}
```

**Metric Updates**:
- Counters increment on event occurrence
- Gauges set to current state value
- Histograms observe event durations
- Labels derived from event fields (job_id, step_type, etc.)

#### 4. Optional PostgreSQL Event Store

**Schema Design**:
```sql
CREATE TABLE workflow_executions (
    id UUID PRIMARY KEY,
    workflow_name TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL,  -- success, failed, cancelled
    duration_ms BIGINT,
    total_steps INT,
    failed_steps INT,
    metadata JSONB
);

CREATE TABLE step_executions (
    id UUID PRIMARY KEY,
    workflow_execution_id UUID REFERENCES workflow_executions(id),
    step_index INT NOT NULL,
    step_type TEXT NOT NULL,
    command TEXT,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL,
    duration_ms BIGINT,
    had_on_failure_recovery BOOLEAN,
    error_message TEXT
);

CREATE TABLE mapreduce_jobs (
    job_id TEXT PRIMARY KEY,
    workflow_execution_id UUID REFERENCES workflow_executions(id),
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    total_items INT,
    successful_items INT,
    failed_items INT,
    dlq_items INT
);

CREATE INDEX idx_workflow_executions_started ON workflow_executions(started_at);
CREATE INDEX idx_workflow_executions_name_started ON workflow_executions(workflow_name, started_at);
CREATE INDEX idx_step_executions_workflow ON step_executions(workflow_execution_id);
CREATE INDEX idx_mapreduce_jobs_started ON mapreduce_jobs(started_at);
```

**Async Event Recording**:
- Non-blocking writes to PostgreSQL via tokio channel
- Background task drains event queue and batches inserts
- Graceful degradation if PostgreSQL unavailable (log warning, continue)
- Configurable batch size and flush interval

**Feature Flag**:
- Cargo feature: `grafana-postgres` (optional)
- Enable via CLI: `--enable-postgres-storage`
- Configuration: `~/.prodigy/config.toml` with connection string

#### 5. Grafana Dashboard Provisioning

**Provided Dashboards** (JSON files in `grafana/dashboards/`):

1. **Workflow Overview Dashboard**:
   - Success rate pie chart
   - Workflows over time (line graph)
   - Average duration gauge
   - Top 5 slowest workflows table
   - Failure rate alert panel

2. **MapReduce Performance Dashboard**:
   - Active agents gauge
   - Items processed rate (stacked area)
   - DLQ size trend line
   - Agent duration heatmap
   - Throughput metrics (items/second)

3. **Step-Level Analysis Dashboard**:
   - Steps by type (stacked bar)
   - Recovery rate percentage
   - Validation pass rate
   - Step duration box plot
   - Failure hotspots

4. **Claude Usage & Cost Dashboard**:
   - Token usage over time (stacked area: input/output/cache)
   - Estimated cost calculation panel
   - Top tool invocations bar chart
   - Tokens per workflow comparison
   - Cost optimization recommendations

**Docker Compose Setup** (`docker-compose.yml`):
```yaml
version: '3.8'
services:
  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    ports:
      - "9091:9090"
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.retention.time=30d'

  grafana:
    image: grafana/grafana:latest
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin
      - GF_INSTALL_PLUGINS=grafana-piechart-panel
    volumes:
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana/datasources:/etc/grafana/provisioning/datasources
      - grafana_data:/var/lib/grafana
    ports:
      - "3000:3000"
    depends_on:
      - prometheus

  postgres:  # Optional
    image: postgres:16
    environment:
      - POSTGRES_DB=prodigy_metrics
      - POSTGRES_USER=prodigy
      - POSTGRES_PASSWORD=changeme
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

volumes:
  prometheus_data:
  grafana_data:
  postgres_data:
```

**Prometheus Scrape Config** (`prometheus.yml`):
```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'prodigy'
    static_configs:
      - targets: ['host.docker.internal:9090']
    metrics_path: /metrics
```

### Non-Functional Requirements

1. **Performance**:
   - Metrics collection overhead: <5% CPU, <10MB memory
   - `/metrics` endpoint latency: <50ms P99
   - No blocking of workflow execution
   - Efficient label cardinality (avoid unbounded label values)

2. **Scalability**:
   - Support 1000+ workflow executions per day
   - Handle 10+ concurrent MapReduce jobs
   - Metrics retention: 30 days default (Prometheus configurable)
   - PostgreSQL: Handle millions of historical events

3. **Reliability**:
   - Metrics server failure does not crash Prodigy
   - Graceful degradation if Prometheus unavailable
   - PostgreSQL failures logged but non-fatal
   - Automatic recovery on transient errors

4. **Usability**:
   - Zero-configuration for basic setup (`prodigy run workflow.yml --metrics-port 9090`)
   - One-command Grafana stack: `docker-compose up -d`
   - Dashboards auto-loaded on first access
   - Clear documentation with screenshots

5. **Maintainability**:
   - Standard Prometheus client library (no custom formats)
   - Well-documented metric names and labels
   - Testable metric collection logic
   - Dashboard JSON checked into version control

## Acceptance Criteria

- [ ] Prometheus metrics registry initialized in `src/metrics/mod.rs`
- [ ] All workflow-level metrics defined and collected
- [ ] All step-level metrics defined and collected
- [ ] All MapReduce metrics defined and collected
- [ ] All Claude usage metrics defined and collected
- [ ] HTTP metrics server implemented with `/metrics` and `/health` endpoints
- [ ] Metrics server configurable via CLI flag and environment variable
- [ ] Event system integration updates metrics on `MapReduceEvent` emission
- [ ] Workflow orchestrator updates metrics on workflow start/complete
- [ ] Step executor updates metrics on step execution
- [ ] Optional PostgreSQL feature flag implemented
- [ ] PostgreSQL schema created with proper indexes
- [ ] Async event recording to PostgreSQL via background task
- [ ] Docker Compose file provided with Prometheus, Grafana, optional PostgreSQL
- [ ] Prometheus scrape configuration provided
- [ ] Grafana datasource provisioning configured
- [ ] Four dashboard JSON files created and tested:
  - [ ] Workflow Overview Dashboard
  - [ ] MapReduce Performance Dashboard
  - [ ] Step-Level Analysis Dashboard
  - [ ] Claude Usage & Cost Dashboard
- [ ] Dashboards display correct data from Prometheus
- [ ] PostgreSQL datasource queries working (if enabled)
- [ ] Documentation: "Getting Started with Metrics and Grafana" guide
- [ ] Documentation: Metric descriptions and label meanings
- [ ] Documentation: Dashboard customization guide
- [ ] Tests: Unit tests for metric collection logic
- [ ] Tests: Integration test for metrics server endpoint
- [ ] Tests: Verify metrics updated on event emission
- [ ] Example workflow runs with metrics visible in Grafana
- [ ] Performance: Metrics overhead <5% CPU, <10MB memory
- [ ] Performance: `/metrics` endpoint responds <50ms
- [ ] CLI help text documents `--metrics-port` flag
- [ ] Config file supports metrics server settings

## Technical Details

### Implementation Approach

#### Phase 1: Prometheus Foundation (Days 1-3)

**1.1 Add Dependencies** (`Cargo.toml`):
```toml
[dependencies]
prometheus = "0.13"
lazy_static = "1.4"

[features]
default = ["metrics"]
metrics = ["prometheus"]
grafana-postgres = ["sqlx", "sqlx/postgres"]
```

**1.2 Create Metrics Module** (`src/metrics/mod.rs`):
```rust
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec,
    CounterVec, GaugeVec, HistogramVec, Registry, TextEncoder, Encoder
};
use lazy_static::lazy_static;

lazy_static! {
    static ref WORKFLOW_EXECUTIONS: CounterVec = register_counter_vec!(
        "prodigy_workflow_executions_total",
        "Total workflow executions by name and status",
        &["workflow_name", "status"]
    ).unwrap();

    static ref WORKFLOW_DURATION: HistogramVec = register_histogram_vec!(
        "prodigy_workflow_duration_seconds",
        "Workflow execution duration in seconds",
        &["workflow_name"],
        vec![1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0]
    ).unwrap();

    static ref STEP_EXECUTIONS: CounterVec = register_counter_vec!(
        "prodigy_step_executions_total",
        "Total step executions by type and status",
        &["step_type", "status"]
    ).unwrap();

    static ref MAPREDUCE_AGENTS_ACTIVE: GaugeVec = register_gauge_vec!(
        "prodigy_mapreduce_agents_active",
        "Currently active MapReduce agents",
        &["job_id"]
    ).unwrap();

    static ref CLAUDE_TOKENS: CounterVec = register_counter_vec!(
        "prodigy_claude_tokens_total",
        "Claude token usage by type",
        &["token_type"]
    ).unwrap();

    // ... additional metrics
}

pub struct PrometheusMetrics;

impl PrometheusMetrics {
    pub fn record_workflow_start(name: &str) {
        // Track in-progress workflows
    }

    pub fn record_workflow_complete(name: &str, status: &str, duration_secs: f64) {
        WORKFLOW_EXECUTIONS.with_label_values(&[name, status]).inc();
        WORKFLOW_DURATION.with_label_values(&[name]).observe(duration_secs);
    }

    pub fn record_step_execution(step_type: &str, status: &str, duration_secs: f64) {
        STEP_EXECUTIONS.with_label_values(&[step_type, status]).inc();
    }

    pub fn export_metrics() -> Result<String, anyhow::Error> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}
```

**1.3 Create HTTP Server** (`src/metrics/server.rs`):
```rust
use axum::{routing::get, Router, response::IntoResponse};
use std::net::SocketAddr;

pub async fn start_metrics_server(port: u16) -> Result<(), anyhow::Error> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting metrics server on {}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        app
    ).await?;

    Ok(())
}

async fn metrics_handler() -> impl IntoResponse {
    match PrometheusMetrics::export_metrics() {
        Ok(metrics) => (
            axum::http::StatusCode::OK,
            [("Content-Type", "text/plain; version=0.0.4")],
            metrics
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [("Content-Type", "text/plain; version=0.0.4")],
            format!("Error exporting metrics: {}", e)
        ),
    }
}

async fn health_handler() -> impl IntoResponse {
    axum::http::StatusCode::OK
}
```

**1.4 CLI Integration** (`src/cli/args.rs`):
```rust
#[derive(Parser)]
pub struct RunArgs {
    // ... existing args

    /// Enable metrics server on specified port
    #[arg(long, env = "PRODIGY_METRICS_PORT")]
    pub metrics_port: Option<u16>,

    /// Disable metrics server
    #[arg(long)]
    pub no_metrics: bool,
}
```

#### Phase 2: Event Integration (Days 4-5)

**2.1 Extend Event Streaming** (`src/cook/execution/events/streaming.rs`):
```rust
impl EventStream {
    pub fn emit_event(&mut self, event: MapReduceEvent) {
        // Existing event logging
        self.log_event(&event);

        // NEW: Update Prometheus metrics
        self.update_metrics(&event);
    }

    fn update_metrics(&self, event: &MapReduceEvent) {
        use crate::metrics::PrometheusMetrics;

        match event {
            MapReduceEvent::AgentStarted { job_id, .. } => {
                MAPREDUCE_AGENTS_ACTIVE.with_label_values(&[job_id]).inc();
            }
            MapReduceEvent::AgentCompleted { job_id, duration, .. } => {
                MAPREDUCE_AGENTS_ACTIVE.with_label_values(&[job_id]).dec();
                MAPREDUCE_ITEMS.with_label_values(&[job_id, "success"]).inc();
                AGENT_DURATION.with_label_values(&[job_id])
                    .observe(duration.num_seconds() as f64);
            }
            MapReduceEvent::AgentFailed { job_id, .. } => {
                MAPREDUCE_AGENTS_ACTIVE.with_label_values(&[job_id]).dec();
                MAPREDUCE_ITEMS.with_label_values(&[job_id, "failed"]).inc();
            }
            MapReduceEvent::DLQItemAdded { job_id, .. } => {
                MAPREDUCE_DLQ_SIZE.with_label_values(&[job_id]).inc();
            }
            MapReduceEvent::ClaudeTokenUsage { input_tokens, output_tokens, cache_tokens, .. } => {
                CLAUDE_TOKENS.with_label_values(&["input"]).inc_by(*input_tokens);
                CLAUDE_TOKENS.with_label_values(&["output"]).inc_by(*output_tokens);
                CLAUDE_TOKENS.with_label_values(&["cache"]).inc_by(*cache_tokens);
            }
            _ => {}
        }
    }
}
```

**2.2 Workflow Orchestrator Integration** (`src/cook/orchestrator/core.rs`):
```rust
impl Orchestrator {
    pub async fn run(&mut self) -> Result<()> {
        let workflow_name = &self.workflow.name;
        let start_time = Instant::now();

        PrometheusMetrics::record_workflow_start(workflow_name);

        let result = self.execute_workflow().await;

        let duration_secs = start_time.elapsed().as_secs_f64();
        let status = if result.is_ok() { "success" } else { "failed" };
        PrometheusMetrics::record_workflow_complete(workflow_name, status, duration_secs);

        result
    }
}
```

#### Phase 3: Optional PostgreSQL (Days 6-7)

**3.1 Schema Migration** (`migrations/001_workflow_metrics.sql`):
```sql
-- See schema in Requirements section above
```

**3.2 Async Event Recorder** (`src/metrics/postgres.rs`):
```rust
#[cfg(feature = "grafana-postgres")]
pub struct PostgresEventRecorder {
    tx: tokio::sync::mpsc::UnboundedSender<MapReduceEvent>,
    _task: tokio::task::JoinHandle<()>,
}

#[cfg(feature = "grafana-postgres")]
impl PostgresEventRecorder {
    pub fn new(pool: sqlx::PgPool) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let task = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(e) = Self::record_event(&pool, &event).await {
                    tracing::warn!("Failed to record event to PostgreSQL: {}", e);
                }
            }
        });

        Self { tx, _task: task }
    }

    pub fn record(&self, event: MapReduceEvent) {
        let _ = self.tx.send(event);
    }

    async fn record_event(pool: &sqlx::PgPool, event: &MapReduceEvent) -> Result<()> {
        // Insert into appropriate table based on event type
        match event {
            MapReduceEvent::JobStarted { job_id, config, .. } => {
                sqlx::query(
                    "INSERT INTO mapreduce_jobs (job_id, started_at, total_items)
                     VALUES ($1, $2, $3)"
                )
                .bind(job_id)
                .bind(Utc::now())
                .bind(config.total_items)
                .execute(pool)
                .await?;
            }
            // ... handle other event types
            _ => {}
        }
        Ok(())
    }
}
```

#### Phase 4: Grafana Provisioning (Days 8-9)

**4.1 Dashboard JSON Files** (`grafana/dashboards/workflow-overview.json`):
```json
{
  "dashboard": {
    "title": "Prodigy Workflow Overview",
    "panels": [
      {
        "title": "Workflow Success Rate",
        "type": "piechart",
        "targets": [
          {
            "expr": "sum by (status) (prodigy_workflow_executions_total)"
          }
        ]
      },
      {
        "title": "Workflows Over Time",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(prodigy_workflow_executions_total[5m])",
            "legendFormat": "{{workflow_name}} - {{status}}"
          }
        ]
      }
    ]
  }
}
```

**4.2 Datasource Provisioning** (`grafana/datasources/prometheus.yml`):
```yaml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true

  - name: PostgreSQL
    type: postgres
    url: postgres:5432
    database: prodigy_metrics
    user: prodigy
    secureJsonData:
      password: changeme
```

#### Phase 5: Documentation (Days 10-11)

**5.1 User Guide** (`docs/metrics-and-grafana.md`):
```markdown
# Metrics and Grafana Integration

## Quick Start

### 1. Start Prodigy with Metrics

prodigy run workflow.yml --metrics-port 9090


### 2. Start Prometheus and Grafana

docker-compose up -d


### 3. Access Dashboards

Open http://localhost:3000 in your browser.

## Available Dashboards

### Workflow Overview
Shows success rates, execution trends, and performance metrics.

### MapReduce Performance
Real-time view of agent execution, DLQ status, and throughput.

### Claude Usage & Cost
Token consumption, estimated costs, and tool usage patterns.

## Customization

### Adding Custom Metrics

Edit `src/metrics/mod.rs` to add new metrics...

### Dashboard Modifications

Dashboards are in `grafana/dashboards/`. Edit JSON and restart Grafana.
```

### Architecture Changes

**New Modules**:
- `src/metrics/mod.rs` - Prometheus metrics registry
- `src/metrics/server.rs` - HTTP metrics server
- `src/metrics/postgres.rs` - Optional PostgreSQL recorder (feature-gated)

**Modified Modules**:
- `src/cook/execution/events/streaming.rs` - Add metric updates
- `src/cook/orchestrator/core.rs` - Track workflow metrics
- `src/cook/workflow/executor.rs` - Track step metrics
- `src/cli/args.rs` - Add metrics CLI flags

**New Files**:
- `docker-compose.yml` - Prometheus + Grafana stack
- `prometheus.yml` - Prometheus scrape configuration
- `grafana/dashboards/*.json` - Pre-built dashboards
- `grafana/datasources/prometheus.yml` - Datasource config
- `migrations/001_workflow_metrics.sql` - PostgreSQL schema (optional)
- `docs/metrics-and-grafana.md` - User documentation

### Data Structures

**Metric Labels** (cardinality considerations):
- `workflow_name`: Low cardinality (10-100 unique workflows)
- `step_type`: Very low cardinality (4-5 types: claude, shell, goal_seek, foreach)
- `status`: Very low cardinality (3-4 values: success, failed, cancelled)
- `job_id`: **High cardinality** - use with caution, may need job_id prefix instead
- `token_type`: Very low cardinality (3 values: input, output, cache)

**Label Best Practices**:
- Avoid unbounded labels (e.g., commit SHAs, full file paths)
- Use aggregatable labels (status, type) over unique identifiers
- Consider job_id prefix (first 8 chars) instead of full UUID
- Monitor Prometheus cardinality metrics

**Histogram Buckets**:
- Workflow duration: `[1, 5, 10, 30, 60, 300, 600, 1800]` seconds
- Step duration: `[0.1, 1, 5, 10, 30, 60]` seconds
- Agent duration: `[1, 10, 30, 60, 300, 600]` seconds

### APIs and Interfaces

**Metrics Server Endpoints**:
```
GET /metrics
  Returns: Prometheus text format metrics
  Content-Type: text/plain; version=0.0.4
  Response Time: <50ms

GET /health
  Returns: 200 OK
  Content-Type: text/plain
  Response Time: <10ms
```

**CLI Interface**:
```bash
# Enable metrics on default port (9090)
prodigy run workflow.yml --metrics-port 9090

# Disable metrics
prodigy run workflow.yml --no-metrics

# Configure via environment
export PRODIGY_METRICS_PORT=8080
prodigy run workflow.yml
```

**Configuration File** (`~/.prodigy/config.toml`):
```toml
[metrics]
enabled = true
port = 9090
bind_address = "0.0.0.0"

[metrics.postgres]
enabled = false
connection_string = "postgres://user:pass@localhost/prodigy_metrics"
batch_size = 100
flush_interval_ms = 5000
```

## Dependencies

**Prerequisites**: None (pure additive feature)

**Affected Components**:
- Event streaming system (adds metric updates)
- Workflow orchestrator (tracks execution metrics)
- CLI argument parsing (adds metrics flags)

**External Dependencies**:
- `prometheus` crate (0.13) - Rust Prometheus client
- `lazy_static` (1.4) - For static metric registry
- `axum` (existing) - Reused for HTTP server
- `sqlx` (optional, feature-gated) - PostgreSQL access
- Docker + Docker Compose (for Grafana stack, user-provided)

**Cargo Features**:
- `metrics` (default) - Enable Prometheus metrics
- `grafana-postgres` (optional) - Enable PostgreSQL event storage

## Testing Strategy

### Unit Tests

**Test 1: Metric Collection** (`src/metrics/mod.rs`):
```rust
#[test]
fn test_workflow_metrics() {
    let metrics = PrometheusMetrics;
    metrics.record_workflow_complete("test-workflow", "success", 10.5);

    let exported = metrics.export_metrics().unwrap();
    assert!(exported.contains("prodigy_workflow_executions_total"));
    assert!(exported.contains("test-workflow"));
}
```

**Test 2: Event Integration** (`src/cook/execution/events/streaming_test.rs`):
```rust
#[tokio::test]
async fn test_event_updates_metrics() {
    let mut stream = EventStream::new();

    let event = MapReduceEvent::AgentCompleted {
        job_id: "test-job".to_string(),
        agent_id: "agent-1".to_string(),
        duration: Duration::seconds(30),
        commits: vec![],
        json_log_location: None,
    };

    stream.emit_event(event);

    // Verify metric updated
    let metrics = PrometheusMetrics::export_metrics().unwrap();
    assert!(metrics.contains("prodigy_mapreduce_items_total"));
}
```

**Test 3: Label Cardinality** (`src/metrics/cardinality_test.rs`):
```rust
#[test]
fn test_label_cardinality_bounded() {
    // Simulate 1000 workflow executions
    for i in 0..1000 {
        let workflow_name = format!("workflow-{}", i % 10); // Only 10 unique names
        PrometheusMetrics::record_workflow_complete(&workflow_name, "success", 1.0);
    }

    let metrics = PrometheusMetrics::export_metrics().unwrap();
    let unique_workflows = // Parse metrics and count unique workflow_name labels
    assert!(unique_workflows <= 10, "Label cardinality should be bounded");
}
```

### Integration Tests

**Test 1: Metrics Server** (`tests/metrics_server_test.rs`):
```rust
#[tokio::test]
async fn test_metrics_endpoint() {
    let server_handle = tokio::spawn(start_metrics_server(9999));
    tokio::time::sleep(Duration::from_millis(100)).await;

    let response = reqwest::get("http://localhost:9999/metrics").await.unwrap();
    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();
    assert!(body.contains("# HELP prodigy_workflow_executions_total"));
}
```

**Test 2: End-to-End Workflow** (`tests/workflow_metrics_test.rs`):
```rust
#[tokio::test]
async fn test_workflow_emits_metrics() {
    // Start metrics server
    tokio::spawn(start_metrics_server(9998));

    // Run simple workflow
    let workflow = load_test_workflow("simple.yml");
    run_workflow(workflow).await.unwrap();

    // Check metrics
    let metrics = reqwest::get("http://localhost:9998/metrics")
        .await.unwrap()
        .text().await.unwrap();

    assert!(metrics.contains("prodigy_workflow_executions_total"));
    assert!(metrics.contains("prodigy_step_executions_total"));
}
```

**Test 3: PostgreSQL Recording** (`tests/postgres_metrics_test.rs`):
```rust
#[cfg(feature = "grafana-postgres")]
#[tokio::test]
async fn test_postgres_event_recording() {
    let pool = setup_test_database().await;
    let recorder = PostgresEventRecorder::new(pool.clone());

    let event = MapReduceEvent::JobStarted { /* ... */ };
    recorder.record(event);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mapreduce_jobs")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}
```

### Performance Tests

**Test 1: Metrics Overhead** (`benches/metrics_overhead.rs`):
```rust
fn bench_metric_recording(c: &mut Criterion) {
    c.bench_function("record_workflow_metric", |b| {
        b.iter(|| {
            PrometheusMetrics::record_workflow_complete(
                "bench-workflow",
                "success",
                1.0
            );
        });
    });
}
```

**Test 2: Export Performance** (`benches/metrics_export.rs`):
```rust
fn bench_metrics_export(c: &mut Criterion) {
    // Record 1000 metrics
    for i in 0..1000 {
        PrometheusMetrics::record_workflow_complete("test", "success", 1.0);
    }

    c.bench_function("export_metrics", |b| {
        b.iter(|| {
            PrometheusMetrics::export_metrics().unwrap();
        });
    });
}
```

### User Acceptance

1. **Dashboard Usability**:
   - User can start Grafana with `docker-compose up -d`
   - Dashboards appear in Grafana UI automatically
   - Metrics populate within 30 seconds of workflow start
   - Charts update in real-time during execution

2. **Monitoring Workflow**:
   - User runs: `prodigy run workflow.yml --metrics-port 9090`
   - Opens Grafana and sees workflow appear in dashboard
   - Watches progress bars and success rates update
   - Reviews DLQ items and recovery rates

3. **Alerting Setup**:
   - User configures Grafana alert for DLQ size > 10
   - Triggers alert by causing failures
   - Receives notification (email/Slack)
   - Verifies alert clears when DLQ drained

## Documentation Requirements

### Code Documentation

**Module Documentation** (`src/metrics/mod.rs`):
```rust
//! Prometheus metrics collection for Prodigy workflows.
//!
//! This module provides metrics collection for workflow execution,
//! MapReduce jobs, and Claude usage, exposed via HTTP in Prometheus format.
//!
//! # Metrics Categories
//!
//! - **Workflow Metrics**: Execution counts, durations, success rates
//! - **Step Metrics**: Step-level execution and recovery statistics
//! - **MapReduce Metrics**: Agent activity, DLQ size, throughput
//! - **Claude Metrics**: Token usage, tool invocations, costs
//!
//! # Usage
//!
//! Start Prodigy with metrics enabled:
//! ```bash
//! prodigy run workflow.yml --metrics-port 9090
//! ```
//!
//! Access metrics:
//! ```bash
//! curl http://localhost:9090/metrics
//! ```
//!
//! # Label Cardinality
//!
//! To avoid cardinality explosion, labels are carefully chosen:
//! - Use bounded values (status: success/failed/cancelled)
//! - Avoid unique identifiers (use job_id prefix, not full UUID)
//! - Monitor Prometheus cardinality with queries
```

### User Documentation

**Getting Started Guide** (`docs/metrics-and-grafana.md`):
- Overview of metrics and observability
- Quick start: Running Prodigy with metrics + Docker Compose
- Dashboard walkthrough with screenshots
- Interpreting metrics and graphs
- Customizing dashboards
- Alerting setup
- Troubleshooting common issues

**Metric Reference** (`docs/metrics-reference.md`):
- Complete list of all metrics
- Label descriptions and cardinality
- Example PromQL queries
- Best practices for querying
- Performance considerations

**PostgreSQL Integration Guide** (`docs/postgres-metrics.md`):
- Enabling PostgreSQL feature
- Schema setup and migrations
- Connecting Grafana to PostgreSQL
- Example queries for detailed analysis
- Backup and retention strategies

### Architecture Updates

No `ARCHITECTURE.md` updates needed - metrics are instrumentation, not core architecture.

## Implementation Notes

### Metric Naming Conventions

Follow Prometheus best practices:
- Use base unit (seconds, not milliseconds)
- Suffix with unit: `_seconds`, `_bytes`, `_total`
- Use descriptive names: `prodigy_workflow_executions_total` not `prodigy_wf_exec`
- Avoid redundancy: `prodigy_*` prefix (not `prodigy_prodigy_*`)

### Label Cardinality Management

**High Cardinality Labels to Avoid**:
- Full UUIDs (use first 8 chars)
- File paths (use directory or filename only)
- Commit SHAs (use short SHA)
- Timestamps (use Prometheus time-series, not labels)

**Monitor Cardinality**:
```promql
# Check cardinality per metric
count by (__name__) ({__name__=~"prodigy_.*"})

# Check label value distribution
count by (workflow_name) (prodigy_workflow_executions_total)
```

### Histogram Bucket Selection

Choose buckets based on expected latency distribution:
- **Workflow duration**: Most workflows <1 minute, some up to 30 minutes
- **Step duration**: Most steps <10 seconds, Claude steps up to 60 seconds
- **Agent duration**: Variable, 10 seconds to 10 minutes

Adjust buckets based on actual P50/P95/P99 latencies observed in production.

### PostgreSQL Async Recording

**Batching Strategy**:
- Buffer up to 100 events or 5 seconds (whichever first)
- Batch INSERT for efficiency
- Retry transient errors with exponential backoff
- Drop events if buffer exceeds 10,000 (log warning)

**Graceful Degradation**:
- PostgreSQL failure should not crash Prodigy
- Log errors but continue workflow execution
- Metrics server continues serving Prometheus data

### Dashboard JSON Maintenance

**Version Control**:
- Dashboard JSON files checked into Git
- Update dashboards via Grafana UI, then export JSON
- Test dashboard changes before committing
- Document custom panel queries

**Dashboard Variables**:
- Use Grafana variables for filtering (e.g., `$workflow_name`)
- Provide sensible defaults
- Document variable usage in dashboard description

## Migration and Compatibility

### Breaking Changes

**None** - This is a purely additive feature.

### Compatibility Considerations

- Metrics server is optional (disabled by default or via `--no-metrics`)
- No changes to existing event system (only additions)
- PostgreSQL is feature-gated and optional
- Dashboards work with Prometheus data only (PostgreSQL optional)

### Migration Path

**For Existing Users**:

1. **Update Prodigy**: Upgrade to version with metrics support
2. **Enable Metrics**: Add `--metrics-port 9090` to workflow runs
3. **Start Monitoring Stack**: Run `docker-compose up -d` in repo root
4. **Access Grafana**: Open http://localhost:3000
5. **Review Dashboards**: Explore pre-built dashboards

**For New Users**:
- Metrics are opt-in via CLI flag
- Documentation includes complete setup guide
- Example workflows demonstrate metric usage

**Disabling Metrics**:
```bash
# Disable via CLI
prodigy run workflow.yml --no-metrics

# Disable via config
# ~/.prodigy/config.toml
[metrics]
enabled = false
```

### Rollback Plan

If metrics cause issues:
1. Disable via `--no-metrics` flag
2. Metrics server does not start, no overhead
3. Workflow execution unaffected
4. Events still logged to JSONL as before

## Success Metrics

- [ ] Metrics server starts successfully in <1 second
- [ ] `/metrics` endpoint responds in <50ms P99
- [ ] Metrics overhead: <5% CPU, <10MB memory
- [ ] Dashboards display data within 30 seconds of workflow start
- [ ] PostgreSQL recording (if enabled) does not block workflow execution
- [ ] No increase in workflow execution time with metrics enabled
- [ ] Zero crashes or errors related to metrics collection
- [ ] User feedback: "Grafana integration is useful and easy to set up"
- [ ] Documentation completeness: Users can set up without assistance
- [ ] Docker Compose stack starts on first try

## Future Enhancements

### Out of Scope for This Spec

- **Distributed Tracing**: OpenTelemetry integration for request tracing
- **Log Aggregation**: Centralized logging with Loki or Elasticsearch
- **Custom Exporters**: StatsD, Datadog, New Relic exporters
- **Real-Time Alerts in CLI**: Terminal notifications for failures
- **Cost Forecasting**: Predict future Claude costs based on trends
- **Anomaly Detection**: ML-based anomaly detection on metrics
- **Multi-Tenant Metrics**: Separate metrics per project/team

These features may be added in future specifications as observability matures.

## References

- [Prometheus Best Practices](https://prometheus.io/docs/practices/naming/)
- [Grafana Provisioning](https://grafana.com/docs/grafana/latest/administration/provisioning/)
- [Prometheus Rust Client](https://docs.rs/prometheus/latest/prometheus/)
- [PromQL Query Examples](https://prometheus.io/docs/prometheus/latest/querying/examples/)
