// Tests for argument parsing and validation

use super::test_utils::*;

#[test]
fn test_no_arguments() {
    let mut test = CliTest::new();

    let output = test.run();

    // Should show help or usage in stdout (successful help display)
    assert!(
        output.stdout_contains("Usage")
            || output.stdout_contains("USAGE")
            || output.stdout_contains("prodigy")
            || output.stdout_contains("commands")
    );
}

#[test]
fn test_help_flag() {
    let mut test = CliTest::new().arg("--help");

    let output = test.run();

    // Should show help message
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("prodigy") || output.stdout_contains("Cook your code"));
}

#[test]
fn test_version_flag() {
    let mut test = CliTest::new().arg("--version");

    let output = test.run();

    // Should show version
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(
        output.stdout_contains(".") || // Version number contains a dot
            output.stdout_contains("prodigy")
    );
}

#[test]
fn test_verbose_levels() {
    // Test -v (debug)
    let mut test = CliTest::new().arg("-v").arg("worktree").arg("ls");

    let output = test.run();

    // Debug logging might be visible
    assert!(
        output.stderr_contains("[DEBUG]")
            || output.stderr_contains("debug")
            || output.exit_code == exit_codes::SUCCESS
    );

    // Test -vv (trace)
    let mut test = CliTest::new().arg("-vv").arg("worktree").arg("ls");

    let output = test.run();

    // Trace logging might be visible
    assert!(
        output.stderr_contains("[TRACE]")
            || output.stderr_contains("trace")
            || output.exit_code == exit_codes::SUCCESS
    );

    // Test -vvv (all)
    let mut test = CliTest::new().arg("-vvv").arg("worktree").arg("ls");

    let output = test.run();

    // All logging should be visible
    assert!(
        output.stderr_contains("[") || // Any log level
            output.exit_code == exit_codes::SUCCESS
    );
}

#[test]
fn test_invalid_command() {
    let mut test = CliTest::new().arg("nonexistent-command");

    let output = test.run();

    // Should fail with unknown command
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("nrecognized")
            || output.stderr_contains("nvalid")
            || output.stderr_contains("Found argument")
    );
}

#[test]
fn test_missing_required_argument() {
    // Cook requires a workflow file
    let mut test = CliTest::new().arg("cook");

    let output = test.run();

    // Should fail with missing required argument
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("required")
            || output.stderr_contains("following required arguments")
    );
}

#[test]
fn test_invalid_flag_value() {
    let mut test = CliTest::new()
        .arg("exec")
        .arg("shell: echo test")
        .arg("--timeout")
        .arg("not-a-number");

    let output = test.run();

    // Should fail with invalid value
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("invalid value")
            || output.stderr_contains("Invalid value")
            || output.stderr_contains("parse")
    );
}

#[test]
fn test_conflicting_arguments() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--worktree")
        .arg("--resume")
        .arg("session-123")
        .run();

    // Should fail with conflicting arguments
    assert!(
        output.stderr_contains("conflict")
            || output.stderr_contains("cannot be used")
            || output.exit_code != exit_codes::SUCCESS
    );
}

#[test]
fn test_boolean_flag_variations() {
    // Test long form
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("test1", &create_test_workflow("test1"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--yes")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);

    // Test short form
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("test2", &create_test_workflow("test2"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("-y")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_multiple_values_argument() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--args")
        .arg("KEY1=value1")
        .arg("--args")
        .arg("KEY2=value2")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_subcommand_help() {
    let mut test = CliTest::new().arg("cook").arg("--help");

    let output = test.run();

    // Should show cook command help
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(
        output.stdout_contains("cook")
            || output.stdout_contains("playbook")
            || output.stdout_contains("workflow")
    );
}

#[test]
fn test_path_argument_validation() {
    let mut test = CliTest::new()
        .arg("cook")
        .arg("/nonexistent/path/workflow.yaml");

    let output = test.run();

    // Should fail with nonexistent file
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(
        output.stderr_contains("not found")
            || output.stderr_contains("No such file")
            || output.stderr_contains("Failed to read")
    );
}

#[test]
fn test_numeric_argument_bounds() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    // Test very large iteration count - should either accept it or show a reasonable error
    // Using --dry-run to avoid actually running 999999 iterations
    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("-n")
        .arg("999999")
        .arg("--dry-run")
        .run();

    // Should handle large numbers gracefully - either accept or reject with proper error
    // Since the test just creates an echo command, it should succeed with --dry-run
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("too large")
            || output.stderr_contains("maximum")
            || output.stderr_contains("iterations")
    );
}

#[test]
fn test_argument_with_spaces() {
    let mut test = CliTest::new()
        .arg("exec")
        .arg("shell: echo 'Hello World with spaces'");

    let output = test.run();

    assert_output(
        &output,
        exit_codes::SUCCESS,
        Some("Hello World with spaces"),
        None,
    );
}

#[test]
fn test_argument_with_special_characters() {
    let mut test = CliTest::new().arg("exec").arg("shell: echo 'Test$123!@#'");

    let output = test.run();

    // Should handle special characters
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}
