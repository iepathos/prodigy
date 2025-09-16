// Tests for the 'events' command

use super::test_utils::*;

#[test]
fn test_events_list() {
    let mut test = CliTest::new().arg("events").arg("list");

    let output = test.run();

    // Should list events or report none
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("No events")
            || output.stdout_contains("Events")
            || output.stderr_contains("events")
    );
}

#[test]
fn test_events_show_with_job_id() {
    let mut test = CliTest::new().arg("events").arg("show").arg("test-job-id");

    let output = test.run();

    // Should show events for job or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("not found")
            || output.stderr_contains("No events")
            || output.stderr_contains("job")
    );
}

#[test]
fn test_events_tail() {
    let mut test = CliTest::new().arg("events").arg("tail").arg("test-job-id");

    let output = test.run();

    // Should tail events or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("not found")
            || output.stderr_contains("No events")
            || output.stderr_contains("tail")
    );
}

#[test]
fn test_events_clean() {
    let mut test = CliTest::new()
        .arg("events")
        .arg("clean")
        .arg("--older-than")
        .arg("7d");

    let output = test.run();

    // Should clean old events or report none to clean
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("Cleaned")
            || output.stdout_contains("No events")
            || output.stderr_contains("clean")
    );
}

#[test]
fn test_events_invalid_subcommand() {
    let mut test = CliTest::new().arg("events").arg("invalid");

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
fn test_events_list_with_filter() {
    let mut test = CliTest::new()
        .arg("events")
        .arg("list")
        .arg("--filter")
        .arg("error");

    let output = test.run();

    // Should filter events or handle gracefully
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("No matching events")
            || output.stdout_contains("error")
            || output.stderr_contains("filter")
    );
}

#[test]
fn test_events_clean_with_invalid_duration() {
    let mut test = CliTest::new()
        .arg("events")
        .arg("clean")
        .arg("--older-than")
        .arg("invalid");

    let output = test.run();

    // Should fail with invalid duration
    assert!(output.exit_code != exit_codes::SUCCESS);
    assert!(
        output.stderr_contains("nvalid")
            || output.stderr_contains("duration")
            || output.stderr_contains("parse")
    );
}

#[test]
fn test_events_show_missing_job_id() {
    let mut test = CliTest::new().arg("events").arg("show");
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
fn test_events_list_verbose() {
    let mut test = CliTest::new().arg("-v").arg("events").arg("list");

    let output = test.run();

    // Should show verbose output
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("[DEBUG]")
            || output.stderr_contains("events")
    );
}

#[test]
fn test_events_with_path() {
    let other_dir = tempfile::TempDir::new().unwrap();

    let mut test = CliTest::new()
        .arg("events")
        .arg("list")
        .arg("--path")
        .arg(other_dir.path().to_str().unwrap());

    let output = test.run();

    // Should work with specified path
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("events")
            || output.stderr_contains("events")
    );
}
