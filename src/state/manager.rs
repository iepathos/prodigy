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
                serde_json::from_value(snapshot_value).map_err(Error::Serialization)
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
            .ok_or_else(|| Error::Other(format!("Checkpoint '{checkpoint_id}' not found")))?;

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

    pub async fn get_value(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let state = self.get_current_state().await?;

        // Parse the key path (e.g., "project.config.foo" -> ["project", "config", "foo"])
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &state.variables;

        for part in parts {
            match current.get(part) {
                Some(value) => current = value,
                None => return Ok(None),
            }
        }

        Ok(Some(current.clone()))
    }

    pub async fn set_value(&self, key: &str, value: serde_json::Value) -> Result<()> {
        let mut state = self.get_current_state().await?;

        // Parse the key path
        let parts: Vec<&str> = key.split('.').collect();

        // Navigate to the parent and set the final key
        let mut current = &mut state.variables;
        for part in &parts[..parts.len() - 1] {
            if !current.is_object() {
                *current = serde_json::json!({});
            }
            if !current.as_object().unwrap().contains_key(*part) {
                current
                    .as_object_mut()
                    .unwrap()
                    .insert(part.to_string(), serde_json::json!({}));
            }
            current = current.get_mut(part).unwrap();
        }

        if !current.is_object() {
            *current = serde_json::json!({});
        }
        current
            .as_object_mut()
            .unwrap()
            .insert(parts[parts.len() - 1].to_string(), value);

        self.save_state(&state).await?;
        Ok(())
    }

    pub async fn delete_value(&self, key: &str) -> Result<()> {
        let mut state = self.get_current_state().await?;

        // Parse the key path
        let parts: Vec<&str> = key.split('.').collect();

        if parts.is_empty() {
            return Ok(());
        }

        // Navigate to the parent
        let mut current = &mut state.variables;
        for part in &parts[..parts.len() - 1] {
            if let Some(next) = current.get_mut(part) {
                current = next;
            } else {
                return Ok(()); // Key doesn't exist, nothing to delete
            }
        }

        // Remove the final key
        if let Some(obj) = current.as_object_mut() {
            obj.remove(parts[parts.len() - 1]);
        }

        self.save_state(&state).await?;
        Ok(())
    }

    pub async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let state = self.get_current_state().await?;
        let mut keys = Vec::new();

        self.collect_keys(&state.variables, prefix, "", &mut keys);
        Ok(keys)
    }

    fn collect_keys(
        &self,
        value: &serde_json::Value,
        prefix: &str,
        current_path: &str,
        keys: &mut Vec<String>,
    ) {
        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                let new_path = if current_path.is_empty() {
                    key.clone()
                } else {
                    format!("{current_path}.{key}")
                };

                if new_path.starts_with(prefix) {
                    keys.push(new_path.clone());
                }

                self.collect_keys(val, prefix, &new_path, keys);
            }
        }
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
                    id: uuid::Uuid::parse_str(&format!("00000000-0000-0000-0000-{id_str:012}"))
                        .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                    name: row.get("name"),
                    path: std::path::PathBuf::from(row.get::<String, _>("path")),
                    status: "active".to_string(),
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_accessed: Some(
                        chrono::DateTime::parse_from_rfc3339(&updated_str)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now()),
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
