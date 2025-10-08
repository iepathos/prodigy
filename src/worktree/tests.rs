use super::manager_queries::{collect_all_states, filter_sessions_by_status, load_state_from_file};
use super::*;
use crate::subprocess::SubprocessManager;
use crate::testing::fixtures::isolation::TestGitRepo;
use std::process::Command;
use tempfile::TempDir;

fn setup_test_repo() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Configure git user
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["config", "user.email", "test@test.com"])
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
    let interrupted = filter_sessions_by_status(states.clone(), WorktreeStatus::Interrupted);
    assert_eq!(interrupted.len(), 2);
    assert!(interrupted
        .iter()
        .all(|s| s.status == WorktreeStatus::Interrupted));

    // Test filtering for completed sessions
    let completed = filter_sessions_by_status(states.clone(), WorktreeStatus::Completed);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].session_id, "session2");

    // Test filtering for non-existent status
    let merged = filter_sessions_by_status(states, WorktreeStatus::Merged);
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
    let loaded = load_state_from_file(&json_path);
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().session_id, "test-session");

    // Test with non-JSON file
    let txt_path = temp_dir.path().join("state.txt");
    fs::write(&txt_path, "not json").unwrap();
    assert!(load_state_from_file(&txt_path).is_none());

    // Test with invalid JSON
    let bad_json_path = temp_dir.path().join("bad.json");
    fs::write(&bad_json_path, "{ invalid json }").unwrap();
    assert!(load_state_from_file(&bad_json_path).is_none());

    // Test with non-existent file
    let missing_path = temp_dir.path().join("missing.json");
    assert!(load_state_from_file(&missing_path).is_none());
}

#[tokio::test]
async fn test_worktree_tracks_feature_branch() -> anyhow::Result<()> {
    // Use TestGitRepo for isolation
    let repo = TestGitRepo::new()?;

    // Create initial commit
    std::fs::write(repo.path().join("README.md"), "# Test Repo")?;
    Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .output()?;
    repo.commit("Initial commit")?;

    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(repo.path().to_path_buf(), subprocess)?;

    // Create a feature branch and check it out
    repo.create_branch("feature/my-feature")?;

    // Create session from the feature branch
    let session = manager.create_session().await?;

    // Verify the session was created with correct original branch
    let state = manager.get_session_state(&session.name)?;
    assert_eq!(state.original_branch, "feature/my-feature");

    // Verify get_merge_target returns the feature branch
    let merge_target = manager.get_merge_target(&session.name).await?;
    assert_eq!(merge_target, "feature/my-feature");

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_worktree_from_detached_head() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Get the current commit hash
    let commit_output = Command::new("git")
        .current_dir(&temp_dir)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let commit_hash = String::from_utf8_lossy(&commit_output.stdout)
        .trim()
        .to_string();

    // Checkout detached HEAD
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["checkout", &commit_hash])
        .output()?;

    // Determine the default branch
    let default_branch_output = Command::new("git")
        .current_dir(&temp_dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    let default_branch_str = String::from_utf8_lossy(&default_branch_output.stdout);
    let default_branch = default_branch_str.trim();

    // If HEAD is detached, this should return "HEAD", so we need to find the default branch
    let default_branch = if default_branch == "HEAD" {
        // Get the default branch from symbolic-ref
        let symbolic_output = Command::new("git")
            .current_dir(&temp_dir)
            .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
            .output();

        if let Ok(output) = symbolic_output {
            let symbolic_ref = String::from_utf8_lossy(&output.stdout);
            symbolic_ref
                .trim()
                .strip_prefix("refs/remotes/origin/")
                .unwrap_or("master")
                .to_string()
        } else {
            // Fallback: check if master or main exists
            let branches_output = Command::new("git")
                .current_dir(&temp_dir)
                .args(["branch", "--list", "master", "main"])
                .output()?;
            let branches = String::from_utf8_lossy(&branches_output.stdout);
            if branches.contains("master") {
                "master".to_string()
            } else if branches.contains("main") {
                "main".to_string()
            } else {
                "master".to_string()
            }
        }
    } else {
        default_branch.to_string()
    };

    // Create session from detached HEAD
    let session = manager.create_session().await?;

    // Verify the session tracks the default branch as fallback
    let state = manager.get_session_state(&session.name)?;
    assert_eq!(state.original_branch, default_branch);

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_original_branch_deleted() -> anyhow::Result<()> {
    // Use TestGitRepo for isolation
    let repo = TestGitRepo::new()?;

    // Create initial commit on master
    std::fs::write(repo.path().join("README.md"), "# Test Repo")?;
    Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .output()?;
    repo.commit("Initial commit")?;

    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(repo.path().to_path_buf(), subprocess)?;

    // Create a feature branch
    repo.create_branch("feature/temp-branch")?;

    // Create session from the feature branch
    let session = manager.create_session().await?;

    // Verify original branch is tracked
    let state = manager.get_session_state(&session.name)?;
    assert_eq!(state.original_branch, "feature/temp-branch");

    // Switch back to master and delete the feature branch
    repo.checkout("master")?;
    Command::new("git")
        .current_dir(repo.path())
        .args(["branch", "-D", "feature/temp-branch"])
        .output()?;

    // Get merge target should fall back to default branch
    let merge_target = manager.get_merge_target(&session.name).await?;

    // The default branch should be master or main
    let default_branch_output = Command::new("git")
        .current_dir(repo.path())
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .output();

    let expected_branch = if let Ok(output) = default_branch_output {
        let symbolic_ref = String::from_utf8_lossy(&output.stdout);
        symbolic_ref
            .trim()
            .strip_prefix("refs/remotes/origin/")
            .unwrap_or("master")
            .to_string()
    } else {
        // Check which branch exists
        let branches_output = Command::new("git")
            .current_dir(repo.path())
            .args(["branch", "--list", "master", "main"])
            .output()?;
        let branches = String::from_utf8_lossy(&branches_output.stdout);
        if branches.contains("master") {
            "master".to_string()
        } else {
            "main".to_string()
        }
    };

    assert_eq!(merge_target, expected_branch);

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}
