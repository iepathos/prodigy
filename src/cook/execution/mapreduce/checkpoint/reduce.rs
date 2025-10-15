//! Reduce phase checkpoint structures and methods for MapReduce workflows
//!
//! This module defines the checkpoint structure used during the reduce phase,
//! allowing the workflow to resume from any point within reduce command execution.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Checkpoint for the reduce phase containing execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhaseCheckpoint {
    /// Version of the checkpoint format
    pub version: u32,

    /// Number of reduce steps completed
    pub completed_steps: usize,

    /// Total number of reduce steps
    pub total_steps: usize,

    /// Results from each completed reduce step
    pub step_results: Vec<StepResult>,

    /// Variables captured during reduce execution
    pub variables: HashMap<String, String>,

    /// Aggregated results from the map phase
    pub map_results: Vec<Value>,

    /// When this checkpoint was created
    pub timestamp: DateTime<Utc>,
}

/// Result from executing a single reduce step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Index of the step (0-based)
    pub step_index: usize,

    /// Whether the step succeeded
    pub success: bool,

    /// Output captured from the step
    pub output: Option<String>,

    /// Error message if step failed
    pub error: Option<String>,

    /// Duration of step execution in seconds
    pub duration_secs: f64,
}

impl ReducePhaseCheckpoint {
    /// Create a new reduce phase checkpoint
    pub fn new(
        total_steps: usize,
        completed_steps: usize,
        step_results: Vec<StepResult>,
        variables: HashMap<String, String>,
        map_results: Vec<Value>,
    ) -> Self {
        Self {
            version: 1,
            completed_steps,
            total_steps,
            step_results,
            variables,
            map_results,
            timestamp: Utc::now(),
        }
    }

    /// Check if the reduce phase can be resumed from this checkpoint
    pub fn can_resume(&self) -> bool {
        // Can resume if there are still steps remaining
        self.completed_steps < self.total_steps
    }

    /// Get the index of the next step to execute
    pub fn next_step_index(&self) -> Option<usize> {
        if self.can_resume() {
            Some(self.completed_steps)
        } else {
            None
        }
    }

    /// Get the number of remaining steps
    pub fn remaining_steps(&self) -> usize {
        self.total_steps.saturating_sub(self.completed_steps)
    }

    /// Get progress as a percentage (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.total_steps == 0 {
            1.0
        } else {
            self.completed_steps as f64 / self.total_steps as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_checkpoint(completed: usize, total: usize) -> ReducePhaseCheckpoint {
        let step_results: Vec<StepResult> = (0..completed)
            .map(|i| StepResult {
                step_index: i,
                success: true,
                output: Some(format!("Output from step {}", i)),
                error: None,
                duration_secs: 1.5,
            })
            .collect();

        let mut variables = HashMap::new();
        variables.insert("map.successful".to_string(), "10".to_string());
        variables.insert("map.failed".to_string(), "2".to_string());

        ReducePhaseCheckpoint::new(
            total,
            completed,
            step_results,
            variables,
            vec![],
        )
    }

    #[test]
    fn test_reduce_checkpoint_creation() {
        let checkpoint = create_test_checkpoint(3, 5);

        assert_eq!(checkpoint.version, 1);
        assert_eq!(checkpoint.completed_steps, 3);
        assert_eq!(checkpoint.total_steps, 5);
        assert_eq!(checkpoint.step_results.len(), 3);
        assert!(checkpoint.variables.contains_key("map.successful"));
    }

    #[test]
    fn test_can_resume() {
        let checkpoint_incomplete = create_test_checkpoint(3, 5);
        assert!(checkpoint_incomplete.can_resume());

        let checkpoint_complete = create_test_checkpoint(5, 5);
        assert!(!checkpoint_complete.can_resume());
    }

    #[test]
    fn test_next_step_index() {
        let checkpoint = create_test_checkpoint(3, 5);
        assert_eq!(checkpoint.next_step_index(), Some(3));

        let complete_checkpoint = create_test_checkpoint(5, 5);
        assert_eq!(complete_checkpoint.next_step_index(), None);
    }

    #[test]
    fn test_remaining_steps() {
        let checkpoint = create_test_checkpoint(3, 5);
        assert_eq!(checkpoint.remaining_steps(), 2);

        let complete = create_test_checkpoint(5, 5);
        assert_eq!(complete.remaining_steps(), 0);
    }

    #[test]
    fn test_progress() {
        let checkpoint_start = create_test_checkpoint(0, 5);
        assert_eq!(checkpoint_start.progress(), 0.0);

        let checkpoint_mid = create_test_checkpoint(3, 5);
        assert_eq!(checkpoint_mid.progress(), 0.6);

        let checkpoint_done = create_test_checkpoint(5, 5);
        assert_eq!(checkpoint_done.progress(), 1.0);

        let checkpoint_empty = create_test_checkpoint(0, 0);
        assert_eq!(checkpoint_empty.progress(), 1.0);
    }

    #[test]
    fn test_reduce_checkpoint_serialization() {
        let checkpoint = create_test_checkpoint(3, 5);

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&checkpoint)
            .expect("Failed to serialize checkpoint");

        // Deserialize back
        let deserialized: ReducePhaseCheckpoint = serde_json::from_str(&json)
            .expect("Failed to deserialize checkpoint");

        assert_eq!(deserialized.version, checkpoint.version);
        assert_eq!(deserialized.completed_steps, checkpoint.completed_steps);
        assert_eq!(deserialized.total_steps, checkpoint.total_steps);
        assert_eq!(deserialized.step_results.len(), checkpoint.step_results.len());
        assert_eq!(deserialized.variables.len(), checkpoint.variables.len());
    }

    #[test]
    fn test_step_result() {
        let step = StepResult {
            step_index: 0,
            success: true,
            output: Some("test output".to_string()),
            error: None,
            duration_secs: 2.5,
        };

        assert_eq!(step.step_index, 0);
        assert!(step.success);
        assert_eq!(step.output, Some("test output".to_string()));
        assert!(step.error.is_none());
        assert_eq!(step.duration_secs, 2.5);
    }
}
