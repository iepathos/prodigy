use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{ExecutionEvent, WorkflowState, WorkflowStatus};

#[derive(Debug, Clone)]
pub struct WorkflowExecution {
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Simple JSON-based workflow state manager
pub struct WorkflowStateManager {
    states: Arc<RwLock<HashMap<Uuid, WorkflowState>>>,
    root_path: PathBuf,
}

impl WorkflowStateManager {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            root_path,
        }
    }

    pub async fn create_workflow_state(
        &self,
        workflow_id: Uuid,
        spec_id: Option<String>,
    ) -> Result<()> {
        let state = WorkflowState {
            workflow_id,
            spec_id,
            status: WorkflowStatus::Pending,
            current_stage: None,
            current_step: None,
            variables: HashMap::new(),
            outputs: HashMap::new(),
            history: vec![],
            started_at: chrono::Utc::now(),
            completed_at: None,
        };

        let mut states = self.states.write().await;
        states.insert(workflow_id, state.clone());

        // Save to JSON file
        self.save_state(&state).await?;
        Ok(())
    }

    pub async fn get_workflow_state(&self, workflow_id: &Uuid) -> Result<Option<WorkflowState>> {
        let states = self.states.read().await;
        if let Some(state) = states.get(workflow_id) {
            return Ok(Some(state.clone()));
        }

        // Try to load from file
        let file_path = self.state_file_path(workflow_id);
        if file_path.exists() {
            let content = tokio::fs::read_to_string(&file_path).await?;
            let state: WorkflowState = serde_json::from_str(&content)?;
            return Ok(Some(state));
        }

        Ok(None)
    }

    pub async fn update_workflow_status(
        &self,
        workflow_id: &Uuid,
        status: WorkflowStatus,
    ) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(workflow_id) {
            state.status = status.clone();
            if matches!(
                status,
                WorkflowStatus::Completed | WorkflowStatus::Failed | WorkflowStatus::Cancelled
            ) {
                state.completed_at = Some(chrono::Utc::now());
            }
            self.save_state(state).await?;
        }
        Ok(())
    }

    pub async fn update_current_stage(&self, workflow_id: &Uuid, stage_name: &str) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(workflow_id) {
            state.current_stage = Some(stage_name.to_string());
            self.save_state(state).await?;
        }
        Ok(())
    }

    pub async fn update_current_step(&self, workflow_id: &Uuid, step_name: &str) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(workflow_id) {
            state.current_step = Some(step_name.to_string());
            self.save_state(state).await?;
        }
        Ok(())
    }

    pub async fn set_variable(
        &self,
        workflow_id: &Uuid,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(workflow_id) {
            state.variables.insert(key.to_string(), value);
            self.save_state(state).await?;
        }
        Ok(())
    }

    pub async fn add_execution_event(
        &self,
        workflow_id: &Uuid,
        event: ExecutionEvent,
    ) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(workflow_id) {
            state.history.push(event);
            self.save_state(state).await?;
        }
        Ok(())
    }

    pub async fn update_workflow_state(&self, state: &WorkflowState) -> Result<()> {
        let mut states = self.states.write().await;
        states.insert(state.workflow_id, state.clone());
        self.save_state(state).await?;
        Ok(())
    }

    pub async fn list_workflows(&self) -> Result<Vec<WorkflowState>> {
        let mut workflows = vec![];

        // List all workflow JSON files
        let workflow_dir = self.root_path.join("workflows");
        if workflow_dir.exists() {
            let mut entries = tokio::fs::read_dir(&workflow_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        let content = tokio::fs::read_to_string(entry.path()).await?;
                        if let Ok(state) = serde_json::from_str::<WorkflowState>(&content) {
                            workflows.push(state);
                        }
                    }
                }
            }
        }

        Ok(workflows)
    }

    pub async fn list_workflow_executions(
        &self,
        workflow_filter: Option<&str>,
        status_filter: Option<&str>,
        limit: i64,
    ) -> Result<Vec<WorkflowExecution>> {
        let mut executions = vec![];

        // List all workflow JSON files and convert to executions
        let workflow_dir = self.root_path.join("workflows");
        if workflow_dir.exists() {
            let mut entries = tokio::fs::read_dir(&workflow_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        let content = tokio::fs::read_to_string(entry.path()).await?;
                        if let Ok(state) = serde_json::from_str::<WorkflowState>(&content) {
                            // Convert WorkflowState to WorkflowExecution for display
                            let execution = WorkflowExecution {
                                workflow_id: state.workflow_id.to_string(),
                                workflow_name: workflow_filter.unwrap_or("unknown").to_string(),
                                status: format!("{:?}", state.status),
                                started_at: state.started_at,
                                completed_at: state.completed_at,
                            };

                            // Apply filters
                            if let Some(name_filter) = workflow_filter {
                                if !execution.workflow_name.contains(name_filter) {
                                    continue;
                                }
                            }
                            if let Some(status_filt) = status_filter {
                                if !execution
                                    .status
                                    .to_lowercase()
                                    .contains(&status_filt.to_lowercase())
                                {
                                    continue;
                                }
                            }

                            executions.push(execution);
                        }
                    }
                }
            }
        }

        // Sort by started_at desc and limit
        executions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        executions.truncate(limit as usize);

        Ok(executions)
    }

    async fn save_state(&self, state: &WorkflowState) -> Result<()> {
        let file_path = self.state_file_path(&state.workflow_id);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = serde_json::to_string_pretty(state)?;
        tokio::fs::write(&file_path, content).await?;
        Ok(())
    }

    fn state_file_path(&self, workflow_id: &Uuid) -> PathBuf {
        self.root_path
            .join("workflows")
            .join(format!("{workflow_id}.json"))
    }
}
