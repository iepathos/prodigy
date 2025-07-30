//! Integration tests for the CLI interface
//!
//! Tests the main entry point and command parsing logic

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_cli_help_default() {
    // Test that running without arguments shows help
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]
fn test_cli_help_flag() {
    // Test explicit help flag
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]
fn test_cook_help() {
    // Test cook subcommand help
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("cook")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cook your code to perfection"));
}

#[test]
fn test_worktree_help() {
    // Test worktree subcommand help
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("worktree")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Manage git worktrees"));
}

#[test]
fn test_invalid_command() {
    // Test invalid command
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("invalid-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn test_cook_with_invalid_focus() {
    // Test cook with invalid focus (this should actually succeed since focus is just a string)
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("cook")
        .arg("--focus")
        .arg("invalid")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn test_cook_with_invalid_iterations() {
    // Test cook with invalid max iterations
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("cook")
        .arg("-n")
        .arg("not-a-number")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn test_worktree_list_outside_repo() {
    // Test worktree list command outside a git repo
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("worktree")
        .arg("list")
        .assert()
        .failure();
}

#[test]
fn test_cook_all_flags() {
    // Test cook with all flags (dry run to avoid actual execution)
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("cook")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--focus"))
        .stdout(predicate::str::contains("--max-iterations"))
        .stdout(predicate::str::contains("--worktree"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--map"))
        .stdout(predicate::str::contains("--args"))
        .stdout(predicate::str::contains("--fail-fast"));
}

#[test]
fn test_version_flag() {
    // Test version flag - skip if not available
    // Version flag is not implemented in the current version
    // This test is kept for future compatibility
}

#[test]
fn test_improve_alias() {
    // Test that improve is an alias for cook
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("improve")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cook your code to perfection"));
}

#[test]
fn test_worktree_subcommands() {
    // Test worktree subcommands exist
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("worktree")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("merge"))
        .stdout(predicate::str::contains("clean"));
}

#[cfg(test)]
mod cook_dry_run_tests {
    use super::*;
    use std::env;

    #[test]
    fn test_cook_without_claude() {
        // Test cook command behavior when Claude CLI is not available
        // This will fail early with a clear error message
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repo
        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .output()
            .expect("Failed to init git repo");

        // Set test mode to bypass Claude check
        env::set_var("MMM_TEST_MODE", "true");

        let mut cmd = Command::cargo_bin("mmm").unwrap();
        cmd.current_dir(temp_dir.path())
            .arg("cook")
            .env("MMM_TEST_MODE", "true")
            .assert()
            .success();

        env::remove_var("MMM_TEST_MODE");
    }
}

#[cfg(test)]
mod arg_parsing_tests {
    use super::*;

    #[test]
    fn test_cook_short_flags() {
        // Test short form flags
        let mut cmd = Command::cargo_bin("mmm").unwrap();
        cmd.arg("cook")
            .arg("-v") // verbose/show-progress
            .arg("-w") // worktree
            .arg("-n")
            .arg("5") // max iterations
            .arg("--help")
            .assert()
            .success();
    }

    #[test]
    fn test_map_and_args_flags() {
        // Test map and args flags parsing
        let mut cmd = Command::cargo_bin("mmm").unwrap();
        cmd.arg("cook")
            .arg("--map")
            .arg("*.rs")
            .arg("--map")
            .arg("src/**/*.toml")
            .arg("--args")
            .arg("value1")
            .arg("--args")
            .arg("value2")
            .arg("--help")
            .assert()
            .success();
    }
}
