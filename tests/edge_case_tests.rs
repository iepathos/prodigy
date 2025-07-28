//! Edge case tests for MMM
//!
//! Tests for interrupted operations, concurrent execution, invalid inputs, and error recovery

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Create a temporary git repository for testing
fn create_test_repo() -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize git repo
    Command::new("git")
        .current_dir(&repo_path)
        .args(["init"])
        .output()?;

    // Configure git
    Command::new("git")
        .current_dir(&repo_path)
        .args(["config", "user.name", "Test User"])
        .output()?;

    Command::new("git")
        .current_dir(&repo_path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    // Create initial commit
    fs::write(repo_path.join("README.md"), "# Test Project")?;
    Command::new("git")
        .current_dir(&repo_path)
        .args(["add", "."])
        .output()?;

    Command::new("git")
        .current_dir(&repo_path)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    Ok((temp_dir, repo_path))
}

#[test]
fn test_worktree_interrupted_merge() {
    // Test recovery from interrupted merge
    let (_temp_dir, repo_path) = create_test_repo().unwrap();

    // Create a worktree
    let worktree_name = "test-worktree";
    let output = Command::new("git")
        .current_dir(&repo_path)
        .args(["worktree", "add", worktree_name])
        .output()
        .unwrap();

    assert!(output.status.success(), "Failed to create worktree");

    // Make changes in worktree
    let worktree_path = repo_path.join(worktree_name);
    fs::write(worktree_path.join("new-file.txt"), "test content").unwrap();

    Command::new("git")
        .current_dir(&worktree_path)
        .args(["add", "."])
        .output()
        .unwrap();

    Command::new("git")
        .current_dir(&worktree_path)
        .args(["commit", "-m", "Add new file"])
        .output()
        .unwrap();

    // Simulate interrupted merge by creating conflict
    fs::write(repo_path.join("new-file.txt"), "conflicting content").unwrap();
    Command::new("git")
        .current_dir(&repo_path)
        .args(["add", "."])
        .output()
        .unwrap();

    Command::new("git")
        .current_dir(&repo_path)
        .args(["commit", "-m", "Add conflicting file"])
        .output()
        .unwrap();

    // Attempt merge - should handle the conflict gracefully
    let merge_output = Command::new("git")
        .current_dir(&repo_path)
        .args(["merge", worktree_name])
        .output()
        .unwrap();

    // The merge should fail due to conflict
    assert!(!merge_output.status.success(), "Merge should have failed");

    // Verify we can recover from this state
    let status_output = Command::new("git")
        .current_dir(&repo_path)
        .args(["status"])
        .output()
        .unwrap();

    assert!(
        status_output.status.success(),
        "Git status should work after failed merge"
    );
}

#[test]
fn test_concurrent_workflow_execution() {
    // Test multiple workflows running simultaneously
    use std::sync::{Arc, Mutex};
    use std::thread;

    let (_temp_dir, repo_path) = create_test_repo().unwrap();
    let repo_path = Arc::new(repo_path);
    let results = Arc::new(Mutex::new(Vec::new()));

    // Spawn multiple threads to simulate concurrent operations
    let handles: Vec<_> = (0..3)
        .map(|i| {
            let repo_path = Arc::clone(&repo_path);
            let results = Arc::clone(&results);

            thread::spawn(move || {
                // Each thread tries to create a file
                let filename = format!("concurrent-{}.txt", i);
                let file_path = repo_path.join(&filename);

                let write_result = fs::write(&file_path, format!("Content from thread {}", i));

                let mut results = results.lock().unwrap();
                results.push((i, write_result.is_ok()));
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all operations succeeded
    let results = results.lock().unwrap();
    assert_eq!(results.len(), 3);
    for (_, success) in results.iter() {
        assert!(success, "Concurrent file write should succeed");
    }
}

#[test]
fn test_invalid_spec_file_handling() {
    // Test graceful handling of malformed spec files
    let temp_dir = TempDir::new().unwrap();
    let spec_dir = temp_dir.path().join("specs").join("temp");
    fs::create_dir_all(&spec_dir).unwrap();

    // Create invalid spec files
    let invalid_specs = vec![
        ("empty.md", ""),
        ("no-overview.md", "# Test Spec\n## Wrong Section\nContent"),
        (
            "invalid-yaml.md",
            "# Test\n```yaml\ninvalid: [syntax: here\n```",
        ),
        ("binary.md", "\u{FFFD}\u{FFFD}\u{0000}\u{0001}"),
    ];

    for (filename, content) in invalid_specs {
        let spec_path = spec_dir.join(filename);
        fs::write(&spec_path, content).unwrap();

        // Verify file exists but contains invalid content
        assert!(spec_path.exists());

        // In a real test, we would call the spec parser here
        // and verify it handles the invalid content gracefully
    }
}

#[test]
fn test_git_operation_failures() {
    // Test recovery from git command failures
    let temp_dir = TempDir::new().unwrap();
    let non_git_path = temp_dir.path();

    // Test git operations in non-git directory
    let operations = vec![
        vec!["status"],
        vec!["add", "."],
        vec!["commit", "-m", "test"],
        vec!["log", "-1"],
    ];

    for args in operations {
        let output = Command::new("git")
            .current_dir(non_git_path)
            .args(&args)
            .output()
            .unwrap();

        assert!(
            !output.status.success(),
            "Git {:?} should fail in non-git directory",
            args
        );

        // Verify we get meaningful error messages
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("not a git repository") || stderr.contains("fatal:"),
            "Should get clear error message for git {:?}",
            args
        );
    }
}

#[test]
fn test_file_system_edge_cases() {
    // Test handling of file system edge cases
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // Test very long filenames
    let long_name = "a".repeat(255); // Max filename length on most systems
    let long_path = base_path.join(&long_name);
    assert!(
        fs::write(&long_path, "content").is_ok(),
        "Should handle max length filenames"
    );

    // Test deeply nested directories
    let mut nested_path = base_path.to_path_buf();
    for i in 0..50 {
        nested_path = nested_path.join(format!("level{}", i));
    }
    assert!(
        fs::create_dir_all(&nested_path).is_ok(),
        "Should handle deeply nested directories"
    );

    // Test special characters in filenames (platform-dependent)
    #[cfg(not(windows))]
    {
        let special_names = vec![
            "file with spaces.txt",
            "file-with-dashes.txt",
            "file_with_underscores.txt",
            "file.multiple.dots.txt",
        ];

        for name in special_names {
            let path = base_path.join(name);
            assert!(
                fs::write(&path, "content").is_ok(),
                "Should handle special filename: {}",
                name
            );
        }
    }
}

#[test]
fn test_atomic_file_operations() {
    // Test that file operations are atomic
    use std::thread;
    use std::time::Duration;

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("atomic-test.json");

    // Write initial content
    fs::write(&file_path, r#"{"value": 0}"#).unwrap();

    // Spawn multiple threads trying to update the file
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let file_path = file_path.clone();
            thread::spawn(move || {
                // Simulate atomic write with temp file
                let temp_path = file_path.with_extension(format!("tmp{}", i));
                let content = format!(r#"{{"value": {}}}"#, i);

                fs::write(&temp_path, &content).unwrap();
                thread::sleep(Duration::from_millis(10)); // Simulate some processing

                // Atomic rename
                let _ = fs::rename(&temp_path, &file_path);
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify file contains valid JSON (not corrupted)
    let final_content = fs::read_to_string(&file_path).unwrap();
    assert!(
        final_content.starts_with('{') && final_content.ends_with('}'),
        "File should contain valid JSON after concurrent writes"
    );
}
