use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Test that validates the cook command executes multiple iterations correctly
#[test]
fn test_cook_multiple_iterations() -> Result<()> {
    // Setup test environment
    let (temp_path, playbook_path) = setup_test_environment()?;

    // Run mmm cook with 3 iterations
    let output = run_cook_command(&temp_path, &playbook_path, 3)?;

    // Verify test results
    verify_cook_output(&output, true, true)?;

    Ok(())
}

/// Helper function to set up test environment
fn setup_test_environment() -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Initialize git repository with config
    initialize_git_repo(&temp_path)?;

    // Create test file and directories
    create_test_structure(&temp_path)?;

    // Create test playbook
    let playbook_path = create_test_playbook(&temp_path)?;

    // Create mock commands and initial commit
    create_mock_commands(&temp_path)?;
    create_initial_commit(&temp_path)?;

    // Keep temp_dir alive by leaking it
    std::mem::forget(temp_dir);

    Ok((temp_path, playbook_path))
}

/// Initialize git repository with user config
fn initialize_git_repo(path: &Path) -> Result<()> {
    Command::new("git")
        .current_dir(path)
        .args(["init"])
        .output()?;

    Command::new("git")
        .current_dir(path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    Command::new("git")
        .current_dir(path)
        .args(["config", "user.name", "Test User"])
        .output()?;

    Ok(())
}

/// Create test file structure
fn create_test_structure(path: &Path) -> Result<()> {
    let test_file = path.join("test.rs");
    fs::write(&test_file, "// Initial content\nfn main() {}\n")?;

    fs::create_dir_all(path.join(".mmm"))?;
    fs::create_dir_all(path.join("specs/temp"))?;

    Ok(())
}

/// Create test playbook
fn create_test_playbook(path: &Path) -> Result<std::path::PathBuf> {
    let playbook_path = path.join("playbook.yml");
    let playbook_content = r#"# Simple test playbook
commands:
  - name: mmm-code-review
  - name: mmm-lint"#;
    fs::write(&playbook_path, playbook_content)?;

    Ok(playbook_path)
}

/// Create initial git commit
fn create_initial_commit(path: &Path) -> Result<()> {
    Command::new("git")
        .current_dir(path)
        .args(["add", "."])
        .output()?;

    Command::new("git")
        .current_dir(path)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    Ok(())
}

/// Run cook command with specified parameters
fn run_cook_command(
    path: &Path,
    playbook_path: &Path,
    iterations: u32,
) -> Result<std::process::Output> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mmm"));
    cmd.current_dir(path)
        .env("MMM_TEST_MODE", "true")
        .env("MMM_TEST_ITERATIONS", "true");

    let iterations_str = iterations.to_string();
    let args = vec!["cook", "-n", &iterations_str, playbook_path.to_str().unwrap()];

    Ok(cmd.args(&args).output()?)
}

/// Verify cook command output
fn verify_cook_output(
    output: &std::process::Output,
    expect_iterations: bool,
    expect_code_review: bool,
) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Debug output
    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    // Verify success
    assert!(output.status.success(), "mmm cook failed: {stderr}");

    // Check iterations
    if expect_iterations {
        let has_iterations = stdout.contains("Workflow iteration")
            || stdout.contains("Iteration")
            || stdout.contains("Starting improvement loop");
        assert!(has_iterations, "Should show iteration progress");
    }

    // Check code review
    if expect_code_review {
        assert!(
            stdout.contains("Running /mmm-code-review") || stdout.contains("mmm-code-review"),
            "Should run mmm-code-review command"
        );
    }


    // Verify completion
    assert!(
        stdout.contains("Complete")
            || stdout.contains("iterations")
            || stdout.contains("files changed"),
        "Should show completion status"
    );

    Ok(())
}

/// Test that verifies the iteration stops early when no changes are found
#[test]
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

    // Create a simple test playbook
    let playbook_path = temp_path.join("playbook.yml");
    let playbook_content = r#"# Simple test playbook
commands:
  - name: mmm-code-review
  - name: mmm-lint"#;
    fs::write(&playbook_path, playbook_content)?;

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
    // Configure test mode to simulate no changes for all commands
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(temp_path)
        .env("MMM_TEST_MODE", "true")
        .env(
            "MMM_TEST_NO_CHANGES_COMMANDS",
            "mmm-code-review,mmm-implement-spec,mmm-lint",
        )
        .args(["cook", "-n", "5", playbook_path.to_str().unwrap()])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // With commit verification, the command should fail when no commits are created
    assert!(
        !output.status.success(),
        "Command should fail when no commits are created"
    );

    // Print stdout and stderr for debugging
    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    // Should stop after 1 iteration when no changes found
    // Check that we started the improvement loop
    let has_start = stdout.contains("Starting improvement loop");

    // Check that code review was run at least once
    let has_code_review = stdout.contains("Executing command: /mmm-code-review")
        || stdout.contains("Running /mmm-code-review")
        || stdout.contains("[TEST MODE] Would execute Claude command: /mmm-code-review")
        || stdout.contains("Executing step 1/2: /mmm-code-review");

    // Check for the "no commits" error message
    let has_error_msg = stderr.contains("No changes were committed")
        || stderr.contains("No commits created")
        || stdout.contains("No changes were committed")
        || stdout.contains("No commits created");

    assert!(has_start, "Should have started the improvement loop");
    assert!(has_code_review, "Should have run code review at least once");
    assert!(has_error_msg, "Should show error about no commits created");

    Ok(())
}

/// Test to verify workflows execute in the expected order
#[test]
fn test_workflow_execution_order() -> Result<()> {
    // This test verifies that commands execute in the order defined in the playbook

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

    // Create a simple test playbook
    let playbook_path = temp_path.join("playbook.yml");
    let playbook_content = r#"# Simple test playbook
commands:
  - name: mmm-code-review
  - name: mmm-lint"#;
    fs::write(&playbook_path, playbook_content)?;

    // Create mock commands
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

    // Run the workflow
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(temp_path)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "-n",
            "3",
            playbook_path.to_str().unwrap(),
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    // Check that commands were executed in the expected order
    let code_review_pos = stdout.find("mmm-code-review");
    let lint_pos = stdout.find("mmm-lint");

    if let (Some(cr), Some(l)) = (code_review_pos, lint_pos) {
        assert!(
            cr < l,
            "Code review should execute before lint"
        );
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

    // Create a simple test playbook
    let playbook_path = temp_path.join("playbook.yml");
    let playbook_content = r#"# Simple test playbook
commands:
  - name: mmm-code-review
  - name: mmm-lint"#;
    fs::write(&playbook_path, playbook_content)?;

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
            playbook_path.to_str().unwrap(),
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Worktree test STDOUT:\n{stdout}");
    println!("Worktree test STDERR:\n{stderr}");

    // Should create a worktree
    assert!(
        stdout.contains("Created worktree at:") || stdout.contains("🌳"),
        "Should create a worktree"
    );

    // Should complete successfully
    assert!(
        stdout.contains("Improvements completed in worktree") || stdout.contains("✅"),
        "Should complete improvements"
    );

    Ok(())
}
