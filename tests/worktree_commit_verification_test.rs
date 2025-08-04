use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a test git repository with proper structure
fn setup_test_repo_with_worktree_support(temp_dir: &Path) -> Result<()> {
    // Initialize git
    Command::new("git")
        .current_dir(temp_dir)
        .args(["init"])
        .assert()
        .success();

    // Configure git
    Command::new("git")
        .current_dir(temp_dir)
        .args(["config", "user.email", "test@example.com"])
        .assert()
        .success();

    Command::new("git")
        .current_dir(temp_dir)
        .args(["config", "user.name", "Test User"])
        .assert()
        .success();

    // Create initial structure
    fs::create_dir_all(temp_dir.join(".mmm"))?;
    fs::create_dir_all(temp_dir.join(".claude/commands"))?;
    fs::create_dir_all(temp_dir.join("src"))?;
    fs::create_dir_all(temp_dir.join("specs"))?;

    // Create implement.yml workflow
    fs::write(
        temp_dir.join("implement.yml"),
        r#"# implement.yml - Example configuration for implementing specifications
commands:
  - name: mmm-implement-spec
    args: ["$ARG"]
  
  - name: mmm-lint
    commit_required: false
"#,
    )?;

    // Create a mock mmm-implement-spec command that simulates not creating commits
    fs::write(
        temp_dir.join(".claude/commands/mmm-implement-spec.md"),
        r#"# mmm-implement-spec
This command is supposed to implement specs but in this test it won't create commits.
"#,
    )?;

    // Create a mock mmm-lint command
    fs::write(
        temp_dir.join(".claude/commands/mmm-lint.md"),
        r#"# mmm-lint
Linting command
"#,
    )?;

    // Create a dummy spec
    fs::write(
        temp_dir.join("specs/49-test-spec.md"),
        "# Spec 49: Test Specification\nThis is a test spec.",
    )?;

    // Add some test files
    fs::write(temp_dir.join("src/main.rs"), "fn main() {}")?;
    fs::write(
        temp_dir.join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"",
    )?;

    // Initial commit
    Command::new("git")
        .current_dir(temp_dir)
        .args(["add", "."])
        .assert()
        .success();

    Command::new("git")
        .current_dir(temp_dir)
        .args(["commit", "-m", "Initial commit"])
        .assert()
        .success();

    Ok(())
}

#[test]
fn test_worktree_workflow_detects_no_commits() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_test_repo_with_worktree_support(temp_dir.path())?;

    // Run workflow with worktree option and spec that won't create commits
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.current_dir(temp_dir.path())
        .args(["cook", "implement.yml", "-wn", "1", "--args", "49"])
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_NO_CHANGES_COMMANDS", "mmm-implement-spec");

    // Should fail with error about no commits
    cmd.assert().failure().stderr(predicate::str::contains(
        "No changes were committed by /mmm-implement-spec",
    ));

    Ok(())
}

#[test]
fn test_regular_workflow_detects_no_commits() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_test_repo_with_worktree_support(temp_dir.path())?;

    // Run workflow WITHOUT worktree option
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.current_dir(temp_dir.path())
        .args(["cook", "implement.yml", "-n", "1", "--args", "49"])
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_NO_CHANGES_COMMANDS", "mmm-implement-spec");

    // Should also fail with error about no commits
    cmd.assert().failure().stderr(predicate::str::contains(
        "No changes were committed by /mmm-implement-spec",
    ));

    Ok(())
}
