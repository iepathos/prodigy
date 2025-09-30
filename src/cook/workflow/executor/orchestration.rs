//! Orchestration helpers for workflow execution
//!
//! This module contains helper functions for workflow orchestration extracted from
//! WorkflowExecutor. It focuses on reducing complexity in the main execution loop
//! by providing clean, testable helper functions for:
//!
//! - Iteration management and progress tracking
//! - Step tracking and result aggregation
//! - Checkpoint creation and management
//! - Progress reporting and user interaction
//!
//! ## Design Principles
//!
//! 1. **Small, focused helpers**: Each function does one thing well
//! 2. **Reduce main loop complexity**: Extract repetitive or complex logic
//! 3. **Testability**: Pure functions where possible, clear interfaces
//! 4. **Backward compatible**: Drop-in replacements for existing code

use std::sync::Arc;
use std::time::Duration;

use crate::cook::workflow::WorkflowStep;

use super::{normalized, CheckpointCompletedStep, StepResult, WorkflowContext};

#[cfg(test)]
use std::collections::HashMap;

// ============================================================================
// Step Tracking Helpers
// ============================================================================

/// Build a completed step record for session tracking
///
/// This helper creates a standardized step result record for the session manager.
pub fn build_session_step_result(
    step_index: usize,
    step_display: String,
    step: &WorkflowStep,
    step_result: &StepResult,
    command_duration: Duration,
    step_started_at: chrono::DateTime<chrono::Utc>,
    step_completed_at: chrono::DateTime<chrono::Utc>,
) -> crate::cook::session::StepResult {
    crate::cook::session::StepResult {
        step_index,
        command: step_display,
        success: step_result.success,
        output: if step.capture_output.is_enabled() {
            Some(step_result.stdout.clone())
        } else {
            None
        },
        duration: command_duration,
        error: if !step_result.success {
            Some(step_result.stderr.clone())
        } else {
            None
        },
        started_at: step_started_at,
        completed_at: step_completed_at,
        exit_code: step_result.exit_code,
    }
}

/// Build a checkpoint step record
///
/// This helper creates a checkpoint-compatible step record with captured variables.
pub fn build_checkpoint_step(
    step_index: usize,
    step_display: String,
    step: &WorkflowStep,
    step_result: &StepResult,
    workflow_context: &WorkflowContext,
    command_duration: Duration,
    step_completed_at: chrono::DateTime<chrono::Utc>,
) -> CheckpointCompletedStep {
    CheckpointCompletedStep {
        step_index,
        command: step_display,
        success: step_result.success,
        output: if step.capture_output.is_enabled() {
            Some(step_result.stdout.clone())
        } else {
            None
        },
        captured_variables: workflow_context.captured_outputs.clone(),
        duration: command_duration,
        completed_at: step_completed_at,
        retry_state: None,
    }
}

// ============================================================================
// Progress Reporting Helpers
// ============================================================================

/// Format a step progress message
///
/// Creates a consistent progress message showing current step position.
pub fn format_step_progress(step_index: usize, total_steps: usize, step_display: &str) -> String {
    format!(
        "Executing step {}/{}: {}",
        step_index + 1,
        total_steps,
        step_display
    )
}

/// Format an iteration progress message
///
/// Creates a consistent progress message showing current iteration.
pub fn format_iteration_progress(iteration: u32, max_iterations: u32) -> String {
    format!("Starting iteration {}/{}", iteration, max_iterations)
}

/// Format workflow start message
///
/// Creates the initial workflow execution message.
pub fn format_workflow_start(workflow_name: &str, max_iterations: u32) -> String {
    format!(
        "Executing workflow: {} (max {} iterations)",
        workflow_name, max_iterations
    )
}

/// Format skip step message
///
/// Creates a message for when a step is skipped.
pub fn format_skip_step(step_index: usize, total_steps: usize, step_display: &str) -> String {
    format!(
        "Skipping already completed step {}/{}: {}",
        step_index + 1,
        total_steps,
        step_display
    )
}

// ============================================================================
// Iteration Management Helpers
// ============================================================================

/// Check if iteration should continue based on changes
///
/// Helper to determine if the next iteration should run based on whether
/// changes were made in the current iteration.
#[allow(dead_code)] // Available for future iteration logic refactoring
pub fn should_continue_iteration(
    has_changes: bool,
    is_iterative: bool,
    current_iteration: u32,
    max_iterations: u32,
) -> bool {
    if current_iteration >= max_iterations {
        return false;
    }

    // Always continue if workflow is not iterative (single-pass)
    if !is_iterative {
        return current_iteration == 0;
    }

    // For iterative workflows, continue if we have changes
    has_changes
}

/// Calculate workflow progress percentage
///
/// Returns a percentage (0-100) indicating workflow completion.
#[allow(dead_code)] // Available for future progress reporting features
pub fn calculate_progress_percentage(
    current_iteration: u32,
    max_iterations: u32,
    current_step: usize,
    total_steps: usize,
) -> u8 {
    if max_iterations == 0 || total_steps == 0 {
        return 0;
    }

    let iteration_progress = (current_iteration as f64 / max_iterations as f64) * 100.0;
    let step_progress =
        (current_step as f64 / total_steps as f64) * (100.0 / max_iterations as f64);

    (iteration_progress + step_progress).min(100.0) as u8
}

// ============================================================================
// Checkpoint Helpers
// ============================================================================

/// Create a simplified workflow hash for checkpointing
///
/// Generates a deterministic hash representing the workflow structure.
pub fn create_workflow_hash(workflow_name: &str, step_count: usize) -> String {
    // Simple hash based on name and step count
    // In production, this could use a proper hash function
    format!("{}-{}", workflow_name, step_count)
}

/// Create a normalized workflow for checkpointing
///
/// Converts workflow context into a minimal normalized representation
/// for checkpoint storage.
pub fn create_normalized_workflow(
    workflow_name: &str,
    workflow_context: &WorkflowContext,
) -> normalized::NormalizedWorkflow {
    normalized::NormalizedWorkflow {
        name: Arc::from(workflow_name),
        steps: Arc::from([]), // Simplified - actual steps not needed for checkpoint
        execution_mode: normalized::ExecutionMode::Sequential,
        variables: Arc::new(workflow_context.variables.clone()),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_step_progress() {
        let msg = format_step_progress(0, 5, "test command");
        assert_eq!(msg, "Executing step 1/5: test command");

        let msg = format_step_progress(4, 5, "last step");
        assert_eq!(msg, "Executing step 5/5: last step");
    }

    #[test]
    fn test_format_iteration_progress() {
        let msg = format_iteration_progress(1, 10);
        assert_eq!(msg, "Starting iteration 1/10");

        let msg = format_iteration_progress(10, 10);
        assert_eq!(msg, "Starting iteration 10/10");
    }

    #[test]
    fn test_format_workflow_start() {
        let msg = format_workflow_start("test-workflow", 5);
        assert_eq!(msg, "Executing workflow: test-workflow (max 5 iterations)");
    }

    #[test]
    fn test_format_skip_step() {
        let msg = format_skip_step(2, 5, "skipped command");
        assert_eq!(msg, "Skipping already completed step 3/5: skipped command");
    }

    #[test]
    fn test_should_continue_iteration_non_iterative() {
        // Non-iterative workflows only run once
        assert!(should_continue_iteration(false, false, 0, 1));
        assert!(!should_continue_iteration(false, false, 1, 1));
        assert!(!should_continue_iteration(true, false, 1, 1));
    }

    #[test]
    fn test_should_continue_iteration_iterative_with_changes() {
        // Iterative workflows continue if there are changes
        assert!(should_continue_iteration(true, true, 0, 10));
        assert!(should_continue_iteration(true, true, 5, 10));
        assert!(!should_continue_iteration(true, true, 10, 10));
    }

    #[test]
    fn test_should_continue_iteration_iterative_no_changes() {
        // Iterative workflows stop if no changes
        assert!(!should_continue_iteration(false, true, 1, 10));
        assert!(!should_continue_iteration(false, true, 5, 10));
    }

    #[test]
    fn test_should_continue_iteration_max_reached() {
        // Never continue if max iterations reached
        assert!(!should_continue_iteration(true, true, 10, 10));
        assert!(!should_continue_iteration(false, true, 10, 10));
    }

    #[test]
    fn test_calculate_progress_percentage() {
        // Single iteration, all steps
        assert_eq!(calculate_progress_percentage(1, 1, 0, 1), 100);

        // Multiple iterations
        assert_eq!(calculate_progress_percentage(1, 10, 0, 5), 10);
        assert_eq!(calculate_progress_percentage(5, 10, 0, 5), 50);
        assert_eq!(calculate_progress_percentage(10, 10, 0, 5), 100);

        // Edge cases
        assert_eq!(calculate_progress_percentage(0, 0, 0, 0), 0);
        assert_eq!(calculate_progress_percentage(0, 10, 0, 0), 0);
    }

    #[test]
    fn test_create_workflow_hash() {
        let hash1 = create_workflow_hash("test-workflow", 5);
        let hash2 = create_workflow_hash("test-workflow", 5);
        assert_eq!(hash1, hash2); // Deterministic

        let hash3 = create_workflow_hash("other-workflow", 5);
        assert_ne!(hash1, hash3); // Different names produce different hashes

        let hash4 = create_workflow_hash("test-workflow", 10);
        assert_ne!(hash1, hash4); // Different step counts produce different hashes
    }

    #[test]
    fn test_create_normalized_workflow() {
        use crate::cook::workflow::variables::VariableStore;

        let context = WorkflowContext {
            variables: HashMap::from([
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ]),
            captured_outputs: HashMap::new(),
            iteration_vars: HashMap::new(),
            validation_results: HashMap::new(),
            variable_store: Arc::new(VariableStore::new()),
            git_tracker: None,
        };

        let normalized = create_normalized_workflow("test-workflow", &context);

        assert_eq!(normalized.name.as_ref(), "test-workflow");
        assert_eq!(
            normalized.variables.get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(
            normalized.variables.get("key2"),
            Some(&"value2".to_string())
        );
    }
}
