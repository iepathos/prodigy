use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub mod checkpoint;
pub mod condition;
pub mod engine;
pub mod event;
pub mod executor;
pub mod parser;
pub mod state;
pub mod template;

pub use checkpoint::CheckpointManager;
pub use condition::ConditionEvaluator;
pub use engine::WorkflowEngine;
pub use event::{EventBus, WorkflowEvent};
pub use executor::{ParallelExecutor, SequentialExecutor, WorkflowExecutor};
pub use parser::WorkflowParser;
pub use state::WorkflowStateManager;
pub use template::TemplateResolver;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub triggers: Vec<Trigger>,
    pub parameters: HashMap<String, Parameter>,
    pub stages: Vec<Stage>,
    pub on_success: Vec<Action>,
    pub on_failure: Vec<Action>,
    pub extends: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    #[serde(rename = "type")]
    pub trigger_type: TriggerType,
    pub filter: Option<String>,
    pub cron: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    SpecAdded,
    SpecModified,
    Manual,
    Schedule,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    #[serde(rename = "type")]
    pub param_type: ParameterType,
    pub default: Option<serde_json::Value>,
    pub description: Option<String>,
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterType {
    Boolean,
    String,
    Integer,
    Float,
    Array,
    Object,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub condition: Option<String>,
    pub parallel: Option<bool>,
    pub for_each: Option<String>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    #[serde(rename = "type")]
    pub step_type: Option<StepType>,
    pub command: Option<String>,
    pub condition: Option<String>,
    pub outputs: Option<Vec<String>>,
    pub on_failure: Option<FailureStrategy>,
    pub max_retries: Option<u32>,
    pub timeout: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Command,
    Checkpoint,
    Parallel,
    Sequential,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FailureStrategy {
    Simple(String),
    Complex {
        command: Option<String>,
        retry: Option<u32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(rename = "type")]
    pub action_type: ActionType,
    pub message: Option<String>,
    pub workflow: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Notification,
    TriggerWorkflow,
    CreateIssue,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowContext {
    pub workflow: Workflow,
    pub spec_id: Option<String>,
    pub parameters: HashMap<String, serde_json::Value>,
    pub variables: HashMap<String, serde_json::Value>,
    pub outputs: HashMap<String, serde_json::Value>,
    pub project: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub workflow_id: Uuid,
    pub spec_id: Option<String>,
    pub status: WorkflowStatus,
    pub current_stage: Option<String>,
    pub current_step: Option<String>,
    pub variables: HashMap<String, serde_json::Value>,
    pub outputs: HashMap<String, serde_json::Value>,
    pub history: Vec<ExecutionEvent>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Pending,
    Running,
    Paused,
    WaitingForCheckpoint,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: ExecutionEventType,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEventType {
    StageStarted,
    StageCompleted,
    StageFailed,
    StepStarted,
    StepCompleted,
    StepFailed,
    CheckpointReached,
    CheckpointResolved,
    VariableSet,
    OutputProduced,
}

#[derive(Debug, Clone)]
pub struct WorkflowResult {
    pub status: WorkflowStatus,
    pub outputs: HashMap<String, serde_json::Value>,
    pub duration: std::time::Duration,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StageResult {
    pub name: String,
    pub status: WorkflowStatus,
    pub outputs: HashMap<String, serde_json::Value>,
    pub steps_completed: usize,
    pub steps_total: usize,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: WorkflowStatus,
    pub outputs: HashMap<String, serde_json::Value>,
    pub error: Option<String>,
    pub retry_count: u32,
}
