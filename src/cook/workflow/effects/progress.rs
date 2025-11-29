//! Workflow execution progress tracking
//!
//! This module defines types for tracking progress through workflow execution.
//! Progress is accumulated as steps complete, capturing outputs, variables,
//! and timing information.

use super::CommandOutput;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Result from executing a single step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Whether step succeeded
    pub success: bool,
    /// Captured output
    pub output: Option<String>,
    /// Variables captured from this step
    pub captured_variables: HashMap<String, String>,
    /// Execution duration
    pub duration: Duration,
    /// JSON log location (Claude commands only)
    pub json_log_location: Option<String>,
}

impl StepResult {
    /// Create StepResult from CommandOutput
    pub fn from_command_output(output: CommandOutput, duration: Duration) -> Self {
        Self {
            success: output.success,
            output: Some(output.stdout),
            captured_variables: output.variables,
            duration,
            json_log_location: output.json_log_location,
        }
    }

    /// Create a successful step result
    pub fn success(duration: Duration) -> Self {
        Self {
            success: true,
            output: None,
            captured_variables: HashMap::new(),
            duration,
            json_log_location: None,
        }
    }

    /// Create a failed step result
    pub fn failure(duration: Duration) -> Self {
        Self {
            success: false,
            output: None,
            captured_variables: HashMap::new(),
            duration,
            json_log_location: None,
        }
    }

    /// Add captured variables to the result
    pub fn with_variables(mut self, variables: HashMap<String, String>) -> Self {
        self.captured_variables = variables;
        self
    }
}

/// Accumulated progress through workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowProgress {
    /// Completed steps with results
    pub completed_steps: Vec<(usize, StepResult)>,
    /// Accumulated variables from all steps
    pub variables: HashMap<String, Value>,
    /// Current step index
    pub current_step: usize,
    /// Total execution duration
    pub total_duration: Duration,
}

impl WorkflowProgress {
    /// Create new empty progress
    pub fn new() -> Self {
        Self {
            completed_steps: Vec::new(),
            variables: HashMap::new(),
            current_step: 0,
            total_duration: Duration::ZERO,
        }
    }

    /// Add a completed step result and capture its variables
    pub fn with_step_result(mut self, idx: usize, result: StepResult) -> Self {
        // Capture variables from step output
        for (k, v) in &result.captured_variables {
            self.variables.insert(k.clone(), Value::String(v.clone()));
        }
        self.total_duration += result.duration;
        self.completed_steps.push((idx, result));
        self.current_step = idx + 1;
        self
    }

    /// Convert to final workflow result
    pub fn into_result(self) -> WorkflowResult {
        WorkflowResult {
            success: self.completed_steps.iter().all(|(_, r)| r.success),
            steps_completed: self.completed_steps.len(),
            final_variables: self.variables,
            total_duration: self.total_duration,
        }
    }
}

impl Default for WorkflowProgress {
    fn default() -> Self {
        Self::new()
    }
}

/// Final result of workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    /// Whether all steps succeeded
    pub success: bool,
    /// Number of steps completed
    pub steps_completed: usize,
    /// Final variables after all steps
    pub final_variables: HashMap<String, Value>,
    /// Total execution duration
    pub total_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_result_from_command_output() {
        let mut vars = HashMap::new();
        vars.insert("key".to_string(), "value".to_string());

        let output = CommandOutput::success("output".to_string()).with_variables(vars);
        let result = StepResult::from_command_output(output, Duration::from_secs(1));

        assert!(result.success);
        assert_eq!(result.duration, Duration::from_secs(1));
        assert_eq!(
            result.captured_variables.get("key"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_workflow_progress_accumulates_variables() {
        let mut vars1 = HashMap::new();
        vars1.insert("foo".to_string(), "bar".to_string());
        let step1 = StepResult::success(Duration::from_secs(1)).with_variables(vars1);

        let mut vars2 = HashMap::new();
        vars2.insert("baz".to_string(), "qux".to_string());
        let step2 = StepResult::success(Duration::from_secs(2)).with_variables(vars2);

        let progress = WorkflowProgress::new()
            .with_step_result(0, step1)
            .with_step_result(1, step2);

        assert_eq!(progress.current_step, 2);
        assert_eq!(progress.total_duration, Duration::from_secs(3));
        assert_eq!(progress.variables.len(), 2);
        assert_eq!(
            progress.variables.get("foo"),
            Some(&Value::String("bar".to_string()))
        );
    }

    #[test]
    fn test_workflow_result_success() {
        let step1 = StepResult::success(Duration::from_secs(1));
        let step2 = StepResult::success(Duration::from_secs(2));

        let progress = WorkflowProgress::new()
            .with_step_result(0, step1)
            .with_step_result(1, step2);

        let result = progress.into_result();
        assert!(result.success);
        assert_eq!(result.steps_completed, 2);
    }

    #[test]
    fn test_workflow_result_failure() {
        let step1 = StepResult::success(Duration::from_secs(1));
        let step2 = StepResult::failure(Duration::from_secs(2));

        let progress = WorkflowProgress::new()
            .with_step_result(0, step1)
            .with_step_result(1, step2);

        let result = progress.into_result();
        assert!(!result.success);
        assert_eq!(result.steps_completed, 2);
    }
}
