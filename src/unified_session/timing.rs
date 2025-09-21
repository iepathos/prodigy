//! Timing tracker for measuring execution durations
//!
//! Provides accurate time measurement for iterations and commands,
//! handling clock adjustments gracefully for long-running sessions.

use std::time::{Duration, Instant};

/// Tracks timing information for sessions
#[derive(Debug, Clone)]
pub struct TimingTracker {
    /// Current iteration start time
    current_iteration_start: Option<Instant>,
    /// Current command start time
    current_command_start: Option<Instant>,
    /// Current command name
    current_command_name: Option<String>,
}

impl TimingTracker {
    /// Create a new timing tracker
    pub fn new() -> Self {
        Self {
            current_iteration_start: None,
            current_command_start: None,
            current_command_name: None,
        }
    }

    /// Start timing an iteration
    pub fn start_iteration(&mut self) {
        self.current_iteration_start = Some(Instant::now());
    }

    /// Complete iteration timing
    pub fn complete_iteration(&mut self) -> Option<Duration> {
        self.current_iteration_start
            .take()
            .map(|start| start.elapsed())
    }

    /// Start timing a command
    pub fn start_command(&mut self, command_name: String) {
        self.current_command_start = Some(Instant::now());
        self.current_command_name = Some(command_name);
    }

    /// Complete command timing
    pub fn complete_command(&mut self) -> Option<(String, Duration)> {
        match (
            self.current_command_start.take(),
            self.current_command_name.take(),
        ) {
            (Some(start), Some(name)) => Some((name, start.elapsed())),
            _ => None,
        }
    }

    /// Get current iteration duration (without completing)
    pub fn current_iteration_duration(&self) -> Option<Duration> {
        self.current_iteration_start.map(|start| start.elapsed())
    }

    /// Get current command duration (without completing)
    pub fn current_command_duration(&self) -> Option<Duration> {
        self.current_command_start.map(|start| start.elapsed())
    }

    /// Check if iteration is in progress
    pub fn is_iteration_in_progress(&self) -> bool {
        self.current_iteration_start.is_some()
    }

    /// Check if command is in progress
    pub fn is_command_in_progress(&self) -> bool {
        self.current_command_start.is_some()
    }
}

impl Default for TimingTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Format duration in human-readable format
pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();

    if total_secs < 60 {
        format!("{total_secs}s")
    } else if total_secs < 3600 {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if secs > 0 {
            format!("{mins}m {secs}s")
        } else {
            format!("{mins}m")
        }
    } else {
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        if mins > 0 {
            format!("{hours}h {mins}m")
        } else {
            format!("{hours}h")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_tracker() {
        let mut tracker = TimingTracker::new();

        // Initially no timing in progress
        assert!(!tracker.is_iteration_in_progress());
        assert!(!tracker.is_command_in_progress());

        // Start iteration
        tracker.start_iteration();
        assert!(tracker.is_iteration_in_progress());

        // Start command
        tracker.start_command("test-command".to_string());
        assert!(tracker.is_command_in_progress());

        // Complete command
        let (name, _duration) = tracker.complete_command().unwrap();
        assert_eq!(name, "test-command");
        assert!(!tracker.is_command_in_progress());

        // Complete iteration
        let _duration = tracker.complete_iteration().unwrap();
        assert!(!tracker.is_iteration_in_progress());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m");
        assert_eq!(format_duration(Duration::from_secs(7200)), "2h");
        assert_eq!(format_duration(Duration::from_secs(7320)), "2h 2m");
    }
}