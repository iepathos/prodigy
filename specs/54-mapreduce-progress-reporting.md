---
number: 54
title: MapReduce Enhanced Progress Reporting
category: parallel
priority: high
status: draft
dependencies: [51]
created: 2025-01-29
---

# Specification 54: MapReduce Enhanced Progress Reporting

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [51 - Event Logging]

## Context

The current progress reporting in MapReduce is limited to basic progress bars that show agent status. Users lack detailed visibility into job execution, including per-agent progress, estimated completion times, throughput metrics, and detailed status of individual work items. This makes it difficult to monitor long-running jobs and identify performance bottlenecks.

## Objective

Implement comprehensive progress reporting for MapReduce jobs that provides real-time visibility into execution status, performance metrics, completion estimates, and detailed agent-level progress tracking.

## Requirements

### Functional Requirements
- Real-time progress updates for all agents
- Estimated time to completion (ETC)
- Throughput and performance metrics
- Detailed agent status breakdown
- Progress persistence across restarts
- Web-based progress dashboard
- CLI progress viewer
- Progress event streaming
- Historical progress replay

### Non-Functional Requirements
- Progress updates with < 100ms latency
- Support monitoring 1000+ concurrent agents
- Progress UI updates at 10 FPS minimum
- Dashboard accessible via HTTP
- Minimal performance overhead (< 1%)

## Acceptance Criteria

- [ ] Enhanced ProgressTracker with detailed metrics
- [ ] Web dashboard accessible at http://localhost:8080
- [ ] Real-time WebSocket progress streaming
- [ ] ETC calculation with rolling average
- [ ] Throughput metrics (items/sec, commits/sec)
- [ ] Agent state visualization
- [ ] Progress persistence to disk
- [ ] CLI command `mmm progress <job-id>`
- [ ] Progress history viewable after completion
- [ ] Export progress data to JSON/CSV

## Technical Details

### Implementation Approach

1. **Enhanced Progress Tracker**
```rust
pub struct EnhancedProgressTracker {
    pub job_id: String,
    pub total_items: usize,
    pub start_time: Instant,
    pub agents: Arc<RwLock<HashMap<String, AgentProgress>>>,
    pub metrics: Arc<RwLock<ProgressMetrics>>,
    pub event_stream: Arc<Mutex<EventStream>>,
    pub web_server: Option<ProgressWebServer>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentProgress {
    pub agent_id: String,
    pub item_id: String,
    pub state: AgentState,
    pub current_step: String,
    pub steps_completed: usize,
    pub total_steps: usize,
    pub progress_percentage: f32,
    pub started_at: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub error_count: usize,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub enum AgentState {
    Queued,
    Initializing,
    Running { step: String, progress: f32 },
    Merging,
    Completed,
    Failed { error: String },
    Retrying { attempt: u32 },
    DeadLettered,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressMetrics {
    pub completed_items: usize,
    pub failed_items: usize,
    pub pending_items: usize,
    pub active_agents: usize,
    pub throughput_current: f64,  // items/sec
    pub throughput_average: f64,
    pub success_rate: f64,
    pub average_duration_ms: u64,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub memory_usage_mb: usize,
    pub cpu_usage_percent: f32,
}
```

2. **Progress Web Dashboard**
```rust
pub struct ProgressWebServer {
    port: u16,
    tracker: Arc<EnhancedProgressTracker>,
    connections: Arc<RwLock<HashMap<Uuid, WebSocketConnection>>>,
}

impl ProgressWebServer {
    pub async fn start(&self) -> Result<()> {
        let app = Router::new()
            .route("/", get(Self::dashboard_html))
            .route("/api/progress", get(Self::get_progress))
            .route("/api/agents", get(Self::get_agents))
            .route("/api/metrics", get(Self::get_metrics))
            .route("/ws", get(Self::websocket_handler))
            .layer(CorsLayer::permissive());
        
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;
        
        Ok(())
    }
    
    async fn broadcast_update(&self, update: ProgressUpdate) {
        let connections = self.connections.read().await;
        for (_, conn) in connections.iter() {
            let _ = conn.send(Message::Text(
                serde_json::to_string(&update).unwrap()
            )).await;
        }
    }
}
```

3. **Progress Dashboard HTML**
```html
<!DOCTYPE html>
<html>
<head>
    <title>MapReduce Progress Dashboard</title>
    <style>
        .agent-grid {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
            gap: 10px;
        }
        .agent-card {
            border: 1px solid #ddd;
            padding: 10px;
            border-radius: 5px;
        }
        .progress-bar {
            width: 100%;
            height: 20px;
            background: #f0f0f0;
            border-radius: 10px;
            overflow: hidden;
        }
        .progress-fill {
            height: 100%;
            background: linear-gradient(90deg, #4CAF50, #45a049);
            transition: width 0.3s ease;
        }
    </style>
</head>
<body>
    <div id="dashboard">
        <h1>MapReduce Job: <span id="job-id"></span></h1>
        <div id="metrics">
            <div>Total Progress: <span id="total-progress">0%</span></div>
            <div>Throughput: <span id="throughput">0</span> items/sec</div>
            <div>ETC: <span id="etc">Calculating...</span></div>
        </div>
        <div id="agent-grid" class="agent-grid"></div>
    </div>
    <script>
        const ws = new WebSocket('ws://localhost:8080/ws');
        ws.onmessage = (event) => {
            const update = JSON.parse(event.data);
            updateDashboard(update);
        };
    </script>
</body>
</html>
```

4. **CLI Progress Viewer**
```rust
pub struct CLIProgressViewer {
    tracker: Arc<EnhancedProgressTracker>,
    update_interval: Duration,
}

impl CLIProgressViewer {
    pub async fn display(&self) -> Result<()> {
        loop {
            self.clear_screen();
            self.render_header().await?;
            self.render_metrics().await?;
            self.render_agents().await?;
            
            tokio::time::sleep(self.update_interval).await;
        }
    }
    
    async fn render_agents(&self) -> Result<()> {
        let agents = self.tracker.agents.read().await;
        
        println!("\n📊 Agent Status:");
        println!("─".repeat(80));
        
        for (id, progress) in agents.iter().take(20) {
            let bar = self.create_progress_bar(progress.progress_percentage);
            println!("{}: {} [{}] {:.1}%",
                id,
                progress.current_step,
                bar,
                progress.progress_percentage
            );
        }
        
        if agents.len() > 20 {
            println!("... and {} more agents", agents.len() - 20);
        }
        
        Ok(())
    }
}
```

### Architecture Changes
- Add `EnhancedProgressTracker` to MapReduceExecutor
- Integrate web server component
- Add progress persistence layer
- Implement WebSocket broadcasting

### Data Structures
```rust
pub struct ProgressSnapshot {
    pub timestamp: DateTime<Utc>,
    pub job_id: String,
    pub metrics: ProgressMetrics,
    pub agent_states: HashMap<String, AgentState>,
}

pub struct ProgressHistory {
    pub snapshots: Vec<ProgressSnapshot>,
    pub interval_seconds: u32,
}

#[derive(Serialize)]
pub struct ProgressUpdate {
    pub update_type: UpdateType,
    pub timestamp: DateTime<Utc>,
    pub data: Value,
}

pub enum UpdateType {
    AgentProgress,
    MetricsUpdate,
    JobCompleted,
    Error,
}
```

### APIs and Interfaces
```rust
pub trait ProgressReporter {
    async fn update_agent_progress(&self, agent_id: &str, progress: AgentProgress) -> Result<()>;
    async fn get_overall_progress(&self) -> Result<f32>;
    async fn get_estimated_completion(&self) -> Result<Option<DateTime<Utc>>>;
    async fn export_progress(&self, format: ExportFormat) -> Result<Vec<u8>>;
}

pub enum ExportFormat {
    Json,
    Csv,
    Html,
}
```

## Dependencies

- **Prerequisites**: [51 - Event Logging]
- **Affected Components**: 
  - `src/cook/execution/mapreduce.rs`
  - Progress tracking system
  - CLI interface
- **External Dependencies**: 
  - `axum` for web server
  - `tokio-tungstenite` for WebSocket

## Testing Strategy

- **Unit Tests**: 
  - Test progress calculations
  - Verify ETC algorithms
  - Test metric aggregation
  - Validate state transitions
  
- **Integration Tests**: 
  - Test web dashboard functionality
  - Verify WebSocket updates
  - Test progress persistence
  - Validate CLI viewer
  
- **Performance Tests**: 
  - Test with 1000+ agents
  - Measure update latency
  - Benchmark web server load
  
- **User Acceptance**: 
  - Monitor job via dashboard
  - View progress in CLI
  - Export progress reports

## Documentation Requirements

- **Code Documentation**: 
  - Document progress calculation
  - Explain ETC algorithm
  - Document WebSocket protocol
  
- **User Documentation**: 
  - Dashboard user guide
  - CLI progress commands
  - Progress export formats
  
- **Architecture Updates**: 
  - Add progress system diagram
  - Document web architecture

## Implementation Notes

- Use Server-Sent Events as WebSocket fallback
- Implement progress sampling for large jobs
- Add progress caching for performance
- Consider using Canvas/WebGL for visualization
- Implement progress alerts/notifications
- Add Prometheus metrics export
- Use rolling window for throughput calculation

## Migration and Compatibility

- Old progress system remains functional
- Gradual migration to enhanced tracking
- Web dashboard optional feature
- Backward compatible CLI output
- Progressive enhancement approach