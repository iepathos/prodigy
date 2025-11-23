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
// Commit Verification Functions
// ============================================================================

/// Action to take when no commits were created after a step
#[derive(Debug, Clone, PartialEq)]
pub enum CommitVerificationAction {
    /// Create an auto-commit (message will be generated separately)
    CreateAutoCommit,
    /// Fail with commit required error
    RequireCommitError,
    /// No action needed
    NoAction,
}

/// Determine action when no commits were created after a step
///
/// Pure function that encapsulates the decision logic for handling
/// steps that didn't create commits. Returns an action enum to reduce
/// cognitive complexity in the caller.
pub fn determine_no_commit_action(
    step: &WorkflowStep,
    has_changes: Result<bool>,
) -> CommitVerificationAction {
    if !step.auto_commit {
        // Auto-commit disabled
        return if step.commit_required {
            CommitVerificationAction::RequireCommitError
        } else {
            CommitVerificationAction::NoAction
        };
    }

    // Auto-commit enabled - check if there are changes
    match has_changes {
        Ok(true) => {
            // Has changes - create auto-commit
            CommitVerificationAction::CreateAutoCommit
        }
        Ok(false) => {
            // No changes
            if step.commit_required {
                CommitVerificationAction::RequireCommitError
            } else {
                CommitVerificationAction::NoAction
            }
        }
        Err(_) => {
            // Failed to check changes
            if step.commit_required {
                CommitVerificationAction::RequireCommitError
            } else {
                CommitVerificationAction::NoAction
            }
        }
    }
}

// ============================================================================
// Command Type Determination Functions
// ============================================================================

/// Count how many command fields are specified in a step
pub fn count_specified_commands(step: &WorkflowStep) -> usize {
    let mut count = 0;
    if step.claude.is_some() {
        count += 1;
    }
    if step.shell.is_some() {
        count += 1;
    }
    if step.test.is_some() {
        count += 1;
    }
    if step.handler.is_some() {
        count += 1;
    }
    if step.goal_seek.is_some() {
        count += 1;
    }
    if step.foreach.is_some() {
        count += 1;
    }
    if step.write_file.is_some() {
        count += 1;
    }
    if step.name.is_some() || step.command.is_some() {
        count += 1;
    }
    count
}

/// Validate that exactly one command type is specified
pub fn validate_single_command_type(count: usize) -> Result<()> {
    if count > 1 {
        return Err(anyhow::anyhow!(
            "Multiple command types specified. Use only one of: claude, shell, test, handler, goal_seek, foreach, write_file, or name/command"
        ));
    }
    if count == 0 {
        return Err(anyhow::anyhow!(
            "No command specified. Use one of: claude, shell, test, handler, goal_seek, foreach, write_file, or name/command"
        ));
    }
    Ok(())
}

/// Normalize legacy command format (prepend / if needed)
pub fn normalize_legacy_command(name: &str) -> String {
    if name.starts_with('/') {
        name.to_string()
    } else {
        format!("/{name}")
    }
}

// ============================================================================
// Workflow State Construction Functions
// ============================================================================

/// Build workflow state from execution parameters
///
/// Pure function that constructs WorkflowState without performing I/O.
pub fn build_workflow_state(
    iteration: usize,
    step_index: usize,
    completed_steps: Vec<crate::cook::session::StepResult>,
    workflow_path: std::path::PathBuf,
) -> crate::cook::session::WorkflowState {
    crate::cook::session::WorkflowState {
        current_iteration: iteration.saturating_sub(1), // Convert to 0-based index
        current_step: step_index + 1,                   // Next step to execute
        completed_steps,
        workflow_path,
        input_args: Vec::new(),
        map_patterns: Vec::new(),
        using_worktree: true, // Always true since worktrees are mandatory (spec 109)
    }
}

// ============================================================================
// Error Message Formatting Functions
// ============================================================================

/// Check if command name is expected to not create commits
fn is_analysis_command(command_name: &str) -> bool {
    matches!(
        command_name,
        "prodigy-lint" | "prodigy-code-review" | "prodigy-analyze"
    )
}

/// Get context-specific message for why no commits might be expected
fn get_no_commit_context_message(command_name: &str) -> String {
    if command_name == "prodigy-lint" {
        "This may be expected if there were no linting issues to fix.".to_string()
    } else if command_name == "prodigy-code-review" {
        "This may be expected if there were no issues found to fix.".to_string()
    } else if command_name == "prodigy-analyze" {
        "This may be expected if there were no changes needed.".to_string()
    } else {
        String::new()
    }
}

/// Extract command name from CommandType for error messages
pub fn extract_command_name(command_type: &super::CommandType) -> &str {
    match command_type {
        super::CommandType::Claude(cmd) | super::CommandType::Legacy(cmd) => cmd
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or(""),
        super::CommandType::Shell(cmd) => cmd,
        super::CommandType::Test(test_cmd) => &test_cmd.command,
        super::CommandType::Handler { handler_name, .. } => handler_name,
        super::CommandType::GoalSeek(config) => &config.goal,
        super::CommandType::Foreach(_) => "foreach",
        super::CommandType::WriteFile(config) => &config.path,
    }
}

/// Build error message for when no commits were created
///
/// Pure function that constructs comprehensive error message with context.
pub fn build_no_commits_error_message(command_name: &str, step_display: &str) -> String {
    let mut message = format!("\nWorkflow stopped: No changes were committed by {step_display}\n");
    message.push_str("\nThe command executed successfully but did not create any git commits.\n");

    if is_analysis_command(command_name) {
        message.push('\n');
        message.push_str(&get_no_commit_context_message(command_name));
        message.push_str("\n\nTo allow this command to proceed without commits, set commit_required: false in your workflow\n");
    } else {
        message.push_str("\nPossible reasons:\n");
        message.push_str("- The specification may already be implemented\n");
        message
            .push_str("- The command may have encountered an issue without reporting an error\n");
        message.push_str("- No changes were needed\n");
        message.push_str("\nTo investigate:\n");
        message.push_str("- Check if the spec is already implemented\n");
        message.push_str("- Review the command output above for any warnings\n");
        message.push_str("- Run 'git status' to check for uncommitted changes\n");
    }

    message.push_str(
        "\nAlternatively, run with PRODIGY_NO_COMMIT_VALIDATION=true to skip all validation.\n",
    );
    message
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

    // Tests for determine_no_commit_action

    #[test]
    fn test_determine_no_commit_action_auto_commit_has_changes() {
        let step = WorkflowStep {
            auto_commit: true,
            commit_required: false,
            ..Default::default()
        };

        let action = determine_no_commit_action(&step, Ok(true));
        assert_eq!(action, CommitVerificationAction::CreateAutoCommit);
    }

    #[test]
    fn test_determine_no_commit_action_auto_commit_no_changes_commit_required() {
        let step = WorkflowStep {
            auto_commit: true,
            commit_required: true,
            ..Default::default()
        };

        let action = determine_no_commit_action(&step, Ok(false));
        assert_eq!(action, CommitVerificationAction::RequireCommitError);
    }

    #[test]
    fn test_determine_no_commit_action_auto_commit_no_changes_not_required() {
        let step = WorkflowStep {
            auto_commit: true,
            commit_required: false,
            ..Default::default()
        };

        let action = determine_no_commit_action(&step, Ok(false));
        assert_eq!(action, CommitVerificationAction::NoAction);
    }

    #[test]
    fn test_determine_no_commit_action_auto_commit_check_failed_commit_required() {
        let step = WorkflowStep {
            auto_commit: true,
            commit_required: true,
            ..Default::default()
        };

        let action = determine_no_commit_action(&step, Err(anyhow::anyhow!("check failed")));
        assert_eq!(action, CommitVerificationAction::RequireCommitError);
    }

    #[test]
    fn test_determine_no_commit_action_auto_commit_check_failed_not_required() {
        let step = WorkflowStep {
            auto_commit: true,
            commit_required: false,
            ..Default::default()
        };

        let action = determine_no_commit_action(&step, Err(anyhow::anyhow!("check failed")));
        assert_eq!(action, CommitVerificationAction::NoAction);
    }

    #[test]
    fn test_determine_no_commit_action_no_auto_commit_required() {
        let step = WorkflowStep {
            auto_commit: false,
            commit_required: true,
            ..Default::default()
        };

        // Should fail regardless of has_changes result
        let action = determine_no_commit_action(&step, Ok(true));
        assert_eq!(action, CommitVerificationAction::RequireCommitError);

        let action = determine_no_commit_action(&step, Ok(false));
        assert_eq!(action, CommitVerificationAction::RequireCommitError);

        let action = determine_no_commit_action(&step, Err(anyhow::anyhow!("error")));
        assert_eq!(action, CommitVerificationAction::RequireCommitError);
    }

    #[test]
    fn test_determine_no_commit_action_no_auto_commit_not_required() {
        let step = WorkflowStep {
            auto_commit: false,
            commit_required: false,
            ..Default::default()
        };

        // Should be no action regardless of has_changes result
        let action = determine_no_commit_action(&step, Ok(true));
        assert_eq!(action, CommitVerificationAction::NoAction);

        let action = determine_no_commit_action(&step, Ok(false));
        assert_eq!(action, CommitVerificationAction::NoAction);

        let action = determine_no_commit_action(&step, Err(anyhow::anyhow!("error")));
        assert_eq!(action, CommitVerificationAction::NoAction);
    }
}
