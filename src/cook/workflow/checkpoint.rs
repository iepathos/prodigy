//! Workflow checkpoint management for resume capability
//!
//! Provides checkpoint creation, persistence, and restoration for workflow execution.

use crate::cook::workflow::checkpoint_path::CheckpointStorage;
use crate::cook::workflow::executor::WorkflowContext;
use crate::cook::workflow::normalized::NormalizedWorkflow;
use crate::cook::workflow::variable_checkpoint::VariableCheckpointState;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tracing::{debug, info, warn};

/// Checkpoint interval default (60 seconds)
const DEFAULT_CHECKPOINT_INTERVAL: Duration = Duration::from_secs(60);

/// Version for checkpoint format compatibility
pub const CHECKPOINT_VERSION: u32 = 1;

/// Complete workflow checkpoint for resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Unique workflow execution ID
    pub workflow_id: String,
    /// Current execution state
    pub execution_state: ExecutionState,
    /// Completed steps with results
    pub completed_steps: Vec<CompletedStep>,
    /// Variable state for interpolation
    pub variable_state: HashMap<String, Value>,
    /// MapReduce state if applicable
    pub mapreduce_state: Option<MapReduceCheckpoint>,
    /// Timestamp of checkpoint
    pub timestamp: DateTime<Utc>,
    /// Checkpoint format version
    pub version: u32,
    /// Hash of original workflow for validation
    pub workflow_hash: String,
    /// Total number of steps in workflow
    pub total_steps: usize,
    /// Workflow name for reference
    pub workflow_name: Option<String>,
    /// Path to workflow file for resume
    pub workflow_path: Option<PathBuf>,
    /// Error recovery state (stored in variable_state as __error_recovery_state)
    #[serde(skip)]
    pub error_recovery_state: Option<crate::cook::workflow::error_recovery::ErrorRecoveryState>,
    /// Enhanced retry state for comprehensive persistence
    pub retry_checkpoint_state: Option<crate::cook::retry_state::RetryCheckpointState>,
    /// Enhanced variable checkpoint state for comprehensive variable persistence
    pub variable_checkpoint_state: Option<VariableCheckpointState>,
}

/// Current state of workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    /// Index of current step being executed
    pub current_step_index: usize,
    /// Total number of steps
    pub total_steps: usize,
    /// Current workflow status
    pub status: WorkflowStatus,
    /// When execution started
    pub start_time: DateTime<Utc>,
    /// Last checkpoint timestamp
    pub last_checkpoint: DateTime<Utc>,
    /// Current iteration for iterative workflows
    pub current_iteration: Option<usize>,
    /// Total iterations for iterative workflows
    pub total_iterations: Option<usize>,
}

/// Workflow execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    /// Workflow is running
    Running,
    /// Workflow is paused
    Paused,
    /// Workflow completed successfully
    Completed,
    /// Workflow failed
    Failed,
    /// Workflow was interrupted
    Interrupted,
}

/// Record of a completed workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedStep {
    /// Step index in workflow
    pub step_index: usize,
    /// Command that was executed
    pub command: String,
    /// Whether step succeeded
    pub success: bool,
    /// Captured output if any
    pub output: Option<String>,
    /// Variables captured from this step
    pub captured_variables: HashMap<String, String>,
    /// Duration of execution
    pub duration: Duration,
    /// Timestamp when completed
    pub completed_at: DateTime<Utc>,
    /// Retry state if this step is being retried
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_state: Option<RetryState>,
}

/// State of a step being retried
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryState {
    /// Current attempt number (1-based)
    pub current_attempt: usize,
    /// Maximum attempts allowed
    pub max_attempts: usize,
    /// Failure reasons from each attempt
    pub failure_history: Vec<String>,
    /// Whether currently in retry loop
    pub in_retry_loop: bool,
}

/// MapReduce job checkpoint state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceCheckpoint {
    /// Items that have been completed
    pub completed_items: HashSet<String>,
    /// Items that failed
    pub failed_items: Vec<String>,
    /// Items currently being processed
    pub in_progress_items: HashMap<String, AgentState>,
    /// Whether reduce phase completed
    pub reduce_completed: bool,
    /// Results from completed agents
    pub agent_results: HashMap<String, Value>,
    /// Total number of original items
    pub total_items: usize,
    /// MapReduce aggregate variables (map.successful, map.failed, etc.)
    pub aggregate_variables: HashMap<String, String>,
}

/// State of an agent processing an item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Agent ID
    pub agent_id: String,
    /// Item being processed
    pub item_id: String,
    /// When processing started
    pub started_at: DateTime<Utc>,
    /// Last update time
    pub last_update: DateTime<Utc>,
}

/// Context for resuming workflow execution
#[derive(Debug, Clone)]
pub struct ResumeContext {
    /// Steps to skip (already completed)
    pub skip_steps: Vec<CompletedStep>,
    /// Variable state to restore
    pub variable_state: HashMap<String, Value>,
    /// MapReduce state if applicable
    pub mapreduce_state: Option<MapReduceCheckpoint>,
    /// Starting step index
    pub start_from_step: usize,
    /// Iteration to resume from
    pub resume_iteration: Option<usize>,
    /// Original checkpoint for reference
    pub checkpoint: Option<Box<WorkflowCheckpoint>>,
}

/// Options for resuming workflow
#[derive(Debug, Clone, Default)]
pub struct ResumeOptions {
    /// Force resume even if marked complete
    pub force: bool,
    /// Resume from specific step
    pub from_step: Option<usize>,
    /// Reset failed items for retry
    pub reset_failures: bool,
    /// Skip validation of workflow compatibility
    pub skip_validation: bool,
}

/// Manager for workflow checkpoints
pub struct CheckpointManager {
    /// Checkpoint storage strategy
    storage: CheckpointStorage,
    /// Checkpoint interval
    checkpoint_interval: Duration,
    /// Whether checkpointing is enabled
    enabled: bool,
}

impl CheckpointManager {
    /// Create a new checkpoint manager with explicit storage strategy
    pub fn with_storage(storage: CheckpointStorage) -> Self {
        Self {
            storage,
            checkpoint_interval: DEFAULT_CHECKPOINT_INTERVAL,
            enabled: true,
        }
    }

    /// Create a new checkpoint manager (deprecated - use with_storage)
    ///
    /// This constructor is maintained for backwards compatibility but is deprecated.
    /// New code should use `with_storage()` with an explicit CheckpointStorage strategy.
    #[deprecated(
        since = "0.1.0",
        note = "Use CheckpointManager::with_storage() with explicit storage strategy instead"
    )]
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            storage: CheckpointStorage::Local(storage_path),
            checkpoint_interval: DEFAULT_CHECKPOINT_INTERVAL,
            enabled: true,
        }
    }

    /// Configure checkpoint settings
    pub fn configure(&mut self, interval: Duration, enabled: bool) {
        self.checkpoint_interval = interval;
        self.enabled = enabled;
    }

    /// Save a checkpoint for the workflow
    pub async fn save_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let checkpoint_path = self
            .storage
            .checkpoint_file_path(&checkpoint.workflow_id)
            .context("Failed to resolve checkpoint path")?;
        let temp_path = checkpoint_path.with_extension("tmp");

        // Ensure directory exists
        if let Some(parent) = checkpoint_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create checkpoint directory")?;
        }

        // Write to temp file first
        let json = serde_json::to_string_pretty(checkpoint)?;
        fs::write(&temp_path, json)
            .await
            .context("Failed to write checkpoint to temp file")?;

        // Atomic rename
        fs::rename(temp_path, &checkpoint_path)
            .await
            .context("Failed to move checkpoint to final location")?;

        info!(
            "Saved checkpoint for workflow {} at step {}",
            checkpoint.workflow_id, checkpoint.execution_state.current_step_index
        );

        Ok(())
    }

    /// Save an intervention request to checkpoint metadata
    pub async fn save_intervention_request(&self, workflow_id: &str, message: &str) -> Result<()> {
        // Load existing checkpoint
        let mut checkpoint = self.load_checkpoint(workflow_id).await?;

        // Add intervention request to variable_state (used as metadata storage)
        checkpoint.variable_state.insert(
            "__intervention_required".to_string(),
            serde_json::Value::String(message.to_string()),
        );
        checkpoint.variable_state.insert(
            "__intervention_timestamp".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );

        // Save updated checkpoint
        self.save_checkpoint(&checkpoint).await?;

        info!(
            "Saved intervention request for workflow {}: {}",
            workflow_id, message
        );

        Ok(())
    }

    /// Load a checkpoint for resuming
    pub async fn load_checkpoint(&self, workflow_id: &str) -> Result<WorkflowCheckpoint> {
        let checkpoint_path = self
            .storage
            .checkpoint_file_path(workflow_id)
            .context("Failed to resolve checkpoint path")?;

        let content = fs::read_to_string(&checkpoint_path)
            .await
            .with_context(|| {
                format!(
                    "No checkpoint found for workflow {} at path {}",
                    workflow_id,
                    checkpoint_path.display()
                )
            })?;

        let checkpoint: WorkflowCheckpoint = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse checkpoint for workflow {}", workflow_id))?;

        // Validate version compatibility
        if checkpoint.version > CHECKPOINT_VERSION {
            return Err(anyhow!(
                "Checkpoint version {} is newer than supported version {}",
                checkpoint.version,
                CHECKPOINT_VERSION
            ));
        }

        Ok(checkpoint)
    }

    /// Check if an auto-checkpoint is needed
    pub async fn should_checkpoint(&self, last_checkpoint: DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }

        let elapsed = Utc::now().signed_duration_since(last_checkpoint);
        elapsed.num_seconds() as u64 >= self.checkpoint_interval.as_secs()
    }

    /// Delete a checkpoint after successful completion
    pub async fn delete_checkpoint(&self, workflow_id: &str) -> Result<()> {
        let checkpoint_path = self
            .storage
            .checkpoint_file_path(workflow_id)
            .context("Failed to resolve checkpoint path")?;
        if checkpoint_path.exists() {
            fs::remove_file(checkpoint_path)
                .await
                .context("Failed to delete checkpoint")?;
            debug!("Deleted checkpoint for completed workflow {}", workflow_id);
        }
        Ok(())
    }

    /// List all available checkpoints
    pub async fn list_checkpoints(&self) -> Result<Vec<String>> {
        let mut checkpoints = Vec::new();

        let base_dir = self
            .storage
            .resolve_base_dir()
            .context("Failed to resolve checkpoint base directory")?;

        if !base_dir.exists() {
            return Ok(checkpoints);
        }

        let mut entries = fs::read_dir(&base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".checkpoint.json") {
                    if let Some(workflow_id) = name.strip_suffix(".checkpoint.json") {
                        checkpoints.push(workflow_id.to_string());
                    }
                }
            }
        }

        Ok(checkpoints)
    }

    /// Validate checkpoint compatibility with current workflow
    pub fn validate_checkpoint(checkpoint: &WorkflowCheckpoint, workflow_hash: &str) -> Result<()> {
        // Check workflow hasn't changed incompatibly
        if checkpoint.workflow_hash != workflow_hash {
            warn!("Workflow has changed since checkpoint was created");
            // In future, could do more sophisticated compatibility checking
        }

        // Validate checkpoint integrity
        if checkpoint.execution_state.current_step_index > checkpoint.execution_state.total_steps {
            return Err(anyhow!("Invalid checkpoint: step index out of bounds"));
        }

        Ok(())
    }
}

/// Create a checkpoint from current workflow state
pub fn create_checkpoint(
    workflow_id: String,
    workflow: &NormalizedWorkflow,
    context: &WorkflowContext,
    completed_steps: Vec<CompletedStep>,
    current_step: usize,
    workflow_hash: String,
) -> WorkflowCheckpoint {
    create_checkpoint_with_total_steps(
        workflow_id,
        workflow,
        context,
        completed_steps,
        current_step,
        workflow_hash,
        workflow.steps.len(),
    )
}

/// Create a checkpoint from current workflow state with explicit total steps
pub fn create_checkpoint_with_total_steps(
    workflow_id: String,
    workflow: &NormalizedWorkflow,
    context: &WorkflowContext,
    completed_steps: Vec<CompletedStep>,
    current_step: usize,
    workflow_hash: String,
    total_steps: usize,
) -> WorkflowCheckpoint {
    // Convert WorkflowContext variables to Value map
    let mut variable_state = HashMap::new();
    for (key, value) in &context.variables {
        variable_state.insert(key.clone(), Value::String(value.clone()));
    }
    for (key, value) in &context.captured_outputs {
        variable_state.insert(key.clone(), Value::String(value.clone()));
    }

    // Create enhanced variable checkpoint state
    let variable_checkpoint_state = {
        use crate::cook::workflow::variable_checkpoint::VariableResumeManager;
        let manager = VariableResumeManager::new();
        manager
            .create_checkpoint(
                &context.variables,
                &context.captured_outputs,
                &context.iteration_vars,
                &context.variable_store,
            )
            .ok()
    };

    WorkflowCheckpoint {
        workflow_id,
        execution_state: ExecutionState {
            current_step_index: current_step,
            total_steps,
            status: WorkflowStatus::Running,
            start_time: Utc::now(),
            last_checkpoint: Utc::now(),
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps,
        variable_state,
        mapreduce_state: None,
        timestamp: Utc::now(),
        version: CHECKPOINT_VERSION,
        workflow_hash,
        total_steps,
        workflow_name: Some(workflow.name.to_string()),
        workflow_path: None,          // Will be set by the executor if available
        error_recovery_state: None,   // Will be set if error handlers are present
        retry_checkpoint_state: None, // Will be set by the executor if retry state exists
        variable_checkpoint_state,
    }
}

/// Build resume context from a checkpoint
pub fn build_resume_context(checkpoint: WorkflowCheckpoint) -> ResumeContext {
    let completed_steps = checkpoint.completed_steps.clone();
    let variable_state = checkpoint.variable_state.clone();
    let mapreduce_state = checkpoint.mapreduce_state.clone();
    let start_from_step = checkpoint.execution_state.current_step_index;
    let resume_iteration = checkpoint.execution_state.current_iteration;

    ResumeContext {
        skip_steps: completed_steps,
        variable_state,
        mapreduce_state,
        start_from_step,
        resume_iteration,
        checkpoint: Some(Box::new(checkpoint)),
    }
}
