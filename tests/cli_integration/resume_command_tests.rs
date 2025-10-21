// Tests for the 'resume' command

use super::test_utils::*;

#[test]
fn test_resume_command_without_workflow_id() {
    let mut test = CliTest::new().arg("resume");

    let output = test.run();

    // session_id is optional but if not provided it should error with anyhow (exit code 2)
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("No session ID provided")
            || output.stderr_contains("no checkpoints found")
            || output.stderr_contains("No session")
            || output.stderr_contains("required")
    );
}

#[test]
fn test_resume_command_with_nonexistent_workflow() {
    let mut test = CliTest::new().arg("resume").arg("nonexistent-session-123");

    let output = test.run();

    // Should fail when session doesn't exist
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(
        output.stderr_contains("not found")
            || output.stderr_contains("does not exist")
            || output.stderr_contains("No checkpoints")
            || output.stderr_contains("Session not found"),
        "Unexpected error message. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_resume_command_with_force_flag() {
    let mut test = CliTest::new()
        .arg("resume")
        .arg("test-session-456")
        .arg("--force");

    let output = test.run();

    // Should attempt to force resume (will fail as session doesn't exist)
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
}

#[test]
fn test_resume_command_with_custom_path() {
    let test_dir = tempfile::TempDir::new().unwrap();

    let mut test = CliTest::new()
        .arg("resume")
        .arg("test-session-789")
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap());

    let output = test.run();

    // Should attempt to resume in specified path (will fail as session doesn't exist)
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
}

#[test]
fn test_resume_command_help() {
    let mut test = CliTest::new().arg("resume").arg("--help");

    let output = test.run();

    // Should show help text
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Resume") || output.stdout_contains("resume"));
    assert!(output.stdout_contains("session") || output.stdout_contains("Session"));
    assert!(output.stdout_contains("checkpoint") || output.stdout_contains("force"));
}

#[test]
fn test_resume_command_invalid_arguments() {
    let mut test = CliTest::new()
        .arg("resume")
        .arg("session-id")
        .arg("--invalid-flag");

    let output = test.run();

    // Should fail with unknown argument
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("nrecognized")
            || output.stderr_contains("nknown")
            || output.stderr_contains("nexpected")
    );
}
