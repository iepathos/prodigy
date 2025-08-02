//! Session state machine implementation

use super::{CommitInfo, ExecutedCommand, IterationChanges};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
        matches!(
            self,
            SessionState::Completed { .. } | SessionState::Failed { .. }
        )
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
    pub iteration_timings: Vec<IterationTiming>,
    pub workflow_timing: WorkflowTiming,
}

/// Timing information for a single iteration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IterationTiming {
    pub iteration_number: u32,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub command_timings: HashMap<String, Duration>,
    pub total_duration: Option<Duration>,
}

impl IterationTiming {
    /// Create a new iteration timing
    pub fn new(iteration_number: u32) -> Self {
        Self {
            iteration_number,
            start_time: chrono::Utc::now(),
            end_time: None,
            command_timings: HashMap::new(),
            total_duration: None,
        }
    }

    /// Complete the iteration timing
    pub fn complete(&mut self) {
        let end_time = chrono::Utc::now();
        self.total_duration = Some(
            end_time
                .signed_duration_since(self.start_time)
                .to_std()
                .unwrap_or_default(),
        );
        self.end_time = Some(end_time);
    }

    /// Add command timing
    pub fn add_command_timing(&mut self, command: String, duration: Duration) {
        self.command_timings.insert(command, duration);
    }
}

/// Workflow timing statistics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowTiming {
    pub total_duration: Duration,
    pub iteration_count: usize,
    pub average_iteration_time: Duration,
    pub slowest_iteration: Option<(u32, Duration)>,
    pub fastest_iteration: Option<(u32, Duration)>,
}

impl WorkflowTiming {
    /// Calculate workflow timing from iteration timings
    pub fn from_iterations(iterations: &[IterationTiming], total_duration: Duration) -> Self {
        let iteration_count = iterations.len();

        if iteration_count == 0 {
            return Self {
                total_duration,
                iteration_count: 0,
                average_iteration_time: Duration::ZERO,
                slowest_iteration: None,
                fastest_iteration: None,
            };
        }

        let mut slowest: Option<(u32, Duration)> = None;
        let mut fastest: Option<(u32, Duration)> = None;
        let mut total_iteration_time = Duration::ZERO;

        for iter in iterations {
            if let Some(duration) = iter.total_duration {
                total_iteration_time += duration;

                match &mut slowest {
                    None => slowest = Some((iter.iteration_number, duration)),
                    Some((_, d)) if duration > *d => {
                        slowest = Some((iter.iteration_number, duration))
                    }
                    _ => {}
                }

                match &mut fastest {
                    None => fastest = Some((iter.iteration_number, duration)),
                    Some((_, d)) if duration < *d => {
                        fastest = Some((iter.iteration_number, duration))
                    }
                    _ => {}
                }
            }
        }

        let average_iteration_time = if iteration_count > 0 {
            Duration::from_secs(total_iteration_time.as_secs() / iteration_count as u64)
        } else {
            Duration::ZERO
        };

        Self {
            total_duration,
            iteration_count,
            average_iteration_time,
            slowest_iteration: slowest,
            fastest_iteration: fastest,
        }
    }
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
    pub iteration_timings: Vec<IterationTiming>,
    pub current_iteration_timing: Option<IterationTiming>,
    pub workflow_start_time: Option<Instant>,
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
            iteration_timings: Vec::new(),
            current_iteration_timing: None,
            workflow_start_time: None,
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

    /// Start workflow timing
    pub fn start_workflow(&mut self) {
        self.workflow_start_time = Some(Instant::now());
    }

    /// Start a new iteration
    pub fn start_iteration(&mut self, iteration_number: u32) {
        let timing = IterationTiming::new(iteration_number);
        self.current_iteration_timing = Some(timing);
    }

    /// Complete current iteration
    pub fn complete_iteration(&mut self) {
        if let Some(mut timing) = self.current_iteration_timing.take() {
            timing.complete();
            self.iteration_timings.push(timing);
        }
    }

    /// Record command timing
    pub fn record_command_timing(&mut self, command: String, duration: Duration) {
        if let Some(ref mut timing) = self.current_iteration_timing {
            timing.add_command_timing(command, duration);
        }
    }

    /// Get workflow timing summary
    pub fn get_workflow_timing(&self) -> WorkflowTiming {
        let total_duration = self
            .workflow_start_time
            .map(|start| start.elapsed())
            .unwrap_or_default();
        WorkflowTiming::from_iterations(&self.iteration_timings, total_duration)
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
                iteration_timings: vec![],
                workflow_timing: WorkflowTiming {
                    total_duration: Duration::from_secs(300),
                    iteration_count: 10,
                    average_iteration_time: Duration::from_secs(30),
                    slowest_iteration: None,
                    fastest_iteration: None,
                },
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
