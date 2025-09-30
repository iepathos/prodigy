use super::*;
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeState, WorktreeStatus};
use std::process::Command;
use tempfile::TempDir;

fn setup_test_repo() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize a git repository
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init", "--initial-branch=master"])
        .output()?;

    // Create an initial commit
    std::fs::write(temp_dir.path().join("README.md"), "# Test Repo\n")?;
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

fn cleanup_worktree_dir(manager: &WorktreeManager) {
    // Clean up the worktree directory in home
    if manager.base_dir.exists() {
        let _ = std::fs::remove_dir_all(&manager.base_dir);
    }
}

#[tokio::test]
async fn test_state_file_creation() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    // Check that .metadata directory was created
    let metadata_dir = manager.base_dir.join(".metadata");
    assert!(metadata_dir.exists());
    assert!(metadata_dir.is_dir());

    // Check that state file was created
    let state_file = metadata_dir.join(format!("{}.json", session.name));
    assert!(state_file.exists());

    // Read and verify state content
    let state_json = std::fs::read_to_string(&state_file)?;
    let state: WorktreeState = serde_json::from_str(&state_json)?;

    // Use the validation method
    assert!(
        state.validate_initial_state(&session.name, &session.branch),
        "State validation failed"
    );

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_state_updates() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    // Update state
    manager.update_session_state(&session.name, |state| {
        state.iterations.completed = 5;
        state.stats.files_changed = 10;
        state.stats.commits = 15;
        state.status = WorktreeStatus::Completed;
    })?;

    // Read state file and verify updates
    let state_file = manager
        .base_dir
        .join(".metadata")
        .join(format!("{}.json", session.name));
    let state_json = std::fs::read_to_string(&state_file)?;
    let state: WorktreeState = serde_json::from_str(&state_json)?;

    assert_eq!(state.iterations.completed, 5);
    assert_eq!(state.stats.files_changed, 10);
    assert_eq!(state.stats.commits, 15);
    assert!(matches!(state.status, WorktreeStatus::Completed));

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[test]
fn test_gitignore_creation() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Check that .gitignore was created
    let gitignore_path = manager.base_dir.join(".gitignore");
    assert!(gitignore_path.exists());

    // Verify content
    let gitignore_content = std::fs::read_to_string(&gitignore_path)?;
    assert!(gitignore_content.contains(".metadata/"));

    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_list_sessions_with_state() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create sessions with different states
    let session1 = manager.create_session().await?;
    let session2 = manager.create_session().await?;

    // Update first session to completed
    manager.update_session_state(&session1.name, |state| {
        state.status = WorktreeStatus::Completed;
        state.iterations.completed = 3;
    })?;

    // List sessions - the list_sessions method should load from state
    let sessions = manager.list_sessions().await?;
    assert_eq!(sessions.len(), 2);

    // Find sessions by name
    let _s1 = sessions.iter().find(|s| s.name == session1.name).unwrap();
    let _s2 = sessions.iter().find(|s| s.name == session2.name).unwrap();

    // Clean up
    manager.cleanup_session(&session1.name, false).await?;
    manager.cleanup_session(&session2.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_merge_updates_state() -> anyhow::Result<()> {
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

    // Note: We can't actually test the merge without Claude CLI
    // But we can test that state update would work
    manager.update_session_state(&session.name, |state| {
        state.merged = true;
        state.merged_at = Some(chrono::Utc::now());
    })?;

    // Verify state was updated
    let state_file = manager
        .base_dir
        .join(".metadata")
        .join(format!("{}.json", session.name));
    let state_json = std::fs::read_to_string(&state_file)?;
    let state: WorktreeState = serde_json::from_str(&state_json)?;

    assert!(state.merged);
    assert!(state.merged_at.is_some());

    // Clean up - use force=true since we made commits in the worktree
    manager.cleanup_session(&session.name, true).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[test]
fn test_state_error_handling() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Test updating non-existent session
    let result = manager.update_session_state("non-existent", |state| {
        state.status = WorktreeStatus::Failed;
    });
    assert!(result.is_err());

    cleanup_worktree_dir(&manager);
    Ok(())
}

#[test]
fn test_validate_initial_state_valid() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
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
        resumable: false,
    };

    assert!(state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_wrong_session_id() {
    let state = WorktreeState {
        session_id: "wrong-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
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
        resumable: false,
    };

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_wrong_status() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::Completed,
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
        resumable: false,
    };

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_non_zero_iterations() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
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
        resumable: false,
    };

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_already_merged() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
        iterations: IterationInfo {
            completed: 0,
            max: 10,
        },
        stats: WorktreeStats::default(),
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

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_has_error() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
        iterations: IterationInfo {
            completed: 0,
            max: 10,
        },
        stats: WorktreeStats::default(),
        merged: false,
        merged_at: None,
        error: Some("Error occurred".to_string()),
        merge_prompt_shown: false,
        merge_prompt_response: None,
        interrupted_at: None,
        interruption_type: None,
        last_checkpoint: None,
        resumable: false,
    };

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_wrong_branch() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "wrong-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
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
        resumable: false,
    };

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_wrong_worktree_name() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "wrong-worktree".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
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
        resumable: false,
    };

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}

#[test]
fn test_validate_initial_state_wrong_max_iterations() {
    let state = WorktreeState {
        session_id: "test-session".to_string(),
        worktree_name: "test-session".to_string(),
        branch: "test-branch".to_string(),
        original_branch: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: WorktreeStatus::InProgress,
        iterations: IterationInfo {
            completed: 0,
            max: 5, // Wrong max iterations (should be 10)
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
        resumable: false,
    };

    assert!(!state.validate_initial_state("test-session", "test-branch"));
}
