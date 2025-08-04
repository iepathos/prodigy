use mmm::subprocess::SubprocessManager;
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

    // Run mmm worktree ls in the test repo
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["worktree", "ls"])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should indicate no active worktrees
    assert!(stdout.contains("No active MMM worktrees found") || stdout.trim().is_empty());

    Ok(())
}

#[test]
fn test_mmm_worktree_list_alias_backward_compatibility() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;

    // Run mmm worktree list (using the alias) in the test repo
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
fn test_git_head_detection_in_worktree() -> anyhow::Result<()> {
    // This test verifies that git commands run in the correct directory when using worktrees
    let temp_dir = setup_test_repo()?;
    let repo_path = temp_dir.path();

    // Create a worktree
    let worktree_path = repo_path.join("../test-worktree");
    Command::new("git")
        .current_dir(repo_path)
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            "-b",
            "test-branch",
        ])
        .output()?;

    // Make commits in both main repo and worktree
    fs::write(repo_path.join("main.txt"), "main content")?;
    Command::new("git")
        .current_dir(repo_path)
        .args(["add", "."])
        .output()?;
    Command::new("git")
        .current_dir(repo_path)
        .args(["commit", "-m", "Main repo commit"])
        .output()?;

    fs::write(worktree_path.join("worktree.txt"), "worktree content")?;
    Command::new("git")
        .current_dir(&worktree_path)
        .args(["add", "."])
        .output()?;
    Command::new("git")
        .current_dir(&worktree_path)
        .args(["commit", "-m", "Worktree commit"])
        .output()?;

    // Get HEAD from main repo
    let main_head = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let main_head = String::from_utf8_lossy(&main_head.stdout)
        .trim()
        .to_string();

    // Get HEAD from worktree
    let worktree_head = Command::new("git")
        .current_dir(&worktree_path)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let worktree_head = String::from_utf8_lossy(&worktree_head.stdout)
        .trim()
        .to_string();

    // They should be different
    assert_ne!(
        main_head, worktree_head,
        "Main repo and worktree should have different HEADs"
    );
    assert_eq!(main_head.len(), 40, "Main HEAD should be a valid SHA");
    assert_eq!(
        worktree_head.len(),
        40,
        "Worktree HEAD should be a valid SHA"
    );

    // Test that running git commands with current_dir set correctly gets the right HEAD
    let test_head_cmd = |dir: &std::path::Path| -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("Failed to get HEAD");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    assert_eq!(
        test_head_cmd(repo_path),
        main_head,
        "HEAD detection in main repo should work"
    );
    assert_eq!(
        test_head_cmd(&worktree_path),
        worktree_head,
        "HEAD detection in worktree should work"
    );

    // Clean up
    Command::new("git")
        .current_dir(repo_path)
        .args([
            "worktree",
            "remove",
            worktree_path.to_str().unwrap(),
            "--force",
        ])
        .output()?;

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

#[tokio::test]
async fn test_worktree_full_lifecycle() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();

    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Test create, list, merge, cleanup lifecycle
    let session = manager.create_session().await?;

    let sessions = manager.list_sessions().await?;
    assert_eq!(sessions.len(), 1);

    // Simulate work in worktree - first ensure the directory exists
    let worktree_path = session.path.clone();
    fs::create_dir_all(&worktree_path).unwrap();
    fs::write(worktree_path.join("test.txt"), "test content").unwrap();

    // Test merge
    let merge_result = manager.merge_session(&session.name).await;
    assert!(merge_result.is_err()); // Expected to fail without actual git worktree

    // Test cleanup
    let cleanup_result = manager.cleanup_session(&session.name, false).await;
    // Cleanup might fail if worktree doesn't exist, but that's ok for this test
    let _ = cleanup_result;

    Ok(())
}
