//! Pure checkpoint validation with error accumulation
//!
//! This module uses Stillwater's `Validation` pattern to accumulate ALL errors
//! instead of failing fast on the first error.

use crate::cook::execution::mapreduce::checkpoint::{MapReduceCheckpoint, PhaseType};
use stillwater::Validation;

/// Checkpoint validation errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum CheckpointValidationError {
    #[error("Work item count mismatch: expected {expected}, got {actual}")]
    WorkItemCountMismatch { expected: usize, actual: usize },

    #[error("Agent {agent_id} has assignments but is not active")]
    OrphanedAgentAssignment { agent_id: String },

    #[error("Integrity hash mismatch: expected {expected}, got {actual}")]
    IntegrityHashMismatch { expected: String, actual: String },

    #[error("Invalid phase state: {phase:?} - {reason}")]
    InvalidPhaseState { phase: PhaseType, reason: String },

    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },

    #[error("Inconsistent timestamps: {field} - {issue}")]
    InconsistentTimestamps { field: String, issue: String },

    #[error("Duplicate work item ID: {id}")]
    DuplicateWorkItemId { id: String },

    #[error("Work item {item_id} in multiple states")]
    WorkItemInMultipleStates { item_id: String },
}

/// Validate a checkpoint with error accumulation
///
/// Uses `Validation` to collect ALL errors instead of failing fast.
/// This gives users complete feedback in a single validation pass.
pub fn validate_checkpoint(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    // Collect all validation results
    let mut all_errors = Vec::new();

    // Validate work item counts
    if let Validation::Failure(errors) = validate_work_item_counts(checkpoint) {
        all_errors.extend(errors);
    }

    // Validate agent consistency
    if let Validation::Failure(errors) = validate_agent_consistency(checkpoint) {
        all_errors.extend(errors);
    }

    // Validate no duplicate work item IDs
    if let Validation::Failure(errors) = validate_no_duplicate_ids(checkpoint) {
        all_errors.extend(errors);
    }

    // Validate work items not in multiple states
    if let Validation::Failure(errors) = validate_no_items_in_multiple_states(checkpoint) {
        all_errors.extend(errors);
    }

    // Validate phase state
    if let Validation::Failure(errors) = validate_phase_state(checkpoint) {
        all_errors.extend(errors);
    }

    if all_errors.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(all_errors)
    }
}

/// Pure: Validate work item counts match metadata
fn validate_work_item_counts(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    let total_accounted = checkpoint.work_item_state.completed_items.len()
        + checkpoint.work_item_state.failed_items.len()
        + checkpoint.work_item_state.pending_items.len()
        + checkpoint.work_item_state.in_progress_items.len();

    // Note: total_work_items might be 0 for initial checkpoints
    if checkpoint.metadata.total_work_items > 0
        && total_accounted != checkpoint.metadata.total_work_items
    {
        Validation::Failure(vec![CheckpointValidationError::WorkItemCountMismatch {
            expected: checkpoint.metadata.total_work_items,
            actual: total_accounted,
        }])
    } else {
        Validation::Success(())
    }
}

/// Pure: Validate agent assignment consistency
fn validate_agent_consistency(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    let orphaned: Vec<_> = checkpoint
        .agent_state
        .agent_assignments
        .keys()
        .filter(|agent_id| !checkpoint.agent_state.active_agents.contains_key(*agent_id))
        .map(
            |agent_id| CheckpointValidationError::OrphanedAgentAssignment {
                agent_id: agent_id.clone(),
            },
        )
        .collect();

    if orphaned.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(orphaned)
    }
}

/// Pure: Validate no duplicate work item IDs across all states
fn validate_no_duplicate_ids(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();

    // Check pending items
    for item in &checkpoint.work_item_state.pending_items {
        if !seen.insert(item.id.clone()) {
            duplicates.push(CheckpointValidationError::DuplicateWorkItemId {
                id: item.id.clone(),
            });
        }
    }

    // Check in-progress items
    for id in checkpoint.work_item_state.in_progress_items.keys() {
        if !seen.insert(id.clone()) {
            duplicates.push(CheckpointValidationError::DuplicateWorkItemId { id: id.clone() });
        }
    }

    // Check completed items
    for item in &checkpoint.work_item_state.completed_items {
        if !seen.insert(item.work_item.id.clone()) {
            duplicates.push(CheckpointValidationError::DuplicateWorkItemId {
                id: item.work_item.id.clone(),
            });
        }
    }

    // Check failed items
    for item in &checkpoint.work_item_state.failed_items {
        if !seen.insert(item.work_item.id.clone()) {
            duplicates.push(CheckpointValidationError::DuplicateWorkItemId {
                id: item.work_item.id.clone(),
            });
        }
    }

    if duplicates.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(duplicates)
    }
}

/// Pure: Validate work items are not in multiple states simultaneously
fn validate_no_items_in_multiple_states(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    use std::collections::HashMap;

    let mut item_states: HashMap<String, Vec<&str>> = HashMap::new();

    // Track which states each item is in
    for item in &checkpoint.work_item_state.pending_items {
        item_states
            .entry(item.id.clone())
            .or_default()
            .push("pending");
    }

    for id in checkpoint.work_item_state.in_progress_items.keys() {
        item_states
            .entry(id.clone())
            .or_default()
            .push("in_progress");
    }

    for item in &checkpoint.work_item_state.completed_items {
        item_states
            .entry(item.work_item.id.clone())
            .or_default()
            .push("completed");
    }

    for item in &checkpoint.work_item_state.failed_items {
        item_states
            .entry(item.work_item.id.clone())
            .or_default()
            .push("failed");
    }

    // Find items in multiple states
    let multi_state_items: Vec<_> = item_states
        .into_iter()
        .filter(|(_, states)| states.len() > 1)
        .map(|(id, _)| CheckpointValidationError::WorkItemInMultipleStates { item_id: id })
        .collect();

    if multi_state_items.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(multi_state_items)
    }
}

/// Pure: Validate phase state is consistent
fn validate_phase_state(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    let mut errors = Vec::new();

    // Check that metadata phase matches execution state phase
    if checkpoint.metadata.phase != checkpoint.execution_state.current_phase {
        errors.push(CheckpointValidationError::InvalidPhaseState {
            phase: checkpoint.metadata.phase,
            reason: format!(
                "Metadata phase {:?} doesn't match execution state phase {:?}",
                checkpoint.metadata.phase, checkpoint.execution_state.current_phase
            ),
        });
    }

    // Check that completed_items count matches actual completed items
    let actual_completed = checkpoint.work_item_state.completed_items.len();
    if checkpoint.metadata.completed_items != actual_completed {
        errors.push(CheckpointValidationError::InvalidPhaseState {
            phase: checkpoint.metadata.phase,
            reason: format!(
                "Metadata completed_items ({}) doesn't match actual count ({})",
                checkpoint.metadata.completed_items, actual_completed
            ),
        });
    }

    if errors.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(errors)
    }
}

/// Validate integrity hash matches calculated value
pub fn validate_integrity_hash(
    checkpoint: &MapReduceCheckpoint,
) -> Validation<(), Vec<CheckpointValidationError>> {
    let calculated = super::preparation::calculate_integrity_hash(checkpoint);

    if calculated == checkpoint.metadata.integrity_hash {
        Validation::Success(())
    } else {
        Validation::Failure(vec![CheckpointValidationError::IntegrityHashMismatch {
            expected: checkpoint.metadata.integrity_hash.clone(),
            actual: calculated,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::agent::{AgentResult, AgentStatus};
    use crate::cook::execution::mapreduce::checkpoint::{CompletedWorkItem, WorkItem};
    use chrono::Utc;
    use serde_json::json;
    use std::time::Duration;

    fn create_test_checkpoint(job_id: &str) -> MapReduceCheckpoint {
        // Use 0 for total_work_items to match the empty work item state
        super::super::preparation::create_initial_checkpoint(job_id, 0, PhaseType::Map)
    }

    #[test]
    fn test_validate_checkpoint_success() {
        let checkpoint = create_test_checkpoint("test-job");
        let result = validate_checkpoint(&checkpoint);
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_work_item_count_mismatch() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint.metadata.total_work_items = 5;
        // But no actual work items

        let result = validate_checkpoint(&checkpoint);
        match result {
            Validation::Failure(errors) => {
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, CheckpointValidationError::WorkItemCountMismatch { .. })));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_orphaned_agent_assignment() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint
            .agent_state
            .agent_assignments
            .insert("orphaned-agent".to_string(), vec!["item-1".to_string()]);
        // But agent is not in active_agents

        let result = validate_checkpoint(&checkpoint);
        match result {
            Validation::Failure(errors) => {
                assert!(errors.iter().any(|e| matches!(
                    e,
                    CheckpointValidationError::OrphanedAgentAssignment { agent_id } if agent_id == "orphaned-agent"
                )));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_duplicate_work_item_ids() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint.work_item_state.pending_items = vec![
            WorkItem {
                id: "duplicate-id".to_string(),
                data: json!({}),
            },
            WorkItem {
                id: "duplicate-id".to_string(),
                data: json!({}),
            },
        ];
        checkpoint.metadata.total_work_items = 2;

        let result = validate_checkpoint(&checkpoint);
        match result {
            Validation::Failure(errors) => {
                assert!(errors.iter().any(|e| matches!(
                    e,
                    CheckpointValidationError::DuplicateWorkItemId { id } if id == "duplicate-id"
                )));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_accumulates_multiple_errors() {
        let mut checkpoint = create_test_checkpoint("test-job");

        // Add multiple issues
        checkpoint.metadata.total_work_items = 100; // Count mismatch
        checkpoint
            .agent_state
            .agent_assignments
            .insert("orphan1".to_string(), vec![]);
        checkpoint
            .agent_state
            .agent_assignments
            .insert("orphan2".to_string(), vec![]);

        let result = validate_checkpoint(&checkpoint);
        match result {
            Validation::Failure(errors) => {
                // Should have at least 3 errors: 1 count mismatch + 2 orphaned agents
                assert!(
                    errors.len() >= 3,
                    "Expected at least 3 errors, got {}",
                    errors.len()
                );
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_phase_state_mismatch() {
        let mut checkpoint = create_test_checkpoint("test-job");
        checkpoint.metadata.phase = PhaseType::Map;
        checkpoint.execution_state.current_phase = PhaseType::Reduce;

        let result = validate_checkpoint(&checkpoint);
        match result {
            Validation::Failure(errors) => {
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, CheckpointValidationError::InvalidPhaseState { .. })));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_item_in_multiple_states() {
        let mut checkpoint = create_test_checkpoint("test-job");

        // Same item in both pending and completed
        checkpoint.work_item_state.pending_items.push(WorkItem {
            id: "multi-state-item".to_string(),
            data: json!({}),
        });
        checkpoint
            .work_item_state
            .completed_items
            .push(CompletedWorkItem {
                work_item: WorkItem {
                    id: "multi-state-item".to_string(),
                    data: json!({}),
                },
                result: AgentResult {
                    item_id: "multi-state-item".to_string(),
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

        let result = validate_checkpoint(&checkpoint);
        match result {
            Validation::Failure(errors) => {
                assert!(errors.iter().any(|e| matches!(
                    e,
                    CheckpointValidationError::WorkItemInMultipleStates { item_id } if item_id == "multi-state-item"
                )));
            }
            _ => panic!("Expected validation failure"),
        }
    }
}
