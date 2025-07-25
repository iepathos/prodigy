use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvent {
    SpecAdded {
        path: PathBuf,
    },
    SpecModified {
        path: PathBuf,
    },
    TestsFailed {
        spec: String,
    },
    ReviewRequested {
        spec: String,
    },
    DeploymentReady {
        version: String,
    },
    WorkflowStarted {
        workflow_id: Uuid,
        workflow_name: String,
    },
    WorkflowCompleted {
        workflow_id: Uuid,
        workflow_name: String,
    },
    WorkflowFailed {
        workflow_id: Uuid,
        workflow_name: String,
        error: String,
    },
    StageStarted {
        workflow_id: Uuid,
        stage_name: String,
    },
    StageCompleted {
        workflow_id: Uuid,
        stage_name: String,
    },
    Custom {
        name: String,
        data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub event_type: Option<String>,
    pub path_pattern: Option<String>,
    pub custom_filter: Option<String>,
}

impl EventFilter {
    pub fn matches(&self, event: &WorkflowEvent) -> bool {
        if let Some(event_type) = &self.event_type {
            let actual_type = match event {
                WorkflowEvent::SpecAdded { .. } => "spec_added",
                WorkflowEvent::SpecModified { .. } => "spec_modified",
                WorkflowEvent::TestsFailed { .. } => "tests_failed",
                WorkflowEvent::ReviewRequested { .. } => "review_requested",
                WorkflowEvent::DeploymentReady { .. } => "deployment_ready",
                WorkflowEvent::WorkflowStarted { .. } => "workflow_started",
                WorkflowEvent::WorkflowCompleted { .. } => "workflow_completed",
                WorkflowEvent::WorkflowFailed { .. } => "workflow_failed",
                WorkflowEvent::StageStarted { .. } => "stage_started",
                WorkflowEvent::StageCompleted { .. } => "stage_completed",
                WorkflowEvent::Custom { name, .. } => name,
            };

            if actual_type != event_type {
                return false;
            }
        }

        if let Some(path_pattern) = &self.path_pattern {
            match event {
                WorkflowEvent::SpecAdded { path } | WorkflowEvent::SpecModified { path } => {
                    let glob = glob::Pattern::new(path_pattern).ok();
                    if let Some(glob) = glob {
                        if !glob.matches_path(path) {
                            return false;
                        }
                    }
                }
                _ => {}
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
pub struct EventTrigger {
    pub id: Uuid,
    pub workflow_name: String,
    pub event_filter: EventFilter,
    pub parameters: HashMap<String, serde_json::Value>,
    pub enabled: bool,
}

pub struct EventBus {
    sender: broadcast::Sender<WorkflowEvent>,
    triggers: Arc<RwLock<Vec<EventTrigger>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self {
            sender,
            triggers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn emit(&self, event: WorkflowEvent) -> Result<()> {
        self.sender
            .send(event.clone())
            .map_err(|_| anyhow::anyhow!("Failed to send event"))?;

        let triggers = self.triggers.read().await;
        let matching_triggers: Vec<_> = triggers
            .iter()
            .filter(|t| t.enabled && t.event_filter.matches(&event))
            .cloned()
            .collect();

        drop(triggers);

        for trigger in matching_triggers {
            self.handle_trigger(trigger, &event).await?;
        }

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WorkflowEvent> {
        self.sender.subscribe()
    }

    pub async fn register_trigger(&self, trigger: EventTrigger) -> Result<()> {
        let mut triggers = self.triggers.write().await;
        triggers.push(trigger);
        Ok(())
    }

    pub async fn unregister_trigger(&self, trigger_id: &Uuid) -> Result<()> {
        let mut triggers = self.triggers.write().await;
        triggers.retain(|t| t.id != *trigger_id);
        Ok(())
    }

    pub async fn list_triggers(&self) -> Vec<EventTrigger> {
        let triggers = self.triggers.read().await;
        triggers.clone()
    }

    pub async fn enable_trigger(&self, trigger_id: &Uuid, enabled: bool) -> Result<()> {
        let mut triggers = self.triggers.write().await;
        if let Some(trigger) = triggers.iter_mut().find(|t| t.id == *trigger_id) {
            trigger.enabled = enabled;
        }
        Ok(())
    }

    async fn handle_trigger(&self, trigger: EventTrigger, event: &WorkflowEvent) -> Result<()> {
        println!(
            "🎯 Event {:?} triggered workflow '{}'",
            event, trigger.workflow_name
        );

        // In a real implementation, this would start the workflow
        // For now, we just log it

        Ok(())
    }
}

pub struct EventLogger {
    file_path: Option<PathBuf>,
}

impl EventLogger {
    pub fn new(file_path: Option<PathBuf>) -> Self {
        Self { file_path }
    }

    pub async fn log_event(&self, event: &WorkflowEvent) -> Result<()> {
        let timestamp = Utc::now();
        let log_entry = serde_json::json!({
            "timestamp": timestamp,
            "event": event,
        });

        if let Some(path) = &self.file_path {
            let content = serde_json::to_string_pretty(&log_entry)?;
            tokio::fs::write(path, content)
                .await
                .context("Failed to write event log")?;
        } else {
            println!("📝 Event: {}", serde_json::to_string(&log_entry)?);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_filter_matching() {
        let filter = EventFilter {
            event_type: Some("spec_added".to_string()),
            path_pattern: Some("specs/features/*.md".to_string()),
            custom_filter: None,
        };

        let matching_event = WorkflowEvent::SpecAdded {
            path: PathBuf::from("specs/features/auth.md"),
        };
        assert!(filter.matches(&matching_event));

        let non_matching_event = WorkflowEvent::SpecAdded {
            path: PathBuf::from("specs/other/test.md"),
        };
        assert!(!filter.matches(&non_matching_event));

        let wrong_type_event = WorkflowEvent::SpecModified {
            path: PathBuf::from("specs/features/auth.md"),
        };
        assert!(!filter.matches(&wrong_type_event));
    }

    #[tokio::test]
    async fn test_event_bus() {
        let event_bus = EventBus::new();
        let mut receiver = event_bus.subscribe();

        let trigger = EventTrigger {
            id: Uuid::new_v4(),
            workflow_name: "test-workflow".to_string(),
            event_filter: EventFilter {
                event_type: Some("spec_added".to_string()),
                path_pattern: None,
                custom_filter: None,
            },
            parameters: HashMap::new(),
            enabled: true,
        };

        event_bus.register_trigger(trigger).await.unwrap();

        let event = WorkflowEvent::SpecAdded {
            path: PathBuf::from("test.md"),
        };
        event_bus.emit(event.clone()).await.unwrap();

        let received = receiver.recv().await.unwrap();
        match (event, received) {
            (WorkflowEvent::SpecAdded { path: p1 }, WorkflowEvent::SpecAdded { path: p2 }) => {
                assert_eq!(p1, p2);
            }
            _ => panic!("Events don't match"),
        }
    }
}
