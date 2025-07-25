use super::{init_database, Checkpoint, ProjectState};
use crate::{Error, Result};
use sqlx::{Pool, Row, Sqlite};
use std::path::PathBuf;

pub struct StateManager {
    pool: Pool<Sqlite>,
    project_id: i64,
}

impl StateManager {
    pub async fn new(db_path: PathBuf, project_name: &str) -> Result<Self> {
        let pool = init_database(&db_path).await?;

        // Insert or get project
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO projects (name, path)
            VALUES (?1, ?2)
            "#,
        )
        .bind(project_name)
        .bind(db_path.parent().unwrap().to_str().unwrap())
        .execute(&pool)
        .await?;

        let row = sqlx::query("SELECT id FROM projects WHERE name = ?1")
            .bind(project_name)
            .fetch_one(&pool)
            .await?;

        let project_id: i64 = row.get("id");

        Ok(Self { pool, project_id })
    }

    pub async fn get_current_state(&self) -> Result<ProjectState> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, snapshot_data, created_at
            FROM state_snapshots
            WHERE project_id = ?1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(self.project_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let snapshot_data: String = row.get("snapshot_data");
                let snapshot_value: serde_json::Value = serde_json::from_str(&snapshot_data)?;
                serde_json::from_value(snapshot_value).map_err(|e| Error::Serialization(e))
            }
            None => Ok(ProjectState::default()),
        }
    }

    pub async fn save_state(&self, state: &ProjectState) -> Result<i64> {
        let snapshot_data = serde_json::to_string(state)?;

        let result = sqlx::query(
            r#"
            INSERT INTO state_snapshots (project_id, snapshot_data)
            VALUES (?1, ?2)
            "#,
        )
        .bind(self.project_id)
        .bind(snapshot_data)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn create_checkpoint(&self, spec_id: &str, description: &str) -> Result<String> {
        let current_state = self.get_current_state().await?;
        let snapshot_id = self.save_state(&current_state).await?;

        let checkpoint_id = format!("checkpoint-{}-{}", spec_id, chrono::Utc::now().timestamp());

        let checkpoint = Checkpoint {
            id: checkpoint_id.clone(),
            spec_id: spec_id.to_string(),
            created_at: chrono::Utc::now(),
            description: description.to_string(),
            state_snapshot_id: snapshot_id,
        };

        let mut state = current_state;
        state.checkpoints.push(checkpoint);
        self.save_state(&state).await?;

        Ok(checkpoint_id)
    }

    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let current_state = self.get_current_state().await?;

        let checkpoint = current_state
            .checkpoints
            .iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or_else(|| Error::Other(format!("Checkpoint '{}' not found", checkpoint_id)))?;

        let row = sqlx::query(
            r#"
            SELECT id, project_id, snapshot_data, created_at
            FROM state_snapshots
            WHERE id = ?1
            "#,
        )
        .bind(checkpoint.state_snapshot_id)
        .fetch_one(&self.pool)
        .await?;

        let snapshot_data: String = row.get("snapshot_data");
        let state: ProjectState = serde_json::from_str(&snapshot_data)?;
        self.save_state(&state).await?;

        Ok(())
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    pub async fn record_spec_completion(&self, spec_id: &str) -> Result<()> {
        let mut state = self.get_current_state().await?;

        if !state.completed_specs.contains(&spec_id.to_string()) {
            state.completed_specs.push(spec_id.to_string());
        }

        state.failed_specs.retain(|s| s != spec_id);

        if state.current_spec.as_ref() == Some(&spec_id.to_string()) {
            state.current_spec = None;
        }

        self.save_state(&state).await?;
        Ok(())
    }

    pub async fn record_spec_failure(&self, spec_id: &str) -> Result<()> {
        let mut state = self.get_current_state().await?;

        if !state.failed_specs.contains(&spec_id.to_string()) {
            state.failed_specs.push(spec_id.to_string());
        }

        if state.current_spec.as_ref() == Some(&spec_id.to_string()) {
            state.current_spec = None;
        }

        self.save_state(&state).await?;
        Ok(())
    }

    pub async fn set_current_spec(&self, spec_id: &str) -> Result<()> {
        let mut state = self.get_current_state().await?;
        state.current_spec = Some(spec_id.to_string());
        self.save_state(&state).await?;
        Ok(())
    }

    pub async fn get_spec_executions(
        &self,
        spec_id: &str,
    ) -> Result<Vec<crate::spec::SpecExecution>> {
        // First get the spec database ID
        let spec_row =
            sqlx::query("SELECT id FROM specifications WHERE name = ?1 AND project_id = ?2")
                .bind(spec_id)
                .bind(self.project_id)
                .fetch_optional(&self.pool)
                .await?;

        let spec_db_id: i64 = match spec_row {
            Some(row) => row.get("id"),
            None => return Ok(Vec::new()),
        };

        let rows = sqlx::query(
            r#"
            SELECT id, spec_id, iteration, command, input, output, status, started_at, completed_at
            FROM executions
            WHERE spec_id = ?1
            ORDER BY started_at DESC
            "#,
        )
        .bind(spec_db_id)
        .fetch_all(&self.pool)
        .await?;

        let executions = rows
            .into_iter()
            .map(|row| {
                let status = match row.get::<String, _>("status").as_str() {
                    "running" => crate::spec::ExecutionStatus::Running,
                    "success" => crate::spec::ExecutionStatus::Success,
                    "failed" => crate::spec::ExecutionStatus::Failed,
                    "timeout" => crate::spec::ExecutionStatus::Timeout,
                    _ => crate::spec::ExecutionStatus::Failed,
                };

                crate::spec::SpecExecution {
                    spec_id: spec_id.to_string(),
                    iteration: row.get::<i32, _>("iteration") as u32,
                    command: row.get("command"),
                    input: row.get("input"),
                    output: row.get("output"),
                    status,
                    started_at: chrono::DateTime::parse_from_rfc3339(
                        &row.get::<String, _>("started_at"),
                    )
                    .unwrap()
                    .with_timezone(&chrono::Utc),
                    completed_at: row.get::<Option<String>, _>("completed_at").and_then(|dt| {
                        chrono::DateTime::parse_from_rfc3339(&dt)
                            .ok()
                            .map(|d| d.with_timezone(&chrono::Utc))
                    }),
                }
            })
            .collect();

        Ok(executions)
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, path, created_at, updated_at
            FROM projects
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let projects = rows
            .into_iter()
            .map(|row| {
                let id_str: String = row.get::<i64, _>("id").to_string();
                let created_str: String = row.get("created_at");
                let updated_str: String = row.get("updated_at");

                ProjectInfo {
                    id: uuid::Uuid::parse_str(&format!("00000000-0000-0000-0000-{:012}", id_str))
                        .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                    name: row.get("name"),
                    path: std::path::PathBuf::from(row.get::<String, _>("path")),
                    status: "active".to_string(),
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                        .unwrap_or_else(|_| chrono::Utc::now())
                        .with_timezone(&chrono::Utc),
                    last_accessed: Some(
                        chrono::DateTime::parse_from_rfc3339(&updated_str)
                            .unwrap_or_else(|_| chrono::Utc::now())
                            .with_timezone(&chrono::Utc),
                    ),
                }
            })
            .collect();

        Ok(projects)
    }
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub id: uuid::Uuid,
    pub name: String,
    pub path: std::path::PathBuf,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: Option<chrono::DateTime<chrono::Utc>>,
}
