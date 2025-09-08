//! Session state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Status of a cooking session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    /// Session is actively running
    InProgress,
    /// Session completed successfully
    Completed,
    /// Session failed with error
    Failed,
    /// Session was interrupted
    Interrupted,
}

/// Type of workflow being executed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowType {
    /// Standard single-run workflow
    Standard,
    /// Iterative workflow with multiple runs
    Iterative,
    /// Structured workflow with outputs
    StructuredWithOutputs,
    /// MapReduce parallel workflow
    MapReduce,
}

/// State of a cooking session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Unique session identifier
    pub session_id: String,
    /// Current status
    pub status: SessionStatus,
    /// When session started
    pub started_at: DateTime<Utc>,
    /// When session ended (if applicable)
    pub ended_at: Option<DateTime<Utc>>,
    /// Number of iterations completed
    pub iterations_completed: usize,
    /// Total files changed
    pub files_changed: usize,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// Working directory
    pub working_directory: PathBuf,
    /// Worktree name if using worktree
    pub worktree_name: Option<String>,
    /// When workflow started
    pub workflow_started_at: Option<DateTime<Utc>>,
    /// Current iteration start time
    pub current_iteration_started_at: Option<DateTime<Utc>>,
    /// Current iteration number
    pub current_iteration_number: Option<u32>,
    /// Iteration timings (iteration number, duration)
    pub iteration_timings: Vec<(u32, Duration)>,
    /// Command timings (command name, duration)
    pub command_timings: Vec<(String, Duration)>,
    /// Workflow execution state for resuming
    pub workflow_state: Option<WorkflowState>,
    /// Execution environment for resuming
    pub execution_environment: Option<ExecutionEnvironment>,
    /// Last checkpoint timestamp
    pub last_checkpoint: Option<DateTime<Utc>>,
    /// Hash of the workflow configuration for validation
    pub workflow_hash: Option<String>,
    /// Type of workflow being executed
    pub workflow_type: Option<WorkflowType>,
    /// Execution context with variables and outputs
    pub execution_context: Option<ExecutionContext>,
    /// Checkpoint version for compatibility
    pub checkpoint_version: u32,
    /// Last time the checkpoint was validated
    pub last_validated_at: Option<DateTime<Utc>>,
}

/// State of workflow execution for resume capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    /// Current iteration index (for multi-iteration workflows)
    pub current_iteration: usize,
    /// Current step index within the workflow
    pub current_step: usize,
    /// Completed steps with their results
    pub completed_steps: Vec<StepResult>,
    /// Workflow file path
    pub workflow_path: PathBuf,
    /// Input arguments provided
    pub input_args: Vec<String>,
    /// Map patterns provided
    pub map_patterns: Vec<String>,
    /// Whether worktree was being used
    pub using_worktree: bool,
}

/// Result of a completed workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step index
    pub step_index: usize,
    /// Command that was executed
    pub command: String,
    /// Whether the step succeeded
    pub success: bool,
    /// Output from the command (if captured)
    pub output: Option<String>,
    /// Time taken to execute
    pub duration: Duration,
    /// Error message if step failed
    pub error: Option<String>,
    /// When step started
    pub started_at: DateTime<Utc>,
    /// When step completed
    pub completed_at: DateTime<Utc>,
    /// Exit code from the command
    pub exit_code: Option<i32>,
}

/// Execution context for variable interpolation and outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Variables for interpolation
    pub variables: HashMap<String, String>,
    /// Captured step outputs indexed by step number
    pub step_outputs: HashMap<usize, String>,
    /// Environment variables
    pub environment: HashMap<String, String>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            step_outputs: HashMap::new(),
            environment: HashMap::new(),
        }
    }

    /// Restore context from workflow state
    pub fn restore_from_state(workflow_state: &WorkflowState) -> Self {
        let mut context = Self::new();
        for step in &workflow_state.completed_steps {
            if let Some(ref output) = step.output {
                context.step_outputs.insert(step.step_index, output.clone());
                context.variables.insert(
                    format!("step_{}_output", step.step_index),
                    output.clone(),
                );
            }
        }
        context
    }
}

/// Execution environment for resuming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEnvironment {
    /// Working directory path
    pub working_directory: PathBuf,
    /// Worktree name if using worktree
    pub worktree_name: Option<String>,
    /// Environment variables
    pub environment_vars: HashMap<String, String>,
    /// Original command arguments
    pub command_args: Vec<String>,
}

impl SessionState {
    /// Create a new session state
    pub fn new(session_id: String, working_directory: PathBuf) -> Self {
        Self {
            session_id,
            status: SessionStatus::InProgress,
            started_at: Utc::now(),
            ended_at: None,
            iterations_completed: 0,
            files_changed: 0,
            errors: Vec::new(),
            working_directory,
            worktree_name: None,
            workflow_started_at: None,
            current_iteration_started_at: None,
            current_iteration_number: None,
            iteration_timings: Vec::new(),
            command_timings: Vec::new(),
            workflow_state: None,
            execution_environment: None,
            last_checkpoint: None,
            workflow_hash: None,
            workflow_type: None,
            execution_context: None,
            checkpoint_version: 1,
            last_validated_at: None,
        }
    }

    /// Mark session as completed
    pub fn complete(&mut self) {
        self.status = SessionStatus::Completed;
        self.ended_at = Some(Utc::now());
    }

    /// Mark session as failed
    pub fn fail(&mut self, error: String) {
        self.status = SessionStatus::Failed;
        self.ended_at = Some(Utc::now());
        self.errors.push(error);
    }

    /// Mark session as interrupted
    pub fn interrupt(&mut self) {
        self.status = SessionStatus::Interrupted;
        self.ended_at = Some(Utc::now());
    }

    /// Add files changed count
    pub fn add_files_changed(&mut self, count: usize) {
        self.files_changed += count;
    }

    /// Increment iteration count
    pub fn increment_iteration(&mut self) {
        self.iterations_completed += 1;
    }

    /// Get session duration
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.ended_at
            .map(|end| end.signed_duration_since(self.started_at))
    }

    /// Check if session is resumable
    pub fn is_resumable(&self) -> bool {
        matches!(
            self.status,
            SessionStatus::InProgress | SessionStatus::Interrupted
        ) && self.workflow_state.is_some()
    }

    /// Update workflow state for checkpoint
    pub fn update_workflow_state(&mut self, state: WorkflowState) {
        self.workflow_state = Some(state);
        self.last_checkpoint = Some(Utc::now());
    }

    /// Get resume information for display
    pub fn get_resume_info(&self) -> Option<String> {
        self.workflow_state.as_ref().map(|ws| {
            format!(
                "Step {}/{} in iteration {}",
                ws.current_step + 1,
                ws.completed_steps.len() + 1,
                ws.current_iteration + 1
            )
        })
    }
}
