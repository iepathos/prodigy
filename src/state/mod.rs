use crate::Result;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::path::Path;

pub mod manager;
pub mod migrations;

pub use manager::StateManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub id: i64,
    pub project_id: i64,
    pub snapshot_data: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub current_spec: Option<String>,
    pub completed_specs: Vec<String>,
    pub failed_specs: Vec<String>,
    pub variables: serde_json::Value,
    pub checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub spec_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub description: String,
    pub state_snapshot_id: i64,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            current_spec: None,
            completed_specs: Vec::new(),
            failed_specs: Vec::new(),
            variables: serde_json::json!({}),
            checkpoints: Vec::new(),
        }
    }
}

pub async fn init_database(db_path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db_url = format!("sqlite:{}", db_path.display());

    let pool = SqlitePool::connect(&db_url).await?;

    // Run migrations manually since we can't use sqlx::migrate! at runtime
    sqlx::query(migrations::INITIAL_MIGRATION)
        .execute(&pool)
        .await?;

    sqlx::query(migrations::WORKFLOW_MIGRATION)
        .execute(&pool)
        .await?;

    sqlx::query(migrations::MONITORING_MIGRATION)
        .execute(&pool)
        .await?;

    Ok(pool)
}
