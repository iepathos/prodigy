//! Demonstration of using the new session management system
//!
//! This example shows how to use the refactored session management
//! with the cook module.

use mmm::cook::session::SessionManagerAdapter;
use mmm::session::{
    ExecutionMode, FileSessionStorage, InMemorySessionManager, LoggingObserver, SessionConfig,
    SessionEvent, SessionOptions,
};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Session Management Refactor Demo\n");

    // Option 1: Use the adapter for backward compatibility
    demo_adapter_usage().await?;

    // Option 2: Use the new session management directly
    demo_new_session_management().await?;

    Ok(())
}

/// Demonstrate using the adapter for backward compatibility
async fn demo_adapter_usage() -> anyhow::Result<()> {
    println!("=== Using SessionManagerAdapter ===\n");

    // Create adapter (drop-in replacement for SessionTrackerImpl)
    let session_manager = Arc::new(SessionManagerAdapter::new(PathBuf::from(".")));

    // Use it exactly like the old SessionTrackerImpl
    session_manager.start_session("demo-session").await?;
    
    // Update session
    session_manager
        .update_session(mmm::cook::session::SessionUpdate::IncrementIteration)
        .await?;
    
    session_manager
        .update_session(mmm::cook::session::SessionUpdate::AddFilesChanged(3))
        .await?;

    // Get state (synchronous like before)
    let state = session_manager.get_state();
    println!("Old-style state: {:?}\n", state);

    // Complete session
    let summary = session_manager.complete_session().await?;
    println!("Session summary: {} iterations, {} files changed\n", 
        summary.iterations, summary.files_changed);

    Ok(())
}

/// Demonstrate using the new session management directly
async fn demo_new_session_management() -> anyhow::Result<()> {
    println!("=== Using New Session Management ===\n");

    // Create storage backend
    let storage = Arc::new(FileSessionStorage::new(PathBuf::from(".mmm/sessions")));

    // Create session manager with persistence
    let manager = Arc::new(InMemorySessionManager::new(Some(storage)));

    // Add logging observer
    let observer = Arc::new(LoggingObserver::new(true));
    manager.add_observer(observer).await;

    // Create session configuration
    let config = SessionConfig {
        project_path: PathBuf::from("."),
        workflow: Default::default(),
        execution_mode: ExecutionMode::Worktree {
            name: "demo-worktree".to_string(),
        },
        max_iterations: 5,
        focus: Some("performance".to_string()),
        options: SessionOptions {
            fail_fast: false,
            auto_merge: true,
            collect_metrics: true,
            verbose: true,
        },
    };

    // Create and start session
    let session_id = manager.create_session(config).await?;
    println!("Created session: {}\n", session_id);

    manager.start_session(&session_id).await?;

    // Simulate workflow execution
    for iteration in 1..=3 {
        // Start iteration
        manager
            .record_event(
                &session_id,
                SessionEvent::IterationStarted { number: iteration },
            )
            .await?;

        // Simulate command execution
        manager
            .record_event(
                &session_id,
                SessionEvent::CommandExecuted {
                    command: format!("cargo fmt --check (iteration {})", iteration),
                    success: true,
                },
            )
            .await?;

        // Complete iteration with changes
        let changes = mmm::session::IterationChanges {
            files_modified: vec![
                PathBuf::from(format!("src/file{}.rs", iteration)),
                PathBuf::from(format!("tests/test{}.rs", iteration)),
            ],
            lines_added: 20 * iteration as usize,
            lines_removed: 10 * iteration as usize,
            commands_run: vec![
                "cargo fmt".to_string(),
                "cargo clippy".to_string(),
                "cargo test".to_string(),
            ],
            git_commits: vec![mmm::session::CommitInfo {
                sha: format!("abc{:03}", iteration),
                message: format!("Iteration {} improvements", iteration),
                timestamp: chrono::Utc::now(),
            }],
        };

        manager
            .record_event(&session_id, SessionEvent::IterationCompleted { changes })
            .await?;

        // Save checkpoint
        manager.save_checkpoint(&session_id).await?;
        println!("Checkpoint saved for iteration {}\n", iteration);

        // Get progress
        let progress = manager.get_progress(&session_id).await?;
        println!(
            "Progress: {:.1}% complete, {} files changed, {} commands executed\n",
            progress.completion_percentage(),
            progress.files_changed.len(),
            progress.commands_executed.len()
        );
    }

    // Complete session
    let summary = manager.complete_session(&session_id).await?;
    println!("\nSession completed!");
    println!("  Total iterations: {}", summary.total_iterations);
    println!("  Files changed: {}", summary.files_changed);
    println!("  Total commits: {}", summary.total_commits);
    println!("  Duration: {:?}", summary.duration);
    println!("  Success rate: {:.1}%", summary.success_rate * 100.0);

    // List sessions
    let active_sessions = manager.list_active_sessions().await?;
    println!("\nActive sessions: {}", active_sessions.len());

    Ok(())
}

/// Demonstrate session recovery
#[allow(dead_code)]
async fn demo_session_recovery(session_id: mmm::session::SessionId) -> anyhow::Result<()> {
    println!("=== Session Recovery Demo ===\n");

    // Create manager with same storage
    let storage = Arc::new(FileSessionStorage::new(PathBuf::from(".mmm/sessions")));
    let manager = Arc::new(InMemorySessionManager::new(Some(storage)));

    // Restore session
    manager.restore_session(&session_id).await?;
    println!("Session restored: {}", session_id);

    // Check state
    let state = manager.get_state(&session_id).await?;
    let progress = manager.get_progress(&session_id).await?;

    println!("State: {:?}", state);
    println!(
        "Progress: {} iterations completed, {} files changed",
        progress.iterations_completed,
        progress.files_changed.len()
    );

    // Resume if possible
    match state {
        mmm::session::SessionState::Paused { reason } => {
            println!("Resuming from pause: {}", reason);
            manager
                .record_event(&session_id, SessionEvent::Resumed)
                .await?;
        }
        mmm::session::SessionState::Running { iteration } => {
            println!("Continuing from iteration {}", iteration);
        }
        _ => {
            println!("Session is not resumable");
        }
    }

    Ok(())
}