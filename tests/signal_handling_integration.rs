use mmm::cook::signal_handler::setup_interrupt_handlers;
use mmm::subprocess::{MockProcessRunner, SubprocessManager};
use mmm::worktree::WorktreeManager;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_signal_handler_setup() {
    let temp_dir = TempDir::new().unwrap();
    let repo_dir = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_dir).unwrap();

    // Initialize a git repo
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    let process_runner = Arc::new(MockProcessRunner::new());
    let subprocess = SubprocessManager::new(process_runner);
    let worktree_manager = Arc::new(WorktreeManager::new(repo_dir, subprocess).unwrap());

    let result = setup_interrupt_handlers(worktree_manager, "test-session".to_string());
    assert!(result.is_ok());

    // Signal handlers are set up - actual signal testing requires process control
}
