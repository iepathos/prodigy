use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a test git repository
fn setup_test_repo(temp_dir: &Path) -> Result<()> {
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

    // Create a simple test command that doesn't make commits
    fs::write(
        temp_dir.join(".claude/commands/test-no-commit.md"),
        r#"# Test No Commit Command
This command succeeds but doesn't create any commits.
"#,
    )?;

    // Create a simple test command that makes commits
    fs::write(
        temp_dir.join(".claude/commands/test-with-commit.md"),
        r#"# Test With Commit Command
This command creates a commit.
"#,
    )?;

    // Create workflow files
    fs::write(
        temp_dir.join("test-no-commit.yml"),
        r#"# Test workflow where command doesn't make commits
commands:
  - test-no-commit
"#,
    )?;

    fs::write(
        temp_dir.join("test-with-commit-required.yml"),
        r#"# Test workflow with commit_required=false
commands:
  - name: test-no-commit
    commit_required: false
"#,
    )?;

    // Add some test files
    fs::write(temp_dir.join("src/main.rs"), "fn main() {}")?;
    fs::write(temp_dir.join("Cargo.toml"), "[package]\nname = \"test\"\nversion = \"0.1.0\"")?;

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
fn test_workflow_fails_when_no_commits_and_required() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_test_repo(temp_dir.path())?;

    // Set environment to simulate command execution without commits

    // Run workflow that expects commits but won't get any
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.current_dir(temp_dir.path())
        .args(["cook", "test-no-commit.yml", "-n", "1"])
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_NO_CHANGES_COMMANDS", "test-no-commit");

    // Should fail with error about no commits
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No changes were committed"));

    // No cleanup needed - env vars were only set on subprocess

    Ok(())
}

#[test]
fn test_workflow_succeeds_with_commit_required_false() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_test_repo(temp_dir.path())?;

    // Set environment to simulate command execution without commits

    // Run workflow with commit_required=false
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.current_dir(temp_dir.path())
        .args(["cook", "test-with-commit-required.yml", "-n", "1"])
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_NO_CHANGES_COMMANDS", "test-no-commit");

    // Should succeed even without commits
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("made no changes").or(predicate::str::contains("0 files changed")))
        .stdout(predicate::str::contains("Cook session completed successfully"));

    // No cleanup needed - env vars were only set on subprocess

    Ok(())
}

#[test]
fn test_mmm_lint_with_commit_required_false() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_test_repo(temp_dir.path())?;

    // Create workflow with mmm-lint having commit_required=false
    fs::write(
        temp_dir.path().join("lint-workflow.yml"),
        r#"# Lint workflow
commands:
  - name: mmm-lint
    commit_required: false
"#,
    )?;

    // Create mock mmm-lint command
    fs::write(
        temp_dir.path().join(".claude/commands/mmm-lint.md"),
        "# mmm-lint\nLinting command",
    )?;


    // Run workflow
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.current_dir(temp_dir.path())
        .args(["cook", "lint-workflow.yml", "-n", "1"])
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_NO_CHANGES_COMMANDS", "mmm-lint");

    // Should succeed without commits because commit_required=false
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("made no changes").or(predicate::str::contains("0 files changed")));

    // No cleanup needed - env vars were only set on subprocess

    Ok(())
}

#[test]
fn test_implement_spec_workflow_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_test_repo(temp_dir.path())?;

    // Create the implement.yml workflow
    fs::write(
        temp_dir.path().join("implement.yml"),
        r#"# implement.yml - Example configuration for implementing specifications
commands:
  - name: mmm-implement-spec
    args: ["$ARG"]
  
  - name: mmm-lint
    commit_required: false
"#,
    )?;

    // Create mock commands
    fs::write(
        temp_dir.path().join(".claude/commands/mmm-implement-spec.md"),
        "# mmm-implement-spec\nImplement specification command",
    )?;
    fs::write(
        temp_dir.path().join(".claude/commands/mmm-lint.md"),
        "# mmm-lint\nLinting command",
    )?;

    // Create a dummy spec
    fs::write(
        temp_dir.path().join("specs/63-test-spec.md"),
        "# Spec 63: Test Specification\nThis is a test spec.",
    )?;


    // Run workflow with spec that won't create commits
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.current_dir(temp_dir.path())
        .args(["cook", "implement.yml", "-n", "1", "--args", "63"])
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_NO_CHANGES_COMMANDS", "mmm-implement-spec,mmm-lint");

    // Should fail because mmm-implement-spec requires commits but won't create any
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No changes were committed by /mmm-implement-spec"));

    // No cleanup needed - env vars were only set on subprocess

    Ok(())
}

#[test]
fn test_skip_commit_validation_flag() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_test_repo(temp_dir.path())?;

    // Create workflow that normally would fail
    fs::write(
        temp_dir.path().join("test-skip-validation.yml"),
        r#"commands:
  - test-no-commit
"#,
    )?;


    // Run workflow with validation disabled
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.current_dir(temp_dir.path())
        .args(["cook", "test-skip-validation.yml", "-n", "1"])
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_NO_CHANGES_COMMANDS", "test-no-commit")
        .env("MMM_NO_COMMIT_VALIDATION", "true");

    // Should succeed even without commits because validation is disabled
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Cook session completed successfully"));

    // No cleanup needed - env vars were only set on subprocess

    Ok(())
}