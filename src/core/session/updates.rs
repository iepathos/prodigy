//! Pure session update functions for immutable session transformations
//!
//! This module provides pure functions that transform `UnifiedSession` instances
//! without mutation. All functions take ownership of the session and return a new
//! modified session, preserving immutability.
//!
//! # Design Principles
//!
//! - **Immutability**: Input sessions are never mutated; new sessions are returned
//! - **Purity**: No I/O operations, no side effects
//! - **Validation**: State transitions are validated before applying
//! - **Testability**: All functions can be tested without mocking
//!
//! # Examples
//!
//! ```
//! use prodigy::core::session::updates::{apply_session_update, SessionUpdate, ProgressUpdate};
//! use prodigy::unified_session::UnifiedSession;
//!
//! let session = UnifiedSession::new_workflow("wf-1".to_string(), "test".to_string());
//!
//! // Apply a progress update
//! let update = SessionUpdate::Progress(ProgressUpdate {
//!     completed_steps: 5,
//!     failed_steps: 1,
//!     current_step: Some("step-6".to_string()),
//! });
//!
//! let updated = apply_session_update(session, update).unwrap();
//! ```

use super::validation::{validate_status_transition, SessionTransitionError};
use crate::unified_session::{SessionStatus, UnifiedSession};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Error type for session updates
#[derive(Debug, Clone, PartialEq)]
pub enum SessionUpdateError {
    /// Invalid state transition
    InvalidTransition(SessionTransitionError),
    /// Missing required session data
    MissingSessionData { session_type: String },
}

impl std::fmt::Display for SessionUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionUpdateError::InvalidTransition(err) => write!(f, "{}", err),
            SessionUpdateError::MissingSessionData { session_type } => {
                write!(f, "Missing {} session data", session_type)
            }
        }
    }
}

impl std::error::Error for SessionUpdateError {}

impl From<SessionTransitionError> for SessionUpdateError {
    fn from(err: SessionTransitionError) -> Self {
        SessionUpdateError::InvalidTransition(err)
    }
}

/// Update types for session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionUpdate {
    /// Update session status with validation
    Status(SessionStatus),
    /// Update progress counters
    Progress(ProgressUpdate),
    /// Merge new variables with existing
    Variables(HashMap<String, Value>),
    /// Add a step to execution history
    AddStep(StepRecord),
}

/// Progress update data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Number of steps completed in this update (added to total)
    pub completed_steps: usize,
    /// Number of steps failed in this update (added to total)
    pub failed_steps: usize,
    /// Current step being executed (replaces existing)
    pub current_step: Option<String>,
}

/// Record of a command execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    /// Command that was executed
    pub command: String,
    /// When the step started
    pub started_at: DateTime<Utc>,
    /// When the step completed (if finished)
    pub completed_at: Option<DateTime<Utc>>,
    /// Status of the step (e.g., "running", "completed", "failed")
    pub status: String,
    /// Optional output or result
    pub output: Option<String>,
}

impl StepRecord {
    /// Create a new step record for a started command
    pub fn started(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            started_at: Utc::now(),
            completed_at: None,
            status: "running".to_string(),
            output: None,
        }
    }

    /// Mark the step as completed with optional output
    pub fn complete(self, output: Option<String>) -> Self {
        Self {
            completed_at: Some(Utc::now()),
            status: "completed".to_string(),
            output,
            ..self
        }
    }

    /// Mark the step as failed with error message
    pub fn fail(self, error: impl Into<String>) -> Self {
        Self {
            completed_at: Some(Utc::now()),
            status: "failed".to_string(),
            output: Some(error.into()),
            ..self
        }
    }
}

/// Pure: Apply a session update, returning a new session
///
/// This function dispatches to the appropriate update function based on the
/// update type. The `updated_at` timestamp is always updated.
///
/// # Arguments
///
/// * `session` - The session to update (takes ownership)
/// * `update` - The update to apply
///
/// # Returns
///
/// A new session with the update applied, or an error if the update is invalid.
///
/// # Examples
///
/// ```
/// use prodigy::core::session::updates::{apply_session_update, SessionUpdate};
/// use prodigy::unified_session::{SessionStatus, UnifiedSession};
///
/// let session = UnifiedSession::new_workflow("wf-1".to_string(), "test".to_string());
///
/// // Transition from Initializing to Running
/// let result = apply_session_update(
///     session,
///     SessionUpdate::Status(SessionStatus::Running),
/// );
///
/// assert!(result.is_ok());
/// assert_eq!(result.unwrap().status, SessionStatus::Running);
/// ```
pub fn apply_session_update(
    session: UnifiedSession,
    update: SessionUpdate,
) -> Result<UnifiedSession, SessionUpdateError> {
    let updated = UnifiedSession {
        updated_at: Utc::now(),
        ..session
    };

    match update {
        SessionUpdate::Status(status) => apply_status_update(updated, status),
        SessionUpdate::Progress(progress) => apply_progress_update(updated, progress),
        SessionUpdate::Variables(vars) => apply_variable_update(updated, vars),
        SessionUpdate::AddStep(step) => apply_add_step(updated, step),
    }
}

/// Pure: Apply status update with validation
///
/// Validates the status transition before applying. Returns an error if the
/// transition is not valid.
///
/// # Arguments
///
/// * `session` - The session to update
/// * `status` - The new status
///
/// # Returns
///
/// A new session with updated status, or an error for invalid transitions.
pub fn apply_status_update(
    session: UnifiedSession,
    status: SessionStatus,
) -> Result<UnifiedSession, SessionUpdateError> {
    // Validate state transition
    validate_status_transition(&session.status, &status)?;

    // Set completed_at for terminal states
    let completed_at = match &status {
        SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Cancelled => {
            Some(Utc::now())
        }
        _ => session.completed_at,
    };

    Ok(UnifiedSession {
        status,
        completed_at,
        ..session
    })
}

/// Pure: Apply progress update to workflow session
///
/// Increments completed_steps and failed_steps counters, and updates current_step.
/// For workflow sessions, this updates the `workflow_data.current_step` field.
///
/// # Arguments
///
/// * `session` - The session to update
/// * `progress` - The progress update to apply
///
/// # Returns
///
/// A new session with updated progress.
pub fn apply_progress_update(
    session: UnifiedSession,
    progress: ProgressUpdate,
) -> Result<UnifiedSession, SessionUpdateError> {
    // Update workflow_data if present
    let workflow_data = session.workflow_data.map(|mut wd| {
        // Increment completed steps
        for _ in 0..progress.completed_steps {
            if wd.current_step < wd.total_steps {
                wd.completed_steps.push(wd.current_step);
                wd.current_step += 1;
            }
        }
        wd
    });

    // Update mapreduce_data if present
    let mapreduce_data = session.mapreduce_data.map(|mut md| {
        md.processed_items = md.processed_items.saturating_add(progress.completed_steps);
        md.failed_items = md.failed_items.saturating_add(progress.failed_steps);
        md
    });

    // Store current step in metadata if provided
    let metadata = if let Some(ref step) = progress.current_step {
        let mut new_metadata = session.metadata.clone();
        new_metadata.insert("current_step".to_string(), Value::String(step.clone()));
        new_metadata
    } else {
        session.metadata.clone()
    };

    Ok(UnifiedSession {
        workflow_data,
        mapreduce_data,
        metadata,
        ..session
    })
}

/// Pure: Apply variable update (merge)
///
/// Merges new variables with existing variables. New values overwrite existing
/// values with the same key. Existing variables not in the update are preserved.
///
/// # Arguments
///
/// * `session` - The session to update
/// * `new_vars` - Variables to merge
///
/// # Returns
///
/// A new session with merged variables.
pub fn apply_variable_update(
    session: UnifiedSession,
    new_vars: HashMap<String, Value>,
) -> Result<UnifiedSession, SessionUpdateError> {
    let mut metadata = session.metadata.clone();
    metadata.extend(new_vars);

    // Also update workflow variables if workflow session
    let workflow_data = session.workflow_data.map(|mut wd| {
        for (key, value) in metadata.iter() {
            if let Some(s) = value.as_str() {
                wd.variables.insert(key.clone(), s.to_string());
            }
        }
        wd
    });

    Ok(UnifiedSession {
        metadata,
        workflow_data,
        ..session
    })
}

/// Pure: Add step to execution history
///
/// Appends a step record to the session's execution history, preserving
/// chronological order.
///
/// # Arguments
///
/// * `session` - The session to update
/// * `step` - The step record to add
///
/// # Returns
///
/// A new session with the step added to metadata.
pub fn apply_add_step(
    session: UnifiedSession,
    step: StepRecord,
) -> Result<UnifiedSession, SessionUpdateError> {
    let mut metadata = session.metadata.clone();

    // Get or create steps array
    let steps_key = "execution_steps".to_string();
    let mut steps: Vec<Value> = metadata
        .get(&steps_key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Add new step
    let step_value = serde_json::to_value(&step).unwrap_or(Value::Null);
    steps.push(step_value);

    metadata.insert(steps_key, Value::Array(steps));

    Ok(UnifiedSession {
        metadata,
        ..session
    })
}

/// Apply multiple updates in sequence
///
/// Applies each update in order, stopping on first error.
pub fn apply_updates(
    session: UnifiedSession,
    updates: Vec<SessionUpdate>,
) -> Result<UnifiedSession, SessionUpdateError> {
    updates.into_iter().try_fold(session, apply_session_update)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session() -> UnifiedSession {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        // Set total_steps for progress testing
        if let Some(ref mut wd) = session.workflow_data {
            wd.total_steps = 10;
        }
        session
    }

    fn create_test_mapreduce_session() -> UnifiedSession {
        UnifiedSession::new_mapreduce("test-job".to_string(), 100)
    }

    // Status update tests

    #[test]
    fn test_apply_status_update_initializing_to_running() {
        let session = create_test_session();
        assert_eq!(session.status, SessionStatus::Initializing);

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Running));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Running);
        assert!(updated.completed_at.is_none());
    }

    #[test]
    fn test_apply_status_update_running_to_completed() {
        let mut session = create_test_session();
        session.status = SessionStatus::Running;

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Completed));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Completed);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn test_apply_status_update_running_to_failed() {
        let mut session = create_test_session();
        session.status = SessionStatus::Running;

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Failed));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Failed);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn test_apply_status_update_running_to_paused() {
        let mut session = create_test_session();
        session.status = SessionStatus::Running;

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Paused));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Paused);
        assert!(updated.completed_at.is_none());
    }

    #[test]
    fn test_apply_status_update_paused_to_running() {
        let mut session = create_test_session();
        session.status = SessionStatus::Paused;

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Running));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Running);
    }

    #[test]
    fn test_apply_status_update_paused_to_cancelled() {
        let mut session = create_test_session();
        session.status = SessionStatus::Paused;

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Cancelled));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Cancelled);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn test_apply_status_update_invalid_completed_to_running() {
        let mut session = create_test_session();
        session.status = SessionStatus::Completed;

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Running));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SessionUpdateError::InvalidTransition(_)));
    }

    #[test]
    fn test_apply_status_update_invalid_initializing_to_completed() {
        let session = create_test_session();

        let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Completed));

        assert!(result.is_err());
    }

    // Progress update tests

    #[test]
    fn test_apply_progress_update_workflow() {
        let session = create_test_session();

        let result = apply_session_update(
            session,
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 3,
                failed_steps: 0,
                current_step: Some("step-4".to_string()),
            }),
        );

        assert!(result.is_ok());
        let updated = result.unwrap();

        // Check workflow data updated
        let wd = updated.workflow_data.unwrap();
        assert_eq!(wd.completed_steps.len(), 3);
        assert_eq!(wd.current_step, 3);

        // Check current_step in metadata
        assert_eq!(
            updated.metadata.get("current_step"),
            Some(&Value::String("step-4".to_string()))
        );
    }

    #[test]
    fn test_apply_progress_update_mapreduce() {
        let session = create_test_mapreduce_session();

        let result = apply_session_update(
            session,
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 10,
                failed_steps: 2,
                current_step: None,
            }),
        );

        assert!(result.is_ok());
        let updated = result.unwrap();

        let md = updated.mapreduce_data.unwrap();
        assert_eq!(md.processed_items, 10);
        assert_eq!(md.failed_items, 2);
    }

    #[test]
    fn test_apply_progress_update_accumulates() {
        let mut session = create_test_session();

        // First update
        session = apply_session_update(
            session,
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 3,
                failed_steps: 0,
                current_step: None,
            }),
        )
        .unwrap();

        // Second update
        session = apply_session_update(
            session,
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 2,
                failed_steps: 1,
                current_step: None,
            }),
        )
        .unwrap();

        let wd = session.workflow_data.unwrap();
        assert_eq!(wd.completed_steps.len(), 5); // 3 + 2
    }

    // Variable update tests

    #[test]
    fn test_apply_variable_update_new_vars() {
        let session = create_test_session();

        let mut new_vars = HashMap::new();
        new_vars.insert("key1".to_string(), Value::String("value1".to_string()));
        new_vars.insert("key2".to_string(), Value::Number(42.into()));

        let result = apply_session_update(session, SessionUpdate::Variables(new_vars));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.metadata.len(), 2);
        assert_eq!(
            updated.metadata.get("key1"),
            Some(&Value::String("value1".to_string()))
        );
        assert_eq!(
            updated.metadata.get("key2"),
            Some(&Value::Number(42.into()))
        );
    }

    #[test]
    fn test_apply_variable_update_merges_with_existing() {
        let mut session = create_test_session();
        session
            .metadata
            .insert("existing".to_string(), Value::String("old".to_string()));

        let mut new_vars = HashMap::new();
        new_vars.insert(
            "new_key".to_string(),
            Value::String("new_value".to_string()),
        );

        let result = apply_session_update(session, SessionUpdate::Variables(new_vars));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.metadata.len(), 2);
        assert!(updated.metadata.contains_key("existing"));
        assert!(updated.metadata.contains_key("new_key"));
    }

    #[test]
    fn test_apply_variable_update_overwrites_existing() {
        let mut session = create_test_session();
        session
            .metadata
            .insert("key".to_string(), Value::String("old".to_string()));

        let mut new_vars = HashMap::new();
        new_vars.insert("key".to_string(), Value::String("new".to_string()));

        let result = apply_session_update(session, SessionUpdate::Variables(new_vars));

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(
            updated.metadata.get("key"),
            Some(&Value::String("new".to_string()))
        );
    }

    // Step record tests

    #[test]
    fn test_apply_add_step() {
        let session = create_test_session();

        let step = StepRecord {
            command: "echo hello".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            status: "running".to_string(),
            output: None,
        };

        let result = apply_session_update(session, SessionUpdate::AddStep(step));

        assert!(result.is_ok());
        let updated = result.unwrap();

        let steps = updated
            .metadata
            .get("execution_steps")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(steps.len(), 1);
    }

    #[test]
    fn test_apply_add_step_preserves_order() {
        let mut session = create_test_session();

        // Add first step
        let step1 = StepRecord::started("step 1");
        session = apply_session_update(session, SessionUpdate::AddStep(step1)).unwrap();

        // Add second step
        let step2 = StepRecord::started("step 2");
        session = apply_session_update(session, SessionUpdate::AddStep(step2)).unwrap();

        // Add third step
        let step3 = StepRecord::started("step 3");
        session = apply_session_update(session, SessionUpdate::AddStep(step3)).unwrap();

        let steps = session
            .metadata
            .get("execution_steps")
            .and_then(|v| v.as_array())
            .unwrap();

        assert_eq!(steps.len(), 3);

        // Verify order
        assert_eq!(
            steps[0].get("command").and_then(|v| v.as_str()),
            Some("step 1")
        );
        assert_eq!(
            steps[1].get("command").and_then(|v| v.as_str()),
            Some("step 2")
        );
        assert_eq!(
            steps[2].get("command").and_then(|v| v.as_str()),
            Some("step 3")
        );
    }

    #[test]
    fn test_step_record_lifecycle() {
        let step = StepRecord::started("test command");
        assert_eq!(step.status, "running");
        assert!(step.completed_at.is_none());

        let completed = step.clone().complete(Some("output".to_string()));
        assert_eq!(completed.status, "completed");
        assert!(completed.completed_at.is_some());
        assert_eq!(completed.output, Some("output".to_string()));

        let failed = StepRecord::started("failing command").fail("error message");
        assert_eq!(failed.status, "failed");
        assert!(failed.completed_at.is_some());
        assert_eq!(failed.output, Some("error message".to_string()));
    }

    // Multiple updates test

    #[test]
    fn test_apply_updates_sequence() {
        let session = create_test_session();

        let updates = vec![
            SessionUpdate::Status(SessionStatus::Running),
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 5,
                failed_steps: 0,
                current_step: Some("step-5".to_string()),
            }),
            SessionUpdate::Variables({
                let mut m = HashMap::new();
                m.insert("result".to_string(), Value::String("success".to_string()));
                m
            }),
        ];

        let result = apply_updates(session, updates);

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Running);
        assert!(updated.metadata.contains_key("result"));
    }

    #[test]
    fn test_apply_updates_stops_on_error() {
        let session = create_test_session();

        let updates = vec![
            SessionUpdate::Status(SessionStatus::Running),
            // This should fail - can't go from Running directly to Initializing
            SessionUpdate::Status(SessionStatus::Initializing),
            // This should never be applied
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 100,
                failed_steps: 0,
                current_step: None,
            }),
        ];

        let result = apply_updates(session, updates);

        assert!(result.is_err());
    }

    // Immutability tests

    #[test]
    fn test_updates_preserve_immutability() {
        let original = create_test_session();
        let original_id = original.id.clone();
        let original_status = original.status.clone();

        let updated = apply_session_update(
            original.clone(),
            SessionUpdate::Status(SessionStatus::Running),
        )
        .unwrap();

        // Original unchanged
        assert_eq!(original.id, original_id);
        assert_eq!(original.status, original_status);
        assert_eq!(original.status, SessionStatus::Initializing);

        // Updated has changes
        assert_eq!(updated.id, original_id);
        assert_eq!(updated.status, SessionStatus::Running);
    }

    #[test]
    fn test_updated_at_always_changes() {
        let session = create_test_session();
        let original_updated_at = session.updated_at;

        // Small delay to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));

        let updated = apply_session_update(
            session,
            SessionUpdate::Variables(HashMap::new()), // Even empty update changes updated_at
        )
        .unwrap();

        assert!(updated.updated_at > original_updated_at);
    }

    // Edge cases

    #[test]
    fn test_progress_update_no_overflow() {
        let mut session = create_test_session();
        if let Some(ref mut wd) = session.workflow_data {
            wd.total_steps = 5;
        }

        // Try to complete more steps than total
        let result = apply_session_update(
            session,
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 100,
                failed_steps: 0,
                current_step: None,
            }),
        );

        assert!(result.is_ok());
        let updated = result.unwrap();
        let wd = updated.workflow_data.unwrap();
        // Should cap at total_steps
        assert!(wd.completed_steps.len() <= 5);
    }

    #[test]
    fn test_empty_variable_update() {
        let mut session = create_test_session();
        session
            .metadata
            .insert("key".to_string(), Value::String("value".to_string()));

        let result = apply_session_update(session, SessionUpdate::Variables(HashMap::new()));

        assert!(result.is_ok());
        let updated = result.unwrap();
        // Original variables preserved
        assert!(updated.metadata.contains_key("key"));
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Strategy to generate arbitrary variable keys
    fn key_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,19}".prop_map(|s| s)
    }

    // Strategy to generate arbitrary string values
    fn value_strategy() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ]{0,50}".prop_map(|s| s)
    }

    fn create_test_session() -> UnifiedSession {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        if let Some(ref mut wd) = session.workflow_data {
            wd.total_steps = 100;
        }
        session
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: Updates preserve session ID
        #[test]
        fn prop_updates_preserve_session_id(
            completed in 0usize..50,
            failed in 0usize..50,
        ) {
            let session = create_test_session();
            let original_id = session.id.clone();

            let update = SessionUpdate::Progress(ProgressUpdate {
                completed_steps: completed,
                failed_steps: failed,
                current_step: None,
            });

            let result = apply_session_update(session, update);

            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap().id, original_id);
        }

        /// Property: Variable updates always succeed
        #[test]
        fn prop_variable_updates_always_succeed(
            keys in prop::collection::vec(key_strategy(), 0..10),
            values in prop::collection::vec(value_strategy(), 0..10),
        ) {
            let session = create_test_session();

            let mut new_vars = HashMap::new();
            for (key, value) in keys.into_iter().zip(values.into_iter()) {
                new_vars.insert(key, Value::String(value));
            }

            let result = apply_session_update(session, SessionUpdate::Variables(new_vars));

            prop_assert!(result.is_ok());
        }

        /// Property: Variable merge includes all new keys
        #[test]
        fn prop_variable_merge_includes_all_keys(
            existing_keys in prop::collection::vec(key_strategy(), 0..5),
            existing_values in prop::collection::vec(value_strategy(), 0..5),
            new_keys in prop::collection::vec(key_strategy(), 0..5),
            new_values in prop::collection::vec(value_strategy(), 0..5),
        ) {
            let mut session = create_test_session();

            // Add existing variables
            for (key, value) in existing_keys.iter().zip(existing_values.iter()) {
                session.metadata.insert(key.clone(), Value::String(value.clone()));
            }

            // Create new variables
            let mut new_vars = HashMap::new();
            for (key, value) in new_keys.iter().zip(new_values.iter()) {
                new_vars.insert(key.clone(), Value::String(value.clone()));
            }

            let result = apply_session_update(session, SessionUpdate::Variables(new_vars.clone()));

            prop_assert!(result.is_ok());
            let updated = result.unwrap();

            // All new keys should be present
            for key in new_vars.keys() {
                prop_assert!(updated.metadata.contains_key(key));
            }
        }

        /// Property: Progress updates are monotonically increasing
        #[test]
        fn prop_progress_updates_monotonic(
            updates in prop::collection::vec(0usize..10, 1..5),
        ) {
            let mut session = create_test_session();

            let mut total_completed = 0;
            for completed in updates {
                session = apply_session_update(
                    session,
                    SessionUpdate::Progress(ProgressUpdate {
                        completed_steps: completed,
                        failed_steps: 0,
                        current_step: None,
                    }),
                ).unwrap();

                total_completed += completed;

                let wd = session.workflow_data.as_ref().unwrap();
                prop_assert!(wd.completed_steps.len() <= total_completed);
            }
        }

        /// Property: Step records are always appended, never lost
        #[test]
        fn prop_steps_never_lost(
            commands in prop::collection::vec("[a-z]{1,10}", 1..10),
        ) {
            let mut session = create_test_session();

            for command in &commands {
                let step = StepRecord::started(command.clone());
                session = apply_session_update(session, SessionUpdate::AddStep(step)).unwrap();
            }

            let steps = session
                .metadata
                .get("execution_steps")
                .and_then(|v| v.as_array())
                .unwrap();

            prop_assert_eq!(steps.len(), commands.len());
        }

        /// Property: updated_at always changes (with small delay)
        #[test]
        fn prop_updated_at_changes(
            key in key_strategy(),
            value in value_strategy(),
        ) {
            let session = create_test_session();
            let original_updated = session.updated_at;

            // Small delay to ensure clock moves
            std::thread::sleep(std::time::Duration::from_millis(1));

            let mut vars = HashMap::new();
            vars.insert(key, Value::String(value));

            let result = apply_session_update(session, SessionUpdate::Variables(vars));

            prop_assert!(result.is_ok());
            let updated = result.unwrap();
            prop_assert!(updated.updated_at >= original_updated);
        }
    }
}
