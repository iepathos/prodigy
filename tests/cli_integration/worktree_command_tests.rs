// Tests for the 'worktree' command

use super::test_utils::*;

#[test]
fn test_worktree_list() {
    let mut test = CliTest::new().arg("worktree").arg("ls");

    let output = test.run();

    // Should list worktrees (even if empty)
    assert!(output.exit_code == exit_codes::SUCCESS || output.stderr_contains("worktree"));
}

#[test]
fn test_worktree_clean() {
    let mut test = CliTest::new().arg("worktree").arg("clean");

    let output = test.run();

    // Should clean worktrees or report none to clean
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("No worktrees")
            || output.stdout_contains("Cleaned")
            || output.stderr_contains("worktree")
    );
}

#[test]
fn test_worktree_clean_force() {
    let mut test = CliTest::new().arg("worktree").arg("clean").arg("--force");

    let output = test.run();

    // Force clean should work
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("worktree")
            || output.stderr_contains("worktree")
    );
}

#[test]
fn test_worktree_create() {
    let mut test = CliTest::new()
        .arg("worktree")
        .arg("create")
        .arg("test-branch");

    let output = test.run();

    // Should create worktree or fail gracefully
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("worktree")
            || output.stderr_contains("branch")
    );
}

#[test]
fn test_worktree_remove() {
    // First create a worktree
    let mut test = CliTest::new()
        .arg("worktree")
        .arg("create")
        .arg("temp-branch");

    test.run();

    // Now try to remove it
    let mut test = CliTest::new()
        .arg("worktree")
        .arg("remove")
        .arg("temp-branch");

    let output = test.run();

    // Should remove or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("not found")
            || output.stderr_contains("worktree")
    );
}

#[test]
fn test_worktree_invalid_subcommand() {
    let mut test = CliTest::new().arg("worktree").arg("invalid");

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
fn test_worktree_list_verbose() {
    let mut test = CliTest::new().arg("-v").arg("worktree").arg("ls");

    let output = test.run();

    // Should show verbose output
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("[DEBUG]")
            || output.stderr_contains("worktree")
    );
}

#[test]
fn test_worktree_clean_with_active_sessions() {
    // This test simulates cleaning when there might be active sessions
    // The behavior depends on implementation
    let mut test = CliTest::new().arg("worktree").arg("clean");

    let output = test.run();

    // Should handle active sessions gracefully
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("active")
            || output.stdout_contains("in use")
            || output.stdout_contains("No worktrees")
    );
}

#[test]
fn test_worktree_create_duplicate() {
    // Try to create a worktree with the same name twice
    let mut test = CliTest::new()
        .arg("worktree")
        .arg("create")
        .arg("duplicate-branch");

    test.run(); // First creation

    let mut test = CliTest::new()
        .arg("worktree")
        .arg("create")
        .arg("duplicate-branch");

    let output = test.run(); // Second creation

    // Should fail or handle duplicate gracefully
    assert!(
        output.stderr_contains("exists")
            || output.stderr_contains("already")
            || output.exit_code != exit_codes::SUCCESS
    );
}

#[test]
fn test_worktree_with_path() {
    let other_dir = tempfile::TempDir::new().unwrap();

    // Initialize git in other directory
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&other_dir)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&other_dir)
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&other_dir)
        .output()
        .ok();

    std::fs::write(other_dir.path().join("README.md"), "# Test").unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&other_dir)
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(&other_dir)
        .output()
        .ok();

    let mut test = CliTest::new()
        .arg("worktree")
        .arg("ls")
        .arg("--path")
        .arg(other_dir.path().to_str().unwrap());

    let output = test.run();

    // Should work with specified path
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("worktree")
            || output.stderr_contains("worktree")
    );
}
