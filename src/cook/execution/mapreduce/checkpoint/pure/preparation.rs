//! Pure checkpoint preparation functions
//!
//! This module contains pure functions for preparing checkpoints for saving.
//! Separates business logic from I/O operations.

use crate::cook::execution::mapreduce::checkpoint::{
    CheckpointMetadata, CheckpointReason, MapReduceCheckpoint, PhaseType, WorkItemState,
};
use chrono::Utc;
use sha2::{Digest, Sha256};

/// Pure: Prepare a checkpoint for saving
///
/// This function prepares a checkpoint by:
/// 1. Generating a new checkpoint ID
/// 2. Updating timestamp
/// 3. Setting the checkpoint reason
/// 4. Calculating integrity hash
/// 5. Resetting in-progress items to pending (safe for resume)
///
/// # Arguments
/// * `state` - Current checkpoint state
/// * `reason` - Reason for creating the checkpoint
///
/// # Returns
/// A new checkpoint ready for saving
pub fn prepare_checkpoint(
    state: &MapReduceCheckpoint,
    reason: CheckpointReason,
) -> MapReduceCheckpoint {
    let mut checkpoint = state.clone();

    // Generate new checkpoint ID
    checkpoint.metadata.checkpoint_id = format!("cp-{}", uuid::Uuid::new_v4());
    checkpoint.metadata.created_at = Utc::now();
    checkpoint.metadata.checkpoint_reason = reason;

    // Reset in-progress items to pending for safe resume
    reset_in_progress_items(&mut checkpoint.work_item_state);

    // Update completed items count
    checkpoint.metadata.completed_items = checkpoint.work_item_state.completed_items.len();

    // Calculate integrity hash (after all modifications)
    checkpoint.metadata.integrity_hash = calculate_integrity_hash(&checkpoint);

    checkpoint
}

/// Pure: Reset all in-progress items to pending
///
/// This ensures that on resume, all items that were in-progress are
/// reprocessed to avoid losing work.
pub fn reset_in_progress_items(work_item_state: &mut WorkItemState) {
    for (_, progress) in work_item_state.in_progress_items.drain() {
        work_item_state.pending_items.push(progress.work_item);
    }
}

/// Pure: Calculate integrity hash for a checkpoint
///
/// Uses SHA256 to create a hash of key checkpoint fields.
/// This is used to detect corruption when loading checkpoints.
pub fn calculate_integrity_hash(checkpoint: &MapReduceCheckpoint) -> String {
    let mut hasher = Sha256::new();

    // Include key metadata fields
    hasher.update(checkpoint.metadata.job_id.as_bytes());
    hasher.update(checkpoint.metadata.version.to_string().as_bytes());
    hasher.update(format!("{:?}", checkpoint.metadata.phase).as_bytes());
    hasher.update(checkpoint.metadata.total_work_items.to_string().as_bytes());
    hasher.update(checkpoint.metadata.completed_items.to_string().as_bytes());

    // Include work item counts
    hasher.update(
        checkpoint
            .work_item_state
            .completed_items
            .len()
            .to_string()
            .as_bytes(),
    );
    hasher.update(
        checkpoint
            .work_item_state
            .failed_items
            .len()
            .to_string()
            .as_bytes(),
    );
    hasher.update(
        checkpoint
            .work_item_state
            .pending_items
            .len()
            .to_string()
            .as_bytes(),
    );

    format!("{:x}", hasher.finalize())
}

/// Pure: Verify checkpoint integrity
///
/// Compares the stored hash with a freshly calculated one.
pub fn verify_integrity(checkpoint: &MapReduceCheckpoint) -> bool {
    let calculated = calculate_integrity_hash(checkpoint);
    calculated == checkpoint.metadata.integrity_hash
}

/// Pure: Create an initial checkpoint for a new job
pub fn create_initial_checkpoint(
    job_id: &str,
    total_items: usize,
    phase: PhaseType,
) -> MapReduceCheckpoint {
    use std::collections::HashMap;

    MapReduceCheckpoint {
        metadata: CheckpointMetadata {
            checkpoint_id: format!("cp-{}", uuid::Uuid::new_v4()),
            job_id: job_id.to_string(),
            version: 1,
            created_at: Utc::now(),
            phase,
            total_work_items: total_items,
            completed_items: 0,
            checkpoint_reason: CheckpointReason::PhaseTransition,
            integrity_hash: String::new(), // Will be calculated
        },
        execution_state: crate::cook::execution::mapreduce::checkpoint::ExecutionState {
            current_phase: phase,
            phase_start_time: Utc::now(),
            setup_results: None,
            map_results: None,
            reduce_results: None,
            workflow_variables: HashMap::new(),
        },
        work_item_state: WorkItemState {
            pending_items: Vec::new(),
            in_progress_items: HashMap::new(),
            completed_items: Vec::new(),
            failed_items: Vec::new(),
            current_batch: None,
        },
        agent_state: crate::cook::execution::mapreduce::checkpoint::AgentState {
            active_agents: HashMap::new(),
            agent_assignments: HashMap::new(),
            agent_results: HashMap::new(),
            resource_allocation: HashMap::new(),
        },
        variable_state: crate::cook::execution::mapreduce::checkpoint::VariableState {
            workflow_variables: HashMap::new(),
            captured_outputs: HashMap::new(),
            environment_variables: HashMap::new(),
            item_variables: HashMap::new(),
        },
        resource_state: crate::cook::execution::mapreduce::checkpoint::ResourceState {
            total_agents_allowed: 0,
            current_agents_active: 0,
            worktrees_created: Vec::new(),
            worktrees_cleaned: Vec::new(),
            disk_usage_bytes: None,
        },
        error_state: crate::cook::execution::mapreduce::checkpoint::ErrorState {
            error_count: 0,
            dlq_items: Vec::new(),
            error_threshold_reached: false,
            last_error: None,
        },
    }
}

/// Pure: Update checkpoint metadata with new completed items count
pub fn update_completed_count(checkpoint: &mut MapReduceCheckpoint) {
    checkpoint.metadata.completed_items = checkpoint.work_item_state.completed_items.len();
}

/// Pure: Update checkpoint phase
pub fn update_phase(checkpoint: &mut MapReduceCheckpoint, phase: PhaseType) {
    checkpoint.metadata.phase = phase;
    checkpoint.execution_state.current_phase = phase;
    checkpoint.execution_state.phase_start_time = Utc::now();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::agent::{AgentResult, AgentStatus};
    use crate::cook::execution::mapreduce::checkpoint::{
        CompletedWorkItem, WorkItem, WorkItemProgress,
    };
    use serde_json::json;
    use std::time::Duration;

    fn create_test_checkpoint(job_id: &str) -> MapReduceCheckpoint {
        create_initial_checkpoint(job_id, 10, PhaseType::Map)
    }

    #[test]
    fn test_prepare_checkpoint_generates_new_id() {
        let checkpoint = create_test_checkpoint("test-job");
        let original_id = checkpoint.metadata.checkpoint_id.clone();

        let prepared = prepare_checkpoint(&checkpoint, CheckpointReason::Interval);

        assert_ne!(prepared.metadata.checkpoint_id, original_id);
        assert!(prepared.metadata.checkpoint_id.starts_with("cp-"));
    }

    #[test]
    fn test_prepare_checkpoint_sets_reason() {
        let checkpoint = create_test_checkpoint("test-job");

        let prepared = prepare_checkpoint(&checkpoint, CheckpointReason::BeforeShutdown);

        assert!(matches!(
            prepared.metadata.checkpoint_reason,
            CheckpointReason::BeforeShutdown
        ));
    }

    #[test]
    fn test_prepare_checkpoint_resets_in_progress() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint.work_item_state.in_progress_items.insert(
            "item-1".to_string(),
            WorkItemProgress {
                work_item: WorkItem {
                    id: "item-1".to_string(),
                    data: json!({}),
                },
                agent_id: "agent-1".to_string(),
                started_at: Utc::now(),
                last_update: Utc::now(),
            },
        );

        let prepared = prepare_checkpoint(&checkpoint, CheckpointReason::Interval);

        assert!(prepared.work_item_state.in_progress_items.is_empty());
        assert_eq!(prepared.work_item_state.pending_items.len(), 1);
        assert_eq!(prepared.work_item_state.pending_items[0].id, "item-1");
    }

    #[test]
    fn test_integrity_hash_is_deterministic() {
        let checkpoint = create_test_checkpoint("test-job");

        let hash1 = calculate_integrity_hash(&checkpoint);
        let hash2 = calculate_integrity_hash(&checkpoint);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_integrity_hash_changes_on_state_change() {
        let mut checkpoint = create_test_checkpoint("test-job");
        let hash1 = calculate_integrity_hash(&checkpoint);

        checkpoint.metadata.completed_items += 1;
        let hash2 = calculate_integrity_hash(&checkpoint);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_verify_integrity_valid() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint.metadata.integrity_hash = calculate_integrity_hash(&checkpoint);

        assert!(verify_integrity(&checkpoint));
    }

    #[test]
    fn test_verify_integrity_invalid() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint.metadata.integrity_hash = "invalid-hash".to_string();

        assert!(!verify_integrity(&checkpoint));
    }

    #[test]
    fn test_update_completed_count() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint
            .work_item_state
            .completed_items
            .push(CompletedWorkItem {
                work_item: WorkItem {
                    id: "item-1".to_string(),
                    data: json!({}),
                },
                result: AgentResult {
                    item_id: "item-1".to_string(),
                    status: AgentStatus::Success,
                    output: None,
                    commits: vec![],
                    duration: Duration::from_secs(1),
                    error: None,
                    worktree_path: None,
                    branch_name: None,
                    worktree_session_id: None,
                    files_modified: vec![],
                    json_log_location: None,
                    cleanup_status: None,
                },
                completed_at: Utc::now(),
            });

        update_completed_count(&mut checkpoint);

        assert_eq!(checkpoint.metadata.completed_items, 1);
    }

    #[test]
    fn test_update_phase() {
        let mut checkpoint = create_test_checkpoint("test-job");

        update_phase(&mut checkpoint, PhaseType::Reduce);

        assert_eq!(checkpoint.metadata.phase, PhaseType::Reduce);
        assert_eq!(checkpoint.execution_state.current_phase, PhaseType::Reduce);
    }
}
