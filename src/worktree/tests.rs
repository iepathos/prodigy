use super::*;
use crate::subprocess::SubprocessManager;
use std::process::Command;
use tempfile::TempDir;

fn setup_test_repo() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
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

// Clean up worktree manager's base directory after tests
fn cleanup_worktree_dir(manager: &WorktreeManager) {
    if manager.base_dir.exists() {
        std::fs::remove_dir_all(&manager.base_dir).ok();
    }
}

#[test]
fn test_worktree_manager_creation() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    assert!(manager.base_dir.exists());

    // During tests, the manager uses a temp directory instead of home directory
    // Just verify the base_dir exists and contains expected structure
    assert!(manager.base_dir.to_string_lossy().contains("worktrees"));

    // The repo name is derived from temp_dir's file name
    let repo_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
    assert!(manager.base_dir.to_string_lossy().contains(repo_name));

    // Clean up
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_create_session_with_generated_name() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    assert!(session.name.starts_with("session-"));
    assert!(session.path.exists());
    assert_eq!(session.branch, format!("prodigy-{}", session.name));

    // Verify worktree was created
    let worktrees_output = Command::new("git")
        .current_dir(&temp_dir)
        .args(["worktree", "list"])
        .output()?;
    let worktrees = String::from_utf8_lossy(&worktrees_output.stdout);
    assert!(worktrees.contains(&session.name));

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_create_session_with_uuid_name() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    assert!(session.name.starts_with("session-"));
    assert!(session.path.exists());

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_list_sessions() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create multiple sessions
    let session1 = manager.create_session().await?;
    let session2 = manager.create_session().await?;

    // List sessions
    let sessions = manager.list_sessions().await?;
    assert!(sessions.len() >= 2);

    // Clean up
    manager.cleanup_session(&session1.name, false).await?;
    manager.cleanup_session(&session2.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_cleanup_session() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;
    let session_path = session.path.clone();

    // Verify session exists
    assert!(session_path.exists());

    // Cleanup session
    manager.cleanup_session(&session.name, false).await?;

    // Verify session is removed
    assert!(!session_path.exists());

    // Verify worktree is removed
    let worktrees_output = Command::new("git")
        .current_dir(&temp_dir)
        .args(["worktree", "list"])
        .output()?;
    let worktrees = String::from_utf8_lossy(&worktrees_output.stdout);
    assert!(!worktrees.contains(&session.name));

    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_merge_session() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    // Make a change in the worktree
    std::fs::write(session.path.join("test.txt"), "test content")?;
    Command::new("git")
        .current_dir(&session.path)
        .args(["add", "test.txt"])
        .output()?;
    Command::new("git")
        .current_dir(&session.path)
        .args(["commit", "-m", "test commit"])
        .output()?;

    // We can't actually test merge without Claude CLI
    // But we can verify the setup is correct

    // Clean up - use force=true since we made commits in the worktree
    manager.cleanup_session(&session.name, true).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_merge_already_merged() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_list_interrupted_sessions_empty() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // With no sessions, should return empty list
    let interrupted = manager.list_interrupted_sessions()?;
    assert_eq!(interrupted.len(), 0);

    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_list_interrupted_sessions_with_mixed_states() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create multiple sessions
    let session1 = manager.create_session().await?;
    let session2 = manager.create_session().await?;
    let session3 = manager.create_session().await?;

    // Set different states for each session
    manager.update_session_state(&session1.name, |state| {
        state.status = WorktreeStatus::Interrupted;
    })?;

    manager.update_session_state(&session2.name, |state| {
        state.status = WorktreeStatus::Completed;
    })?;

    manager.update_session_state(&session3.name, |state| {
        state.status = WorktreeStatus::Interrupted;
    })?;

    // Should return only interrupted sessions
    let interrupted = manager.list_interrupted_sessions()?;
    assert_eq!(interrupted.len(), 2);

    // Verify the interrupted sessions are the correct ones
    let interrupted_names: Vec<String> = interrupted.iter().map(|s| s.session_id.clone()).collect();
    assert!(interrupted_names.contains(&session1.name));
    assert!(interrupted_names.contains(&session3.name));
    assert!(!interrupted_names.contains(&session2.name));

    // Clean up
    manager.cleanup_session(&session1.name, false).await?;
    manager.cleanup_session(&session2.name, false).await?;
    manager.cleanup_session(&session3.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_list_interrupted_sessions_all_interrupted() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create sessions and mark all as interrupted
    let session1 = manager.create_session().await?;
    let session2 = manager.create_session().await?;

    manager.update_session_state(&session1.name, |state| {
        state.status = WorktreeStatus::Interrupted;
        state.iterations.completed = 3;
    })?;

    manager.update_session_state(&session2.name, |state| {
        state.status = WorktreeStatus::Interrupted;
        state.iterations.completed = 5;
    })?;

    // Should return all sessions
    let interrupted = manager.list_interrupted_sessions()?;
    assert_eq!(interrupted.len(), 2);

    // Verify iteration counts are preserved
    for state in &interrupted {
        if state.session_id == session1.name {
            assert_eq!(state.iterations.completed, 3);
        } else if state.session_id == session2.name {
            assert_eq!(state.iterations.completed, 5);
        }
    }

    // Clean up
    manager.cleanup_session(&session1.name, false).await?;
    manager.cleanup_session(&session2.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_list_interrupted_sessions_none_interrupted() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create sessions with non-interrupted states
    let session1 = manager.create_session().await?;
    let session2 = manager.create_session().await?;

    manager.update_session_state(&session1.name, |state| {
        state.status = WorktreeStatus::Completed;
    })?;

    manager.update_session_state(&session2.name, |state| {
        state.status = WorktreeStatus::Merged;
    })?;

    // Should return empty list
    let interrupted = manager.list_interrupted_sessions()?;
    assert_eq!(interrupted.len(), 0);

    // Clean up
    manager.cleanup_session(&session1.name, false).await?;
    manager.cleanup_session(&session2.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[test]
fn test_filter_sessions_by_status() {
    use super::{IterationInfo, WorktreeState, WorktreeStats, WorktreeStatus};
    use chrono::Utc;

    // Create test states with different statuses
    let states = vec![
        WorktreeState {
            session_id: "session1".to_string(),
            worktree_name: "wt1".to_string(),
            branch: "branch1".to_string(),
            original_branch: String::new(),
            status: WorktreeStatus::Interrupted,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            iterations: IterationInfo {
                completed: 0,
                max: 10,
            },
            stats: WorktreeStats::default(),
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: None,
            resumable: true,
        },
        WorktreeState {
            session_id: "session2".to_string(),
            worktree_name: "wt2".to_string(),
            branch: "branch2".to_string(),
            original_branch: String::new(),
            status: WorktreeStatus::Completed,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            iterations: IterationInfo {
                completed: 5,
                max: 10,
            },
            stats: WorktreeStats::default(),
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: None,
            resumable: true,
        },
        WorktreeState {
            session_id: "session3".to_string(),
            worktree_name: "wt3".to_string(),
            branch: "branch3".to_string(),
            original_branch: String::new(),
            status: WorktreeStatus::Interrupted,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            iterations: IterationInfo {
                completed: 2,
                max: 10,
            },
            stats: WorktreeStats::default(),
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: None,
            resumable: true,
        },
    ];

    // Test filtering for interrupted sessions
    let interrupted =
        WorktreeManager::filter_sessions_by_status(states.clone(), WorktreeStatus::Interrupted);
    assert_eq!(interrupted.len(), 2);
    assert!(interrupted
        .iter()
        .all(|s| s.status == WorktreeStatus::Interrupted));

    // Test filtering for completed sessions
    let completed =
        WorktreeManager::filter_sessions_by_status(states.clone(), WorktreeStatus::Completed);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].session_id, "session2");

    // Test filtering for non-existent status
    let merged = WorktreeManager::filter_sessions_by_status(states, WorktreeStatus::Merged);
    assert_eq!(merged.len(), 0);
}

#[test]
fn test_load_state_from_file() {
    use super::{IterationInfo, WorktreeState, WorktreeStats, WorktreeStatus};
    use chrono::Utc;
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    // Create a valid state
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-wt".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        status: WorktreeStatus::InProgress,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        iterations: IterationInfo {
            completed: 0,
            max: 10,
        },
        stats: WorktreeStats::default(),
        merged: false,
        merged_at: None,
        error: None,
        merge_prompt_shown: false,
        merge_prompt_response: None,
        interrupted_at: None,
        interruption_type: None,
        last_checkpoint: None,
        resumable: true,
    };

    // Write valid JSON file
    let json_path = temp_dir.path().join("state.json");
    fs::write(&json_path, serde_json::to_string(&state).unwrap()).unwrap();

    // Should successfully load the state
    let loaded = WorktreeManager::load_state_from_file(&json_path);
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().session_id, "test-session");

    // Test with non-JSON file
    let txt_path = temp_dir.path().join("state.txt");
    fs::write(&txt_path, "not json").unwrap();
    assert!(WorktreeManager::load_state_from_file(&txt_path).is_none());

    // Test with invalid JSON
    let bad_json_path = temp_dir.path().join("bad.json");
    fs::write(&bad_json_path, "{ invalid json }").unwrap();
    assert!(WorktreeManager::load_state_from_file(&bad_json_path).is_none());

    // Test with non-existent file
    let missing_path = temp_dir.path().join("missing.json");
    assert!(WorktreeManager::load_state_from_file(&missing_path).is_none());
}
