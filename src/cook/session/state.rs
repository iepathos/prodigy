//! Session state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Status of a cooking session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    /// Session is actively running
    InProgress,
    /// Session completed successfully
    Completed,
    /// Session failed with error
    Failed,
    /// Session was interrupted
    Interrupted,
}

/// State of a cooking session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Unique session identifier
    pub session_id: String,
    /// Current status
    pub status: SessionStatus,
    /// When session started
    pub started_at: DateTime<Utc>,
    /// When session ended (if applicable)
    pub ended_at: Option<DateTime<Utc>>,
    /// Number of iterations completed
    pub iterations_completed: usize,
    /// Total files changed
    pub files_changed: usize,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// Working directory
    pub working_directory: PathBuf,
    /// Worktree name if using worktree
    pub worktree_name: Option<String>,
    /// When workflow started
    pub workflow_started_at: Option<DateTime<Utc>>,
    /// Current iteration start time
    pub current_iteration_started_at: Option<DateTime<Utc>>,
    /// Current iteration number
    pub current_iteration_number: Option<u32>,
    /// Iteration timings (iteration number, duration)
    pub iteration_timings: Vec<(u32, Duration)>,
    /// Command timings (command name, duration)
    pub command_timings: Vec<(String, Duration)>,
}

impl SessionState {
    /// Create a new session state
    pub fn new(session_id: String, working_directory: PathBuf) -> Self {
        Self {
            session_id,
            status: SessionStatus::InProgress,
            started_at: Utc::now(),
            ended_at: None,
            iterations_completed: 0,
            files_changed: 0,
            errors: Vec::new(),
            working_directory,
            worktree_name: None,
            workflow_started_at: None,
            current_iteration_started_at: None,
            current_iteration_number: None,
            iteration_timings: Vec::new(),
            command_timings: Vec::new(),
        }
    }

    /// Mark session as completed
    pub fn complete(&mut self) {
        self.status = SessionStatus::Completed;
        self.ended_at = Some(Utc::now());
    }

    /// Mark session as failed
    pub fn fail(&mut self, error: String) {
        self.status = SessionStatus::Failed;
        self.ended_at = Some(Utc::now());
        self.errors.push(error);
    }

    /// Mark session as interrupted
    pub fn interrupt(&mut self) {
        self.status = SessionStatus::Interrupted;
        self.ended_at = Some(Utc::now());
    }

    /// Add files changed count
    pub fn add_files_changed(&mut self, count: usize) {
        self.files_changed += count;
    }

    /// Increment iteration count
    pub fn increment_iteration(&mut self) {
        self.iterations_completed += 1;
    }

    /// Get session duration
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.ended_at
            .map(|end| end.signed_duration_since(self.started_at))
    }
}
