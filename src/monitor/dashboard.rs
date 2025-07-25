use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use super::alert::AlertManager;
use super::analytics::AnalyticsEngine;
use super::collector::MetricsCollector;
use super::metrics::MetricsDatabase;
use super::report::{ExportFormat, Report, ReportGenerator};
use super::Alert;
use super::TimeFrame;
use crate::error::Result;
use crate::project::ProjectManager;
use crate::state::StateManager;

#[derive(Clone)]
pub struct DashboardState {
    pub state_manager: Arc<StateManager>,
    pub project_manager: Arc<ProjectManager>,
    pub metrics_db: Arc<MetricsDatabase>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub alert_manager: Arc<AlertManager>,
    pub analytics_engine: Arc<AnalyticsEngine>,
    pub report_generator: Arc<ReportGenerator>,
    pub events: broadcast::Sender<DashboardEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum DashboardEvent {
    MetricsUpdate {
        timestamp: DateTime<Utc>,
        metrics: HashMap<String, f64>,
    },
    AlertTriggered {
        alert: Alert,
    },
    SpecCompleted {
        project_id: Uuid,
        spec_name: String,
    },
}

pub struct DashboardServer {
    port: u16,
    state: DashboardState,
}

impl DashboardServer {
    pub fn new(
        port: u16,
        state_manager: Arc<StateManager>,
        project_manager: Arc<ProjectManager>,
        metrics_db: Arc<MetricsDatabase>,
        metrics_collector: Arc<MetricsCollector>,
        alert_manager: Arc<AlertManager>,
        analytics_engine: Arc<AnalyticsEngine>,
        report_generator: Arc<ReportGenerator>,
    ) -> Self {
        let (events, _) = broadcast::channel(100);

        let state = DashboardState {
            state_manager,
            project_manager,
            metrics_db,
            metrics_collector,
            alert_manager,
            analytics_engine,
            report_generator,
            events,
        };

        Self { port, state }
    }

    pub async fn start(self) -> Result<()> {
        let app = Router::new()
            // Dashboard UI
            .route("/", get(dashboard_home))
            // API endpoints
            .route("/api/projects", get(list_projects))
            .route("/api/projects/:id/status", get(project_status))
            .route("/api/projects/:id/metrics", get(project_metrics))
            .route("/api/metrics/query", get(query_metrics))
            .route("/api/metrics/live", get(metrics_websocket))
            .route("/api/alerts", get(list_alerts))
            .route("/api/alerts/:id/acknowledge", post(acknowledge_alert))
            // TODO: Fix handler trait implementations
            // .route("/api/analytics/run", post(run_analytics))
            // .route("/api/reports/generate", post(generate_report))
            .route("/api/reports/:id", get(get_report))
            .route("/api/reports/:id/export", get(export_report))
            .route("/api/health", get(health_check))
            .layer(CorsLayer::permissive())
            .with_state(self.state);

        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        log::info!("Dashboard server listening on http://{addr}");

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

// Route handlers

async fn dashboard_home() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn health_check() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
    })
}

async fn list_projects(
    State(state): State<DashboardState>,
) -> std::result::Result<Json<Vec<ProjectSummary>>, StatusCode> {
    let projects = state.project_manager.list_projects();

    let mut summaries = Vec::new();
    for project in projects {
        let _health: Option<bool> = None; // TODO: Implement health checking

        summaries.push(ProjectSummary {
            id: uuid::Uuid::new_v4(), // Generate a UUID since Project doesn't have id
            name: project.name.clone(),
            path: project.path.to_string_lossy().to_string(),
            status: "active".to_string(), // Default status since Project doesn't have status
            health_status: None,          // TODO: Implement health checking
            created_at: project.created,
            last_accessed: Some(project.last_accessed),
        });
    }

    Ok(Json(summaries))
}

async fn project_status(
    Path(id): Path<Uuid>,
    State(state): State<DashboardState>,
) -> std::result::Result<Json<ProjectStatus>, StatusCode> {
    // Get project metrics
    let timeframe = TimeFrame::last_day();
    let mut labels = HashMap::new();
    labels.insert("project_id".to_string(), id.to_string());

    let completion = state
        .metrics_db
        .query_metrics(
            "specs.completion_percentage",
            timeframe.start,
            timeframe.end,
            Some(labels.clone()),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .next()
        .and_then(|m| match m.value {
            super::MetricValue::Gauge(v) => Some(v),
            _ => None,
        })
        .unwrap_or(0.0);

    let total_specs = state
        .metrics_db
        .query_metrics(
            "specs.total",
            timeframe.start,
            timeframe.end,
            Some(labels.clone()),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .next()
        .and_then(|m| match m.value {
            super::MetricValue::Gauge(v) => Some(v as u32),
            _ => None,
        })
        .unwrap_or(0);

    let completed_specs = state
        .metrics_db
        .query_metrics(
            "specs.completed",
            timeframe.start,
            timeframe.end,
            Some(labels),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .next()
        .and_then(|m| match m.value {
            super::MetricValue::Gauge(v) => Some(v as u32),
            _ => None,
        })
        .unwrap_or(0);

    Ok(Json(ProjectStatus {
        project_id: id,
        completion_percentage: completion,
        total_specs,
        completed_specs,
        in_progress_specs: 0, // TODO: Get from state
        recent_activity: vec![],
    }))
}

async fn project_metrics(
    Path(id): Path<Uuid>,
    Query(params): Query<MetricsQuery>,
    State(state): State<DashboardState>,
) -> std::result::Result<Json<MetricsResponse>, StatusCode> {
    let timeframe = params.timeframe();
    let mut labels = HashMap::new();
    labels.insert("project_id".to_string(), id.to_string());

    let metrics = state
        .metrics_db
        .query_metrics(
            &params.metric_name,
            timeframe.start,
            timeframe.end,
            Some(labels),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(MetricsResponse { metrics }))
}

async fn query_metrics(
    Query(params): Query<MetricsQuery>,
    State(state): State<DashboardState>,
) -> std::result::Result<Json<MetricsResponse>, StatusCode> {
    let timeframe = params.timeframe();

    let metrics = state
        .metrics_db
        .query_metrics(
            &params.metric_name,
            timeframe.start,
            timeframe.end,
            params.labels,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(MetricsResponse { metrics }))
}

async fn metrics_websocket(
    State(_state): State<DashboardState>,
) -> std::result::Result<Json<WebSocketInfo>, StatusCode> {
    // In a real implementation, this would upgrade to WebSocket
    // For now, return connection info
    Ok(Json(WebSocketInfo {
        url: format!("ws://localhost:{}/api/metrics/live", 8080),
        protocol: "mmm-metrics-v1".to_string(),
    }))
}

async fn list_alerts(
    Query(params): Query<AlertsQuery>,
    State(state): State<DashboardState>,
) -> std::result::Result<Json<Vec<Alert>>, StatusCode> {
    let alerts = state
        .alert_manager
        .get_alerts(params.since)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(alerts))
}

async fn acknowledge_alert(
    Path(id): Path<Uuid>,
    State(state): State<DashboardState>,
) -> std::result::Result<StatusCode, StatusCode> {
    state
        .alert_manager
        .acknowledge_alert(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn run_analytics(
    Json(params): Json<AnalyticsRequest>,
    State(state): State<DashboardState>,
) -> std::result::Result<Json<AnalyticsResponse>, StatusCode> {
    let timeframe = params.timeframe();

    let analyses = state
        .analytics_engine
        .run_analysis(timeframe)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AnalyticsResponse { analyses }))
}

async fn generate_report(
    Json(params): Json<GenerateReportRequest>,
    State(state): State<DashboardState>,
) -> std::result::Result<Json<GenerateReportResponse>, StatusCode> {
    // For now, use a default template
    let templates = super::report::default_report_templates();
    let template = templates
        .iter()
        .find(|t| t.name == params.template_name)
        .ok_or(StatusCode::NOT_FOUND)?;

    let report = state
        .report_generator
        .generate_from_template(template, params.timeframe())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(GenerateReportResponse {
        report_id: report.id.clone(),
        status: "completed".to_string(),
    }))
}

async fn get_report(
    Path(_id): Path<String>,
    State(_state): State<DashboardState>,
) -> std::result::Result<Json<Report>, StatusCode> {
    // TODO: Store and retrieve reports
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn export_report(
    Path(_id): Path<String>,
    Query(_params): Query<ExportReportQuery>,
    State(_state): State<DashboardState>,
) -> std::result::Result<Vec<u8>, StatusCode> {
    // TODO: Export reports
    Err(StatusCode::NOT_IMPLEMENTED)
}

// Request/Response types

#[derive(Debug, Serialize)]
struct HealthStatus {
    status: String,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ProjectSummary {
    id: Uuid,
    name: String,
    path: String,
    status: String,
    health_status: Option<String>,
    created_at: DateTime<Utc>,
    last_accessed: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct ProjectStatus {
    project_id: Uuid,
    completion_percentage: f64,
    total_specs: u32,
    completed_specs: u32,
    in_progress_specs: u32,
    recent_activity: Vec<Activity>,
}

#[derive(Debug, Serialize)]
struct Activity {
    timestamp: DateTime<Utc>,
    description: String,
    activity_type: String,
}

#[derive(Debug, Deserialize)]
struct MetricsQuery {
    metric_name: String,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    labels: Option<HashMap<String, String>>,
}

impl MetricsQuery {
    fn timeframe(&self) -> TimeFrame {
        TimeFrame {
            start: self
                .start
                .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24)),
            end: self.end.unwrap_or_else(Utc::now),
        }
    }
}

#[derive(Debug, Serialize)]
struct MetricsResponse {
    metrics: Vec<super::Metric>,
}

#[derive(Debug, Serialize)]
struct WebSocketInfo {
    url: String,
    protocol: String,
}

#[derive(Debug, Deserialize)]
struct AlertsQuery {
    since: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsRequest {
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

impl AnalyticsRequest {
    fn timeframe(&self) -> TimeFrame {
        TimeFrame {
            start: self
                .start
                .unwrap_or_else(|| Utc::now() - chrono::Duration::days(7)),
            end: self.end.unwrap_or_else(Utc::now),
        }
    }
}

#[derive(Debug, Serialize)]
struct AnalyticsResponse {
    analyses: Vec<super::analytics::Analysis>,
}

#[derive(Debug, Deserialize)]
struct GenerateReportRequest {
    template_name: String,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

impl GenerateReportRequest {
    fn timeframe(&self) -> TimeFrame {
        TimeFrame {
            start: self
                .start
                .unwrap_or_else(|| Utc::now() - chrono::Duration::days(7)),
            end: self.end.unwrap_or_else(Utc::now),
        }
    }
}

#[derive(Debug, Serialize)]
struct GenerateReportResponse {
    report_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ExportReportQuery {
    format: ExportFormat,
}

// Basic dashboard HTML
const DASHBOARD_HTML: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>MMM Dashboard</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 0;
            background-color: #f5f5f5;
        }
        .header {
            background-color: #2c3e50;
            color: white;
            padding: 20px;
            text-align: center;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }
        .card {
            background-color: white;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            padding: 20px;
            margin-bottom: 20px;
        }
        h2 {
            margin-top: 0;
            color: #2c3e50;
        }
        .metric {
            display: inline-block;
            margin: 10px 20px 10px 0;
        }
        .metric-value {
            font-size: 24px;
            font-weight: bold;
            color: #3498db;
        }
        .metric-label {
            font-size: 14px;
            color: #666;
        }
        .status {
            display: inline-block;
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 12px;
            font-weight: bold;
        }
        .status.healthy {
            background-color: #27ae60;
            color: white;
        }
        .status.warning {
            background-color: #f39c12;
            color: white;
        }
        .status.error {
            background-color: #e74c3c;
            color: white;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>MMM Monitoring Dashboard</h1>
        <p>Real-time project insights and analytics</p>
    </div>
    <div class="container">
        <div class="card">
            <h2>System Overview</h2>
            <div class="metric">
                <div class="metric-value" id="active-projects">-</div>
                <div class="metric-label">Active Projects</div>
            </div>
            <div class="metric">
                <div class="metric-value" id="total-specs">-</div>
                <div class="metric-label">Total Specs</div>
            </div>
            <div class="metric">
                <div class="metric-value" id="completion-rate">-</div>
                <div class="metric-label">Completion Rate</div>
            </div>
            <div class="metric">
                <div class="metric-value" id="api-calls">-</div>
                <div class="metric-label">API Calls Today</div>
            </div>
        </div>
        
        <div class="card">
            <h2>Recent Alerts</h2>
            <div id="alerts-container">
                <p>Loading alerts...</p>
            </div>
        </div>
        
        <div class="card">
            <h2>Projects</h2>
            <div id="projects-container">
                <p>Loading projects...</p>
            </div>
        </div>
    </div>
    
    <script>
        // Basic dashboard functionality
        async function loadDashboard() {
            try {
                // Load projects
                const projectsRes = await fetch('/api/projects');
                const projects = await projectsRes.json();
                document.getElementById('active-projects').textContent = projects.length;
                
                // Load alerts
                const alertsRes = await fetch('/api/alerts');
                const alerts = await alertsRes.json();
                
                const alertsContainer = document.getElementById('alerts-container');
                if (alerts.length === 0) {
                    alertsContainer.innerHTML = '<p>No recent alerts</p>';
                } else {
                    alertsContainer.innerHTML = alerts.slice(0, 5).map(alert => `
                        <div class="alert">
                            <span class="status ${alert.severity.toLowerCase()}">${alert.severity}</span>
                            <strong>${alert.rule_name}</strong>: ${alert.message}
                            <br><small>${new Date(alert.timestamp).toLocaleString()}</small>
                        </div>
                    `).join('');
                }
                
                // Load projects list
                const projectsContainer = document.getElementById('projects-container');
                projectsContainer.innerHTML = projects.map(project => `
                    <div class="project">
                        <h3>${project.name}</h3>
                        <span class="status ${project.health_status || 'unknown'}">${project.health_status || 'Unknown'}</span>
                        <p>${project.path}</p>
                    </div>
                `).join('');
                
            } catch (error) {
                console.error('Failed to load dashboard:', error);
            }
        }
        
        // Load dashboard on page load
        loadDashboard();
        
        // Refresh every 30 seconds
        setInterval(loadDashboard, 30000);
    </script>
</body>
</html>
"#;
