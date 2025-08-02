//! Event-driven session state management

use super::{IterationChanges, SessionConfig, SessionId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events that can occur during a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEvent {
    /// Session has started
    Started { config: SessionConfig },
    /// Iteration has started
    IterationStarted { number: u32 },
    /// Iteration completed with changes
    IterationCompleted { changes: IterationChanges },
    /// Analysis completed
    AnalysisCompleted { results: serde_json::Value },
    /// Command executed
    CommandExecuted { command: String, success: bool },
    /// Session paused
    Paused { reason: String },
    /// Session resumed
    Resumed,
    /// Session completed successfully
    Completed,
    /// Session failed
    Failed { error: String },
}

impl SessionEvent {
    /// Get a human-readable description of the event
    pub fn description(&self) -> String {
        match self {
            SessionEvent::Started { .. } => "Session started".to_string(),
            SessionEvent::IterationStarted { number } => {
                format!("Iteration {number} started")
            }
            SessionEvent::IterationCompleted { changes } => {
                format!(
                    "Iteration completed: {} files changed",
                    changes.files_modified.len()
                )
            }
            SessionEvent::AnalysisCompleted { .. } => "Analysis completed".to_string(),
            SessionEvent::CommandExecuted { command, success } => {
                format!(
                    "Command '{}' {}",
                    command,
                    if *success { "succeeded" } else { "failed" }
                )
            }
            SessionEvent::Paused { reason } => format!("Session paused: {reason}"),
            SessionEvent::Resumed => "Session resumed".to_string(),
            SessionEvent::Completed => "Session completed".to_string(),
            SessionEvent::Failed { error } => format!("Session failed: {error}"),
        }
    }

    /// Check if this event represents a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionEvent::Completed | SessionEvent::Failed { .. })
    }
}

/// Event with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedEvent {
    pub timestamp: DateTime<Utc>,
    pub event: SessionEvent,
}

impl TimestampedEvent {
    /// Create a new timestamped event
    pub fn new(event: SessionEvent) -> Self {
        Self {
            timestamp: Utc::now(),
            event,
        }
    }
}

/// Observer for session events
#[async_trait]
pub trait SessionObserver: Send + Sync {
    /// Called when an event occurs
    async fn on_event(&self, session_id: &SessionId, event: &SessionEvent);
}

/// No-op observer implementation
pub struct NoOpObserver;

#[async_trait]
impl SessionObserver for NoOpObserver {
    async fn on_event(&self, _session_id: &SessionId, _event: &SessionEvent) {
        // Do nothing
    }
}

/// Logging observer implementation
pub struct LoggingObserver {
    verbose: bool,
}

impl LoggingObserver {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

#[async_trait]
impl SessionObserver for LoggingObserver {
    async fn on_event(&self, session_id: &SessionId, event: &SessionEvent) {
        if self.verbose {
            println!("[{}] {}", session_id, event.description());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_event_descriptions() {
        let event = SessionEvent::Started {
            config: SessionConfig {
                project_path: PathBuf::from("/test"),
                workflow: crate::config::workflow::WorkflowConfig { commands: vec![] },
                execution_mode: crate::session::ExecutionMode::Direct,
                max_iterations: 10,
                options: Default::default(),
            },
        };
        assert_eq!(event.description(), "Session started");

        let event = SessionEvent::IterationStarted { number: 5 };
        assert_eq!(event.description(), "Iteration 5 started");

        let event = SessionEvent::Failed {
            error: "test error".to_string(),
        };
        assert_eq!(event.description(), "Session failed: test error");
        assert!(event.is_terminal());
    }

    #[test]
    fn test_timestamped_event() {
        let event = SessionEvent::Completed;
        let timestamped = TimestampedEvent::new(event.clone());
        assert!(timestamped.timestamp <= Utc::now());
        assert!(matches!(timestamped.event, SessionEvent::Completed));
    }
}
