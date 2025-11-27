//! Session lifecycle management with pure state transition logic

use super::state::{SessionStatus, UnifiedSession};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};

/// Represents a status transition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Used in tests, available for future functional refactoring
pub enum Transition {
    Start,
    Pause,
    Resume,
    Complete,
    Fail,
}

/// Validate a status transition
pub fn validate_transition(from: &SessionStatus, to: &SessionStatus) -> Result<()> {
    use SessionStatus::*;

    let valid = match (from, to) {
        // From Initializing - allow transitions to any non-initializing state
        (Initializing, Running) => true,
        (Initializing, Paused) => true,    // For migration support
        (Initializing, Completed) => true, // Allow direct completion
        (Initializing, Failed) => true,

        // From Running
        (Running, Paused) => true,
        (Running, Completed) => true,
        (Running, Failed) => true,

        // From Paused
        (Paused, Running) => true,
        (Paused, Failed) => true,
        (Paused, Completed) => true,

        // From Failed - allow resume (retry failed workflows)
        (Failed, Running) => true,

        // Same status is always valid (idempotent)
        (a, b) if a == b => true,

        // Terminal states cannot transition (except Failed -> Running above)
        (Completed, _) => false,
        (Cancelled, _) => false,

        // All other transitions are invalid
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(anyhow!(
            "Invalid status transition from {:?} to {:?}",
            from,
            to
        ))
    }
}

/// Apply a status transition to get the new status
#[allow(dead_code)] // Used in tests, available for future functional refactoring
pub fn transition_status(current: &SessionStatus, transition: Transition) -> Result<SessionStatus> {
    let new_status = match transition {
        Transition::Start => SessionStatus::Running,
        Transition::Pause => SessionStatus::Paused,
        Transition::Resume => SessionStatus::Running,
        Transition::Complete => SessionStatus::Completed,
        Transition::Fail => SessionStatus::Failed,
    };

    validate_transition(current, &new_status)?;
    Ok(new_status)
}

/// Calculate session duration
#[allow(dead_code)] // Used in tests, available for future functional refactoring
pub fn calculate_duration(
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
) -> Option<Duration> {
    completed_at.map(|end| end.signed_duration_since(started_at))
}

/// Apply a status update to a session (pure function)
pub fn apply_status_update(session: &mut UnifiedSession, status: SessionStatus) -> Result<()> {
    validate_transition(&session.status, &status)?;

    // Clear completed_at when resuming from Failed
    if session.status == SessionStatus::Failed && status == SessionStatus::Running {
        session.completed_at = None;
    }

    session.status = status;
    session.updated_at = Utc::now();

    if matches!(
        session.status,
        SessionStatus::Completed | SessionStatus::Failed
    ) {
        session.completed_at = Some(Utc::now());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use SessionStatus::*;

    #[test]
    fn test_validate_transition_valid() {
        assert!(validate_transition(&Initializing, &Running).is_ok());
        assert!(validate_transition(&Running, &Paused).is_ok());
        assert!(validate_transition(&Running, &Completed).is_ok());
        assert!(validate_transition(&Running, &Failed).is_ok());
        assert!(validate_transition(&Paused, &Running).is_ok());
        assert!(validate_transition(&Paused, &Completed).is_ok());
    }

    #[test]
    fn test_validate_transition_invalid() {
        assert!(validate_transition(&Completed, &Running).is_err());
        assert!(validate_transition(&Cancelled, &Running).is_err());
        assert!(validate_transition(&Completed, &Paused).is_err());
    }

    #[test]
    fn test_validate_transition_failed_to_running() {
        // Failed -> Running is allowed for resume functionality
        assert!(validate_transition(&Failed, &Running).is_ok());
        // But Failed cannot transition to other states
        assert!(validate_transition(&Failed, &Paused).is_err());
        assert!(validate_transition(&Failed, &Completed).is_err());
    }

    #[test]
    fn test_validate_transition_idempotent() {
        assert!(validate_transition(&Running, &Running).is_ok());
        assert!(validate_transition(&Paused, &Paused).is_ok());
        assert!(validate_transition(&Initializing, &Initializing).is_ok());
    }

    #[test]
    fn test_transition_status() {
        assert_eq!(
            transition_status(&Initializing, Transition::Start).unwrap(),
            Running
        );
        assert_eq!(
            transition_status(&Running, Transition::Pause).unwrap(),
            Paused
        );
        assert_eq!(
            transition_status(&Paused, Transition::Resume).unwrap(),
            Running
        );
        assert_eq!(
            transition_status(&Running, Transition::Complete).unwrap(),
            Completed
        );
        assert_eq!(
            transition_status(&Running, Transition::Fail).unwrap(),
            Failed
        );
    }

    #[test]
    fn test_transition_status_invalid() {
        assert!(transition_status(&Completed, Transition::Start).is_err());
        assert!(transition_status(&Cancelled, Transition::Resume).is_err());
    }

    #[test]
    fn test_transition_status_resume_from_failed() {
        // Resume from Failed should work (for retry functionality)
        assert!(transition_status(&Failed, Transition::Resume).is_ok());
        assert_eq!(
            transition_status(&Failed, Transition::Resume).unwrap(),
            Running
        );
    }

    #[test]
    fn test_calculate_duration() {
        let start = Utc::now();
        let end = start + Duration::hours(2);

        let duration = calculate_duration(start, Some(end)).unwrap();
        assert_eq!(duration.num_hours(), 2);
    }

    #[test]
    fn test_calculate_duration_none() {
        let start = Utc::now();
        assert!(calculate_duration(start, None).is_none());
    }
}
