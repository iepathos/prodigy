//! Pure functions for workflow execution
//!
//! This module contains all pure, side-effect-free functions extracted from the
//! WorkflowExecutor. These functions are easier to test, reason about, and compose.
//!
//! Pure functions have these characteristics:
//! - No side effects (no I/O, no mutation of external state)
//! - Deterministic (same inputs always produce same outputs)
//! - No async operations
//! - No `&mut self` or `Arc` dependencies

use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::workflow::{WorkflowMode, WorkflowStep};
use anyhow::Result;
use std::collections::HashMap;

// Import parent module types
use super::{ExtendedWorkflowConfig, StepResult};

/// Execution flags determined from environment variables
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionFlags {
    pub test_mode: bool,
    pub skip_validation: bool,
}

/// Determine continuation strategy for iterations
#[derive(Debug, Clone, PartialEq)]
pub enum IterationContinuation {
    Stop(String),
    Continue,
    ContinueToMax,
    AskUser,
}

// ============================================================================
// Environment and Configuration Functions
// ============================================================================

/// Determine execution flags from environment variables
///
/// Pure function that reads environment once and returns immutable flags.
pub fn determine_execution_flags() -> ExecutionFlags {
    ExecutionFlags {
        test_mode: std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true",
        skip_validation: std::env::var("PRODIGY_NO_COMMIT_VALIDATION").unwrap_or_default()
            == "true",
    }
}

/// Calculate effective max iterations for a workflow
///
/// In dry-run mode, limit to 1 iteration to avoid redundant simulated steps.
pub fn calculate_effective_max_iterations(workflow: &ExtendedWorkflowConfig, dry_run: bool) -> u32 {
    if dry_run && workflow.max_iterations > 1 {
        1 // Limit to 1 iteration in dry-run mode
    } else {
        workflow.max_iterations
    }
}

/// Build iteration context variables
///
/// Returns a HashMap with iteration-specific variables that can be interpolated.
pub fn build_iteration_context(iteration: u32) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("ITERATION".to_string(), iteration.to_string());
    vars
}

/// Validate workflow configuration
///
/// Ensures workflow has steps (unless in MapReduce mode).
pub fn validate_workflow_config(workflow: &ExtendedWorkflowConfig) -> Result<()> {
    if workflow.steps.is_empty() && workflow.mode != WorkflowMode::MapReduce {
        return Err(anyhow::anyhow!("Workflow has no steps to execute"));
    }
    Ok(())
}

// ============================================================================
// Iteration and Flow Control Functions
// ============================================================================

/// Determine if a step should be skipped based on completion history
pub fn should_skip_step_execution(
    step_index: usize,
    completed_steps: &[crate::cook::session::StepResult],
) -> bool {
    completed_steps
        .iter()
        .any(|completed| completed.step_index == step_index && completed.success)
}

/// Determine if workflow should continue based on state
///
/// This is a complex decision function that encapsulates all continuation logic.
#[allow(clippy::too_many_arguments)]
pub fn determine_iteration_continuation(
    workflow: &ExtendedWorkflowConfig,
    iteration: u32,
    max_iterations: u32,
    any_changes: bool,
    execution_flags: &ExecutionFlags,
    is_focus_tracking_test: bool,
    should_stop_early_in_test: bool,
) -> IterationContinuation {
    if !workflow.iterate {
        return IterationContinuation::Stop("Single iteration workflow".to_string());
    }

    if iteration >= max_iterations {
        return IterationContinuation::Stop("Max iterations reached".to_string());
    }

    // Check for focus tracking test before checking for changes
    // This ensures tests that track focus always run to completion
    if is_focus_tracking_test {
        return IterationContinuation::ContinueToMax;
    }

    if !any_changes {
        return IterationContinuation::Stop("No changes were made".to_string());
    }

    if execution_flags.test_mode && should_stop_early_in_test {
        return IterationContinuation::Stop("Early termination in test mode".to_string());
    }

    if execution_flags.test_mode {
        return IterationContinuation::Continue;
    }

    IterationContinuation::AskUser
}

// ============================================================================
// Step Handling Functions
// ============================================================================

/// Get step display name for logging and error messages
pub fn get_step_display_name(step: &WorkflowStep) -> String {
    if let Some(claude_cmd) = &step.claude {
        format!("claude: {claude_cmd}")
    } else if let Some(shell_cmd) = &step.shell {
        format!("shell: {shell_cmd}")
    } else if let Some(test_cmd) = &step.test {
        format!("test: {}", test_cmd.command)
    } else if let Some(handler_step) = &step.handler {
        format!("handler: {}", handler_step.name)
    } else if let Some(name) = &step.name {
        name.clone()
    } else if let Some(command) = &step.command {
        format!("command: {command}")
    } else {
        "unknown step".to_string()
    }
}

/// Determine if workflow should fail based on step result
pub fn should_fail_workflow_for_step(step_result: &StepResult, step: &WorkflowStep) -> bool {
    if step_result.success {
        return false; // Command succeeded, don't fail
    }

    // Command failed, check on_failure configuration
    if let Some(on_failure_config) = &step.on_failure {
        on_failure_config.should_fail_workflow()
    } else if let Some(test_cmd) = &step.test {
        // Legacy test command handling
        if let Some(test_on_failure) = &test_cmd.on_failure {
            test_on_failure.fail_workflow
        } else {
            true // No on_failure config, fail on error
        }
    } else {
        true // No on_failure handler, fail on error
    }
}

/// Build error message for failed step
pub fn build_step_error_message(step: &WorkflowStep, result: &StepResult) -> String {
    let step_display = get_step_display_name(step);
    let mut error_msg = format!("Step '{}' failed", step_display);

    if let Some(exit_code) = result.exit_code {
        error_msg.push_str(&format!(" with exit code {}", exit_code));
    }

    // Add stderr if available
    if !result.stderr.trim().is_empty() {
        error_msg.push_str("\n\n=== Error Output (stderr) ===");
        append_truncated_output(&mut error_msg, &result.stderr);
    }

    // Add stdout if stderr was empty but stdout has content
    if result.stderr.trim().is_empty() && !result.stdout.trim().is_empty() {
        error_msg.push_str("\n\n=== Standard Output (stdout) ===");
        append_truncated_output(&mut error_msg, &result.stdout);
    }

    error_msg
}

/// Append truncated output to error message
///
/// Shows first 25 and last 25 lines for large outputs.
fn append_truncated_output(error_msg: &mut String, output: &str) {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= 50 {
        error_msg.push('\n');
        error_msg.push_str(output);
    } else {
        // Show first 25 and last 25 lines for large outputs
        error_msg.push('\n');
        for line in lines.iter().take(25) {
            error_msg.push_str(line);
            error_msg.push('\n');
        }
        error_msg.push_str(&format!(
            "\n... ({} lines truncated) ...\n\n",
            lines.len() - 50
        ));
        for line in lines.iter().skip(lines.len() - 25) {
            error_msg.push_str(line);
            error_msg.push('\n');
        }
    }
}

// ============================================================================
// Git and Commit Functions
// ============================================================================

/// Validate commit requirement for a step
///
/// Returns Ok if validation passes, Err with descriptive message otherwise.
#[allow(clippy::too_many_arguments)]
pub fn validate_commit_requirement(
    step: &WorkflowStep,
    tracked_commits_empty: bool,
    head_before: &str,
    head_after: &str,
    dry_run: bool,
    step_name: &str,
    assumed_commits: &[String],
    json_log_location: Option<&str>,
) -> Result<()> {
    if !step.commit_required {
        return Ok(());
    }

    if !tracked_commits_empty || head_after != head_before {
        return Ok(());
    }

    if dry_run {
        // Build the command description based on which command field is present
        let command_desc = if let Some(ref cmd) = step.claude {
            format!("claude: {}", cmd)
        } else if let Some(ref cmd) = step.shell {
            format!("shell: {}", cmd)
        } else if let Some(ref cmd) = step.command {
            format!("command: {}", cmd)
        } else {
            step_name.to_string()
        };

        if assumed_commits.iter().any(|c| c.contains(&command_desc)) {
            return Ok(()); // Skip validation for assumed commits
        }
    }

    // Build error message with optional log location
    let mut error_msg = format!(
        "Step '{}' has commit_required=true but no commits were created",
        step_name
    );

    if let Some(log_path) = json_log_location {
        error_msg.push_str(&format!("\n📝 Claude log: {}", log_path));
    }

    Err(anyhow::anyhow!(error_msg))
}

/// Build step commit variables from tracked commits
pub fn build_commit_variables(
    tracked_commits: &[crate::cook::commit_tracker::TrackedCommit],
) -> Result<HashMap<String, String>> {
    if tracked_commits.is_empty() {
        return Ok(HashMap::new());
    }

    let tracking_result =
        crate::cook::commit_tracker::CommitTrackingResult::from_commits(tracked_commits.to_vec());

    let mut vars = HashMap::new();
    vars.insert(
        "step.commits".to_string(),
        serde_json::to_string(tracked_commits)?,
    );
    vars.insert(
        "step.files_changed".to_string(),
        tracking_result.total_files_changed.to_string(),
    );
    vars.insert(
        "step.insertions".to_string(),
        tracking_result.total_insertions.to_string(),
    );
    vars.insert(
        "step.deletions".to_string(),
        tracking_result.total_deletions.to_string(),
    );

    Ok(vars)
}

// ============================================================================
// Formatting and Display Functions
// ============================================================================

/// Safely format environment variable value for logging
///
/// Redacts sensitive values and truncates long values.
pub fn format_env_var_for_logging(key: &str, value: &str) -> String {
    if key.to_lowercase().contains("secret")
        || key.to_lowercase().contains("token")
        || key.to_lowercase().contains("password")
        || key.to_lowercase().contains("key")
    {
        "<redacted>".to_string()
    } else if value.len() > 100 {
        format!("{}... (truncated)", &value[..100])
    } else {
        value.to_string()
    }
}

/// Format variable value for logging
///
/// Truncates long values for readability.
pub fn format_variable_for_logging(value: &str) -> String {
    if value.len() > 100 {
        format!("{}... (truncated)", &value[..100])
    } else {
        value.to_string()
    }
}

/// Get summary of available variables for debugging
pub fn get_available_variable_summary(context: &InterpolationContext) -> String {
    let mut variables: Vec<String> = context.variables.keys().cloned().collect();
    variables.sort();

    if variables.is_empty() {
        "none".to_string()
    } else if variables.len() > 10 {
        format!(
            "{} variables ({}...)",
            variables.len(),
            variables[..3].join(", ")
        )
    } else {
        variables.join(", ")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a workflow config for testing
    fn create_test_workflow() -> ExtendedWorkflowConfig {
        ExtendedWorkflowConfig {
            name: "test".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        }
    }

    #[test]
    fn test_calculate_effective_max_iterations_dry_run() {
        let mut workflow = create_test_workflow();
        workflow.max_iterations = 10;

        assert_eq!(calculate_effective_max_iterations(&workflow, true), 1);
        assert_eq!(calculate_effective_max_iterations(&workflow, false), 10);
    }

    #[test]
    fn test_calculate_effective_max_iterations_single_iteration() {
        let mut workflow = create_test_workflow();
        workflow.max_iterations = 1;

        assert_eq!(calculate_effective_max_iterations(&workflow, true), 1);
        assert_eq!(calculate_effective_max_iterations(&workflow, false), 1);
    }

    #[test]
    fn test_build_iteration_context() {
        let ctx = build_iteration_context(5);
        assert_eq!(ctx.get("ITERATION"), Some(&"5".to_string()));
    }

    #[test]
    fn test_validate_workflow_config_empty_steps() {
        let mut workflow = create_test_workflow();
        workflow.steps = vec![];
        workflow.mode = WorkflowMode::Sequential;

        let result = validate_workflow_config(&workflow);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no steps"));
    }

    #[test]
    fn test_validate_workflow_config_mapreduce_empty_steps() {
        let mut workflow = create_test_workflow();
        workflow.steps = vec![];
        workflow.mode = WorkflowMode::MapReduce;

        let result = validate_workflow_config(&workflow);
        assert!(result.is_ok());
    }

    // Helper to create StepResult for testing
    fn create_step_result(step_index: usize, success: bool) -> crate::cook::session::StepResult {
        crate::cook::session::StepResult {
            step_index,
            command: "test".to_string(),
            success,
            output: Some("".to_string()),
            duration: std::time::Duration::from_secs(1),
            exit_code: Some(if success { 0 } else { 1 }),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            error: None,
        }
    }

    #[test]
    fn test_should_skip_step_execution() {
        let completed_steps = vec![create_step_result(0, true), create_step_result(2, true)];

        assert!(should_skip_step_execution(0, &completed_steps));
        assert!(!should_skip_step_execution(1, &completed_steps));
        assert!(should_skip_step_execution(2, &completed_steps));
    }

    #[test]
    fn test_determine_iteration_continuation_single_iteration() {
        let mut workflow = create_test_workflow();
        workflow.iterate = false;

        let flags = ExecutionFlags {
            test_mode: false,
            skip_validation: false,
        };

        let result = determine_iteration_continuation(&workflow, 1, 10, true, &flags, false, false);

        match result {
            IterationContinuation::Stop(msg) => {
                assert!(msg.contains("Single iteration"));
            }
            _ => panic!("Expected Stop variant"),
        }
    }

    #[test]
    fn test_determine_iteration_continuation_max_reached() {
        let mut workflow = create_test_workflow();
        workflow.iterate = true;

        let flags = ExecutionFlags {
            test_mode: false,
            skip_validation: false,
        };

        let result =
            determine_iteration_continuation(&workflow, 10, 10, true, &flags, false, false);

        match result {
            IterationContinuation::Stop(msg) => {
                assert!(msg.contains("Max iterations"));
            }
            _ => panic!("Expected Stop variant"),
        }
    }

    #[test]
    fn test_determine_iteration_continuation_no_changes() {
        let mut workflow = create_test_workflow();
        workflow.iterate = true;

        let flags = ExecutionFlags {
            test_mode: false,
            skip_validation: false,
        };

        let result =
            determine_iteration_continuation(&workflow, 1, 10, false, &flags, false, false);

        match result {
            IterationContinuation::Stop(msg) => {
                assert!(msg.contains("No changes"));
            }
            _ => panic!("Expected Stop variant"),
        }
    }

    #[test]
    fn test_determine_iteration_continuation_test_mode() {
        let mut workflow = create_test_workflow();
        workflow.iterate = true;

        let flags = ExecutionFlags {
            test_mode: true,
            skip_validation: false,
        };

        let result = determine_iteration_continuation(&workflow, 1, 10, true, &flags, false, false);

        assert!(matches!(result, IterationContinuation::Continue));
    }

    #[test]
    fn test_get_step_display_name_claude() {
        let step = WorkflowStep {
            claude: Some("test command".to_string()),
            ..Default::default()
        };
        assert_eq!(get_step_display_name(&step), "claude: test command");
    }

    #[test]
    fn test_get_step_display_name_shell() {
        let step = WorkflowStep {
            shell: Some("ls -la".to_string()),
            ..Default::default()
        };
        assert_eq!(get_step_display_name(&step), "shell: ls -la");
    }

    #[test]
    fn test_format_env_var_for_logging_secret() {
        assert_eq!(
            format_env_var_for_logging("API_SECRET", "secret123"),
            "<redacted>"
        );
        assert_eq!(
            format_env_var_for_logging("TOKEN", "token123"),
            "<redacted>"
        );
        assert_eq!(
            format_env_var_for_logging("PASSWORD", "pass123"),
            "<redacted>"
        );
    }

    #[test]
    fn test_format_env_var_for_logging_long_value() {
        let long_value = "a".repeat(150);
        let result = format_env_var_for_logging("LONG_VAR", &long_value);
        assert!(result.contains("truncated"));
        assert!(result.len() < long_value.len());
    }

    #[test]
    fn test_format_env_var_for_logging_normal() {
        assert_eq!(format_env_var_for_logging("PATH", "/usr/bin"), "/usr/bin");
    }

    #[test]
    fn test_format_variable_for_logging_truncates() {
        let long_value = "x".repeat(150);
        let result = format_variable_for_logging(&long_value);
        assert!(result.contains("truncated"));
        assert_eq!(result.len(), 115); // 100 + "... (truncated)".len() = 115
    }

    #[test]
    fn test_get_available_variable_summary_empty() {
        let ctx = InterpolationContext::new();
        assert_eq!(get_available_variable_summary(&ctx), "none");
    }

    #[test]
    fn test_validate_commit_requirement_not_required() {
        let step = WorkflowStep {
            commit_required: false,
            ..Default::default()
        };

        let result =
            validate_commit_requirement(&step, true, "abc", "abc", false, "test", &[], None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_commit_requirement_commit_created() {
        let step = WorkflowStep {
            commit_required: true,
            ..Default::default()
        };

        // Commits were tracked
        let result =
            validate_commit_requirement(&step, false, "abc", "abc", false, "test", &[], None);
        assert!(result.is_ok());

        // HEAD changed
        let result =
            validate_commit_requirement(&step, true, "abc", "def", false, "test", &[], None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_commit_requirement_failure() {
        let step = WorkflowStep {
            commit_required: true,
            claude: Some("test".to_string()),
            ..Default::default()
        };

        let result =
            validate_commit_requirement(&step, true, "abc", "abc", false, "test step", &[], None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no commits"));
    }

    #[test]
    fn test_build_commit_variables_empty() {
        let result = build_commit_variables(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
