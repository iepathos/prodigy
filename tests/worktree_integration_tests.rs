use std::process::Command;
use tempfile::TempDir;

fn setup_test_repo() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Configure git user for commits
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["config", "user.email", "test@example.com"])
        .output()?;
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["config", "user.name", "Test User"])
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
                for entry in entries {
                    if let Ok(entry) = entry {
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
    assert!(stdout.contains("No active worktrees") || stdout.trim().is_empty());

    Ok(())
}

#[test]
fn test_mmm_improve_with_worktree_flag() -> anyhow::Result<()> {
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

    // Run mmm improve with worktree flag (but with 0 iterations to avoid Claude calls)
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["improve", "--worktree", "--max-iterations", "0"])
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
#[ignore] // This test requires Claude CLI to be installed
fn test_mmm_worktree_merge_command() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    // Would need to create a worktree first, then merge it
    // This requires full Claude CLI integration

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

    // Run mmm worktree clean
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["worktree", "clean"])
        .output()?;

    // Should succeed even with no worktrees
    assert!(output.status.success());

    Ok(())
}

#[test]
fn test_deprecated_env_var_warning() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    // Add .mmm directory for state management
    std::fs::create_dir_all(temp_dir.path().join(".mmm"))?;

    // Run mmm improve with deprecated env var
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .env("MMM_USE_WORKTREE", "true")
        .args(["improve", "--max-iterations", "0"])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show deprecation warning
    assert!(
        stderr.contains("MMM_USE_WORKTREE is deprecated"),
        "Should warn about deprecated env var. STDERR: {stderr}"
    );

    // Clean up any created worktrees
    cleanup_mmm_worktrees();

    Ok(())
}
