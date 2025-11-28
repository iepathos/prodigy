//! Pure work item state machine
//!
//! This module contains pure functions for work item state transitions.
//! The state machine is fully deterministic and testable without I/O.

use crate::cook::execution::mapreduce::agent::AgentResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Work item status for state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkItemStatus {
    /// Item has not been started
    Pending,
    /// Item is currently being processed
    InProgress {
        agent_id: String,
        started_at: DateTime<Utc>,
    },
    /// Item completed successfully
    Completed { result: Box<AgentResult> },
    /// Item failed and may be retried
    Failed { error: String, retry_count: usize },
    /// Item exhausted retries and is in DLQ
    DeadLettered {
        error: String,
        retry_count: usize,
        dlq_at: DateTime<Utc>,
    },
}

impl WorkItemStatus {
    /// Get the status discriminant for comparison
    pub fn discriminant(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::InProgress { .. } => "InProgress",
            Self::Completed { .. } => "Completed",
            Self::Failed { .. } => "Failed",
            Self::DeadLettered { .. } => "DeadLettered",
        }
    }

    /// Check if this status matches another (by discriminant)
    pub fn matches(&self, other: &Self) -> bool {
        self.discriminant() == other.discriminant()
    }
}

impl WorkItemStatus {
    /// Check if the item can be retried
    pub fn can_retry(&self, max_retries: usize) -> bool {
        match self {
            Self::Failed { retry_count, .. } => *retry_count < max_retries,
            _ => false,
        }
    }

    /// Check if the item is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::DeadLettered { .. })
    }

    /// Check if the item needs processing
    pub fn needs_processing(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Events that trigger state transitions
#[derive(Debug, Clone)]
pub enum WorkItemEvent {
    /// Agent started processing the item
    AgentStart { agent_id: String },
    /// Agent completed successfully
    AgentComplete { result: Box<AgentResult> },
    /// Agent failed with an error
    AgentFailed { error: String },
    /// Workflow was interrupted
    Interrupt,
    /// Item should be retried
    Retry,
    /// Item should be moved to DLQ (max retries exhausted)
    MoveToDeadLetter,
}

/// Error during state transition
#[derive(Debug, Clone, thiserror::Error)]
pub enum TransitionError {
    #[error("Invalid transition from {current} with event {event}")]
    Invalid { current: String, event: String },
}

/// Pure: Transition work item state
///
/// Applies a state transition based on the current state and event.
/// Returns the new state or an error if the transition is invalid.
///
/// # State Machine
///
/// ```text
/// ┌─────────┐  agent_start   ┌────────────┐
/// │ Pending │ ─────────────> │ InProgress │
/// └────┬────┘                └──────┬─────┘
///      ^                            │
///      │                            │ agent_complete
///      │ interrupt/                 v
///      │ retry        ┌──────────┐  │    ┌────────┐
///      └──────────────┤ Completed│ <─────┤ Failed │
///                     └──────────┘       └────┬───┘
///                                              │
///                                              v (max retries)
///                                        ┌────────────┐
///                                        │DeadLettered│
///                                        └────────────┘
/// ```
pub fn transition_work_item(
    current: WorkItemStatus,
    event: WorkItemEvent,
) -> Result<WorkItemStatus, TransitionError> {
    match (&current, &event) {
        // Pending -> InProgress
        (WorkItemStatus::Pending, WorkItemEvent::AgentStart { agent_id }) => {
            Ok(WorkItemStatus::InProgress {
                agent_id: agent_id.clone(),
                started_at: Utc::now(),
            })
        }

        // InProgress -> Completed
        (WorkItemStatus::InProgress { .. }, WorkItemEvent::AgentComplete { result }) => {
            Ok(WorkItemStatus::Completed {
                result: result.clone(),
            })
        }

        // InProgress -> Failed
        (WorkItemStatus::InProgress { .. }, WorkItemEvent::AgentFailed { error }) => {
            Ok(WorkItemStatus::Failed {
                error: error.clone(),
                retry_count: 1,
            })
        }

        // InProgress -> Pending (on interrupt)
        (WorkItemStatus::InProgress { .. }, WorkItemEvent::Interrupt) => {
            Ok(WorkItemStatus::Pending)
        }

        // Failed -> Pending (on retry)
        (WorkItemStatus::Failed { .. }, WorkItemEvent::Retry) => {
            // Reset to pending for retry
            Ok(WorkItemStatus::Pending)
        }

        // Failed -> DeadLettered
        (
            WorkItemStatus::Failed {
                error, retry_count, ..
            },
            WorkItemEvent::MoveToDeadLetter,
        ) => Ok(WorkItemStatus::DeadLettered {
            error: error.clone(),
            retry_count: *retry_count,
            dlq_at: Utc::now(),
        }),

        // Failed -> Failed (increment retry count on another failure after retry)
        (
            WorkItemStatus::Failed {
                retry_count,
                error: _,
            },
            WorkItemEvent::AgentFailed { error },
        ) => Ok(WorkItemStatus::Failed {
            error: error.clone(),
            retry_count: retry_count + 1,
        }),

        // Invalid transitions
        _ => Err(TransitionError::Invalid {
            current: format!("{:?}", current),
            event: format!("{:?}", event),
        }),
    }
}

/// Pure: Apply interrupt to all in-progress items
///
/// Resets all in-progress items back to pending for resume.
pub fn apply_interrupt_to_all<I>(statuses: I) -> Vec<(String, WorkItemStatus)>
where
    I: IntoIterator<Item = (String, WorkItemStatus)>,
{
    statuses
        .into_iter()
        .map(|(id, status)| {
            let new_status = match status {
                WorkItemStatus::InProgress { .. } => WorkItemStatus::Pending,
                other => other,
            };
            (id, new_status)
        })
        .collect()
}

/// Pure: Count items by status
pub fn count_by_status<'a, I>(statuses: I) -> StatusCounts
where
    I: IntoIterator<Item = &'a WorkItemStatus>,
{
    let mut counts = StatusCounts::default();
    for status in statuses {
        match status {
            WorkItemStatus::Pending => counts.pending += 1,
            WorkItemStatus::InProgress { .. } => counts.in_progress += 1,
            WorkItemStatus::Completed { .. } => counts.completed += 1,
            WorkItemStatus::Failed { .. } => counts.failed += 1,
            WorkItemStatus::DeadLettered { .. } => counts.dead_lettered += 1,
        }
    }
    counts
}

/// Counts of items in each status
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusCounts {
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub failed: usize,
    pub dead_lettered: usize,
}

impl StatusCounts {
    pub fn total(&self) -> usize {
        self.pending + self.in_progress + self.completed + self.failed + self.dead_lettered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::agent::{AgentResult, AgentStatus};
    use std::time::Duration;

    fn mock_result() -> AgentResult {
        AgentResult {
            item_id: "test".to_string(),
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
        }
    }

    #[test]
    fn test_pending_to_in_progress() {
        let result = transition_work_item(
            WorkItemStatus::Pending,
            WorkItemEvent::AgentStart {
                agent_id: "agent-1".to_string(),
            },
        );
        assert!(matches!(result, Ok(WorkItemStatus::InProgress { .. })));
    }

    #[test]
    fn test_in_progress_to_completed() {
        let result = transition_work_item(
            WorkItemStatus::InProgress {
                agent_id: "agent-1".to_string(),
                started_at: Utc::now(),
            },
            WorkItemEvent::AgentComplete {
                result: Box::new(mock_result()),
            },
        );
        assert!(matches!(result, Ok(WorkItemStatus::Completed { .. })));
    }

    #[test]
    fn test_in_progress_to_failed() {
        let result = transition_work_item(
            WorkItemStatus::InProgress {
                agent_id: "agent-1".to_string(),
                started_at: Utc::now(),
            },
            WorkItemEvent::AgentFailed {
                error: "Test error".to_string(),
            },
        );
        assert!(matches!(
            result,
            Ok(WorkItemStatus::Failed { retry_count: 1, .. })
        ));
    }

    #[test]
    fn test_in_progress_to_pending_on_interrupt() {
        let result = transition_work_item(
            WorkItemStatus::InProgress {
                agent_id: "agent-1".to_string(),
                started_at: Utc::now(),
            },
            WorkItemEvent::Interrupt,
        );
        assert!(matches!(result, Ok(WorkItemStatus::Pending)));
    }

    #[test]
    fn test_failed_to_pending_on_retry() {
        let result = transition_work_item(
            WorkItemStatus::Failed {
                error: "Test error".to_string(),
                retry_count: 1,
            },
            WorkItemEvent::Retry,
        );
        assert!(matches!(result, Ok(WorkItemStatus::Pending)));
    }

    #[test]
    fn test_failed_to_dead_lettered() {
        let result = transition_work_item(
            WorkItemStatus::Failed {
                error: "Test error".to_string(),
                retry_count: 3,
            },
            WorkItemEvent::MoveToDeadLetter,
        );
        assert!(matches!(
            result,
            Ok(WorkItemStatus::DeadLettered { retry_count: 3, .. })
        ));
    }

    #[test]
    fn test_invalid_transition_pending_to_completed() {
        let result = transition_work_item(
            WorkItemStatus::Pending,
            WorkItemEvent::AgentComplete {
                result: Box::new(mock_result()),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_interrupt_to_all() {
        let items = vec![
            (
                "item-1".to_string(),
                WorkItemStatus::InProgress {
                    agent_id: "agent-1".to_string(),
                    started_at: Utc::now(),
                },
            ),
            ("item-2".to_string(), WorkItemStatus::Pending),
            (
                "item-3".to_string(),
                WorkItemStatus::Completed {
                    result: Box::new(mock_result()),
                },
            ),
        ];

        let result = apply_interrupt_to_all(items);

        assert_eq!(result.len(), 3);
        assert!(matches!(result[0].1, WorkItemStatus::Pending));
        assert!(matches!(result[1].1, WorkItemStatus::Pending));
        assert!(matches!(result[2].1, WorkItemStatus::Completed { .. }));
    }

    #[test]
    fn test_count_by_status() {
        let statuses = vec![
            WorkItemStatus::Pending,
            WorkItemStatus::Pending,
            WorkItemStatus::InProgress {
                agent_id: "agent-1".to_string(),
                started_at: Utc::now(),
            },
            WorkItemStatus::Completed {
                result: Box::new(mock_result()),
            },
            WorkItemStatus::Failed {
                error: "error".to_string(),
                retry_count: 1,
            },
        ];

        let counts = count_by_status(statuses.iter());

        assert_eq!(counts.pending, 2);
        assert_eq!(counts.in_progress, 1);
        assert_eq!(counts.completed, 1);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.dead_lettered, 0);
        assert_eq!(counts.total(), 5);
    }

    #[test]
    fn test_can_retry() {
        let failed = WorkItemStatus::Failed {
            error: "error".to_string(),
            retry_count: 2,
        };
        assert!(failed.can_retry(3));
        assert!(!failed.can_retry(2));
        assert!(!failed.can_retry(1));

        let completed = WorkItemStatus::Completed {
            result: Box::new(mock_result()),
        };
        assert!(!completed.can_retry(10));
    }

    #[test]
    fn test_is_terminal() {
        assert!(!WorkItemStatus::Pending.is_terminal());
        assert!(!WorkItemStatus::InProgress {
            agent_id: "a".to_string(),
            started_at: Utc::now()
        }
        .is_terminal());
        assert!(WorkItemStatus::Completed {
            result: Box::new(mock_result())
        }
        .is_terminal());
        assert!(!WorkItemStatus::Failed {
            error: "e".to_string(),
            retry_count: 1
        }
        .is_terminal());
        assert!(WorkItemStatus::DeadLettered {
            error: "e".to_string(),
            retry_count: 3,
            dlq_at: Utc::now()
        }
        .is_terminal());
    }
}
