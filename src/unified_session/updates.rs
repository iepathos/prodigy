//! Pure functions for applying session updates

use super::state::{Checkpoint, CheckpointId, UnifiedSession};
use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;

/// Apply metadata update to session (pure function)
pub fn apply_metadata_update(
    session: &mut UnifiedSession,
    metadata: HashMap<String, serde_json::Value>,
) {
    // Handle special metadata keys
    for (key, value) in metadata.iter() {
        match key.as_str() {
            "files_changed_delta" => {
                if let Some(count) = value.as_u64() {
                    if let Some(workflow) = &mut session.workflow_data {
                        workflow.files_changed += count as u32;
                    }
                }
            }
            "increment_iteration" => {
                if value.as_bool().unwrap_or(false) {
                    if let Some(workflow) = &mut session.workflow_data {
                        workflow.iterations_completed += 1;
                    }
                }
            }
            _ => {}
        }
    }
    session.metadata.extend(metadata);
}

/// Apply checkpoint state update to session (pure function)
pub fn apply_checkpoint_update(
    session: &mut UnifiedSession,
    state: serde_json::Value,
) -> Checkpoint {
    let checkpoint = Checkpoint {
        id: CheckpointId::new(),
        created_at: Utc::now(),
        state,
        metadata: HashMap::new(),
    };
    session.checkpoints.push(checkpoint.clone());
    checkpoint
}

/// Apply error update to session (pure function)
pub fn apply_error_update(session: &mut UnifiedSession, error: String) {
    session.error = Some(error);
    session.status = super::state::SessionStatus::Failed;
}

/// Apply progress update to session (pure function)
pub fn apply_progress_update(session: &mut UnifiedSession, current: usize, total: usize) {
    if let Some(workflow) = &mut session.workflow_data {
        workflow.current_step = current;
        workflow.total_steps = total;
    } else if let Some(mapreduce) = &mut session.mapreduce_data {
        mapreduce.processed_items = current;
        mapreduce.total_items = total;
    }
}

/// Apply timing update to session (pure function)
pub fn apply_timing_update(session: &mut UnifiedSession, operation: String, duration: Duration) {
    session.timings.insert(operation, duration);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_metadata_update() {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());

        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), serde_json::json!("value1"));
        metadata.insert("key2".to_string(), serde_json::json!(42));

        apply_metadata_update(&mut session, metadata);

        assert_eq!(
            session.metadata.get("key1"),
            Some(&serde_json::json!("value1"))
        );
        assert_eq!(session.metadata.get("key2"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_apply_metadata_update_files_changed() {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());

        let mut metadata = HashMap::new();
        metadata.insert("files_changed_delta".to_string(), serde_json::json!(5));

        apply_metadata_update(&mut session, metadata);

        if let Some(workflow) = &session.workflow_data {
            assert_eq!(workflow.files_changed, 5);
        } else {
            panic!("Expected workflow data");
        }
    }

    #[test]
    fn test_apply_checkpoint_update() {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        let state = serde_json::json!({"test": "data"});

        let checkpoint = apply_checkpoint_update(&mut session, state.clone());

        assert_eq!(session.checkpoints.len(), 1);
        assert_eq!(session.checkpoints[0].id, checkpoint.id);
        assert_eq!(session.checkpoints[0].state, state);
    }

    #[test]
    fn test_apply_error_update() {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());

        apply_error_update(&mut session, "Test error".to_string());

        assert_eq!(session.error, Some("Test error".to_string()));
        assert_eq!(session.status, super::super::state::SessionStatus::Failed);
    }

    #[test]
    fn test_apply_progress_update_workflow() {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());

        apply_progress_update(&mut session, 3, 10);

        if let Some(workflow) = &session.workflow_data {
            assert_eq!(workflow.current_step, 3);
            assert_eq!(workflow.total_steps, 10);
        } else {
            panic!("Expected workflow data");
        }
    }

    #[test]
    fn test_apply_progress_update_mapreduce() {
        let mut session = UnifiedSession::new_mapreduce("test-job".to_string(), 10);

        apply_progress_update(&mut session, 5, 10);

        if let Some(mapreduce) = &session.mapreduce_data {
            assert_eq!(mapreduce.processed_items, 5);
            assert_eq!(mapreduce.total_items, 10);
        } else {
            panic!("Expected mapreduce data");
        }
    }

    #[test]
    fn test_apply_timing_update() {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        let duration = Duration::from_secs(10);

        apply_timing_update(&mut session, "test_operation".to_string(), duration);

        assert_eq!(session.timings.get("test_operation"), Some(&duration));
    }
}
