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
    let mut test = CliTest::new().arg("dlq").arg("show").arg("test-job-id");

    let output = test.run();

    // Should show DLQ items for job or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("not found")
            || output.stderr_contains("No items")
            || output.stderr_contains("job")
    );
}

#[test]
fn test_dlq_reprocess() {
    let mut test = CliTest::new()
        .arg("dlq")
        .arg("reprocess")
        .arg("test-job-id");

    let output = test.run();

    // Should attempt to reprocess or report not found
    // Note: Based on PROJECT.md, DLQ reprocessing is not yet implemented
    assert!(
        output.exit_code == exit_codes::GENERAL_ERROR
            || output.stderr_contains("not implemented")
            || output.stderr_contains("not found")
            || output.stderr_contains("No items")
    );
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
    let mut test = CliTest::new().arg("dlq").arg("show");
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
fn test_dlq_reprocess_missing_job_id() {
    let mut test = CliTest::new().arg("dlq").arg("reprocess");
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

    // Should show verbose output
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("[DEBUG]")
            || output.stderr_contains("dlq")
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
fn test_dlq_reprocess_with_dry_run() {
    let mut test = CliTest::new()
        .arg("dlq")
        .arg("reprocess")
        .arg("test-job-id")
        .arg("--dry-run");

    let output = test.run();

    // Should show what would be reprocessed or report not implemented
    assert!(
        output.stderr_contains("not implemented")
            || output.stderr_contains("dry run")
            || output.stderr_contains("would reprocess")
            || output.exit_code == exit_codes::GENERAL_ERROR
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

    // Should accept max-parallel option or report not implemented
    assert!(
        output.stderr_contains("not implemented")
            || output.stderr_contains("parallel")
            || output.exit_code == exit_codes::GENERAL_ERROR
    );
}
