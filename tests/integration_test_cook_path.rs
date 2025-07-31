//! Integration tests for the cook command path argument functionality

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test that cook command works without path argument (backward compatibility)
#[test]
fn test_cook_without_path() {
    // Create a temporary git repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create git repo");

    // Create a simple file
    fs::write(repo_path.join("test.rs"), "fn main() {}\n").unwrap();

    // Create a test playbook
    let playbook_path = repo_path.join("test.yml");
    fs::write(&playbook_path, "commands:\n  - name: mmm-lint").unwrap();

    // Change to the directory and run mmm cook
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.current_dir(repo_path)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "--max-iterations",
            "0",
            playbook_path.to_str().unwrap(),
        ])
        .assert()
        .success();
}

/// Test cook command with absolute path
#[test]
fn test_cook_with_absolute_path() {
    // Create a temporary git repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create git repo");

    // Create a simple file
    fs::write(repo_path.join("test.rs"), "fn main() {}\n").unwrap();

    // Create a test playbook
    let playbook_path = repo_path.join("test.yml");
    fs::write(&playbook_path, "commands:\n  - name: mmm-lint").unwrap();

    // Run mmm cook with absolute path
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-p",
            repo_path.to_str().unwrap(),
            "--max-iterations",
            "0",
            playbook_path.to_str().unwrap(),
        ])
        .assert()
        .success();
}

/// Test cook command with relative path
#[test]
fn test_cook_with_relative_path() {
    // Create a temporary directory structure
    let temp_dir = TempDir::new().unwrap();
    let parent_path = temp_dir.path();
    let repo_path = parent_path.join("myrepo");
    fs::create_dir(&repo_path).unwrap();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to create git repo");

    // Create a simple file
    fs::write(repo_path.join("test.rs"), "fn main() {}\n").unwrap();

    // Create a test playbook
    let playbook_path = repo_path.join("test.yml");
    fs::write(&playbook_path, "commands:\n  - name: mmm-lint").unwrap();

    // Run mmm cook with relative path from parent directory
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.current_dir(parent_path)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-p",
            "./myrepo",
            "--max-iterations",
            "0",
            "./myrepo/test.yml",
        ])
        .assert()
        .success();
}

/// Test error when path does not exist
#[test]
fn test_cook_path_not_found() {
    // Create a temp dir just for the playbook
    let temp_dir = TempDir::new().unwrap();
    let playbook_path = temp_dir.path().join("test.yml");
    fs::write(&playbook_path, "commands:\n  - name: mmm-lint").unwrap();

    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-p",
            "/path/that/does/not/exist",
            playbook_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Directory not found"));
}

/// Test error when path is not a directory
#[test]
fn test_cook_path_not_directory() {
    // Create a temporary file
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("file.txt");
    fs::write(&file_path, "content").unwrap();

    // Create a test playbook
    let playbook_path = temp_dir.path().join("test.yml");
    fs::write(&playbook_path, "commands:\n  - name: mmm-lint").unwrap();

    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-p",
            file_path.to_str().unwrap(),
            playbook_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Path is not a directory"));
}

/// Test error when path is not a git repository
#[test]
fn test_cook_path_not_git_repo() {
    // Create a temporary directory without git
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create a test playbook
    let playbook_path = dir_path.join("test.yml");
    fs::write(&playbook_path, "commands:\n  - name: mmm-lint").unwrap();

    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-p",
            dir_path.to_str().unwrap(),
            playbook_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

/// Test cook command with path and other flags
#[test]
fn test_cook_path_with_flags() {
    // Create a temporary git repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create git repo");

    // Create a simple file
    fs::write(repo_path.join("test.rs"), "fn main() {}\n").unwrap();

    // Create a test playbook
    let playbook_path = repo_path.join("test.yml");
    fs::write(&playbook_path, "commands:\n  - name: mmm-lint").unwrap();

    // Run mmm cook with path and other flags
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-p",
            repo_path.to_str().unwrap(),
            "--focus",
            "security",
            "--max-iterations",
            "0",
            playbook_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Starting improvement loop")
                .or(predicate::str::contains("Focus: security")),
        );
}

/// Test tilde expansion in path (Unix only)
#[cfg(unix)]
#[test]
fn test_cook_path_tilde_expansion() {
    use std::env;

    // Get home directory
    let home = env::var("HOME").expect("HOME not set");

    // Create a test directory in home
    let test_dir = format!("{}/mmm_test_repo_{}", home, std::process::id());
    fs::create_dir(&test_dir).unwrap();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to create git repo");

    // Create a simple file
    fs::write(format!("{test_dir}/test.rs"), "fn main() {}\n").unwrap();

    // Create a test playbook
    fs::write(
        format!("{test_dir}/test.yml"),
        "commands:\n  - name: mmm-lint",
    )
    .unwrap();

    // Run mmm cook with tilde path
    let tilde_path = format!("~/mmm_test_repo_{}", std::process::id());
    let playbook_path = format!("{test_dir}/test.yml");
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-p",
            &tilde_path,
            "--max-iterations",
            "0",
            &playbook_path,
        ])
        .assert()
        .success();

    // Clean up
    fs::remove_dir_all(&test_dir).unwrap();
}
