use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::ws::WebSocket;
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use super::progress_tracker::{ProgressTracker, SerializableProgressSnapshot};

pub struct DashboardServer {
    progress_tracker: Arc<ProgressTracker>,
    port: u16,
    update_channel: broadcast::Sender<String>,
    log_buffer: Arc<RwLock<VecDeque<LogEntry>>>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct LogQuery {
    agent_id: Option<String>,
    level: Option<String>,
    limit: Option<usize>,
}

impl DashboardServer {
    pub fn new(progress_tracker: Arc<ProgressTracker>, port: u16) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            progress_tracker,
            port,
            update_channel: tx,
            log_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let app = Router::new()
            .route("/", get(serve_dashboard))
            .route("/api/progress", get(progress_endpoint))
            .route("/api/logs", get(logs_endpoint))
            .route("/ws", get(websocket_handler))
            .with_state(self.clone());

        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        info!("Dashboard available at http://localhost:{}", self.port);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    pub async fn broadcast_update(&self, update: SerializableProgressSnapshot) -> Result<()> {
        let json = serde_json::to_string(&update)?;
        self.update_channel.send(json).ok();
        Ok(())
    }

    pub async fn add_log(&self, level: &str, message: &str, agent_id: Option<String>) {
        let mut logs = self.log_buffer.write().await;

        // Keep buffer size limited
        if logs.len() >= 1000 {
            logs.pop_front();
        }

        logs.push_back(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: level.to_string(),
            message: message.to_string(),
            agent_id,
        });
    }
}

async fn serve_dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn progress_endpoint(State(server): State<Arc<DashboardServer>>) -> Json<serde_json::Value> {
    let snapshot = server.progress_tracker.serializable_snapshot().await;
    Json(json!(snapshot))
}

async fn logs_endpoint(
    State(server): State<Arc<DashboardServer>>,
    Query(params): Query<LogQuery>,
) -> Json<serde_json::Value> {
    let logs = server.log_buffer.read().await;

    let mut filtered_logs: Vec<LogEntry> = logs
        .iter()
        .filter(|log| {
            // Filter by agent_id if specified
            if let Some(ref agent_id) = params.agent_id {
                if let Some(ref log_agent) = log.agent_id {
                    if log_agent != agent_id {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Filter by level if specified
            if let Some(ref level) = params.level {
                if log.level.to_lowercase() != level.to_lowercase() {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect();

    // Apply limit
    let limit = params.limit.unwrap_or(100);
    filtered_logs.truncate(limit);

    Json(json!({
        "logs": filtered_logs
    }))
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(server): State<Arc<DashboardServer>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, server))
}

async fn handle_socket(socket: WebSocket, server: Arc<DashboardServer>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = server.update_channel.subscribe();

    use axum::extract::ws::Message;

    // Send initial state
    let snapshot = server.progress_tracker.serializable_snapshot().await;
    if let Ok(json) = serde_json::to_string(&snapshot) {
        sender.send(Message::Text(json.into())).await.ok();
    }

    // Spawn task to handle incoming messages
    let server_clone = server.clone();
    tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            if let Ok(Message::Text(text)) = msg {
                // Handle client commands if needed
                if text == "ping" {
                    // Respond with current snapshot
                    let snapshot = server_clone.progress_tracker.serializable_snapshot().await;
                    if let Ok(json) = serde_json::to_string(&snapshot) {
                        // Can't send here, would need channel back to sender
                    }
                }
            }
        }
    });

    // Send updates to client
    while let Ok(update) = rx.recv().await {
        if sender.send(Message::Text(update.into())).await.is_err() {
            break;
        }
    }
}

const DASHBOARD_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Prodigy Progress Dashboard</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: #1a1a2e;
            color: #eee;
            padding: 20px;
        }

        .container {
            max-width: 1400px;
            margin: 0 auto;
        }

        h1 {
            margin-bottom: 20px;
            color: #4fbdba;
        }

        .workflow-card {
            background: #16213e;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 20px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
        }

        .progress-bar {
            width: 100%;
            height: 30px;
            background: #0f3460;
            border-radius: 15px;
            overflow: hidden;
            margin: 10px 0;
        }

        .progress-fill {
            height: 100%;
            background: linear-gradient(90deg, #4fbdba, #7ec8e3);
            transition: width 0.3s ease;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #fff;
            font-weight: bold;
        }

        .stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 15px;
            margin-top: 20px;
        }

        .stat-card {
            background: #0f3460;
            padding: 15px;
            border-radius: 8px;
            text-align: center;
        }

        .stat-value {
            font-size: 2em;
            font-weight: bold;
            color: #4fbdba;
        }

        .stat-label {
            font-size: 0.9em;
            color: #aaa;
            margin-top: 5px;
        }

        .phase-container {
            margin-top: 20px;
        }

        .phase-card {
            background: #0f3460;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 10px;
        }

        .phase-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 10px;
        }

        .agent-list {
            margin-top: 10px;
            padding-left: 20px;
        }

        .agent-item {
            background: #16213e;
            padding: 8px;
            margin: 5px 0;
            border-radius: 4px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        .status-badge {
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 0.85em;
            font-weight: bold;
        }

        .status-running { background: #4fbdba; }
        .status-completed { background: #4caf50; }
        .status-failed { background: #f44336; }
        .status-pending { background: #666; }

        .resource-meter {
            display: flex;
            align-items: center;
            gap: 10px;
            margin: 5px 0;
        }

        .meter-bar {
            flex: 1;
            height: 10px;
            background: #0f3460;
            border-radius: 5px;
            overflow: hidden;
        }

        .meter-fill {
            height: 100%;
            transition: width 0.3s ease;
        }

        .meter-cpu { background: #4fbdba; }
        .meter-memory { background: #7ec8e3; }
        .meter-disk { background: #c7ceea; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 Prodigy Progress Dashboard</h1>

        <div class="workflow-card">
            <h2 id="workflow-name">Loading...</h2>
            <div class="progress-bar">
                <div class="progress-fill" id="main-progress">0%</div>
            </div>
            <div id="workflow-status"></div>

            <div class="stats">
                <div class="stat-card">
                    <div class="stat-value" id="completed-steps">0</div>
                    <div class="stat-label">Completed Steps</div>
                </div>
                <div class="stat-card">
                    <div class="stat-value" id="success-rate">0%</div>
                    <div class="stat-label">Success Rate</div>
                </div>
                <div class="stat-card">
                    <div class="stat-value" id="throughput">0</div>
                    <div class="stat-label">Items/sec</div>
                </div>
                <div class="stat-card">
                    <div class="stat-value" id="eta">--</div>
                    <div class="stat-label">ETA</div>
                </div>
            </div>

            <div class="resource-meter">
                <span>CPU:</span>
                <div class="meter-bar">
                    <div class="meter-fill meter-cpu" id="cpu-meter" style="width: 0%"></div>
                </div>
                <span id="cpu-value">0%</span>
            </div>
            <div class="resource-meter">
                <span>Memory:</span>
                <div class="meter-bar">
                    <div class="meter-fill meter-memory" id="mem-meter" style="width: 0%"></div>
                </div>
                <span id="mem-value">0 MB</span>
            </div>
        </div>

        <div class="phase-container" id="phases">
            <!-- Phases will be inserted here -->
        </div>
    </div>

    <script>
        const ws = new WebSocket('ws://localhost:8080/ws');

        ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            updateDashboard(data);
        };

        ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };

        function updateDashboard(data) {
            // Update workflow info
            if (data.workflow) {
                document.getElementById('workflow-name').textContent = data.workflow.name;

                const progress = data.workflow.total_steps > 0
                    ? Math.round((data.workflow.completed_steps / data.workflow.total_steps) * 100)
                    : 0;

                const progressBar = document.getElementById('main-progress');
                progressBar.style.width = progress + '%';
                progressBar.textContent = progress + '%';

                document.getElementById('completed-steps').textContent = data.workflow.completed_steps;

                if (data.workflow.eta) {
                    const etaSeconds = Math.round(data.workflow.eta.secs);
                    const minutes = Math.floor(etaSeconds / 60);
                    const seconds = etaSeconds % 60;
                    document.getElementById('eta').textContent = minutes + 'm ' + seconds + 's';
                } else {
                    document.getElementById('eta').textContent = '--';
                }

                // Update resource usage
                const cpu = data.workflow.resource_usage.cpu_percent;
                document.getElementById('cpu-meter').style.width = cpu + '%';
                document.getElementById('cpu-value').textContent = cpu.toFixed(1) + '%';

                const memMB = Math.round(data.workflow.resource_usage.memory_bytes / 1048576);
                const memPercent = Math.min(100, memMB / 10); // Assume 1GB = 100%
                document.getElementById('mem-meter').style.width = memPercent + '%';
                document.getElementById('mem-value').textContent = memMB + ' MB';
            }

            // Update phases
            if (data.phases) {
                updatePhases(data.phases);
            }
        }

        function updatePhases(phases) {
            const container = document.getElementById('phases');
            container.innerHTML = '';

            Object.values(phases).forEach(phase => {
                const successRate = phase.processed_items > 0
                    ? Math.round((phase.successful_items / phase.processed_items) * 100)
                    : 0;

                const phaseHtml = `
                    <div class="phase-card">
                        <div class="phase-header">
                            <h3>${phase.name}</h3>
                            <span class="status-badge status-${phase.status.toLowerCase()}">${phase.status}</span>
                        </div>
                        <div class="progress-bar">
                            <div class="progress-fill" style="width: ${(phase.processed_items / phase.total_items * 100)}%">
                                ${phase.processed_items} / ${phase.total_items}
                            </div>
                        </div>
                        <div>Success Rate: ${successRate}% | Throughput: ${phase.throughput.toFixed(2)}/s</div>
                        ${phase.active_agents.length > 0 ? renderAgents(phase.active_agents) : ''}
                    </div>
                `;
                container.innerHTML += phaseHtml;
            });
        }

        function renderAgents(agents) {
            let html = '<div class="agent-list"><h4>Active Agents:</h4>';
            agents.forEach(agent => {
                html += `
                    <div class="agent-item">
                        <span>${agent.id}: ${agent.current_item || 'Idle'}</span>
                        <span class="status-badge status-${agent.status.toLowerCase()}">${agent.status}</span>
                    </div>
                `;
            });
            html += '</div>';
            return html;
        }

        // Fetch initial data
        fetch('/api/progress')
            .then(response => response.json())
            .then(data => updateDashboard(data));
    </script>
</body>
</html>
"#;
