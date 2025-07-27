use std::process::Command;
use tempfile::TempDir;

/// Test error handling when claude CLI is not installed
#[test]
fn test_claude_cli_not_found() {
    // Try to run a non-existent claude-like command
    let result = Command::new("claude_not_installed_xyz")
        .args(["--dangerously-skip-permissions", "/mmm-code-review"])
        .output();

    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
    }
}

/// Test handling of subprocess failures
#[test]
fn test_subprocess_failure_handling() {
    // Test git command with invalid arguments
    let output = Command::new("git")
        .args(["invalid-command-xyz"])
        .output()
        .expect("Failed to execute git");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid-command-xyz"));
}

/// Test concurrent git operations safety
#[test]
fn test_concurrent_git_operations() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to init git repo");

    // Configure git (use --local to ensure we don't modify global config)
    Command::new("git")
        .args(["config", "--local", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "--local", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to config name");

    // Create a mutex to ensure atomic git operations
    let git_mutex = Arc::new(Mutex::new(()));
    let results = Arc::new(Mutex::new(Vec::new()));

    // Spawn multiple threads trying to create commits
    let mut handles = vec![];

    for i in 0..5 {
        let mutex_clone = Arc::clone(&git_mutex);
        let results_clone = Arc::clone(&results);
        let path_clone = repo_path.clone();

        let handle = thread::spawn(move || {
            // Lock mutex before git operation
            let _lock = mutex_clone.lock().unwrap();

            let output = Command::new("git")
                .args(["commit", "--allow-empty", "-m", &format!("Commit {i}")])
                .current_dir(path_clone)
                .output()
                .expect("Failed to create commit");

            results_clone.lock().unwrap().push(output.status.success());
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Check that all operations succeeded
    let results = results.lock().unwrap();
    assert_eq!(results.len(), 5);
    assert!(results.iter().all(|&success| success));
}

/// Test error recovery in improve loop
#[cfg(test)]
mod improve_error_recovery {
    use anyhow::{anyhow, Result};

    fn simulate_claude_cli_call(should_fail: bool) -> Result<String> {
        if should_fail {
            Err(anyhow!("Claude CLI failed"))
        } else {
            Ok("Success".to_string())
        }
    }

    #[test]
    fn test_error_recovery() {
        // Test successful call
        let result = simulate_claude_cli_call(false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");

        // Test failed call
        let result = simulate_claude_cli_call(true);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Claude CLI failed");
    }

    #[test]
    fn test_retry_logic() {
        let mut attempts = 0;
        let max_retries = 3;

        loop {
            attempts += 1;

            // Simulate failure for first 2 attempts
            let result = simulate_claude_cli_call(attempts < 3);

            if result.is_ok() || attempts >= max_retries {
                break;
            }
        }

        assert_eq!(attempts, 3);
    }
}
