//! Pure state transition validation for session status
//!
//! This module provides deterministic validation of session status transitions.
//! All functions are pure (no I/O, no side effects) and can be tested without mocking.
//!
//! # Valid State Transitions
//!
//! ```text
//!     Initializing
//!         │
//!         ▼
//!       Running ──────────────────┐
//!         │                       │
//!    ┌────┼────┐                  │
//!    │    │    │                  │
//!    ▼    ▼    ▼                  │
//! Paused Completed Failed         │
//!    │                            │
//!    ├──────────────────►Running──┘
//!    │
//!    ▼
//! Cancelled
//! ```

use crate::unified_session::SessionStatus;
use std::fmt;

/// Error type for session state transitions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTransitionError {
    /// Invalid state transition attempted
    InvalidTransition {
        from: SessionStatus,
        to: SessionStatus,
    },
}

impl fmt::Display for SessionTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionTransitionError::InvalidTransition { from, to } => {
                write!(f, "Invalid session transition from {:?} to {:?}", from, to)
            }
        }
    }
}

impl std::error::Error for SessionTransitionError {}

/// Pure: Validate status transition
///
/// Returns Ok(()) if the transition is valid, otherwise returns an error
/// describing the invalid transition.
///
/// # Valid Transitions
///
/// - Initializing → Running
/// - Running → Paused
/// - Running → Completed
/// - Running → Failed
/// - Paused → Running
/// - Paused → Cancelled
///
/// # Examples
///
/// ```
/// use prodigy::core::session::validation::validate_status_transition;
/// use prodigy::unified_session::SessionStatus;
///
/// // Valid transition
/// assert!(validate_status_transition(&SessionStatus::Initializing, &SessionStatus::Running).is_ok());
///
/// // Invalid transition
/// assert!(validate_status_transition(&SessionStatus::Completed, &SessionStatus::Running).is_err());
/// ```
pub fn validate_status_transition(
    from: &SessionStatus,
    to: &SessionStatus,
) -> Result<(), SessionTransitionError> {
    use SessionStatus::*;

    let valid = matches!(
        (from, to),
        (Initializing, Running)
            | (Running, Paused)
            | (Running, Completed)
            | (Running, Failed)
            | (Paused, Running)
            | (Paused, Cancelled)
    );

    if valid {
        Ok(())
    } else {
        Err(SessionTransitionError::InvalidTransition {
            from: from.clone(),
            to: to.clone(),
        })
    }
}

/// Pure: Check if a status is terminal (no further transitions allowed)
///
/// Terminal statuses are: Completed, Failed, Cancelled
pub fn is_terminal_status(status: &SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Cancelled
    )
}

/// Pure: Get all valid transitions from a given status
///
/// Returns a list of statuses that can be transitioned to from the given status.
pub fn valid_transitions_from(status: &SessionStatus) -> Vec<SessionStatus> {
    use SessionStatus::*;
    match status {
        Initializing => vec![Running],
        Running => vec![Paused, Completed, Failed],
        Paused => vec![Running, Cancelled],
        Completed | Failed | Cancelled => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        use SessionStatus::*;

        let valid_transitions = vec![
            (Initializing, Running),
            (Running, Paused),
            (Running, Completed),
            (Running, Failed),
            (Paused, Running),
            (Paused, Cancelled),
        ];

        for (from, to) in valid_transitions {
            assert!(
                validate_status_transition(&from, &to).is_ok(),
                "Transition {:?} -> {:?} should be valid",
                from,
                to
            );
        }
    }

    #[test]
    fn test_invalid_transitions() {
        use SessionStatus::*;

        let invalid_transitions = vec![
            // From terminal states
            (Completed, Running),
            (Completed, Paused),
            (Failed, Running),
            (Failed, Paused),
            (Cancelled, Running),
            (Cancelled, Paused),
            // Invalid from Initializing
            (Initializing, Completed),
            (Initializing, Failed),
            (Initializing, Paused),
            (Initializing, Cancelled),
            // Invalid from Running
            (Running, Initializing),
            (Running, Cancelled),
            // Invalid from Paused
            (Paused, Completed),
            (Paused, Failed),
            (Paused, Initializing),
        ];

        for (from, to) in invalid_transitions {
            assert!(
                validate_status_transition(&from, &to).is_err(),
                "Transition {:?} -> {:?} should be invalid",
                from,
                to
            );
        }
    }

    #[test]
    fn test_transition_error_display() {
        use SessionStatus::*;

        let error = SessionTransitionError::InvalidTransition {
            from: Completed,
            to: Running,
        };

        let display = format!("{}", error);
        assert!(display.contains("Completed"));
        assert!(display.contains("Running"));
        assert!(display.contains("Invalid"));
    }

    #[test]
    fn test_is_terminal_status() {
        use SessionStatus::*;

        // Terminal statuses
        assert!(is_terminal_status(&Completed));
        assert!(is_terminal_status(&Failed));
        assert!(is_terminal_status(&Cancelled));

        // Non-terminal statuses
        assert!(!is_terminal_status(&Initializing));
        assert!(!is_terminal_status(&Running));
        assert!(!is_terminal_status(&Paused));
    }

    #[test]
    fn test_valid_transitions_from_initializing() {
        let transitions = valid_transitions_from(&SessionStatus::Initializing);
        assert_eq!(transitions.len(), 1);
        assert!(transitions.contains(&SessionStatus::Running));
    }

    #[test]
    fn test_valid_transitions_from_running() {
        let transitions = valid_transitions_from(&SessionStatus::Running);
        assert_eq!(transitions.len(), 3);
        assert!(transitions.contains(&SessionStatus::Paused));
        assert!(transitions.contains(&SessionStatus::Completed));
        assert!(transitions.contains(&SessionStatus::Failed));
    }

    #[test]
    fn test_valid_transitions_from_paused() {
        let transitions = valid_transitions_from(&SessionStatus::Paused);
        assert_eq!(transitions.len(), 2);
        assert!(transitions.contains(&SessionStatus::Running));
        assert!(transitions.contains(&SessionStatus::Cancelled));
    }

    #[test]
    fn test_valid_transitions_from_terminal() {
        let terminal_statuses = vec![
            SessionStatus::Completed,
            SessionStatus::Failed,
            SessionStatus::Cancelled,
        ];

        for status in terminal_statuses {
            let transitions = valid_transitions_from(&status);
            assert!(
                transitions.is_empty(),
                "Terminal status {:?} should have no valid transitions",
                status
            );
        }
    }

    #[test]
    fn test_self_transitions_are_invalid() {
        use SessionStatus::*;

        let all_statuses = vec![Initializing, Running, Paused, Completed, Failed, Cancelled];

        for status in all_statuses {
            assert!(
                validate_status_transition(&status, &status).is_err(),
                "Self-transition {:?} -> {:?} should be invalid",
                status,
                status
            );
        }
    }
}
