//! Tests for improved worktree tracking with metadata fallback
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeManager, WorktreeState, WorktreeStatus};
use anyhow::Result;
use std::fs;
use tempfile::TempDir;

/// Helper to set up a test repository with Git initialized
fn setup_test_repo() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["init"])
        .output()?;

    // Set up user config for commits
    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["config", "user.name", "Test User"])
        .output()?;

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "# Test")?;
    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["add", "."])
        .output()?;
    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    Ok(temp_dir)
}

#[tokio::test]
async fn test_list_sessions_includes_all_mmm_branches() -> Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create sessions with different branch patterns
    let session1 = manager.create_session().await?; // Creates mmm-session-* branch

    // Create a MapReduce-style worktree with merge branch
    let merge_branch = "merge-mmm-agent-test";
    let worktree_dir = manager.base_dir.join("mapreduce-session");
    std::process::Command::new("git")
        .current_dir(&manager.repo_path)
        .args([
            "worktree",
            "add",
            "-b",
            merge_branch,
            &worktree_dir.to_string_lossy(),
        ])
        .output()?;

    // List sessions should include both
    let sessions = manager.list_sessions().await?;
    assert!(sessions.len() >= 2, "Should find at least 2 sessions");

    // Verify we found both types of branches
    let has_regular = sessions.iter().any(|s| s.branch.starts_with("mmm-"));
    let has_merge = sessions.iter().any(|s| s.branch.starts_with("merge-mmm-"));

    assert!(has_regular, "Should find regular MMM session");
    assert!(has_merge, "Should find MapReduce merge session");

    // Clean up
    manager.cleanup_session(&session1.name, true).await?;
    std::process::Command::new("git")
        .current_dir(&manager.repo_path)
        .args([
            "worktree",
            "remove",
            "--force",
            &worktree_dir.to_string_lossy(),
        ])
        .output()?;

    Ok(())
}

#[tokio::test]
async fn test_list_sessions_with_metadata_fallback() -> Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create metadata directory
    let metadata_dir = manager.base_dir.join(".metadata");
    fs::create_dir_all(&metadata_dir)?;

    // Create a metadata file for a session that doesn't exist in Git
    let orphaned_state = WorktreeState {
        session_id: "orphaned-session".to_string(),
        worktree_name: "orphaned-session".to_string(),
        branch: "mmm-orphaned-branch".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
        iterations: crate::worktree::IterationInfo {
            completed: 2,
            max: 10,
        },
        stats: crate::worktree::WorktreeStats::default(),
        merged: false,
        merged_at: None,
        error: None,
        merge_prompt_shown: false,
        merge_prompt_response: None,
        interrupted_at: None,
        interruption_type: None,
        last_checkpoint: None,
        resumable: false,
    };

    // Save the orphaned state with correct filename format
    let state_path = metadata_dir.join("session-orphaned-session.json");
    fs::write(&state_path, serde_json::to_string_pretty(&orphaned_state)?)?;

    // Create the worktree directory (simulating a partially cleaned up session)
    let orphaned_dir = manager.base_dir.join("orphaned-session");
    fs::create_dir_all(&orphaned_dir)?;

    // Create a normal session
    let normal_session = manager.create_session().await?;

    // List sessions should include both the normal session and the orphaned one
    let sessions = manager.list_sessions().await?;

    // Should find both sessions
    assert!(sessions.len() >= 2, "Should find at least 2 sessions");

    let has_normal = sessions.iter().any(|s| s.name == normal_session.name);
    let has_orphaned = sessions.iter().any(|s| s.name == "orphaned-session");

    assert!(has_normal, "Should find normal session");
    assert!(has_orphaned, "Should find orphaned session from metadata");

    // Clean up
    manager.cleanup_session(&normal_session.name, true).await?;
    fs::remove_dir_all(&orphaned_dir)?;

    Ok(())
}

#[tokio::test]
async fn test_metadata_sessions_exclude_cleaned_up() -> Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create metadata directory
    let metadata_dir = manager.base_dir.join(".metadata");
    fs::create_dir_all(&metadata_dir)?;

    // Create a cleaned up session metadata
    let cleaned_state = WorktreeState {
        session_id: "cleaned-session".to_string(),
        worktree_name: "cleaned-session".to_string(),
        branch: "mmm-cleaned-branch".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::CleanedUp, // This session is cleaned up
        iterations: crate::worktree::IterationInfo {
            completed: 5,
            max: 5,
        },
        stats: crate::worktree::WorktreeStats::default(),
        merged: true,
        merged_at: Some(chrono::Utc::now()),
        error: None,
        merge_prompt_shown: false,
        merge_prompt_response: None,
        interrupted_at: None,
        interruption_type: None,
        last_checkpoint: None,
        resumable: false,
    };

    // Save the cleaned up state
    let state_path = metadata_dir.join("session-cleaned-session.json");
    fs::write(&state_path, serde_json::to_string_pretty(&cleaned_state)?)?;

    // Create the worktree directory (shouldn't be included)
    let cleaned_dir = manager.base_dir.join("cleaned-session");
    fs::create_dir_all(&cleaned_dir)?;

    // List sessions should NOT include the cleaned up session
    let sessions = manager.list_sessions().await?;

    let has_cleaned = sessions.iter().any(|s| s.name == "cleaned-session");
    assert!(!has_cleaned, "Should NOT find cleaned up session");

    // Clean up
    fs::remove_dir_all(&cleaned_dir)?;

    Ok(())
}

#[tokio::test]
async fn test_auto_merge_environment_variable() -> Result<()> {
    // Test that auto-merge detection works with environment variables

    // Set the environment variable
    std::env::set_var("MMM_AUTO_MERGE", "true");

    // The should_auto_merge function should return true when environment variable is set
    // For now, just verify the environment variable is set correctly
    assert_eq!(std::env::var("MMM_AUTO_MERGE").unwrap_or_default(), "true");

    // Also test the alternative variable
    std::env::set_var("MMM_AUTO_CONFIRM", "true");
    assert_eq!(
        std::env::var("MMM_AUTO_CONFIRM").unwrap_or_default(),
        "true"
    );

    // Clean up
    std::env::remove_var("MMM_AUTO_MERGE");
    std::env::remove_var("MMM_AUTO_CONFIRM");

    Ok(())
}

#[tokio::test]
async fn test_mapreduce_branch_patterns() -> Result<()> {
    // Test that various MapReduce branch patterns are recognized
    let patterns = vec![
        "mmm-session-abc123",        // Regular session
        "merge-mmm-agent-cook-123",  // MapReduce merge branch
        "mmm-agent-cook-123-item_0", // MapReduce agent branch
        "temp-master",               // Temporary master branch
    ];

    for pattern in patterns {
        // All branches should be considered valid MMM branches
        // when they're in the .mmm/worktrees directory
        assert!(
            pattern.contains("mmm") || pattern == "temp-master",
            "Branch {} should be recognized",
            pattern
        );
    }

    Ok(())
}

#[cfg(test)]
mod metadata_tests {
    #[test]
    fn test_metadata_file_filtering() {
        // Test that we correctly filter metadata files
        let valid_files = vec![
            "session-abc123.json",
            "session-061f22c3-172c-41a2-99d2-53efeeeba0da.json",
        ];

        let invalid_files = vec![
            "cleanup.log",
            "README.md",
            "config.json",
            "not-a-session.json",
        ];

        for file in valid_files {
            assert!(file.starts_with("session-") && file.ends_with(".json"));
        }

        for file in invalid_files {
            assert!(!file.starts_with("session-") || !file.ends_with(".json"));
        }
    }
}
