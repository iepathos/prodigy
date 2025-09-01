//! Migration guide for transitioning to new session management
//!
//! This module demonstrates how to migrate from the old session tracking
//! to the new event-driven session management system.

use crate::session::{
    ExecutionMode, FileSessionStorage, InMemorySessionManager, LoggingObserver, SessionConfig,
    SessionEvent, SessionId, SessionManager as NewSessionManager, SessionOptions,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Example of creating a session manager with all features
pub async fn create_full_featured_manager(
    storage_path: PathBuf,
    verbose: bool,
) -> Arc<InMemorySessionManager> {
    // Create storage backend
    let storage = Arc::new(FileSessionStorage::new(storage_path));

    // Create session manager with persistence
    let manager = Arc::new(InMemorySessionManager::new(Some(storage)));

    // Add logging observer if verbose
    if verbose {
        let observer = Arc::new(LoggingObserver::new(verbose));
        manager.add_observer(observer).await;
    }

    manager
}

/// Example of migrating cook orchestrator to use new session management
pub async fn example_migration() -> anyhow::Result<()> {
    // Create session manager
    let storage_path = PathBuf::from(".prodigy/sessions");
    let manager = create_full_featured_manager(storage_path, true).await;

    // Create session configuration
    let config = SessionConfig {
        project_path: PathBuf::from("."),
        workflow: Default::default(),
        execution_mode: ExecutionMode::Direct,
        max_iterations: 10,
        options: SessionOptions::from_flags(
            false, // fail_fast
            false, // auto_accept
            true,  // metrics
            true,  // verbose
        ),
    };

    // Create and start session
    let session_id = manager.create_session(config).await?;
    manager.start_session(&session_id).await?;

    // Example workflow execution
    for iteration in 1..=3 {
        // Start iteration
        manager
            .record_event(
                &session_id,
                SessionEvent::IterationStarted { number: iteration },
            )
            .await?;

        // Execute commands
        manager
            .record_event(
                &session_id,
                SessionEvent::CommandExecuted {
                    command: "cargo fmt".to_string(),
                    success: true,
                },
            )
            .await?;

        // Complete iteration
        let changes = crate::session::IterationChanges {
            files_modified: vec![PathBuf::from("src/main.rs")],
            lines_added: 10,
            lines_removed: 5,
            commands_run: vec!["cargo fmt".to_string()],
            git_commits: vec![],
        };

        manager
            .record_event(&session_id, SessionEvent::IterationCompleted { changes })
            .await?;

        // Save checkpoint after each iteration
        manager.save_checkpoint(&session_id).await?;
    }

    // Complete session
    let summary = manager.complete_session(&session_id).await?;
    println!(
        "Session completed: {} iterations, {} files changed",
        summary.total_iterations, summary.files_changed
    );

    Ok(())
}

/// Example of recovering from an interrupted session
pub async fn example_recovery(
    storage_path: PathBuf,
    session_id: SessionId,
) -> anyhow::Result<()> {
    // Create manager with same storage
    let manager = create_full_featured_manager(storage_path, true).await;

    // Restore session
    manager.restore_session(&session_id).await?;

    // Check state and resume if possible
    let state = manager.get_state(&session_id).await?;
    match state {
        crate::session::SessionState::Paused { reason } => {
            println!("Resuming paused session: {}", reason);
            manager
                .record_event(&session_id, SessionEvent::Resumed)
                .await?;
        }
        crate::session::SessionState::Running { iteration } => {
            println!("Continuing from iteration {}", iteration);
        }
        _ => {
            println!("Session is not resumable");
            return Ok(());
        }
    }

    // Continue execution...
    Ok(())
}

/// Migration checklist for cook module
///
/// 1. Replace SessionTrackerImpl with SessionManagerAdapter for compatibility
/// 2. Use new SessionManager directly for new features
/// 3. Update event recording throughout the workflow
/// 4. Add session persistence at checkpoints
/// 5. Implement recovery logic for interrupted sessions
/// 6. Add observers for logging and monitoring
/// 7. Update tests to use new session management
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_migration_example() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("sessions");

        // This would normally run the full example
        let manager = create_full_featured_manager(storage_path, false).await;
        assert!(manager.list_active_sessions().await.unwrap().is_empty());
    }
}