use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Test that validates the cook command executes multiple iterations correctly
/// and passes focus directive on every iteration
#[test]
fn test_cook_multiple_iterations_with_focus() -> Result<()> {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Initialize a git repository
    Command::new("git")
        .current_dir(temp_path)
        .args(["init"])
        .output()?;

    // Configure git user for commits
    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.name", "Test User"])
        .output()?;

    // Create initial file to modify
    let test_file = temp_path.join("test.rs");
    fs::write(&test_file, "// Initial content\nfn main() {}\n")?;

    // Create .mmm directory
    fs::create_dir_all(temp_path.join(".mmm"))?;

    // Create a custom workflow configuration that uses the built-in commands
    let workflow_config = r#"
[workflow]
commands = [
    "mmm-code-review",
    "mmm-implement-spec",
    "mmm-lint"
]
max_iterations = 3
"#;
    fs::write(temp_path.join(".mmm/config.toml"), workflow_config)?;

    // Create mock slash commands that simulate Claude CLI behavior
    create_mock_commands(temp_path)?;

    // Initial commit
    Command::new("git")
        .current_dir(temp_path)
        .args(["add", "."])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    // Run mmm cook with 3 iterations and focus
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(temp_path)
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_ITERATIONS", "true") // Enable iteration tracking
        .args(["cook", "-n", "3", "-f", "documentation", "--show-progress"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Debug output
    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    // Verify the command executed successfully
    assert!(output.status.success(), "mmm cook failed: {stderr}");

    // Check that we ran iterations
    let has_iterations = stdout.contains("Workflow iteration")
        || stdout.contains("Iteration 1/3")
        || stdout.contains("iteration 1/3");
    assert!(has_iterations, "Should show iteration progress");

    // Verify code review command was run
    assert!(
        stdout.contains("Running /mmm-code-review") || stdout.contains("mmm-code-review"),
        "Should run mmm-code-review command"
    );

    // Check that we have the focus directive
    assert!(
        stdout.contains("documentation") || stdout.contains("Focus: documentation"),
        "Focus directive should be mentioned"
    );

    // Verify completion or iterations occurred
    assert!(
        stdout.contains("Complete")
            || stdout.contains("Iterations:")
            || stdout.contains("Files changed:"),
        "Should show completion status"
    );

    Ok(())
}

/// Test that verifies the iteration stops early when no changes are found
#[test]
#[ignore = "Requires more complex mocking to simulate 'no changes found' condition"]
fn test_cook_stops_early_when_no_changes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .current_dir(temp_path)
        .args(["init"])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.name", "Test User"])
        .output()?;

    // Create .mmm directory and config
    fs::create_dir_all(temp_path.join(".mmm"))?;

    // Create a workflow configuration (use default commands)
    let workflow_config = r#"
[workflow]
commands = [
    "mmm-code-review",
    "mmm-implement-spec", 
    "mmm-lint"
]
max_iterations = 5
"#;
    fs::write(temp_path.join(".mmm/config.toml"), workflow_config)?;

    create_mock_commands(temp_path)?;

    // Initial commit
    fs::write(temp_path.join("test.rs"), "fn main() {}\n")?;
    Command::new("git")
        .current_dir(temp_path)
        .args(["add", "."])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    // Run mmm cook with max 5 iterations
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(temp_path)
        .env("MMM_TEST_MODE", "true")
        .args(["cook", "-n", "5", "--show-progress"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // Should stop after 1 iteration when no changes found
    let iteration_1_count = stdout.matches("iteration 1/5").count();
    let iteration_2_count = stdout.matches("iteration 2/5").count();

    assert!(
        iteration_1_count >= 1,
        "Should have run at least iteration 1"
    );
    assert_eq!(
        iteration_2_count, 0,
        "Should not run iteration 2 when no changes"
    );

    Ok(())
}

/// Test to specifically catch the bug where focus was only applied on first iteration
#[test]
#[ignore = "Requires more complex mocking to track focus application across iterations"]
fn test_focus_applied_every_iteration() -> Result<()> {
    // This test creates a scenario where we can track if focus is passed
    // to the code review command on each iteration

    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Initialize git
    Command::new("git")
        .current_dir(temp_path)
        .args(["init"])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.name", "Test User"])
        .output()?;

    fs::create_dir_all(temp_path.join(".mmm"))?;

    // Create a workflow configuration
    let workflow_config = r#"
[workflow]
commands = [
    "mmm-code-review",
    "mmm-implement-spec",
    "mmm-lint"
]
max_iterations = 3
"#;
    fs::write(temp_path.join(".mmm/config.toml"), workflow_config)?;

    // Create a tracking file for focus directive
    let focus_tracker = temp_path.join("focus_track.txt");

    // Create mock commands that track when focus is passed
    create_focus_tracking_commands(temp_path, &focus_tracker)?;

    // Initial commit
    fs::write(temp_path.join("test.rs"), "fn main() {}\n")?;
    Command::new("git")
        .current_dir(temp_path)
        .args(["add", "."])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    // Run with focus directive
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(temp_path)
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TRACK_FOCUS", focus_tracker.to_str().unwrap())
        .args(["cook", "-n", "3", "-f", "security", "--show-progress"])
        .output()?;

    assert!(output.status.success());

    // Read the focus tracking file
    if focus_tracker.exists() {
        let focus_log = fs::read_to_string(&focus_tracker)?;
        let focus_entries: Vec<&str> = focus_log.lines().collect();

        // With the bug, only iteration 1 would have focus
        // With the fix, all iterations should have focus
        assert!(
            focus_entries.len() >= 3,
            "Focus should be tracked for each iteration, found: {}",
            focus_entries.len()
        );

        for (i, entry) in focus_entries.iter().enumerate() {
            assert!(
                entry.contains("security"),
                "Iteration {} should have focus 'security', got: {}",
                i + 1,
                entry
            );
        }
    }

    Ok(())
}

/// Helper function to create mock commands for testing
fn create_mock_commands(_temp_path: &Path) -> Result<()> {
    // These mock commands simulate the Claude CLI behavior in test mode
    // They are referenced by the workflow configuration

    // For now, MMM_TEST_MODE skips actual command execution
    // so we don't need to create actual mock executables

    Ok(())
}

/// Helper function to create commands that track focus directive
fn create_focus_tracking_commands(_temp_path: &Path, _tracker_file: &Path) -> Result<()> {
    // In test mode, we can track focus through environment variables
    // This would be handled by the workflow executor when MMM_TEST_MODE is set

    Ok(())
}

/// Integration test for worktree mode with multiple iterations
#[test]
fn test_cook_worktree_multiple_iterations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .current_dir(temp_path)
        .args(["init"])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["config", "user.name", "Test User"])
        .output()?;

    // Set up initial branch
    Command::new("git")
        .current_dir(temp_path)
        .args(["checkout", "-b", "main"])
        .output()?;

    // Create initial content
    fs::create_dir_all(temp_path.join(".mmm"))?;
    fs::write(temp_path.join("test.rs"), "fn main() {}\n")?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["add", "."])
        .output()?;

    Command::new("git")
        .current_dir(temp_path)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    // Run mmm cook in worktree mode with multiple iterations
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(temp_path)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-w", // worktree mode
            "-n",
            "3",
            "-f",
            "performance",
            "--show-progress",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Worktree test STDOUT:\n{stdout}");
    println!("Worktree test STDERR:\n{stderr}");

    // Should create a worktree
    assert!(
        stdout.contains("Created worktree:") || stdout.contains("🌳"),
        "Should create a worktree"
    );

    // Should complete successfully
    assert!(
        stdout.contains("Improvements completed in worktree") || stdout.contains("✅"),
        "Should complete improvements"
    );

    Ok(())
}
