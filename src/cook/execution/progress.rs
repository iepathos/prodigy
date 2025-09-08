//! Enhanced progress tracking for MapReduce jobs
//!
//! Provides real-time progress monitoring, web dashboard, and performance metrics
//! for parallel job execution.

use crate::cook::execution::errors::MapReduceResult;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, Json, Response, Sse},
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use futures_util::stream::Stream;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Enhanced progress tracker with detailed metrics
#[derive(Clone)]
pub struct EnhancedProgressTracker {
    pub job_id: String,
    pub total_items: usize,
    pub start_time: Instant,
    pub agents: Arc<RwLock<HashMap<String, AgentProgress>>>,
    pub metrics: Arc<RwLock<ProgressMetrics>>,
    pub event_sender: mpsc::UnboundedSender<ProgressUpdate>,
    pub event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ProgressUpdate>>>,
    pub web_server: Option<Arc<ProgressWebServer>>,
}

/// Progress information for a single agent
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// State of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
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

/// Aggregate progress metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMetrics {
    pub completed_items: usize,
    pub failed_items: usize,
    pub pending_items: usize,
    pub active_agents: usize,
    pub throughput_current: f64, // items/sec
    pub throughput_average: f64,
    pub success_rate: f64,
    pub average_duration_ms: u64,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub memory_usage_mb: usize,
    pub cpu_usage_percent: f32,
}

impl Default for ProgressMetrics {
    fn default() -> Self {
        Self {
            completed_items: 0,
            failed_items: 0,
            pending_items: 0,
            active_agents: 0,
            throughput_current: 0.0,
            throughput_average: 0.0,
            success_rate: 100.0,
            average_duration_ms: 0,
            estimated_completion: None,
            memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
        }
    }
}

/// Progress update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub update_type: UpdateType,
    pub timestamp: DateTime<Utc>,
    pub data: Value,
}

/// Type of progress update
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateType {
    AgentProgress,
    MetricsUpdate,
    JobCompleted,
    Error,
}

/// Web server for progress dashboard
pub struct ProgressWebServer {
    port: u16,
    tracker: Arc<EnhancedProgressTracker>,
    connections: Arc<RwLock<HashMap<Uuid, mpsc::UnboundedSender<String>>>>,
}

/// Progress snapshot for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    pub timestamp: DateTime<Utc>,
    pub job_id: String,
    pub metrics: ProgressMetrics,
    pub agent_states: HashMap<String, AgentState>,
}

/// Progress history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressHistory {
    pub snapshots: Vec<ProgressSnapshot>,
    pub interval_seconds: u32,
}

/// Export format for progress data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
    Html,
}

/// Progress persistence manager
pub struct ProgressPersistence {
    base_path: PathBuf,
    job_id: String,
    save_interval: Duration,
}

impl ProgressPersistence {
    /// Create new persistence manager
    pub fn new(job_id: String) -> Self {
        let base_path = PathBuf::from(".prodigy/progress");
        Self {
            base_path,
            job_id,
            save_interval: Duration::from_secs(5),
        }
    }

    /// Get the path for progress snapshots
    fn snapshot_path(&self) -> PathBuf {
        self.base_path.join(format!("{}.json", self.job_id))
    }

    /// Get the path for progress history
    fn history_path(&self) -> PathBuf {
        self.base_path.join(format!("{}_history.json", self.job_id))
    }

    /// Save progress snapshot to disk
    pub async fn save_snapshot(&self, snapshot: &ProgressSnapshot) -> MapReduceResult<()> {
        // Ensure directory exists
        fs::create_dir_all(&self.base_path).await?;

        let path = self.snapshot_path();
        let json = serde_json::to_string_pretty(snapshot)?;
        fs::write(&path, json).await?;

        info!("Saved progress snapshot to {:?}", path);
        Ok(())
    }

    /// Load progress snapshot from disk
    pub async fn load_snapshot(&self) -> MapReduceResult<Option<ProgressSnapshot>> {
        let path = self.snapshot_path();
        if !path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&path).await?;
        let snapshot: ProgressSnapshot = serde_json::from_str(&json)?;
        info!("Loaded progress snapshot from {:?}", path);
        Ok(Some(snapshot))
    }

    /// Append to progress history
    pub async fn append_to_history(&self, snapshot: &ProgressSnapshot) -> MapReduceResult<()> {
        fs::create_dir_all(&self.base_path).await?;

        let path = self.history_path();
        let mut history = if path.exists() {
            let json = fs::read_to_string(&path).await?;
            serde_json::from_str::<ProgressHistory>(&json).unwrap_or_else(|_| ProgressHistory {
                snapshots: Vec::new(),
                interval_seconds: self.save_interval.as_secs() as u32,
            })
        } else {
            ProgressHistory {
                snapshots: Vec::new(),
                interval_seconds: self.save_interval.as_secs() as u32,
            }
        };

        // Limit history size to last 1000 snapshots
        if history.snapshots.len() >= 1000 {
            history.snapshots.remove(0);
        }
        history.snapshots.push(snapshot.clone());

        let json = serde_json::to_string_pretty(&history)?;
        fs::write(&path, json).await?;

        Ok(())
    }

    /// Clean up persistence files
    pub async fn cleanup(&self) -> MapReduceResult<()> {
        let snapshot_path = self.snapshot_path();
        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path).await?;
        }

        let history_path = self.history_path();
        if history_path.exists() {
            fs::remove_file(&history_path).await?;
        }

        info!("Cleaned up progress persistence files for job {}", self.job_id);
        Ok(())
    }
}

/// Progress sampler for performance optimization
pub struct ProgressSampler {
    sample_rate: Duration,
    cache: Arc<RwLock<ProgressCache>>,
}

#[derive(Clone, Debug)]
struct ProgressCache {
    last_update: Instant,
    snapshot: Option<ProgressSnapshot>,
    metrics: ProgressMetrics,
}

impl ProgressSampler {
    /// Create new progress sampler
    pub fn new(sample_rate: Duration) -> Self {
        Self {
            sample_rate,
            cache: Arc::new(RwLock::new(ProgressCache {
                last_update: Instant::now(),
                snapshot: None,
                metrics: ProgressMetrics::default(),
            })),
        }
    }

    /// Check if we should sample progress
    pub async fn should_sample(&self) -> bool {
        let cache = self.cache.read().await;
        cache.last_update.elapsed() >= self.sample_rate
    }

    /// Update cache with new snapshot
    pub async fn update_cache(&self, snapshot: ProgressSnapshot, metrics: ProgressMetrics) {
        let mut cache = self.cache.write().await;
        cache.last_update = Instant::now();
        cache.snapshot = Some(snapshot);
        cache.metrics = metrics;
    }

    /// Get cached progress
    pub async fn get_cached(&self) -> Option<(ProgressSnapshot, ProgressMetrics)> {
        let cache = self.cache.read().await;
        cache.snapshot.as_ref().map(|s| (s.clone(), cache.metrics.clone()))
    }
}

impl EnhancedProgressTracker {
    /// Create a new enhanced progress tracker
    pub fn new(job_id: String, total_items: usize) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            job_id,
            total_items,
            start_time: Instant::now(),
            agents: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(ProgressMetrics {
                pending_items: total_items,
                ..Default::default()
            })),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            web_server: None,
        }
    }

    /// Start persistence background task
    pub async fn start_persistence(&self) -> MapReduceResult<()> {
        let persistence = ProgressPersistence::new(self.job_id.clone());
        let tracker = self.clone();

        tokio::spawn(async move {
            let mut interval = interval(persistence.save_interval);
            loop {
                interval.tick().await;
                let snapshot = tracker.create_snapshot().await;
                if let Err(e) = persistence.save_snapshot(&snapshot).await {
                    warn!("Failed to save progress snapshot: {}", e);
                }
                if let Err(e) = persistence.append_to_history(&snapshot).await {
                    warn!("Failed to append to progress history: {}", e);
                }
            }
        });

        info!("Started progress persistence for job {}", self.job_id);
        Ok(())
    }

    /// Restore from persisted snapshot
    pub async fn restore_from_disk(&mut self) -> MapReduceResult<bool> {
        let persistence = ProgressPersistence::new(self.job_id.clone());
        
        if let Some(snapshot) = persistence.load_snapshot().await? {
            // Restore metrics
            let mut metrics = self.metrics.write().await;
            *metrics = snapshot.metrics;

            // Restore agent states
            let mut agents = self.agents.write().await;
            for (agent_id, state) in snapshot.agent_states {
                agents.insert(agent_id.clone(), AgentProgress {
                    agent_id: agent_id.clone(),
                    item_id: String::new(),
                    state,
                    current_step: String::new(),
                    steps_completed: 0,
                    total_steps: 0,
                    progress_percentage: 0.0,
                    started_at: snapshot.timestamp,
                    last_update: snapshot.timestamp,
                    estimated_completion: None,
                    error_count: 0,
                    retry_count: 0,
                });
            }

            info!("Restored progress from disk for job {}", self.job_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Start web dashboard server
    pub async fn start_web_server(&mut self, port: u16) -> MapReduceResult<()> {
        let server = Arc::new(ProgressWebServer {
            port,
            tracker: Arc::new(self.clone()),
            connections: Arc::new(RwLock::new(HashMap::new())),
        });

        self.web_server = Some(server.clone());

        // Spawn server task
        let server_clone = server.clone();
        tokio::spawn(async move {
            if let Err(e) = server_clone.start().await {
                error!("Failed to start progress web server: {}", e);
            }
        });

        info!("Progress dashboard available at http://localhost:{}", port);
        Ok(())
    }

    /// Update agent progress
    pub async fn update_agent_progress(
        &self,
        agent_id: &str,
        progress: AgentProgress,
    ) -> MapReduceResult<()> {
        let mut agents = self.agents.write().await;
        agents.insert(agent_id.to_string(), progress.clone());

        // Send update event
        let update = ProgressUpdate {
            update_type: UpdateType::AgentProgress,
            timestamp: Utc::now(),
            data: json!({
                "agent_id": agent_id,
                "progress": progress,
            }),
        };

        let _ = self.event_sender.send(update);
        self.recalculate_metrics().await?;

        Ok(())
    }

    /// Update agent state
    pub async fn update_agent_state(
        &self,
        agent_id: &str,
        state: AgentState,
    ) -> MapReduceResult<()> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.state = state;
            agent.last_update = Utc::now();
        }

        self.recalculate_metrics().await?;
        Ok(())
    }

    /// Mark item as completed
    pub async fn mark_item_completed(&self, agent_id: &str) -> MapReduceResult<()> {
        self.update_agent_state(agent_id, AgentState::Completed)
            .await?;

        let mut metrics = self.metrics.write().await;
        metrics.completed_items += 1;
        metrics.pending_items = metrics.pending_items.saturating_sub(1);

        Ok(())
    }

    /// Mark item as failed
    pub async fn mark_item_failed(&self, agent_id: &str, error: String) -> MapReduceResult<()> {
        self.update_agent_state(agent_id, AgentState::Failed { error })
            .await?;

        let mut metrics = self.metrics.write().await;
        metrics.failed_items += 1;
        metrics.pending_items = metrics.pending_items.saturating_sub(1);

        Ok(())
    }

    /// Recalculate aggregate metrics
    async fn recalculate_metrics(&self) -> MapReduceResult<()> {
        let agents = self.agents.read().await;
        let mut metrics = self.metrics.write().await;

        // Count active agents
        metrics.active_agents = agents
            .values()
            .filter(|a| {
                matches!(
                    a.state,
                    AgentState::Running { .. } | AgentState::Initializing
                )
            })
            .count();

        // Calculate throughput
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            metrics.throughput_average = metrics.completed_items as f64 / elapsed;
        }

        // Calculate success rate
        let total_processed = metrics.completed_items + metrics.failed_items;
        if total_processed > 0 {
            metrics.success_rate =
                (metrics.completed_items as f64 / total_processed as f64) * 100.0;
        }

        // Estimate completion time
        if metrics.throughput_average > 0.0 && metrics.pending_items > 0 {
            let remaining_seconds = metrics.pending_items as f64 / metrics.throughput_average;
            metrics.estimated_completion =
                Some(Utc::now() + chrono::Duration::seconds(remaining_seconds as i64));
        }

        // Send metrics update
        let update = ProgressUpdate {
            update_type: UpdateType::MetricsUpdate,
            timestamp: Utc::now(),
            data: serde_json::to_value(&*metrics).unwrap_or(json!({})),
        };

        let _ = self.event_sender.send(update);

        Ok(())
    }

    /// Get overall progress percentage
    pub async fn get_overall_progress(&self) -> f32 {
        let metrics = self.metrics.read().await;
        let processed = metrics.completed_items + metrics.failed_items;

        if self.total_items > 0 {
            (processed as f32 / self.total_items as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Get estimated completion time
    pub async fn get_estimated_completion(&self) -> Option<DateTime<Utc>> {
        let metrics = self.metrics.read().await;
        metrics.estimated_completion
    }

    /// Export progress data
    pub async fn export_progress(&self, format: ExportFormat) -> MapReduceResult<Vec<u8>> {
        let agents = self.agents.read().await;
        let metrics = self.metrics.read().await;

        let snapshot = ProgressSnapshot {
            timestamp: Utc::now(),
            job_id: self.job_id.clone(),
            metrics: metrics.clone(),
            agent_states: agents
                .iter()
                .map(|(id, agent)| (id.clone(), agent.state.clone()))
                .collect(),
        };

        match format {
            ExportFormat::Json => {
                let json = serde_json::to_vec_pretty(&snapshot)?;
                Ok(json)
            }
            ExportFormat::Csv => {
                use std::fmt::Write;
                let mut csv_data = String::new();

                // Write header
                let _ = writeln!(
                    &mut csv_data,
                    "timestamp,job_id,completed_items,failed_items,pending_items,success_rate,throughput_average"
                );

                // Write data
                let _ = writeln!(
                    &mut csv_data,
                    "{},{},{},{},{},{:.2},{:.2}",
                    snapshot.timestamp.to_rfc3339(),
                    snapshot.job_id,
                    metrics.completed_items,
                    metrics.failed_items,
                    metrics.pending_items,
                    metrics.success_rate,
                    metrics.throughput_average,
                );

                Ok(csv_data.into_bytes())
            }
            ExportFormat::Html => {
                let html = format!(
                    r#"<!DOCTYPE html>
<html>
<head>
    <title>Progress Report - {}</title>
    <style>
        body {{ font-family: sans-serif; margin: 20px; }}
        h1 {{ color: #333; }}
        .metrics {{ background: #f5f5f5; padding: 15px; border-radius: 5px; }}
        .metric {{ margin: 10px 0; }}
        .label {{ font-weight: bold; }}
    </style>
</head>
<body>
    <h1>MapReduce Job Progress Report</h1>
    <div class="metrics">
        <div class="metric"><span class="label">Job ID:</span> {}</div>
        <div class="metric"><span class="label">Timestamp:</span> {}</div>
        <div class="metric"><span class="label">Completed:</span> {}/{}</div>
        <div class="metric"><span class="label">Failed:</span> {}</div>
        <div class="metric"><span class="label">Success Rate:</span> {:.1}%</div>
        <div class="metric"><span class="label">Throughput:</span> {:.2} items/sec</div>
    </div>
</body>
</html>"#,
                    snapshot.job_id,
                    snapshot.job_id,
                    snapshot.timestamp.to_rfc3339(),
                    metrics.completed_items,
                    self.total_items,
                    metrics.failed_items,
                    metrics.success_rate,
                    metrics.throughput_average,
                );

                Ok(html.into_bytes())
            }
        }
    }

    /// Create progress snapshot
    pub async fn create_snapshot(&self) -> ProgressSnapshot {
        let agents = self.agents.read().await;
        let metrics = self.metrics.read().await;

        ProgressSnapshot {
            timestamp: Utc::now(),
            job_id: self.job_id.clone(),
            metrics: metrics.clone(),
            agent_states: agents
                .iter()
                .map(|(id, agent)| (id.clone(), agent.state.clone()))
                .collect(),
        }
    }
}

impl ProgressWebServer {
    /// Start the web server
    pub async fn start(self: Arc<Self>) -> MapReduceResult<()> {
        let app = Router::new()
            .route("/", get(Self::dashboard_html))
            .route("/api/progress", get(Self::get_progress))
            .route("/api/agents", get(Self::get_agents))
            .route("/api/metrics", get(Self::get_metrics))
            .route("/ws", get(Self::websocket_handler))
            .route("/sse", get(Self::sse_handler))
            .route("/api/prometheus", get(Self::prometheus_metrics))
            .layer(CorsLayer::permissive())
            .with_state(self.clone());

        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));

        // Start event broadcaster
        let broadcaster_self = self.clone();
        tokio::spawn(async move {
            broadcaster_self.broadcast_events().await;
        });

        info!("Starting progress web server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    /// Broadcast events to all connected WebSocket clients
    async fn broadcast_events(self: Arc<Self>) {
        let mut receiver = self.tracker.event_receiver.lock().await;

        while let Some(update) = receiver.recv().await {
            let connections = self.connections.read().await;
            let message = serde_json::to_string(&update).unwrap_or_default();

            for (_, sender) in connections.iter() {
                let _ = sender.send(message.clone());
            }
        }
    }

    /// Serve dashboard HTML
    async fn dashboard_html() -> Html<&'static str> {
        Html(include_str!("progress_dashboard.html"))
    }

    /// Get current progress
    async fn get_progress(State(server): State<Arc<ProgressWebServer>>) -> Json<Value> {
        let progress = server.tracker.get_overall_progress().await;
        let metrics = server.tracker.metrics.read().await;

        Json(json!({
            "job_id": server.tracker.job_id,
            "progress": progress,
            "total_items": server.tracker.total_items,
            "metrics": *metrics,
        }))
    }

    /// Get agent states
    async fn get_agents(State(server): State<Arc<ProgressWebServer>>) -> Json<Value> {
        let agents = server.tracker.agents.read().await;
        Json(json!({
            "agents": agents.clone(),
        }))
    }

    /// Get metrics
    async fn get_metrics(State(server): State<Arc<ProgressWebServer>>) -> Json<ProgressMetrics> {
        let metrics = server.tracker.metrics.read().await;
        Json(metrics.clone())
    }

    /// Handle WebSocket connections
    async fn websocket_handler(
        ws: WebSocketUpgrade,
        State(server): State<Arc<ProgressWebServer>>,
    ) -> Response {
        ws.on_upgrade(move |socket| Self::handle_socket(socket, server))
    }

    /// Handle Server-Sent Events as fallback
    async fn sse_handler(
        State(server): State<Arc<ProgressWebServer>>,
    ) -> Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let client_id = Uuid::new_v4();

        // Register SSE connection
        {
            let mut connections = server.connections.write().await;
            connections.insert(client_id, tx);
        }

        // Create stream from receiver
        let stream = UnboundedReceiverStream::new(rx).map(|msg| {
            Ok(axum::response::sse::Event::default().data(msg))
        });

        // Clean up on disconnect
        let server_clone = server.clone();
        tokio::spawn(async move {
            // This will run when the SSE connection closes
            let mut connections = server_clone.connections.write().await;
            connections.remove(&client_id);
        });

        Sse::new(stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(30))
                .text("keep-alive"),
        )
    }

    /// Export Prometheus metrics
    async fn prometheus_metrics(State(server): State<Arc<ProgressWebServer>>) -> String {
        let metrics = server.tracker.metrics.read().await;
        let agents = server.tracker.agents.read().await;

        let mut output = String::new();

        // Job metrics
        output.push_str(&format!("# HELP mapreduce_items_completed Number of completed items\n"));
        output.push_str(&format!("# TYPE mapreduce_items_completed counter\n"));
        output.push_str(&format!("mapreduce_items_completed{{job_id=\"{}\"}} {}\n", 
            server.tracker.job_id, metrics.completed_items));

        output.push_str(&format!("# HELP mapreduce_items_failed Number of failed items\n"));
        output.push_str(&format!("# TYPE mapreduce_items_failed counter\n"));
        output.push_str(&format!("mapreduce_items_failed{{job_id=\"{}\"}} {}\n", 
            server.tracker.job_id, metrics.failed_items));

        output.push_str(&format!("# HELP mapreduce_items_pending Number of pending items\n"));
        output.push_str(&format!("# TYPE mapreduce_items_pending gauge\n"));
        output.push_str(&format!("mapreduce_items_pending{{job_id=\"{}\"}} {}\n", 
            server.tracker.job_id, metrics.pending_items));

        output.push_str(&format!("# HELP mapreduce_active_agents Number of active agents\n"));
        output.push_str(&format!("# TYPE mapreduce_active_agents gauge\n"));
        output.push_str(&format!("mapreduce_active_agents{{job_id=\"{}\"}} {}\n", 
            server.tracker.job_id, metrics.active_agents));

        output.push_str(&format!("# HELP mapreduce_throughput_average Average throughput in items/sec\n"));
        output.push_str(&format!("# TYPE mapreduce_throughput_average gauge\n"));
        output.push_str(&format!("mapreduce_throughput_average{{job_id=\"{}\"}} {}\n", 
            server.tracker.job_id, metrics.throughput_average));

        output.push_str(&format!("# HELP mapreduce_success_rate Success rate percentage\n"));
        output.push_str(&format!("# TYPE mapreduce_success_rate gauge\n"));
        output.push_str(&format!("mapreduce_success_rate{{job_id=\"{}\"}} {}\n", 
            server.tracker.job_id, metrics.success_rate));

        // Agent states
        let mut state_counts: HashMap<String, usize> = HashMap::new();
        for agent in agents.values() {
            let state_name = match &agent.state {
                AgentState::Queued => "queued",
                AgentState::Initializing => "initializing",
                AgentState::Running { .. } => "running",
                AgentState::Merging => "merging",
                AgentState::Completed => "completed",
                AgentState::Failed { .. } => "failed",
                AgentState::Retrying { .. } => "retrying",
                AgentState::DeadLettered => "dead_lettered",
            };
            *state_counts.entry(state_name.to_string()).or_insert(0) += 1;
        }

        output.push_str(&format!("# HELP mapreduce_agent_states Count of agents by state\n"));
        output.push_str(&format!("# TYPE mapreduce_agent_states gauge\n"));
        for (state, count) in state_counts {
            output.push_str(&format!("mapreduce_agent_states{{job_id=\"{}\",state=\"{}\"}} {}\n", 
                server.tracker.job_id, state, count));
        }

        // Job duration
        let duration = server.tracker.start_time.elapsed().as_secs();
        output.push_str(&format!("# HELP mapreduce_job_duration_seconds Job duration in seconds\n"));
        output.push_str(&format!("# TYPE mapreduce_job_duration_seconds gauge\n"));
        output.push_str(&format!("mapreduce_job_duration_seconds{{job_id=\"{}\"}} {}\n", 
            server.tracker.job_id, duration));

        output
    }

    /// Handle individual WebSocket connection
    async fn handle_socket(socket: WebSocket, server: Arc<ProgressWebServer>) {
        use futures_util::{SinkExt, StreamExt};

        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let client_id = Uuid::new_v4();

        // Register connection
        {
            let mut connections = server.connections.write().await;
            connections.insert(client_id, tx);
        }

        // Spawn sender task
        let mut send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sender.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        });

        // Spawn receiver task
        let mut recv_task = tokio::spawn(async move {
            while let Some(Ok(_msg)) = receiver.next().await {
                // Handle incoming messages if needed
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = &mut send_task => recv_task.abort(),
            _ = &mut recv_task => send_task.abort(),
        }

        // Unregister connection
        let mut connections = server.connections.write().await;
        connections.remove(&client_id);
    }
}

/// CLI progress viewer
pub struct CLIProgressViewer {
    tracker: Arc<EnhancedProgressTracker>,
    update_interval: Duration,
    sampler: Option<ProgressSampler>,
}

impl CLIProgressViewer {
    /// Create a new CLI progress viewer
    pub fn new(tracker: Arc<EnhancedProgressTracker>) -> Self {
        Self {
            tracker,
            update_interval: Duration::from_millis(500),
            sampler: None,
        }
    }

    /// Create with performance optimization for large jobs
    pub fn with_sampling(tracker: Arc<EnhancedProgressTracker>, sample_rate: Duration) -> Self {
        Self {
            tracker,
            update_interval: Duration::from_millis(500),
            sampler: Some(ProgressSampler::new(sample_rate)),
        }
    }

    /// Display progress in the terminal
    pub async fn display(&self) -> MapReduceResult<()> {
        let mut interval = interval(self.update_interval);

        loop {
            interval.tick().await;

            // Use sampler if available for performance
            let should_render = if let Some(ref sampler) = self.sampler {
                if sampler.should_sample().await {
                    let snapshot = self.tracker.create_snapshot().await;
                    let metrics = self.tracker.metrics.read().await;
                    sampler.update_cache(snapshot, metrics.clone()).await;
                    true
                } else {
                    // Use cached data for display
                    if let Some((_, metrics)) = sampler.get_cached().await {
                        self.clear_screen();
                        self.render_header_with_metrics(&metrics).await?;
                        self.render_cached_agents().await?;
                    }
                    false
                }
            } else {
                true
            };

            if should_render {
                self.clear_screen();
                self.render_header().await?;
                self.render_metrics().await?;
                self.render_agents().await?;
            }

            // Check if job is complete
            let metrics = self.tracker.metrics.read().await;
            if metrics.pending_items == 0 && metrics.active_agents == 0 {
                println!("\n✅ Job completed!");
                break;
            }
        }

        Ok(())
    }

    /// Render header with cached metrics
    async fn render_header_with_metrics(&self, metrics: &ProgressMetrics) -> MapReduceResult<()> {
        let total = metrics.completed_items + metrics.failed_items + metrics.pending_items;
        let progress = if total > 0 {
            ((metrics.completed_items + metrics.failed_items) as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        let elapsed = self.tracker.start_time.elapsed();

        println!("╔════════════════════════════════════════════════════════════════╗");
        println!("║  MapReduce Job: {}  ║", self.tracker.job_id);
        println!("║  Progress: {:.1}% | Elapsed: {:?}  ║", progress, elapsed);
        println!("╚════════════════════════════════════════════════════════════════╝");

        Ok(())
    }

    /// Render cached agents
    async fn render_cached_agents(&self) -> MapReduceResult<()> {
        if let Some(ref sampler) = self.sampler {
            if let Some((snapshot, _)) = sampler.get_cached().await {
                println!("\n👥 Agent Status (cached):");
                println!("{}", "─".repeat(60));
                
                for (id, state) in snapshot.agent_states.iter().take(10) {
                    let state_str = match state {
                        AgentState::Running { step, .. } => format!("🔄 {}", step),
                        AgentState::Completed => "✅ Completed".to_string(),
                        AgentState::Failed { error } => format!("❌ Failed: {}", error),
                        _ => format!("{:?}", state),
                    };
                    
                    println!("  {}: {}", &id[..8.min(id.len())], state_str);
                }
                
                if snapshot.agent_states.len() > 10 {
                    println!("  ... and {} more agents", snapshot.agent_states.len() - 10);
                }
            }
        }
        Ok(())
    }

    /// Clear terminal screen
    fn clear_screen(&self) {
        print!("\x1B[2J\x1B[1;1H");
    }

    /// Render header
    async fn render_header(&self) -> MapReduceResult<()> {
        let progress = self.tracker.get_overall_progress().await;
        let elapsed = self.tracker.start_time.elapsed();

        println!("╔════════════════════════════════════════════════════════════════╗");
        println!("║  MapReduce Job: {}  ║", self.tracker.job_id);
        println!("║  Progress: {:.1}% | Elapsed: {:?}  ║", progress, elapsed);
        println!("╚════════════════════════════════════════════════════════════════╝");

        Ok(())
    }

    /// Render metrics
    async fn render_metrics(&self) -> MapReduceResult<()> {
        let metrics = self.tracker.metrics.read().await;

        println!("\n📊 Metrics:");
        println!("{}", "─".repeat(60));
        println!(
            "  Completed: {} | Failed: {} | Pending: {}",
            metrics.completed_items, metrics.failed_items, metrics.pending_items
        );
        println!(
            "  Active Agents: {} | Success Rate: {:.1}%",
            metrics.active_agents, metrics.success_rate
        );
        println!(
            "  Throughput: {:.2} items/sec (avg)",
            metrics.throughput_average
        );

        if let Some(etc) = metrics.estimated_completion {
            let remaining = etc.signed_duration_since(Utc::now());
            println!(
                "  ETC: {} ({} remaining)",
                etc.format("%H:%M:%S"),
                format_duration(remaining.to_std().unwrap_or_default())
            );
        }

        Ok(())
    }

    /// Render agent states
    async fn render_agents(&self) -> MapReduceResult<()> {
        let agents = self.tracker.agents.read().await;

        println!("\n👥 Agent Status:");
        println!("{}", "─".repeat(60));

        for (id, progress) in agents.iter().take(10) {
            let bar = self.create_progress_bar(progress.progress_percentage);
            let state_str = match &progress.state {
                AgentState::Running { step, .. } => format!("🔄 {}", step),
                AgentState::Completed => "✅ Completed".to_string(),
                AgentState::Failed { error } => format!("❌ Failed: {}", error),
                _ => format!("{:?}", progress.state),
            };

            println!(
                "  {}: {} [{}] {:.1}%",
                &id[..8],
                state_str,
                bar,
                progress.progress_percentage
            );
        }

        if agents.len() > 10 {
            println!("  ... and {} more agents", agents.len() - 10);
        }

        Ok(())
    }

    /// Create ASCII progress bar
    pub fn create_progress_bar(&self, percentage: f32) -> String {
        let width = 20;
        let filled = ((percentage / 100.0) * width as f32) as usize;
        let empty = width - filled;

        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    }
}

/// Format duration for display
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Progress reporter trait for extensibility
#[async_trait::async_trait]
pub trait ProgressReporter: Send + Sync {
    async fn update_agent_progress(
        &self,
        agent_id: &str,
        progress: AgentProgress,
    ) -> MapReduceResult<()>;
    async fn get_overall_progress(&self) -> MapReduceResult<f32>;
    async fn get_estimated_completion(&self) -> MapReduceResult<Option<DateTime<Utc>>>;
    async fn export_progress(&self, format: ExportFormat) -> MapReduceResult<Vec<u8>>;
}

#[async_trait::async_trait]
impl ProgressReporter for EnhancedProgressTracker {
    async fn update_agent_progress(
        &self,
        agent_id: &str,
        progress: AgentProgress,
    ) -> MapReduceResult<()> {
        self.update_agent_progress(agent_id, progress).await
    }

    async fn get_overall_progress(&self) -> MapReduceResult<f32> {
        Ok(self.get_overall_progress().await)
    }

    async fn get_estimated_completion(&self) -> MapReduceResult<Option<DateTime<Utc>>> {
        Ok(self.get_estimated_completion().await)
    }

    async fn export_progress(&self, format: ExportFormat) -> MapReduceResult<Vec<u8>> {
        self.export_progress(format).await
    }
}
