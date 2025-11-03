//! Tests for validation module
//!
//! This module contains all test code for the validation module, separated from
//! the implementation code for better maintainability and organization.

use super::validation::*;
use crate::config::WorkflowCommand;
use crate::cook::workflow::validation::{OnIncompleteConfig, ValidationConfig, ValidationStatus};

// ============================================================================
// Phase 2: Tests for Pure Decision Functions
// ============================================================================

// Tests for should_continue_retry
#[test]
fn test_should_continue_retry_true_when_incomplete_and_attempts_remain() {
    // Should continue: validation incomplete, attempts < max
    assert!(should_continue_retry(1, 3, false));
    assert!(should_continue_retry(0, 1, false));
    assert!(should_continue_retry(2, 5, false));
}

#[test]
fn test_should_continue_retry_false_when_complete() {
    // Should not continue: validation complete
    assert!(!should_continue_retry(0, 3, true));
    assert!(!should_continue_retry(2, 3, true));
}

#[test]
fn test_should_continue_retry_false_when_max_attempts_reached() {
    // Should not continue: attempts >= max_attempts
    assert!(!should_continue_retry(3, 3, false));
    assert!(!should_continue_retry(5, 3, false));
    assert!(!should_continue_retry(0, 0, false));
}

#[test]
fn test_should_continue_retry_boundary_conditions() {
    // Boundary: last attempt before max
    assert!(should_continue_retry(2, 3, false));
    // Boundary: at max attempts
    assert!(!should_continue_retry(3, 3, false));
    // Boundary: complete on first try
    assert!(!should_continue_retry(0, 3, true));
}

// Tests for determine_handler_type
#[test]
fn test_determine_handler_type_multi_command() {
    let on_incomplete = OnIncompleteConfig {
        commands: Some(vec![]),
        claude: None,
        shell: None,
        max_attempts: 1,
        fail_workflow: false,
        prompt: None,
        commit_required: false,
    };
    assert_eq!(
        determine_handler_type(&on_incomplete),
        HandlerType::MultiCommand
    );
}

#[test]
fn test_determine_handler_type_single_command_claude() {
    let on_incomplete = OnIncompleteConfig {
        commands: None,
        claude: Some("/fix".to_string()),
        shell: None,
        max_attempts: 1,
        fail_workflow: false,
        prompt: None,
        commit_required: false,
    };
    assert_eq!(
        determine_handler_type(&on_incomplete),
        HandlerType::SingleCommand
    );
}

#[test]
fn test_determine_handler_type_single_command_shell() {
    let on_incomplete = OnIncompleteConfig {
        commands: None,
        claude: None,
        shell: Some("echo test".to_string()),
        max_attempts: 1,
        fail_workflow: false,
        prompt: None,
        commit_required: false,
    };
    assert_eq!(
        determine_handler_type(&on_incomplete),
        HandlerType::SingleCommand
    );
}

#[test]
fn test_determine_handler_type_no_handler() {
    let on_incomplete = OnIncompleteConfig {
        commands: None,
        claude: None,
        shell: None,
        max_attempts: 1,
        fail_workflow: false,
        prompt: Some("Continue?".to_string()),
        commit_required: false,
    };
    assert_eq!(
        determine_handler_type(&on_incomplete),
        HandlerType::NoHandler
    );
}

// Tests for calculate_retry_progress
#[test]
fn test_calculate_retry_progress_basic() {
    let progress = calculate_retry_progress(2, 5, 60.0);
    assert_eq!(progress.attempts, 2);
    assert_eq!(progress.max_attempts, 5);
    assert_eq!(progress.completion_percentage, 60.0);
}

#[test]
fn test_calculate_retry_progress_zero_completion() {
    let progress = calculate_retry_progress(1, 3, 0.0);
    assert_eq!(progress.completion_percentage, 0.0);
}

#[test]
fn test_calculate_retry_progress_full_completion() {
    let progress = calculate_retry_progress(3, 3, 100.0);
    assert_eq!(progress.attempts, 3);
    assert_eq!(progress.completion_percentage, 100.0);
}

#[test]
fn test_calculate_retry_progress_partial() {
    let progress = calculate_retry_progress(1, 2, 45.5);
    assert_eq!(progress.attempts, 1);
    assert_eq!(progress.max_attempts, 2);
    assert_eq!(progress.completion_percentage, 45.5);
}

// Tests for should_fail_workflow
#[test]
fn test_should_fail_workflow_true_when_incomplete_and_flag_set() {
    // Should fail: incomplete + fail_workflow=true
    assert!(should_fail_workflow(false, true, 3));
    assert!(should_fail_workflow(false, true, 0));
}

#[test]
fn test_should_fail_workflow_false_when_complete() {
    // Should not fail: complete
    assert!(!should_fail_workflow(true, true, 3));
    assert!(!should_fail_workflow(true, false, 3));
}

#[test]
fn test_should_fail_workflow_false_when_flag_not_set() {
    // Should not fail: fail_workflow=false
    assert!(!should_fail_workflow(false, false, 3));
    assert!(!should_fail_workflow(true, false, 0));
}

#[test]
fn test_should_fail_workflow_boundary_conditions() {
    // Boundary: incomplete but flag false
    assert!(!should_fail_workflow(false, false, 3));
    // Boundary: complete but flag true
    assert!(!should_fail_workflow(true, true, 3));
    // Boundary: incomplete and flag true (should fail)
    assert!(should_fail_workflow(false, true, 3));
}

// Tests for determine_validation_execution_mode
#[test]
fn test_determine_validation_execution_mode_commands_array() {
    let config = ValidationConfig {
        commands: Some(vec![WorkflowCommand::Simple("/test".to_string())]),
        claude: None,
        shell: None,
        command: None,
        result_file: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert_eq!(
        determine_validation_execution_mode(&config),
        ValidationExecutionMode::CommandsArray
    );
}

#[test]
fn test_determine_validation_execution_mode_claude() {
    let config = ValidationConfig {
        commands: None,
        claude: Some("/prodigy-validate".to_string()),
        shell: None,
        command: None,
        result_file: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert_eq!(
        determine_validation_execution_mode(&config),
        ValidationExecutionMode::Claude
    );
}

#[test]
fn test_determine_validation_execution_mode_shell() {
    let config = ValidationConfig {
        commands: None,
        claude: None,
        shell: Some("./validate.sh".to_string()),
        command: None,
        result_file: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert_eq!(
        determine_validation_execution_mode(&config),
        ValidationExecutionMode::Shell
    );
}

#[test]
fn test_determine_validation_execution_mode_legacy_command() {
    let config = ValidationConfig {
        commands: None,
        claude: None,
        shell: None,
        command: Some("./validate.sh".to_string()),
        result_file: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert_eq!(
        determine_validation_execution_mode(&config),
        ValidationExecutionMode::Shell
    );
}

#[test]
fn test_determine_validation_execution_mode_no_command() {
    let config = ValidationConfig {
        commands: None,
        claude: None,
        shell: None,
        command: None,
        result_file: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert_eq!(
        determine_validation_execution_mode(&config),
        ValidationExecutionMode::NoCommand
    );
}

#[test]
fn test_determine_validation_execution_mode_priority() {
    // Commands array takes priority over claude/shell
    let config = ValidationConfig {
        commands: Some(vec![WorkflowCommand::Simple("/test".to_string())]),
        claude: Some("/other".to_string()),
        shell: Some("./script.sh".to_string()),
        command: None,
        result_file: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert_eq!(
        determine_validation_execution_mode(&config),
        ValidationExecutionMode::CommandsArray
    );
}

// Tests for should_read_result_file_after_commands
#[test]
fn test_should_read_result_file_after_commands_true() {
    let config = ValidationConfig {
        commands: Some(vec![WorkflowCommand::Simple("echo test".to_string())]),
        result_file: Some("results.json".to_string()),
        claude: None,
        shell: None,
        command: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert!(should_read_result_file_after_commands(&config));
}

#[test]
fn test_should_read_result_file_after_commands_false_no_commands() {
    let config = ValidationConfig {
        commands: None,
        result_file: Some("results.json".to_string()),
        claude: Some("/validate".to_string()),
        shell: None,
        command: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert!(!should_read_result_file_after_commands(&config));
}

#[test]
fn test_should_read_result_file_after_commands_false_no_result_file() {
    let config = ValidationConfig {
        commands: Some(vec![WorkflowCommand::Simple("echo test".to_string())]),
        result_file: None,
        claude: None,
        shell: None,
        command: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert!(!should_read_result_file_after_commands(&config));
}

// Tests for should_use_result_file
#[test]
fn test_should_use_result_file_true() {
    let config = ValidationConfig {
        commands: None,
        result_file: Some("results.json".to_string()),
        claude: Some("/validate".to_string()),
        shell: None,
        command: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert!(should_use_result_file(&config));
}

#[test]
fn test_should_use_result_file_false_with_commands() {
    let config = ValidationConfig {
        commands: Some(vec![WorkflowCommand::Simple("echo test".to_string())]),
        result_file: Some("results.json".to_string()),
        claude: None,
        shell: None,
        command: None,
        expected_schema: None,
        threshold: 100.0,
        on_incomplete: None,
        timeout: None,
    };

    assert!(!should_use_result_file(&config));
}
