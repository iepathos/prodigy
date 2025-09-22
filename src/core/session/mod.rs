//! Pure session state management functions
//!
//! These functions handle session state transformations without performing any I/O operations.

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

/// Session status
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    InProgress,
    Completed,
    Failed,
    Paused,
}

/// Session state
#[derive(Debug, Clone)]
pub struct SessionState {
    pub id: String,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, Value>,
    pub iterations_completed: u32,
    pub files_changed: u32,
    pub current_step: usize,
    pub total_steps: usize,
    pub error: Option<String>,
}

/// Update types for session state
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    Status(SessionStatus),
    Metadata(HashMap<String, Value>),
    Progress { current: usize, total: usize },
    Error(String),
    IncrementIteration,
    FilesChanged(u32),
}

/// Apply an update to session state (pure function)
pub fn apply_session_update(mut state: SessionState, update: SessionUpdate) -> SessionState {
    match update {
        SessionUpdate::Status(status) => {
            state.status = status.clone();
            if matches!(status, SessionStatus::Completed | SessionStatus::Failed) {
                state.completed_at = Some(Utc::now());
            }
        }
        SessionUpdate::Metadata(metadata) => {
            // Handle special metadata keys
            for (key, value) in metadata.iter() {
                match key.as_str() {
                    "files_changed_delta" => {
                        if let Some(count) = value.as_u64() {
                            state.files_changed += count as u32;
                        }
                    }
                    "increment_iteration" => {
                        if value.as_bool().unwrap_or(false) {
                            state.iterations_completed += 1;
                        }
                    }
                    _ => {}
                }
            }
            state.metadata.extend(metadata);
        }
        SessionUpdate::Progress { current, total } => {
            state.current_step = current;
            state.total_steps = total;
        }
        SessionUpdate::Error(error) => {
            state.error = Some(error);
            state.status = SessionStatus::Failed;
            state.completed_at = Some(Utc::now());
        }
        SessionUpdate::IncrementIteration => {
            state.iterations_completed += 1;
        }
        SessionUpdate::FilesChanged(count) => {
            state.files_changed += count;
        }
    }

    state
}

/// Calculate session duration
pub fn calculate_duration(
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
) -> chrono::Duration {
    let end_time = completed_at.unwrap_or_else(Utc::now);
    end_time - started_at
}

/// Format duration for display
pub fn format_duration(duration: chrono::Duration) -> String {
    let total_seconds = duration.num_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Generate session summary
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub status: SessionStatus,
    pub duration: String,
    pub iterations: u32,
    pub files_changed: u32,
    pub progress_percentage: f64,
}

pub fn generate_session_summary(state: &SessionState) -> SessionSummary {
    let duration = calculate_duration(state.started_at, state.completed_at);
    let progress_percentage = if state.total_steps > 0 {
        (state.current_step as f64 / state.total_steps as f64) * 100.0
    } else {
        0.0
    };

    SessionSummary {
        id: state.id.clone(),
        status: state.status.clone(),
        duration: format_duration(duration),
        iterations: state.iterations_completed,
        files_changed: state.files_changed,
        progress_percentage,
    }
}

/// Filter sessions based on criteria
#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub status: Option<SessionStatus>,
    pub min_iterations: Option<u32>,
    pub max_iterations: Option<u32>,
    pub has_error: Option<bool>,
}

pub fn filter_sessions(sessions: &[SessionState], filter: &SessionFilter) -> Vec<SessionState> {
    sessions
        .iter()
        .filter(|s| {
            if let Some(status) = &filter.status {
                if s.status != *status {
                    return false;
                }
            }

            if let Some(min) = filter.min_iterations {
                if s.iterations_completed < min {
                    return false;
                }
            }

            if let Some(max) = filter.max_iterations {
                if s.iterations_completed > max {
                    return false;
                }
            }

            if let Some(has_error) = filter.has_error {
                if has_error != s.error.is_some() {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session() -> SessionState {
        SessionState {
            id: "test-123".to_string(),
            status: SessionStatus::InProgress,
            started_at: Utc::now(),
            completed_at: None,
            metadata: HashMap::new(),
            iterations_completed: 0,
            files_changed: 0,
            current_step: 0,
            total_steps: 10,
            error: None,
        }
    }

    #[test]
    fn test_apply_status_update() {
        let session = create_test_session();
        let updated =
            apply_session_update(session, SessionUpdate::Status(SessionStatus::Completed));

        assert_eq!(updated.status, SessionStatus::Completed);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn test_apply_progress_update() {
        let session = create_test_session();
        let updated = apply_session_update(
            session,
            SessionUpdate::Progress {
                current: 5,
                total: 10,
            },
        );

        assert_eq!(updated.current_step, 5);
        assert_eq!(updated.total_steps, 10);
    }

    #[test]
    fn test_apply_error_update() {
        let session = create_test_session();
        let updated = apply_session_update(session, SessionUpdate::Error("Test error".to_string()));

        assert_eq!(updated.status, SessionStatus::Failed);
        assert_eq!(updated.error, Some("Test error".to_string()));
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn test_increment_iteration() {
        let session = create_test_session();
        let updated = apply_session_update(session, SessionUpdate::IncrementIteration);

        assert_eq!(updated.iterations_completed, 1);
    }

    #[test]
    fn test_format_duration() {
        use chrono::Duration;

        assert_eq!(format_duration(Duration::seconds(45)), "45s");
        assert_eq!(format_duration(Duration::seconds(125)), "2m 5s");
        assert_eq!(format_duration(Duration::seconds(3665)), "1h 1m 5s");
    }

    #[test]
    fn test_generate_session_summary() {
        let mut session = create_test_session();
        session.current_step = 5;
        session.iterations_completed = 3;
        session.files_changed = 10;

        let summary = generate_session_summary(&session);

        assert_eq!(summary.id, "test-123");
        assert_eq!(summary.iterations, 3);
        assert_eq!(summary.files_changed, 10);
        assert_eq!(summary.progress_percentage, 50.0);
    }

    #[test]
    fn test_filter_sessions() {
        let mut sessions = vec![
            create_test_session(),
            create_test_session(),
            create_test_session(),
        ];

        sessions[0].status = SessionStatus::Completed;
        sessions[0].iterations_completed = 5;

        sessions[1].status = SessionStatus::Failed;
        sessions[1].error = Some("Error".to_string());
        sessions[1].iterations_completed = 2;

        sessions[2].iterations_completed = 10;

        // Filter by status
        let filter = SessionFilter {
            status: Some(SessionStatus::Completed),
            ..Default::default()
        };
        let filtered = filter_sessions(&sessions, &filter);
        assert_eq!(filtered.len(), 1);

        // Filter by iterations
        let filter = SessionFilter {
            min_iterations: Some(3),
            ..Default::default()
        };
        let filtered = filter_sessions(&sessions, &filter);
        assert_eq!(filtered.len(), 2);

        // Filter by error
        let filter = SessionFilter {
            has_error: Some(true),
            ..Default::default()
        };
        let filtered = filter_sessions(&sessions, &filter);
        assert_eq!(filtered.len(), 1);
    }
}
