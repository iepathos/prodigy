//! Pure resume planning functions for workflow resumption
//!
//! This module provides pure functions for planning workflow resume from checkpoints.
//! All functions are side-effect free and can be tested without I/O.

use crate::cook::workflow::checkpoint::{WorkflowCheckpoint, WorkflowStatus};

/// Plan for resuming workflow execution
#[derive(Debug, Clone)]
pub struct ResumePlan {
    /// Step index to start from
    pub start_index: usize,
    /// Whether to retry the failed step
    pub retry_failed: bool,
    /// Steps to skip (already completed)
    pub skip_indices: Vec<usize>,
    /// Whether to restore variables from checkpoint
    pub restore_variables: bool,
    /// Warnings about non-idempotent steps or other concerns
    pub warnings: Vec<String>,
}

/// Plan resume from checkpoint (pure function)
///
/// Determines where to resume execution based on checkpoint state.
/// This is a pure function with no I/O.
///
/// The function analyzes the checkpoint to determine:
/// - Which step to start from
/// - Whether to retry a failed step
/// - Which steps can be skipped (already completed)
/// - Any warnings about non-idempotent operations
pub fn plan_resume(checkpoint: &WorkflowCheckpoint) -> ResumePlan {
    let completed_indices: Vec<usize> = checkpoint
        .completed_steps
        .iter()
        .map(|s| s.step_index)
        .collect();

    let start_index = checkpoint.execution_state.current_step_index;

    let retry_failed = matches!(
        checkpoint.execution_state.status,
        WorkflowStatus::Failed | WorkflowStatus::Interrupted
    );

    let mut warnings = Vec::new();

    // Check for non-idempotent steps being resumed
    if retry_failed {
        // Note: In production, we'd check step metadata for idempotency
        warnings.push(format!(
            "Resuming from step {}. Verify step is safe to retry.",
            start_index
        ));
    }

    ResumePlan {
        start_index,
        retry_failed,
        skip_indices: completed_indices,
        restore_variables: true,
        warnings,
    }
}

/// Validate checkpoint is compatible with workflow
///
/// Checks that the checkpoint matches the current workflow definition.
/// Returns Ok if compatible, or an error message describing the incompatibility.
pub fn validate_checkpoint_compatibility(
    checkpoint: &WorkflowCheckpoint,
    workflow_hash: &str,
    total_steps: usize,
) -> Result<(), String> {
    if checkpoint.workflow_hash != workflow_hash {
        return Err(format!(
            "Workflow has changed since checkpoint (hash mismatch). \
             Checkpoint has {} steps, current workflow has {} steps.",
            checkpoint.total_steps, total_steps
        ));
    }

    if checkpoint.execution_state.current_step_index > total_steps {
        return Err(format!(
            "Checkpoint step index {} exceeds workflow steps {}",
            checkpoint.execution_state.current_step_index, total_steps
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::checkpoint::{CompletedStep, ExecutionState};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::time::Duration;

    fn create_test_checkpoint(
        current_step: usize,
        status: WorkflowStatus,
        completed: Vec<usize>,
        workflow_hash: &str,
        total_steps: usize,
    ) -> WorkflowCheckpoint {
        let completed_steps = completed
            .into_iter()
            .map(|idx| CompletedStep {
                step_index: idx,
                command: format!("step-{}", idx),
                success: true,
                output: None,
                captured_variables: HashMap::new(),
                duration: Duration::from_millis(100),
                completed_at: Utc::now(),
                retry_state: None,
            })
            .collect();

        WorkflowCheckpoint {
            workflow_id: "test-workflow".to_string(),
            execution_state: ExecutionState {
                current_step_index: current_step,
                total_steps,
                status,
                start_time: Utc::now(),
                last_checkpoint: Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps,
            variable_state: HashMap::new(),
            mapreduce_state: None,
            timestamp: Utc::now(),
            version: 1,
            workflow_hash: workflow_hash.to_string(),
            total_steps,
            workflow_name: Some("test".to_string()),
            workflow_path: None,
            error_recovery_state: None,
            retry_checkpoint_state: None,
            variable_checkpoint_state: None,
        }
    }

    #[test]
    fn test_plan_resume_from_failed() {
        let checkpoint = create_test_checkpoint(2, WorkflowStatus::Failed, vec![0, 1], "hash", 5);

        let plan = plan_resume(&checkpoint);

        assert_eq!(plan.start_index, 2);
        assert!(plan.retry_failed);
        assert_eq!(plan.skip_indices, vec![0, 1]);
        assert!(plan.restore_variables);
        assert!(!plan.warnings.is_empty());
    }

    #[test]
    fn test_plan_resume_from_interrupted() {
        let checkpoint =
            create_test_checkpoint(3, WorkflowStatus::Interrupted, vec![0, 1, 2], "hash", 5);

        let plan = plan_resume(&checkpoint);

        assert_eq!(plan.start_index, 3);
        assert!(plan.retry_failed);
        assert_eq!(plan.skip_indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_plan_resume_from_paused() {
        let checkpoint = create_test_checkpoint(1, WorkflowStatus::Paused, vec![0], "hash", 5);

        let plan = plan_resume(&checkpoint);

        assert_eq!(plan.start_index, 1);
        assert!(!plan.retry_failed);
        assert_eq!(plan.skip_indices, vec![0]);
    }

    #[test]
    fn test_plan_resume_no_completed_steps() {
        let checkpoint = create_test_checkpoint(0, WorkflowStatus::Failed, vec![], "hash", 5);

        let plan = plan_resume(&checkpoint);

        assert_eq!(plan.start_index, 0);
        assert!(plan.retry_failed);
        assert_eq!(plan.skip_indices, Vec::<usize>::new());
    }

    #[test]
    fn test_validate_checkpoint_compatibility_success() {
        let checkpoint = create_test_checkpoint(2, WorkflowStatus::Running, vec![0, 1], "hash", 5);

        let result = validate_checkpoint_compatibility(&checkpoint, "hash", 5);

        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_checkpoint_compatibility_hash_mismatch() {
        let checkpoint =
            create_test_checkpoint(2, WorkflowStatus::Running, vec![0, 1], "old-hash", 5);

        let result = validate_checkpoint_compatibility(&checkpoint, "new-hash", 5);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("hash mismatch"));
    }

    #[test]
    fn test_validate_checkpoint_compatibility_step_overflow() {
        let checkpoint = create_test_checkpoint(10, WorkflowStatus::Running, vec![0, 1], "hash", 8);

        let result = validate_checkpoint_compatibility(&checkpoint, "hash", 5);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("exceeds workflow steps"));
    }

    #[test]
    fn test_validate_checkpoint_compatibility_different_step_count() {
        let checkpoint = create_test_checkpoint(2, WorkflowStatus::Running, vec![0, 1], "hash", 5);

        let result = validate_checkpoint_compatibility(&checkpoint, "hash", 10);

        // Should succeed - total_steps can change if hash matches
        // (hash mismatch would catch actual incompatibility)
        assert!(result.is_ok());
    }
}
