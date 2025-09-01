use crate::worktree::{InterruptionType, WorktreeManager};
use anyhow::Result;
use chrono::Utc;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::sync::Arc;
use std::thread;

/// Set up interrupt handlers for graceful shutdown
///
/// This function installs signal handlers for SIGINT (Ctrl-C) and SIGTERM
/// that will update the worktree state to mark it as interrupted before exit.
pub fn setup_interrupt_handlers(
    worktree_manager: Arc<WorktreeManager>,
    session_name: String,
) -> Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM])?;

    thread::spawn(move || {
        #[allow(clippy::never_loop)]
        for sig in signals.forever() {
            match sig {
                SIGINT => {
                    update_interrupted_state(
                        &worktree_manager,
                        &session_name,
                        InterruptionType::UserInterrupt,
                    );
                    std::process::exit(130); // Standard exit code for SIGINT
                }
                SIGTERM => {
                    update_interrupted_state(
                        &worktree_manager,
                        &session_name,
                        InterruptionType::Termination,
                    );
                    std::process::exit(143); // Standard exit code for SIGTERM
                }
                _ => unreachable!(),
            }
        }
    });

    Ok(())
}

/// Update the worktree state to mark it as interrupted
fn update_interrupted_state(
    worktree_manager: &WorktreeManager,
    session_name: &str,
    interruption_type: InterruptionType,
) {
    let _ = worktree_manager.update_session_state(session_name, |state| {
        state.status = crate::worktree::WorktreeStatus::Interrupted;
        state.interrupted_at = Some(Utc::now());
        state.interruption_type = Some(interruption_type);
        state.resumable = true;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::SubprocessManager;
    use crate::worktree::WorktreeState;
    use std::fs;
    use tempfile::TempDir;

    pub(super) fn create_test_worktree_manager() -> (TempDir, WorktreeManager) {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir_all(&repo_dir).unwrap();

        // Initialize a git repo
        std::process::Command::new("git")
            .arg("init")
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        let (subprocess, _mock) = SubprocessManager::mock();
        let manager = WorktreeManager::new(repo_dir, subprocess).unwrap();

        (temp_dir, manager)
    }

    fn create_test_session_state(manager: &WorktreeManager, session_name: &str) -> Result<()> {
        let metadata_dir = manager.base_dir.join(".metadata");
        fs::create_dir_all(&metadata_dir)?;

        let state = WorktreeState {
            session_id: session_name.to_string(),
            worktree_name: session_name.to_string(),
            branch: format!("prodigy-{session_name}"),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: crate::worktree::WorktreeStatus::InProgress,
            iterations: crate::worktree::IterationInfo {
                completed: 0,
                max: 10,
            },
            stats: Default::default(),
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

        let state_file = metadata_dir.join(format!("{session_name}.json"));
        let json = serde_json::to_string_pretty(&state)?;
        fs::write(state_file, json)?;

        Ok(())
    }

    #[test]
    fn test_update_interrupted_state() {
        let (_temp_dir, worktree_manager) = create_test_worktree_manager();
        let session_name = "test-session";

        // Create initial state
        create_test_session_state(&worktree_manager, session_name).unwrap();

        // Test interruption state update
        update_interrupted_state(
            &worktree_manager,
            session_name,
            InterruptionType::UserInterrupt,
        );

        // Read back the state to verify
        let state_file = worktree_manager
            .base_dir
            .join(".metadata")
            .join(format!("{session_name}.json"));
        let state_json = fs::read_to_string(&state_file).unwrap();
        let state: WorktreeState = serde_json::from_str(&state_json).unwrap();

        assert_eq!(state.status, crate::worktree::WorktreeStatus::Interrupted);
        assert!(state.resumable);
        assert!(state.interrupted_at.is_some());
        assert_eq!(
            state.interruption_type,
            Some(InterruptionType::UserInterrupt)
        );
    }

    #[test]
    fn test_termination_interrupt() {
        let (_temp_dir, worktree_manager) = create_test_worktree_manager();
        let session_name = "test-session-term";

        // Create initial state
        create_test_session_state(&worktree_manager, session_name).unwrap();

        update_interrupted_state(
            &worktree_manager,
            session_name,
            InterruptionType::Termination,
        );

        // Read back the state to verify
        let state_file = worktree_manager
            .base_dir
            .join(".metadata")
            .join(format!("{session_name}.json"));
        let state_json = fs::read_to_string(&state_file).unwrap();
        let state: WorktreeState = serde_json::from_str(&state_json).unwrap();

        assert_eq!(state.interruption_type, Some(InterruptionType::Termination));
    }
}

#[cfg(test)]
mod signal_tests {
    use super::tests::create_test_worktree_manager;
    use super::*;

    #[test]
    fn test_setup_interrupt_handlers() {
        let (_temp_dir, worktree_manager) = create_test_worktree_manager();
        let arc_manager: std::sync::Arc<WorktreeManager> = std::sync::Arc::new(worktree_manager);
        let session_name = "test-signal-session".to_string();

        // Test that setup doesn't panic
        let result = setup_interrupt_handlers(arc_manager.clone(), session_name.clone());
        assert!(result.is_ok());

        // Allow time for thread to spawn
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    #[test]
    fn test_update_interrupted_state_error_handling() {
        let (_temp_dir, worktree_manager) = create_test_worktree_manager();
        let nonexistent_session = "nonexistent-session";

        // Should not panic even if session doesn't exist
        update_interrupted_state(
            &worktree_manager,
            nonexistent_session,
            InterruptionType::UserInterrupt,
        );
    }
}
