//! Helper utilities for creating test checkpoints and fixtures

use anyhow::Result;
use chrono::Utc;
use prodigy::cook::workflow::checkpoint::{
    ExecutionState, WorkflowCheckpoint, WorkflowStatus, CHECKPOINT_VERSION,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// Builder for creating test checkpoints with customizable properties
pub struct CheckpointTestBuilder {
    workflow_id: String,
    workflow_path: Option<PathBuf>,
    current_step: usize,
    total_steps: usize,
    status: WorkflowStatus,
    completed_steps: Vec<prodigy::cook::workflow::checkpoint::CompletedStep>,
    variables: HashMap<String, String>,
    workflow_hash: String,
    workflow_name: Option<String>,
    version: u32,
}

impl CheckpointTestBuilder {
    /// Create a new checkpoint builder with default values
    pub fn new(workflow_id: impl Into<String>) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            workflow_path: None,
            current_step: 0,
            total_steps: 5,
            status: WorkflowStatus::Running,
            completed_steps: Vec::new(),
            variables: HashMap::new(),
            workflow_hash: "test-hash-123".to_string(),
            workflow_name: Some("test-workflow".to_string()),
            version: CHECKPOINT_VERSION,
        }
    }

    /// Set the workflow file path
    pub fn with_workflow_path(mut self, path: PathBuf) -> Self {
        self.workflow_path = Some(path);
        self
    }

    /// Set the current step index (for simulating partial completion)
    pub fn at_step(mut self, step: usize) -> Self {
        self.current_step = step;
        self
    }

    /// Set the total number of steps
    pub fn with_total_steps(mut self, total: usize) -> Self {
        self.total_steps = total;
        self
    }

    /// Set the workflow status
    pub fn with_status(mut self, status: WorkflowStatus) -> Self {
        self.status = status;
        self
    }

    /// Add a completed step
    pub fn with_completed_step(
        mut self,
        step_index: usize,
        command: impl Into<String>,
        success: bool,
    ) -> Self {
        self.completed_steps
            .push(prodigy::cook::workflow::checkpoint::CompletedStep {
                step_index,
                command: command.into(),
                success,
                timestamp: Utc::now(),
                output: Some("test output".to_string()),
                commits: Vec::new(),
            });
        self
    }

    /// Add a variable to the state
    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }

    /// Set the workflow hash
    pub fn with_workflow_hash(mut self, hash: impl Into<String>) -> Self {
        self.workflow_hash = hash.into();
        self
    }

    /// Set the checkpoint version (for testing version compatibility)
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Build the checkpoint
    pub fn build(self) -> WorkflowCheckpoint {
        WorkflowCheckpoint {
            workflow_id: self.workflow_id,
            workflow_path: self.workflow_path,
            execution_state: ExecutionState {
                current_step_index: self.current_step,
                total_steps: self.total_steps,
                status: self.status,
                start_time: Utc::now(),
                last_checkpoint: Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: self.completed_steps,
            variable_state: self.variables,
            mapreduce_state: None,
            timestamp: Utc::now(),
            variable_checkpoint_state: None,
            version: self.version,
            workflow_hash: self.workflow_hash,
            total_steps: self.total_steps,
            workflow_name: self.workflow_name,
        }
    }
}

/// Corrupt a checkpoint file by writing invalid JSON
pub async fn corrupt_checkpoint_file(checkpoint_path: &PathBuf) -> Result<()> {
    tokio::fs::write(checkpoint_path, b"{ invalid json ").await?;
    Ok(())
}

/// Create a checkpoint file with a future version number
pub async fn create_future_version_checkpoint(
    checkpoint_path: &PathBuf,
    workflow_id: impl Into<String>,
) -> Result<()> {
    let mut checkpoint = CheckpointTestBuilder::new(workflow_id)
        .with_version(CHECKPOINT_VERSION + 100)
        .build();

    // Manually set version after build
    checkpoint.version = CHECKPOINT_VERSION + 100;

    let json = serde_json::to_string_pretty(&checkpoint)?;
    tokio::fs::write(checkpoint_path, json).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_builder_defaults() {
        let checkpoint = CheckpointTestBuilder::new("test-workflow").build();

        assert_eq!(checkpoint.workflow_id, "test-workflow");
        assert_eq!(checkpoint.execution_state.current_step_index, 0);
        assert_eq!(checkpoint.execution_state.total_steps, 5);
        assert!(matches!(
            checkpoint.execution_state.status,
            WorkflowStatus::Running
        ));
    }

    #[test]
    fn test_checkpoint_builder_customization() {
        let checkpoint = CheckpointTestBuilder::new("custom-workflow")
            .at_step(2)
            .with_total_steps(10)
            .with_variable("key", "value")
            .with_workflow_hash("custom-hash")
            .build();

        assert_eq!(checkpoint.execution_state.current_step_index, 2);
        assert_eq!(checkpoint.execution_state.total_steps, 10);
        assert_eq!(checkpoint.variable_state.get("key"), Some(&"value".to_string()));
        assert_eq!(checkpoint.workflow_hash, "custom-hash");
    }
}
