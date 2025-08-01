//! Session persistence types

use super::{SessionConfig, SessionId, SessionState, TimestampedEvent};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// Persisted session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    pub id: SessionId,
    pub config: SessionConfig,
    pub state: SessionState,
    pub events: Vec<TimestampedEvent>,
    pub checkpoints: Vec<SessionCheckpoint>,
}

/// Session checkpoint for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCheckpoint {
    pub iteration: u32,
    pub timestamp: DateTime<Utc>,
    pub state_snapshot: StateSnapshot,
    pub resumable: bool,
}

/// Snapshot of session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub files_changed: HashSet<PathBuf>,
    pub commands_executed: usize,
    pub current_phase: Option<String>,
}

impl PersistedSession {
    /// Get the latest checkpoint
    pub fn latest_checkpoint(&self) -> Option<&SessionCheckpoint> {
        self.checkpoints.iter().max_by_key(|c| c.timestamp)
    }

    /// Check if session is resumable
    pub fn is_resumable(&self) -> bool {
        !self.state.is_terminal()
            && self
                .latest_checkpoint()
                .map(|c| c.resumable)
                .unwrap_or(false)
    }

    /// Get last known iteration
    pub fn last_iteration(&self) -> u32 {
        self.latest_checkpoint()
            .map(|c| c.iteration)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persisted_session() {
        let session = PersistedSession {
            id: SessionId::new(),
            config: SessionConfig::default(),
            state: SessionState::Running { iteration: 3 },
            events: vec![],
            checkpoints: vec![
                SessionCheckpoint {
                    iteration: 1,
                    timestamp: Utc::now() - chrono::Duration::minutes(10),
                    state_snapshot: StateSnapshot {
                        files_changed: HashSet::new(),
                        commands_executed: 5,
                        current_phase: Some("analysis".to_string()),
                    },
                    resumable: true,
                },
                SessionCheckpoint {
                    iteration: 3,
                    timestamp: Utc::now(),
                    state_snapshot: StateSnapshot {
                        files_changed: HashSet::from([PathBuf::from("test.rs")]),
                        commands_executed: 10,
                        current_phase: Some("implementation".to_string()),
                    },
                    resumable: true,
                },
            ],
        };

        assert!(session.is_resumable());
        assert_eq!(session.last_iteration(), 3);
        assert_eq!(
            session.latest_checkpoint().unwrap().state_snapshot.current_phase,
            Some("implementation".to_string())
        );
    }
}