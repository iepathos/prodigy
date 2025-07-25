use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::WorkflowState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCheckpoint {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub step_name: String,
    pub message: String,
    pub options: Vec<CheckpointOption>,
    pub timeout: Option<Duration>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointOption {
    Approve,
    ApproveWithChanges { changes: String },
    Reject { reason: String },
    RequestMoreInfo { questions: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResponse {
    pub checkpoint_id: Uuid,
    pub option: CheckpointOption,
    pub user: String,
    pub timestamp: DateTime<Utc>,
}

pub trait NotificationService: Send + Sync {
    fn notify_checkpoint(&self, checkpoint: &PendingCheckpoint) -> Result<()>;
}

pub struct ConsoleNotificationService;

impl NotificationService for ConsoleNotificationService {
    fn notify_checkpoint(&self, checkpoint: &PendingCheckpoint) -> Result<()> {
        println!("\n🔔 Workflow Checkpoint Reached!");
        println!("  Workflow ID: {}", checkpoint.workflow_id);
        println!("  Step: {}", checkpoint.step_name);
        println!("  Message: {}", checkpoint.message);
        println!("  Options:");
        for (i, option) in checkpoint.options.iter().enumerate() {
            match option {
                CheckpointOption::Approve => println!("    {}: Approve", i + 1),
                CheckpointOption::ApproveWithChanges { .. } => {
                    println!("    {}: Approve with changes", i + 1)
                }
                CheckpointOption::Reject { .. } => println!("    {}: Reject", i + 1),
                CheckpointOption::RequestMoreInfo { .. } => {
                    println!("    {}: Request more information", i + 1)
                }
            }
        }
        if let Some(expires_at) = checkpoint.expires_at {
            println!("  Expires at: {}", expires_at);
        }
        println!(
            "\n  Respond with: mmm workflow checkpoint respond {} <option>\n",
            checkpoint.id
        );
        Ok(())
    }
}

pub struct CheckpointManager {
    pending_checkpoints: Arc<RwLock<HashMap<Uuid, PendingCheckpoint>>>,
    notification_service: Box<dyn NotificationService>,
    state_manager: Arc<super::state::WorkflowStateManager>,
}

impl CheckpointManager {
    pub fn new(
        notification_service: Box<dyn NotificationService>,
        state_manager: Arc<super::state::WorkflowStateManager>,
    ) -> Self {
        Self {
            pending_checkpoints: Arc::new(RwLock::new(HashMap::new())),
            notification_service,
            state_manager,
        }
    }

    pub async fn create_checkpoint(
        &self,
        workflow_id: Uuid,
        step_name: String,
        message: String,
        timeout_seconds: Option<u64>,
    ) -> Result<PendingCheckpoint> {
        let checkpoint = PendingCheckpoint {
            id: Uuid::new_v4(),
            workflow_id,
            step_name,
            message,
            options: vec![
                CheckpointOption::Approve,
                CheckpointOption::ApproveWithChanges {
                    changes: String::new(),
                },
                CheckpointOption::Reject {
                    reason: String::new(),
                },
            ],
            timeout: timeout_seconds.map(|s| Duration::seconds(s as i64)),
            created_at: Utc::now(),
            expires_at: timeout_seconds.map(|s| Utc::now() + Duration::seconds(s as i64)),
        };

        self.notification_service.notify_checkpoint(&checkpoint)?;

        let mut checkpoints = self.pending_checkpoints.write().await;
        checkpoints.insert(checkpoint.id, checkpoint.clone());

        if let Some(mut state) = self.state_manager.get_workflow_state(&workflow_id).await? {
            state.status = super::WorkflowStatus::WaitingForCheckpoint;
            self.state_manager.update_workflow_state(&state).await?;
        }

        Ok(checkpoint)
    }

    pub async fn get_checkpoint(&self, checkpoint_id: &Uuid) -> Option<PendingCheckpoint> {
        let checkpoints = self.pending_checkpoints.read().await;
        checkpoints.get(checkpoint_id).cloned()
    }

    pub async fn respond_to_checkpoint(
        &self,
        checkpoint_id: &Uuid,
        response: CheckpointResponse,
    ) -> Result<()> {
        let mut checkpoints = self.pending_checkpoints.write().await;
        let checkpoint = checkpoints
            .remove(checkpoint_id)
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found"))?;

        if let Some(expires_at) = checkpoint.expires_at {
            if Utc::now() > expires_at {
                anyhow::bail!("Checkpoint has expired");
            }
        }

        if let Some(mut state) = self
            .state_manager
            .get_workflow_state(&checkpoint.workflow_id)
            .await?
        {
            state.status = super::WorkflowStatus::Running;
            state
                .variables
                .insert("checkpoint".to_string(), serde_json::to_value(&response)?);

            let event = super::ExecutionEvent {
                timestamp: Utc::now(),
                event_type: super::ExecutionEventType::CheckpointResolved,
                details: format!(
                    "Checkpoint resolved by {} with option: {:?}",
                    response.user, response.option
                ),
            };
            state.history.push(event);

            self.state_manager.update_workflow_state(&state).await?;
        }

        Ok(())
    }

    pub async fn list_pending_checkpoints(&self) -> Vec<PendingCheckpoint> {
        let checkpoints = self.pending_checkpoints.read().await;
        let mut list: Vec<_> = checkpoints.values().cloned().collect();
        list.sort_by_key(|c| c.created_at);
        list
    }

    pub async fn cleanup_expired_checkpoints(&self) -> Result<()> {
        let mut checkpoints = self.pending_checkpoints.write().await;
        let now = Utc::now();

        let expired_ids: Vec<_> = checkpoints
            .iter()
            .filter_map(|(id, checkpoint)| {
                if let Some(expires_at) = checkpoint.expires_at {
                    if now > expires_at {
                        return Some(*id);
                    }
                }
                None
            })
            .collect();

        for id in expired_ids {
            if let Some(checkpoint) = checkpoints.remove(&id) {
                if let Some(mut state) = self
                    .state_manager
                    .get_workflow_state(&checkpoint.workflow_id)
                    .await?
                {
                    state.status = super::WorkflowStatus::Failed;
                    state.completed_at = Some(now);

                    let event = super::ExecutionEvent {
                        timestamp: now,
                        event_type: super::ExecutionEventType::CheckpointResolved,
                        details: "Checkpoint expired".to_string(),
                    };
                    state.history.push(event);

                    self.state_manager.update_workflow_state(&state).await?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockNotificationService;

    impl NotificationService for MockNotificationService {
        fn notify_checkpoint(&self, _checkpoint: &PendingCheckpoint) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_checkpoint_lifecycle() {
        // This would require a test database setup
        // For now, just ensure the code compiles
    }
}
