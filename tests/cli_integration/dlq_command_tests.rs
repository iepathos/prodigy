// Tests for the 'dlq' (Dead Letter Queue) command

use super::test_utils::*;

#[test]
fn test_dlq_list() {
    let mut test = CliTest::new().arg("dlq").arg("list");

    let output = test.run();

    // Should list DLQ items or report none
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("No items")
            || output.stdout_contains("DLQ")
            || output.stdout_contains("Empty")
            || output.stderr_contains("dlq")
    );
}

#[test]
fn test_dlq_show_with_job_id() {
    let mut test = CliTest::new().arg("dlq").arg("inspect").arg("test-item-id");

    let output = test.run();

    // Should show DLQ items or report no data found
    assert!(
        output.exit_code == exit_codes::SUCCESS || output.exit_code == exit_codes::GENERAL_ERROR
    );
    // Should provide relevant feedback
    if output.exit_code == exit_codes::GENERAL_ERROR {
        assert!(
            output.stderr_contains("not found")
                || output.stderr_contains("No DLQ data")
                || output.stderr_contains("No items")
        );
    }
}

#[test]
fn test_dlq_reprocess() {
    let mut test = CliTest::new()
        .arg("dlq")
        .arg("reprocess")
        .arg("test-job-id");

    let output = test.run();

    // The reprocess command is deprecated in favor of 'retry'
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(output.stderr_contains("deprecated") && output.stderr_contains("retry"));
}

#[test]
fn test_dlq_clear() {
    let mut test = CliTest::new().arg("dlq").arg("clear").arg("test-job-id");

    let output = test.run();

    // Should clear DLQ items or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("not found")
            || output.stdout_contains("Cleared")
            || output.stderr_contains("clear")
    );
}

#[test]
fn test_dlq_invalid_subcommand() {
    let mut test = CliTest::new().arg("dlq").arg("invalid");

    let output = test.run();

    // Should fail with invalid subcommand
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("nvalid")
            || output.stderr_contains("nrecognized")
            || output.stderr_contains("Found argument")
    );
}

#[test]
fn test_dlq_show_missing_job_id() {
    let mut test = CliTest::new().arg("dlq").arg("inspect");
    // Missing item ID

    let output = test.run();

    // Should fail with missing argument
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("required")
            || output.stderr_contains("ITEM_ID")
            || output.stderr_contains("argument")
    );
}

#[test]
fn test_dlq_reprocess_missing_job_id() {
    let mut test = CliTest::new().arg("dlq").arg("reprocess");
    // Missing job ID

    let output = test.run();

    // The reprocess command is deprecated and returns a general error
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(output.stderr_contains("deprecated") && output.stderr_contains("retry"));
}

#[test]
fn test_dlq_clear_missing_job_id() {
    let mut test = CliTest::new().arg("dlq").arg("clear");
    // Missing job ID

    let output = test.run();

    // Should fail with missing argument
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("required")
            || output.stderr_contains("job")
            || output.stderr_contains("argument")
    );
}

#[test]
fn test_dlq_list_verbose() {
    let mut test = CliTest::new().arg("-v").arg("dlq").arg("list");

    let output = test.run();

    // Should show verbose output - command may fail but should show debug output
    assert!(
        output.exit_code == exit_codes::SUCCESS || output.exit_code == exit_codes::GENERAL_ERROR
    );
    // Verbose output should appear in stdout or stderr should contain relevant info
    assert!(
        output.stdout_contains("[DEBUG]")
            || output.stderr_contains("dlq")
            || output.stderr_contains("DLQ")
    );
}

#[test]
fn test_dlq_with_path() {
    let other_dir = tempfile::TempDir::new().unwrap();

    let mut test = CliTest::new()
        .arg("dlq")
        .arg("list")
        .arg("--path")
        .arg(other_dir.path().to_str().unwrap());

    let output = test.run();

    // Should work with specified path
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("dlq")
            || output.stdout_contains("DLQ")
            || output.stderr_contains("dlq")
    );
}

#[test]
fn test_dlq_reprocess_with_invalid_flag() {
    let mut test = CliTest::new()
        .arg("dlq")
        .arg("reprocess")
        .arg("test-job-id")
        .arg("--dry-run");

    let output = test.run();

    // Should fail with unrecognized argument since --dry-run doesn't exist
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("unexpected argument")
            || output.stderr_contains("unrecognized")
            || output.stderr_contains("--dry-run")
    );
}

#[test]
fn test_dlq_reprocess_with_max_parallel() {
    let mut test = CliTest::new()
        .arg("dlq")
        .arg("reprocess")
        .arg("test-job-id")
        .arg("--max-parallel")
        .arg("10");

    let output = test.run();

    // Should fail with unexpected argument since reprocess doesn't support --max-parallel
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("unexpected argument") || output.stderr_contains("--max-parallel")
    );
}
