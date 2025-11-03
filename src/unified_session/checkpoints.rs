//! Checkpoint management with pure functional operations

use super::state::{Checkpoint, CheckpointId, UnifiedSession};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;

/// Create a checkpoint from a session (pure function)
pub fn create_checkpoint_from_session(session: &UnifiedSession) -> Result<Checkpoint> {
    let checkpoint_state = serde_json::to_value(session)?;

    Ok(Checkpoint {
        id: CheckpointId::new(),
        created_at: Utc::now(),
        state: checkpoint_state,
        metadata: HashMap::new(),
    })
}

/// Find a checkpoint by ID (pure function)
pub fn find_checkpoint<'a>(
    checkpoints: &'a [Checkpoint],
    id: &CheckpointId,
) -> Option<&'a Checkpoint> {
    checkpoints.iter().find(|c| c.id == *id)
}

/// Restore session from checkpoint (pure function)
pub fn restore_session_from_checkpoint(checkpoint: &Checkpoint) -> Result<UnifiedSession> {
    let session: UnifiedSession = serde_json::from_value(checkpoint.state.clone())?;
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_session::SessionStatus;

    #[test]
    fn test_create_checkpoint_from_session() {
        let session = UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        let checkpoint = create_checkpoint_from_session(&session).unwrap();

        assert!(!checkpoint.id.as_str().is_empty());
        assert!(checkpoint.state.is_object());
    }

    #[test]
    fn test_find_checkpoint() {
        let session = UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        let checkpoint1 = create_checkpoint_from_session(&session).unwrap();
        let checkpoint2 = create_checkpoint_from_session(&session).unwrap();

        let checkpoints = vec![checkpoint1.clone(), checkpoint2.clone()];

        assert_eq!(
            find_checkpoint(&checkpoints, &checkpoint1.id).unwrap().id,
            checkpoint1.id
        );
        assert_eq!(
            find_checkpoint(&checkpoints, &checkpoint2.id).unwrap().id,
            checkpoint2.id
        );
    }

    #[test]
    fn test_find_checkpoint_not_found() {
        let checkpoints = vec![];
        let id = CheckpointId::new();
        assert!(find_checkpoint(&checkpoints, &id).is_none());
    }

    #[test]
    fn test_restore_session_from_checkpoint() {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        session.status = SessionStatus::Running;

        let checkpoint = create_checkpoint_from_session(&session).unwrap();
        let restored = restore_session_from_checkpoint(&checkpoint).unwrap();

        assert_eq!(restored.status, SessionStatus::Running);
        assert_eq!(restored.id, session.id);
    }
}
