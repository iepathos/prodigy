//! REST API server for analytics endpoints

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

use super::{
    engine::{AnalyticsEngine, CrossSessionAnalysis, SessionComparison},
    models::{Cost, TimeRange, ToolStats},
    persistence::AnalyticsDatabase,
};

/// API server for analytics endpoints
pub struct AnalyticsApiServer {
    engine: Arc<AnalyticsEngine>,
    db: Arc<AnalyticsDatabase>,
    port: u16,
}

impl AnalyticsApiServer {
    /// Create new API server
    pub fn new(engine: Arc<AnalyticsEngine>, db: Arc<AnalyticsDatabase>, port: u16) -> Self {
        Self { engine, db, port }
    }

    /// Start the API server
    pub async fn start(self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let app = self.build_router();

        info!("Starting analytics API server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    /// Build API router
    fn build_router(self) -> Router {
        let shared_state = Arc::new(ApiState {
            engine: self.engine,
            db: self.db,
        });

        Router::new()
            .route("/api/v1/analytics/health", get(health_check))
            .route("/api/v1/analytics/sessions", get(list_sessions))
            .route("/api/v1/analytics/sessions/:id", get(get_session))
            .route("/api/v1/analytics/sessions/:id/cost", get(get_session_cost))
            .route("/api/v1/analytics/sessions/:id/export", get(export_session))
            .route("/api/v1/analytics/sessions/compare", post(compare_sessions))
            .route("/api/v1/analytics/tools/usage", get(get_tool_usage))
            .route("/api/v1/analytics/tools/export", get(export_tool_stats))
            .route(
                "/api/v1/analytics/costs/projection",
                get(get_cost_projection),
            )
            .route("/api/v1/analytics/costs/export", get(export_cost_data))
            .route("/api/v1/analytics/patterns", get(get_usage_patterns))
            .route(
                "/api/v1/analytics/cross-session",
                post(analyze_cross_session),
            )
            .route(
                "/api/v1/analytics/recommendations",
                get(get_recommendations),
            )
            .route("/api/v1/analytics/bottlenecks", get(get_bottlenecks))
            .route("/api/v1/analytics/stats", get(get_database_stats))
            .route(
                "/api/v1/analytics/retention/cleanup",
                post(cleanup_old_sessions),
            )
            .route(
                "/api/v1/analytics/retention/archive",
                post(archive_sessions),
            )
            .layer(CorsLayer::permissive())
            .with_state(shared_state)
    }
}

/// Shared API state
#[derive(Clone)]
struct ApiState {
    engine: Arc<AnalyticsEngine>,
    db: Arc<AnalyticsDatabase>,
}

/// Time range query parameters
#[derive(Debug, Deserialize)]
struct TimeRangeQuery {
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

impl TimeRangeQuery {
    fn to_time_range(&self) -> TimeRange {
        TimeRange {
            start: self
                .start
                .unwrap_or_else(|| Utc::now() - chrono::Duration::days(7)),
            end: self.end.unwrap_or_else(Utc::now),
        }
    }
}

/// Session comparison request
#[derive(Debug, Deserialize)]
struct CompareSessionsRequest {
    session_id_1: String,
    session_id_2: String,
}

/// Cross-session analysis request
#[derive(Debug, Deserialize)]
struct CrossSessionRequest {
    session_ids: Vec<String>,
}

/// Export format query parameter
#[derive(Debug, Deserialize)]
struct ExportQuery {
    format: Option<String>,
}

/// Archive request
#[derive(Debug, Deserialize)]
struct ArchiveRequest {
    before: DateTime<Utc>,
    archive_path: String,
}

/// Cleanup request
#[derive(Debug, Deserialize)]
struct CleanupRequest {
    retention_days: i64,
}

/// API response wrapper
#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

// API Handlers

async fn health_check() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse::success("Analytics API is healthy"))
}

async fn list_sessions(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<TimeRangeQuery>,
) -> Result<Json<ApiResponse<Vec<SessionSummary>>>, StatusCode> {
    let time_range = params.to_time_range();

    match state
        .db
        .query_sessions(time_range.start, time_range.end)
        .await
    {
        Ok(sessions) => {
            let summaries: Vec<SessionSummary> = sessions
                .into_iter()
                .map(|s| SessionSummary {
                    session_id: s.session_id,
                    project_path: s.project_path,
                    started_at: s.started_at,
                    completed_at: s.completed_at,
                    total_tokens: s.total_input_tokens
                        + s.total_output_tokens
                        + s.total_cache_tokens,
                    tool_count: s.tool_invocations.len(),
                })
                .collect();

            Ok(Json(ApiResponse::success(summaries)))
        }
        Err(e) => {
            warn!("Failed to list sessions: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to list sessions: {}",
                e
            ))))
        }
    }
}

async fn get_session(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match state.db.get_session(&id).await {
        Ok(Some(session)) => match serde_json::to_value(session) {
            Ok(json_value) => Ok(Json(ApiResponse::success(json_value))),
            Err(e) => {
                warn!("Failed to serialize session {}: {}", id, e);
                Ok(Json(ApiResponse::error(format!(
                    "Failed to serialize session: {}", e
                ))))
            }
        },
        Ok(None) => Ok(Json(ApiResponse::error(format!(
            "Session {} not found",
            id
        )))),
        Err(e) => {
            warn!("Failed to get session {}: {}", id, e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to get session: {}",
                e
            ))))
        }
    }
}

async fn get_session_cost(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Cost>>, StatusCode> {
    match state.engine.calculate_session_cost(&id).await {
        Ok(cost) => Ok(Json(ApiResponse::success(cost))),
        Err(e) => {
            warn!("Failed to calculate cost for session {}: {}", id, e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to calculate cost: {}",
                e
            ))))
        }
    }
}

async fn export_session(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(params): Query<ExportQuery>,
) -> Result<String, StatusCode> {
    let format = params.format.as_deref().unwrap_or("json");

    match state.db.get_session(&id).await {
        Ok(Some(session)) => {
            match format {
                "json" => serde_json::to_string_pretty(&session)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
                "csv" => {
                    // Export as CSV (simplified - just basic info)
                    Ok(format!(
                        "session_id,project_path,started_at,total_tokens\n{},{},{},{}",
                        session.session_id,
                        session.project_path,
                        session.started_at,
                        session.total_input_tokens
                            + session.total_output_tokens
                            + session.total_cache_tokens
                    ))
                }
                _ => Err(StatusCode::BAD_REQUEST),
            }
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn compare_sessions(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CompareSessionsRequest>,
) -> Result<Json<ApiResponse<SessionComparison>>, StatusCode> {
    match state
        .engine
        .compare_sessions(&request.session_id_1, &request.session_id_2)
        .await
    {
        Ok(comparison) => Ok(Json(ApiResponse::success(comparison))),
        Err(e) => {
            warn!("Failed to compare sessions: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to compare sessions: {}",
                e
            ))))
        }
    }
}

async fn get_tool_usage(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<TimeRangeQuery>,
) -> Result<Json<ApiResponse<ToolStats>>, StatusCode> {
    let time_range = params.to_time_range();

    match state.engine.analyze_tool_usage(time_range).await {
        Ok(stats) => Ok(Json(ApiResponse::success(stats))),
        Err(e) => {
            warn!("Failed to analyze tool usage: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to analyze tool usage: {}",
                e
            ))))
        }
    }
}

async fn export_tool_stats(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<TimeRangeQuery>,
    Query(export): Query<ExportQuery>,
) -> Result<String, StatusCode> {
    let time_range = params.to_time_range();
    let format = export.format.as_deref().unwrap_or("json");

    match state.engine.analyze_tool_usage(time_range).await {
        Ok(stats) => match format {
            "json" => Ok(stats.to_json().to_string()),
            "csv" => Ok(stats.to_csv()),
            _ => Err(StatusCode::BAD_REQUEST),
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_cost_projection(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match state.engine.project_costs(30).await {
        Ok(projection) => match serde_json::to_value(projection) {
            Ok(json_value) => Ok(Json(ApiResponse::success(json_value))),
            Err(e) => {
                warn!("Failed to serialize cost projection: {}", e);
                Ok(Json(ApiResponse::error(format!(
                    "Failed to serialize projection: {}", e
                ))))
            }
        },
        Err(e) => {
            warn!("Failed to project costs: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to project costs: {}",
                e
            ))))
        }
    }
}

async fn export_cost_data(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(export): Query<ExportQuery>,
) -> Result<String, StatusCode> {
    let format = export.format.as_deref().unwrap_or("json");

    match state.engine.calculate_session_cost(&id).await {
        Ok(cost) => match format {
            "json" => Ok(cost.to_json().to_string()),
            "csv" => Ok(format!("{}\n{}", Cost::csv_header(), cost.to_csv_row())),
            _ => Err(StatusCode::BAD_REQUEST),
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_usage_patterns(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match state.engine.generate_usage_patterns().await {
        Ok(patterns) => match serde_json::to_value(patterns) {
            Ok(json_value) => Ok(Json(ApiResponse::success(json_value))),
            Err(e) => {
                warn!("Failed to serialize usage patterns: {}", e);
                Ok(Json(ApiResponse::error(format!(
                    "Failed to serialize patterns: {}", e
                ))))
            }
        },
        Err(e) => {
            warn!("Failed to generate usage patterns: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to generate patterns: {}",
                e
            ))))
        }
    }
}

async fn analyze_cross_session(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CrossSessionRequest>,
) -> Result<Json<ApiResponse<CrossSessionAnalysis>>, StatusCode> {
    match state
        .engine
        .analyze_cross_session_patterns(request.session_ids)
        .await
    {
        Ok(analysis) => Ok(Json(ApiResponse::success(analysis))),
        Err(e) => {
            warn!("Failed to analyze cross-session patterns: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to analyze: {}",
                e
            ))))
        }
    }
}

async fn get_recommendations(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, StatusCode> {
    match state.engine.get_optimization_recommendations().await {
        Ok(recommendations) => {
            let json_recs: Result<Vec<serde_json::Value>, _> = recommendations
                .into_iter()
                .map(|r| serde_json::to_value(r))
                .collect();

            match json_recs {
                Ok(serialized_recs) => Ok(Json(ApiResponse::success(serialized_recs))),
                Err(e) => {
                    warn!("Failed to serialize recommendations: {}", e);
                    Ok(Json(ApiResponse::error(format!(
                        "Failed to serialize recommendations: {}", e
                    ))))
                }
            }
        }
        Err(e) => {
            warn!("Failed to get recommendations: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to get recommendations: {}",
                e
            ))))
        }
    }
}

async fn get_bottlenecks(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, StatusCode> {
    match state.engine.identify_bottlenecks(5000).await {
        Ok(bottlenecks) => {
            let json_bottlenecks: Result<Vec<serde_json::Value>, _> = bottlenecks
                .into_iter()
                .map(|b| serde_json::to_value(b))
                .collect();

            match json_bottlenecks {
                Ok(serialized_bottlenecks) => Ok(Json(ApiResponse::success(serialized_bottlenecks))),
                Err(e) => {
                    warn!("Failed to serialize bottlenecks: {}", e);
                    Ok(Json(ApiResponse::error(format!(
                        "Failed to serialize bottlenecks: {}", e
                    ))))
                }
            }
        }
        Err(e) => {
            warn!("Failed to identify bottlenecks: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to identify bottlenecks: {}",
                e
            ))))
        }
    }
}

async fn get_database_stats(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match state.db.get_stats().await {
        Ok(stats) => match serde_json::to_value(stats) {
            Ok(json_value) => Ok(Json(ApiResponse::success(json_value))),
            Err(e) => {
                warn!("Failed to serialize database stats: {}", e);
                Ok(Json(ApiResponse::error(format!(
                    "Failed to serialize stats: {}", e
                ))))
            }
        },
        Err(e) => {
            warn!("Failed to get database stats: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to get stats: {}",
                e
            ))))
        }
    }
}

async fn cleanup_old_sessions(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CleanupRequest>,
) -> Result<Json<ApiResponse<u64>>, StatusCode> {
    match state.db.cleanup_old_sessions(request.retention_days).await {
        Ok(deleted_count) => Ok(Json(ApiResponse::success(deleted_count))),
        Err(e) => {
            warn!("Failed to cleanup old sessions: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to cleanup: {}",
                e
            ))))
        }
    }
}

async fn archive_sessions(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ArchiveRequest>,
) -> Result<Json<ApiResponse<u64>>, StatusCode> {
    match state
        .db
        .archive_sessions(request.before, request.archive_path)
        .await
    {
        Ok(archived_count) => Ok(Json(ApiResponse::success(archived_count))),
        Err(e) => {
            warn!("Failed to archive sessions: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to archive: {}",
                e
            ))))
        }
    }
}

/// Session summary for list endpoint
#[derive(Debug, Serialize)]
struct SessionSummary {
    session_id: String,
    project_path: String,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    total_tokens: u64,
    tool_count: usize,
}
