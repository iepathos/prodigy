//! Pure agent lifecycle state machine
//!
//! This module implements a pure functional state machine for agent lifecycle
//! management. All functions are pure - they take state and transitions as
//! inputs and return new states, with no side effects or I/O operations.
//!
//! ## State Machine Diagram
//!
//! ```text
//! ┌─────────┐
//! │ Created │
//! └────┬────┘
//!      │
//!      │ Start { worktree_path }
//!      │
//!      ▼
//! ┌─────────┐
//! │ Running │
//! └────┬────┘
//!      │
//!      ├─────────────────────────┬──────────────────────────┐
//!      │                         │                          │
//!      │ Complete                │ Fail                     │
//!      │ { output, commits }     │ { error, json_log }      │
//!      │                         │                          │
//!      ▼                         ▼                          │
//! ┌───────────┐           ┌────────┐                      │
//! │ Completed │           │ Failed │                      │
//! └───────────┘           └────────┘                      │
//! ```
//!
//! ## Valid State Transitions
//!
//! - **Created → Running**: Agent starts execution with worktree path
//! - **Running → Completed**: Agent completes successfully with output and commits
//! - **Running → Failed**: Agent fails with error message and optional log location
//!
//! ## Invalid Transitions
//!
//! All other transitions are invalid and will return `StateError::InvalidTransition`:
//! - Created → Completed (must go through Running)
//! - Created → Failed (must go through Running)
//! - Completed → * (terminal state)
//! - Failed → * (terminal state)
//!
//! ## Usage Example
//!
//! ```rust
//! use prodigy::cook::execution::mapreduce::agent::{
//!     AgentLifecycleState, AgentTransition, apply_transition, state_to_result
//! };
//! use serde_json::json;
//! use std::path::PathBuf;
//!
//! // Create initial state
//! let state = AgentLifecycleState::Created {
//!     agent_id: "agent-1".to_string(),
//!     work_item: json!({"task": "build"}),
//! };
//!
//! // Transition to running
//! let transition = AgentTransition::Start {
//!     worktree_path: PathBuf::from("/tmp/worktree"),
//! };
//! let state = apply_transition(state, transition).unwrap();
//!
//! // Transition to completed
//! let transition = AgentTransition::Complete {
//!     output: Some("Build successful".to_string()),
//!     commits: vec!["abc123".to_string()],
//! };
//! let state = apply_transition(state, transition).unwrap();
//!
//! // Convert to result
//! let result = state_to_result(&state).unwrap();
//! assert!(result.is_success());
//! ```

use super::types::{AgentLifecycleState, AgentResult, AgentStatus, AgentTransition};
use std::time::Instant;

/// Error type for state transitions
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Invalid transition from {from} with {transition}")]
    InvalidTransition { from: String, transition: String },
}

/// Apply a transition to the current state
///
/// This is a pure function that takes a state and transition and returns
/// a new state. It validates that the transition is legal for the current state.
pub fn apply_transition(
    state: AgentLifecycleState,
    transition: AgentTransition,
) -> Result<AgentLifecycleState, StateError> {
    match (state, transition) {
        // Created -> Running
        (
            AgentLifecycleState::Created { agent_id, .. },
            AgentTransition::Start { worktree_path },
        ) => Ok(AgentLifecycleState::Running {
            agent_id,
            started_at: Instant::now(),
            worktree_path,
        }),

        // Running -> Completed
        (
            AgentLifecycleState::Running {
                agent_id,
                started_at,
                ..
            },
            AgentTransition::Complete { output, commits },
        ) => Ok(AgentLifecycleState::Completed {
            agent_id,
            output,
            commits,
            duration: started_at.elapsed(),
        }),

        // Running -> Failed
        (
            AgentLifecycleState::Running {
                agent_id,
                started_at,
                ..
            },
            AgentTransition::Fail {
                error,
                json_log_location,
            },
        ) => Ok(AgentLifecycleState::Failed {
            agent_id,
            error,
            duration: started_at.elapsed(),
            json_log_location,
        }),

        // Invalid transitions
        (state, transition) => Err(StateError::InvalidTransition {
            from: format!("{:?}", state),
            transition: format!("{:?}", transition),
        }),
    }
}

/// Convert a final agent state to an AgentResult
///
/// This is a pure function that extracts result information from terminal states.
/// Returns None for non-terminal states (Created, Running).
pub fn state_to_result(state: &AgentLifecycleState) -> Option<AgentResult> {
    match state {
        AgentLifecycleState::Completed {
            agent_id,
            output,
            commits,
            duration,
        } => Some(AgentResult {
            item_id: agent_id.clone(),
            status: AgentStatus::Success,
            output: output.clone(),
            commits: commits.clone(),
            files_modified: Vec::new(),
            duration: *duration,
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
        }),

        AgentLifecycleState::Failed {
            agent_id,
            error,
            duration,
            json_log_location,
        } => Some(AgentResult {
            item_id: agent_id.clone(),
            status: AgentStatus::Failed(error.clone()),
            output: None,
            commits: Vec::new(),
            files_modified: Vec::new(),
            duration: *duration,
            error: Some(error.clone()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: json_log_location.clone(),
        }),

        // Non-terminal states
        AgentLifecycleState::Created { .. } | AgentLifecycleState::Running { .. } => None,
    }
}

/// Validate that a transition is legal for the current state
///
/// This is a pure function that checks if a transition can be applied
/// without actually applying it.
pub fn is_valid_transition(state: &AgentLifecycleState, transition: &AgentTransition) -> bool {
    matches!(
        (state, transition),
        (
            AgentLifecycleState::Created { .. },
            AgentTransition::Start { .. }
        ) | (
            AgentLifecycleState::Running { .. },
            AgentTransition::Complete { .. }
        ) | (
            AgentLifecycleState::Running { .. },
            AgentTransition::Fail { .. }
        )
    )
}

/// Get the agent ID from any state
///
/// This is a pure function that extracts the agent ID from any state variant.
pub fn get_agent_id(state: &AgentLifecycleState) -> &str {
    match state {
        AgentLifecycleState::Created { agent_id, .. }
        | AgentLifecycleState::Running { agent_id, .. }
        | AgentLifecycleState::Completed { agent_id, .. }
        | AgentLifecycleState::Failed { agent_id, .. } => agent_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_created_to_running() {
        let state = AgentLifecycleState::Created {
            agent_id: "test-1".to_string(),
            work_item: json!({"id": 1}),
        };

        let transition = AgentTransition::Start {
            worktree_path: PathBuf::from("/tmp/worktree"),
        };

        let new_state = apply_transition(state, transition).unwrap();
        assert!(matches!(new_state, AgentLifecycleState::Running { .. }));
        assert_eq!(get_agent_id(&new_state), "test-1");
    }

    #[test]
    fn test_running_to_completed() {
        let state = AgentLifecycleState::Running {
            agent_id: "test-1".to_string(),
            started_at: Instant::now(),
            worktree_path: PathBuf::from("/tmp/worktree"),
        };

        let transition = AgentTransition::Complete {
            output: Some("success".to_string()),
            commits: vec!["abc123".to_string()],
        };

        let new_state = apply_transition(state, transition).unwrap();
        assert!(matches!(new_state, AgentLifecycleState::Completed { .. }));
        assert_eq!(get_agent_id(&new_state), "test-1");
    }

    #[test]
    fn test_running_to_failed() {
        let state = AgentLifecycleState::Running {
            agent_id: "test-1".to_string(),
            started_at: Instant::now(),
            worktree_path: PathBuf::from("/tmp/worktree"),
        };

        let transition = AgentTransition::Fail {
            error: "command failed".to_string(),
            json_log_location: Some("/tmp/log.json".to_string()),
        };

        let new_state = apply_transition(state, transition).unwrap();
        assert!(matches!(new_state, AgentLifecycleState::Failed { .. }));
        assert_eq!(get_agent_id(&new_state), "test-1");
    }

    #[test]
    fn test_invalid_transition_completed_to_running() {
        let state = AgentLifecycleState::Completed {
            agent_id: "test-1".to_string(),
            output: Some("done".to_string()),
            commits: vec![],
            duration: std::time::Duration::from_secs(10),
        };

        let transition = AgentTransition::Start {
            worktree_path: PathBuf::from("/tmp/worktree"),
        };

        let result = apply_transition(state, transition);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StateError::InvalidTransition { .. }
        ));
    }

    #[test]
    fn test_invalid_transition_created_to_completed() {
        let state = AgentLifecycleState::Created {
            agent_id: "test-1".to_string(),
            work_item: json!({"id": 1}),
        };

        let transition = AgentTransition::Complete {
            output: Some("output".to_string()),
            commits: vec!["abc123".to_string()],
        };

        let result = apply_transition(state, transition);
        assert!(result.is_err());
    }

    #[test]
    fn test_state_to_result_completed() {
        let state = AgentLifecycleState::Completed {
            agent_id: "test-1".to_string(),
            output: Some("output".to_string()),
            commits: vec!["abc123".to_string()],
            duration: std::time::Duration::from_secs(30),
        };

        let result = state_to_result(&state).unwrap();
        assert!(result.is_success());
        assert_eq!(result.item_id, "test-1");
        assert_eq!(result.output, Some("output".to_string()));
        assert_eq!(result.commits, vec!["abc123".to_string()]);
    }

    #[test]
    fn test_state_to_result_failed() {
        let state = AgentLifecycleState::Failed {
            agent_id: "test-1".to_string(),
            error: "command failed".to_string(),
            duration: std::time::Duration::from_secs(5),
            json_log_location: Some("/tmp/log.json".to_string()),
        };

        let result = state_to_result(&state).unwrap();
        assert!(!result.is_success());
        assert_eq!(result.item_id, "test-1");
        assert_eq!(result.error, Some("command failed".to_string()));
        assert_eq!(result.json_log_location, Some("/tmp/log.json".to_string()));
    }

    #[test]
    fn test_state_to_result_non_terminal() {
        let created = AgentLifecycleState::Created {
            agent_id: "test-1".to_string(),
            work_item: json!({"id": 1}),
        };
        assert!(state_to_result(&created).is_none());

        let running = AgentLifecycleState::Running {
            agent_id: "test-1".to_string(),
            started_at: Instant::now(),
            worktree_path: PathBuf::from("/tmp/worktree"),
        };
        assert!(state_to_result(&running).is_none());
    }

    #[test]
    fn test_is_valid_transition() {
        let created = AgentLifecycleState::Created {
            agent_id: "test-1".to_string(),
            work_item: json!({"id": 1}),
        };

        let start_transition = AgentTransition::Start {
            worktree_path: PathBuf::from("/tmp/worktree"),
        };
        assert!(is_valid_transition(&created, &start_transition));

        let complete_transition = AgentTransition::Complete {
            output: None,
            commits: vec![],
        };
        assert!(!is_valid_transition(&created, &complete_transition));
    }

    #[test]
    fn test_get_agent_id_all_states() {
        let created = AgentLifecycleState::Created {
            agent_id: "test-1".to_string(),
            work_item: json!({"id": 1}),
        };
        assert_eq!(get_agent_id(&created), "test-1");

        let running = AgentLifecycleState::Running {
            agent_id: "test-2".to_string(),
            started_at: Instant::now(),
            worktree_path: PathBuf::from("/tmp/worktree"),
        };
        assert_eq!(get_agent_id(&running), "test-2");

        let completed = AgentLifecycleState::Completed {
            agent_id: "test-3".to_string(),
            output: None,
            commits: vec![],
            duration: std::time::Duration::from_secs(10),
        };
        assert_eq!(get_agent_id(&completed), "test-3");

        let failed = AgentLifecycleState::Failed {
            agent_id: "test-4".to_string(),
            error: "error".to_string(),
            duration: std::time::Duration::from_secs(5),
            json_log_location: None,
        };
        assert_eq!(get_agent_id(&failed), "test-4");
    }
}
