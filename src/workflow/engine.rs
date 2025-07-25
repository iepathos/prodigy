use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
    checkpoint::{CheckpointManager, ConsoleNotificationService},
    event::{EventBus, WorkflowEvent},
    executor::{SequentialExecutor, WorkflowExecutor},
    parser::WorkflowParser,
    state::WorkflowStateManager,
    Workflow, WorkflowContext, WorkflowResult, WorkflowStatus,
};

pub struct WorkflowEngine {
    pub parser: WorkflowParser,
    executor: Box<dyn WorkflowExecutor>,
    state_manager: Arc<WorkflowStateManager>,
    pub checkpoint_manager: Arc<CheckpointManager>,
    event_bus: Arc<EventBus>,
    dry_run: bool,
}

impl WorkflowEngine {
    pub fn new(state_manager: Arc<WorkflowStateManager>, event_bus: Arc<EventBus>) -> Self {
        let checkpoint_manager = Arc::new(CheckpointManager::new(
            Box::new(ConsoleNotificationService),
            state_manager.clone(),
        ));

        Self {
            parser: WorkflowParser::new(),
            executor: Box::new(SequentialExecutor::new()),
            state_manager,
            checkpoint_manager,
            event_bus,
            dry_run: false,
        }
    }

    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
    }

    pub async fn run_workflow(
        &self,
        workflow_path: &str,
        spec_id: Option<&str>,
        parameters: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<WorkflowResult> {
        let workflow = self.parser.parse_file(workflow_path)?;

        self.event_bus
            .emit(WorkflowEvent::WorkflowStarted {
                workflow_id: Uuid::new_v4(),
                workflow_name: workflow.name.clone(),
            })
            .await?;

        if self.dry_run {
            return self.dry_run_workflow(&workflow).await;
        }

        let mut context = WorkflowContext {
            workflow: workflow.clone(),
            spec_id: spec_id.map(String::from),
            parameters,
            variables: std::collections::HashMap::new(),
            outputs: std::collections::HashMap::new(),
            project: self.load_project_config()?,
        };

        let workflow_state = self
            .state_manager
            .create_workflow_state(&workflow.name, spec_id)
            .await?;

        let result = match self.executor.execute(&workflow, &mut context).await {
            Ok(result) => {
                self.event_bus
                    .emit(WorkflowEvent::WorkflowCompleted {
                        workflow_id: workflow_state.workflow_id,
                        workflow_name: workflow.name.clone(),
                    })
                    .await?;
                result
            }
            Err(e) => {
                self.event_bus
                    .emit(WorkflowEvent::WorkflowFailed {
                        workflow_id: workflow_state.workflow_id,
                        workflow_name: workflow.name.clone(),
                        error: e.to_string(),
                    })
                    .await?;
                return Err(e);
            }
        };

        let mut final_state = workflow_state;
        final_state.status = result.status.clone();
        final_state.outputs = result.outputs.clone();
        final_state.completed_at = Some(chrono::Utc::now());

        self.state_manager
            .update_workflow_state(&final_state)
            .await?;

        Ok(result)
    }

    async fn dry_run_workflow(&self, workflow: &Workflow) -> Result<WorkflowResult> {
        println!("🏃 Dry-run mode: Simulating workflow '{}'", workflow.name);
        println!("📋 Stages:");

        for (i, stage) in workflow.stages.iter().enumerate() {
            println!(
                "  {}. {} {}",
                i + 1,
                stage.name,
                if stage.parallel.unwrap_or(false) {
                    "(parallel)"
                } else {
                    ""
                }
            );

            for (j, step) in stage.steps.iter().enumerate() {
                println!("     {}. {}", j + 1, step.name);
                if let Some(command) = &step.command {
                    println!("        Command: {}", command);
                }
                if let Some(condition) = &step.condition {
                    println!("        Condition: {}", condition);
                }
            }
        }

        Ok(WorkflowResult {
            status: WorkflowStatus::Completed,
            outputs: std::collections::HashMap::new(),
            duration: std::time::Duration::from_secs(0),
            error: None,
        })
    }

    fn load_project_config(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut config = std::collections::HashMap::new();

        config.insert("test_command".to_string(), "cargo test".to_string());
        config.insert(
            "lint_command".to_string(),
            "cargo fmt -- --check".to_string(),
        );
        config.insert("build_command".to_string(), "cargo build".to_string());

        Ok(config)
    }

    pub async fn pause_workflow(&self, workflow_id: &Uuid) -> Result<()> {
        if let Some(mut state) = self.state_manager.get_workflow_state(workflow_id).await? {
            state.status = WorkflowStatus::Paused;
            self.state_manager.update_workflow_state(&state).await?;
        }
        Ok(())
    }

    pub async fn resume_workflow(&self, workflow_id: &Uuid) -> Result<()> {
        if let Some(mut state) = self.state_manager.get_workflow_state(workflow_id).await? {
            if state.status == WorkflowStatus::Paused {
                state.status = WorkflowStatus::Running;
                self.state_manager.update_workflow_state(&state).await?;
            }
        }
        Ok(())
    }

    pub async fn cancel_workflow(&self, workflow_id: &Uuid) -> Result<()> {
        if let Some(mut state) = self.state_manager.get_workflow_state(workflow_id).await? {
            state.status = WorkflowStatus::Cancelled;
            state.completed_at = Some(chrono::Utc::now());
            self.state_manager.update_workflow_state(&state).await?;
        }
        Ok(())
    }
}

pub struct WorkflowDebugger {
    engine: WorkflowEngine,
    breakpoints: Arc<RwLock<Vec<String>>>,
}

impl WorkflowDebugger {
    pub fn new(engine: WorkflowEngine) -> Self {
        Self {
            engine,
            breakpoints: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn add_breakpoint(&self, step_name: String) {
        let mut breakpoints = self.breakpoints.write().await;
        breakpoints.push(step_name);
    }

    pub async fn remove_breakpoint(&self, step_name: &str) {
        let mut breakpoints = self.breakpoints.write().await;
        breakpoints.retain(|bp| bp != step_name);
    }

    pub async fn list_breakpoints(&self) -> Vec<String> {
        let breakpoints = self.breakpoints.read().await;
        breakpoints.clone()
    }

    pub async fn step_through(&self, _workflow_path: &str) -> Result<()> {
        println!("🐛 Debug mode: Step-by-step execution");

        Ok(())
    }
}
