//! Session state machine implementation

use super::{CommitInfo, ExecutedCommand, IterationChanges};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

/// Core session states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session has been created but not started
    Created,
    /// Session is actively running
    Running { iteration: u32 },
    /// Session has been paused
    Paused { reason: String },
    /// Session completed successfully
    Completed { summary: SessionSummary },
    /// Session failed with error
    Failed { error: String },
}

impl SessionState {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionState::Completed { .. } | SessionState::Failed { .. })
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        matches!(self, SessionState::Running { .. })
    }

    /// Get current iteration if running
    pub fn current_iteration(&self) -> Option<u32> {
        match self {
            SessionState::Running { iteration } => Some(*iteration),
            _ => None,
        }
    }
}

/// Summary of a completed session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    pub total_iterations: u32,
    pub files_changed: usize,
    pub total_commits: usize,
    pub duration: Duration,
    pub success_rate: f64,
}

/// Progress tracking for a session
#[derive(Debug, Clone)]
pub struct SessionProgress {
    pub state: SessionState,
    pub iterations_completed: u32,
    pub total_iterations: u32,
    pub files_changed: HashSet<PathBuf>,
    pub commands_executed: Vec<ExecutedCommand>,
    pub duration: Duration,
    pub current_phase: Option<String>,
    pub iteration_changes: Vec<IterationChanges>,
}

impl SessionProgress {
    /// Create new progress tracker
    pub fn new(total_iterations: u32) -> Self {
        Self {
            state: SessionState::Created,
            iterations_completed: 0,
            total_iterations,
            files_changed: HashSet::new(),
            commands_executed: Vec::new(),
            duration: Duration::default(),
            current_phase: None,
            iteration_changes: Vec::new(),
        }
    }

    /// Calculate completion percentage
    pub fn completion_percentage(&self) -> f64 {
        if self.total_iterations == 0 {
            return 0.0;
        }
        (self.iterations_completed as f64 / self.total_iterations as f64) * 100.0
    }

    /// Get total lines changed
    pub fn total_lines_changed(&self) -> usize {
        self.iteration_changes
            .iter()
            .map(|c| c.lines_added + c.lines_removed)
            .sum()
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        if self.commands_executed.is_empty() {
            return 1.0;
        }
        let successful = self.commands_executed.iter().filter(|c| c.success).count();
        successful as f64 / self.commands_executed.len() as f64
    }

    /// Get all git commits
    pub fn all_commits(&self) -> Vec<&CommitInfo> {
        self.iteration_changes
            .iter()
            .flat_map(|c| &c.git_commits)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_transitions() {
        let state = SessionState::Created;
        assert!(!state.is_terminal());
        assert!(!state.is_active());
        assert_eq!(state.current_iteration(), None);

        let state = SessionState::Running { iteration: 5 };
        assert!(!state.is_terminal());
        assert!(state.is_active());
        assert_eq!(state.current_iteration(), Some(5));

        let state = SessionState::Completed {
            summary: SessionSummary {
                total_iterations: 10,
                files_changed: 5,
                total_commits: 3,
                duration: Duration::from_secs(300),
                success_rate: 0.95,
            },
        };
        assert!(state.is_terminal());
        assert!(!state.is_active());
    }

    #[test]
    fn test_session_progress() {
        let mut progress = SessionProgress::new(10);
        assert_eq!(progress.completion_percentage(), 0.0);

        progress.iterations_completed = 5;
        assert_eq!(progress.completion_percentage(), 50.0);

        progress.commands_executed.push(ExecutedCommand {
            command: "test".to_string(),
            success: true,
            duration: Duration::from_secs(1),
            output_size: 100,
        });
        progress.commands_executed.push(ExecutedCommand {
            command: "test2".to_string(),
            success: false,
            duration: Duration::from_secs(1),
            output_size: 50,
        });
        assert_eq!(progress.success_rate(), 0.5);
    }
}