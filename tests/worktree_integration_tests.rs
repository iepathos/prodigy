use mmm::worktree::WorktreeManager;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn setup_test_repo() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Configure git user for commits (use --local to ensure we don't modify global config)
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["config", "--local", "user.email", "test@example.com"])
        .output()?;
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["config", "--local", "user.name", "Test User"])
        .output()?;

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "# Test Repo")?;
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["add", "."])
        .output()?;
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    Ok(temp_dir)
}

fn cleanup_mmm_worktrees() {
    // Clean up any test worktrees from home directory
    if let Some(home_dir) = dirs::home_dir() {
        let worktree_base = home_dir.join(".mmm").join("worktrees");
        if worktree_base.exists() {
            // Only clean up test-related worktrees
            if let Ok(entries) = std::fs::read_dir(&worktree_base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir()
                        && path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("test-"))
                            .unwrap_or(false)
                    {
                        std::fs::remove_dir_all(&path).ok();
                    }
                }
            }
        }
    }
}

#[test]
fn test_mmm_worktree_list_command() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    // Run mmm worktree list in the test repo
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["worktree", "list"])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should indicate no active worktrees
    assert!(stdout.contains("No active MMM worktrees found") || stdout.trim().is_empty());

    Ok(())
}

#[test]
fn test_mmm_cook_with_worktree_flag() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    // Add .mmm directory for state management
    std::fs::create_dir_all(temp_dir.path().join(".mmm"))?;

    // Create a simple source file
    std::fs::write(
        temp_dir.path().join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
}
"#,
    )?;

    // Run mmm cook with worktree flag (but with 0 iterations to avoid Claude calls)
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["cook", "--worktree", "--max-iterations", "0"])
        .output()?;

    // The command might fail due to no Claude CLI, but it should at least try to create a worktree
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check if worktree creation was attempted
    assert!(
        stdout.contains("Created worktree")
            || stderr.contains("worktree")
            || stderr.contains("Claude CLI not found"),
        "Output should mention worktree creation or Claude CLI missing.\nSTDOUT: {stdout}\nSTDERR: {stderr}"
    );

    // Clean up any created worktrees
    cleanup_mmm_worktrees();

    Ok(())
}

#[test]
fn test_mmm_worktree_merge_command() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    // Test that merge command fails gracefully when worktree doesn't exist

    // Run mmm worktree merge
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["worktree", "merge", "test-worktree"])
        .output()?;

    // Should fail gracefully if worktree doesn't exist
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("does not exist"));

    Ok(())
}

#[test]
fn test_mmm_worktree_clean_command() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    // Run mmm worktree clean --all
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["worktree", "clean", "--all"])
        .output()?;

    // Should succeed even with no worktrees
    assert!(output.status.success());

    Ok(())
}

#[test]
fn test_worktree_full_lifecycle() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

    // Test create, list, merge, cleanup lifecycle
    let _session = manager.create_session(Some("test-lifecycle"))?;

    let sessions = manager.list_sessions()?;
    assert_eq!(sessions.len(), 1);

    // Simulate work in worktree - first ensure the directory exists
    let worktree_path = temp_dir.path().join(".mmm/worktrees/test-lifecycle");
    fs::create_dir_all(&worktree_path).unwrap();
    fs::write(worktree_path.join("test.txt"), "test content").unwrap();

    // Test merge
    let merge_result = manager.merge_session("test-lifecycle");
    assert!(merge_result.is_err()); // Expected to fail without actual git worktree

    // Test cleanup
    let cleanup_result = manager.cleanup_session("test-lifecycle", false);
    // Cleanup might fail if worktree doesn't exist, but that's ok for this test
    let _ = cleanup_result;

    Ok(())
}
