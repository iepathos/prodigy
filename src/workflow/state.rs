use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use uuid::Uuid;

use super::{ExecutionEvent, WorkflowState, WorkflowStatus};

pub struct WorkflowStateManager {
    pool: SqlitePool,
}

impl WorkflowStateManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_workflow_state(
        &self,
        workflow_name: &str,
        spec_id: Option<&str>,
    ) -> Result<WorkflowState> {
        let workflow_id = Uuid::new_v4();
        let now = Utc::now();

        let state = WorkflowState {
            workflow_id,
            spec_id: spec_id.map(String::from),
            status: WorkflowStatus::Pending,
            current_stage: None,
            current_step: None,
            variables: HashMap::new(),
            outputs: HashMap::new(),
            history: vec![],
            started_at: now,
            completed_at: None,
        };

        let state_json = serde_json::to_string(&state)?;

        sqlx::query(
            r#"
            INSERT INTO workflow_executions (
                workflow_id, workflow_name, spec_id, status, state_json, started_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(workflow_id.to_string())
        .bind(workflow_name)
        .bind(spec_id)
        .bind("pending")
        .bind(state_json)
        .bind(now.timestamp())
        .execute(&self.pool)
        .await
        .context("Failed to create workflow state")?;

        Ok(state)
    }

    pub async fn get_workflow_state(&self, workflow_id: &Uuid) -> Result<Option<WorkflowState>> {
        let record = sqlx::query(
            r#"
            SELECT state_json FROM workflow_executions
            WHERE workflow_id = ?1
            "#,
        )
        .bind(workflow_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch workflow state")?;

        match record {
            Some(r) => {
                let state_json: String = r.get("state_json");
                let state: WorkflowState = serde_json::from_str(&state_json)
                    .context("Failed to deserialize workflow state")?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    pub async fn update_workflow_state(&self, state: &WorkflowState) -> Result<()> {
        let state_json = serde_json::to_string(state)?;
        let status_str = match state.status {
            WorkflowStatus::Pending => "pending",
            WorkflowStatus::Running => "running",
            WorkflowStatus::Paused => "paused",
            WorkflowStatus::WaitingForCheckpoint => "waiting_checkpoint",
            WorkflowStatus::Completed => "completed",
            WorkflowStatus::Failed => "failed",
            WorkflowStatus::Cancelled => "cancelled",
        };

        sqlx::query(
            r#"
            UPDATE workflow_executions
            SET status = ?1, state_json = ?2, completed_at = ?3
            WHERE workflow_id = ?4
            "#,
        )
        .bind(status_str)
        .bind(state_json)
        .bind(state.completed_at.map(|dt| dt.timestamp()))
        .bind(state.workflow_id.to_string())
        .execute(&self.pool)
        .await
        .context("Failed to update workflow state")?;

        Ok(())
    }

    pub async fn add_execution_event(
        &self,
        workflow_id: &Uuid,
        event: ExecutionEvent,
    ) -> Result<()> {
        if let Some(mut state) = self.get_workflow_state(workflow_id).await? {
            state.history.push(event);
            self.update_workflow_state(&state).await?;
        }
        Ok(())
    }

    pub async fn list_workflow_executions(
        &self,
        workflow_name: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<WorkflowExecutionSummary>> {
        let query = if let Some(name) = workflow_name {
            if let Some(status) = status {
                sqlx::query(
                    r#"
                    SELECT workflow_id, workflow_name, spec_id, status, started_at, completed_at
                    FROM workflow_executions
                    WHERE workflow_name = ?1 AND status = ?2
                    ORDER BY started_at DESC
                    LIMIT ?3
                    "#,
                )
                .bind(name)
                .bind(status)
                .bind(limit)
            } else {
                sqlx::query(
                    r#"
                    SELECT workflow_id, workflow_name, spec_id, status, started_at, completed_at
                    FROM workflow_executions
                    WHERE workflow_name = ?1
                    ORDER BY started_at DESC
                    LIMIT ?2
                    "#,
                )
                .bind(name)
                .bind(limit)
            }
        } else if let Some(status) = status {
            sqlx::query(
                r#"
                SELECT workflow_id, workflow_name, spec_id, status, started_at, completed_at
                FROM workflow_executions
                WHERE status = ?1
                ORDER BY started_at DESC
                LIMIT ?2
                "#,
            )
            .bind(status)
            .bind(limit)
        } else {
            sqlx::query(
                r#"
                SELECT workflow_id, workflow_name, spec_id, status, started_at, completed_at
                FROM workflow_executions
                ORDER BY started_at DESC
                LIMIT ?1
                "#,
            )
            .bind(limit)
        };

        let rows = query
            .fetch_all(&self.pool)
            .await
            .context("Failed to list workflow executions")?;

        let summaries: Vec<WorkflowExecutionSummary> = rows
            .into_iter()
            .map(|row| WorkflowExecutionSummary {
                workflow_id: row.get("workflow_id"),
                workflow_name: row.get("workflow_name"),
                spec_id: row.get("spec_id"),
                status: row.get("status"),
                started_at: DateTime::from_timestamp(row.get("started_at"), 0)
                    .unwrap_or_else(Utc::now),
                completed_at: row
                    .get::<Option<i64>, _>("completed_at")
                    .and_then(|ts| DateTime::from_timestamp(ts, 0)),
            })
            .collect();

        Ok(summaries)
    }

    pub async fn create_checkpoint(&self, workflow_id: &Uuid, description: &str) -> Result<Uuid> {
        let checkpoint_id = Uuid::new_v4();
        let state = self
            .get_workflow_state(workflow_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Workflow not found"))?;

        let state_json = serde_json::to_string(&state)?;

        sqlx::query(
            r#"
            INSERT INTO workflow_checkpoints (
                checkpoint_id, workflow_id, description, state_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(checkpoint_id.to_string())
        .bind(workflow_id.to_string())
        .bind(description)
        .bind(state_json)
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .context("Failed to create checkpoint")?;

        Ok(checkpoint_id)
    }

    pub async fn restore_checkpoint(&self, checkpoint_id: &Uuid) -> Result<WorkflowState> {
        let record = sqlx::query(
            r#"
            SELECT state_json FROM workflow_checkpoints
            WHERE checkpoint_id = ?1
            "#,
        )
        .bind(checkpoint_id.to_string())
        .fetch_one(&self.pool)
        .await
        .context("Failed to fetch checkpoint")?;

        let state_json: String = record.get("state_json");
        let state: WorkflowState =
            serde_json::from_str(&state_json).context("Failed to deserialize checkpoint state")?;

        self.update_workflow_state(&state).await?;

        Ok(state)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionSummary {
    pub workflow_id: String,
    pub workflow_name: String,
    pub spec_id: Option<String>,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
