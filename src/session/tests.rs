//! Comprehensive tests for session management

use super::*;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_session_state_machine() {
    let manager = InMemorySessionManager::new(None);
    let config = SessionConfig::default();

    // Create session - should start in Created state
    let id = manager.create_session(config).await.unwrap();
    let state = manager.get_state(&id).await.unwrap();
    assert_eq!(state, SessionState::Created);

    // Start session - should transition to Running
    manager.start_session(&id).await.unwrap();
    let state = manager.get_state(&id).await.unwrap();
    assert!(matches!(state, SessionState::Running { iteration: 0 }));

    // Pause session
    manager
        .record_event(
            &id,
            SessionEvent::Paused {
                reason: "User requested".to_string(),
            },
        )
        .await
        .unwrap();
    let state = manager.get_state(&id).await.unwrap();
    assert!(matches!(state, SessionState::Paused { .. }));

    // Resume session
    manager.record_event(&id, SessionEvent::Resumed).await.unwrap();
    let state = manager.get_state(&id).await.unwrap();
    assert!(matches!(state, SessionState::Running { .. }));

    // Complete session
    manager.complete_session(&id).await.unwrap();
    let state = manager.get_state(&id).await.unwrap();
    assert!(matches!(state, SessionState::Completed { .. }));
    assert!(state.is_terminal());
}

#[tokio::test]
async fn test_concurrent_sessions() {
    let manager = Arc::new(InMemorySessionManager::new(None));
    let num_sessions = 10;
    let mut handles = vec![];

    // Create multiple concurrent sessions
    for i in 0..num_sessions {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let config = SessionConfig {
                max_iterations: 5,
                focus: Some(format!("test-{}", i)),
                ..Default::default()
            };

            let id = manager_clone.create_session(config).await.unwrap();
            manager_clone.start_session(&id).await.unwrap();

            // Simulate some work
            for iter in 1..=3 {
                manager_clone
                    .record_event(&id, SessionEvent::IterationStarted { number: iter })
                    .await
                    .unwrap();

                let changes = IterationChanges {
                    files_modified: vec![std::path::PathBuf::from(format!("file{}.rs", i))],
                    ..Default::default()
                };

                manager_clone
                    .record_event(&id, SessionEvent::IterationCompleted { changes })
                    .await
                    .unwrap();
            }

            manager_clone.complete_session(&id).await.unwrap()
        });
        handles.push(handle);
    }

    // Wait for all sessions to complete
    for handle in handles {
        let summary = handle.await.unwrap();
        assert_eq!(summary.total_iterations, 3);
    }

    // Verify no active sessions remain
    let active = manager.list_active_sessions().await.unwrap();
    assert_eq!(active.len(), 0);
}

#[tokio::test]
async fn test_session_persistence_and_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(FileSessionStorage::new(temp_dir.path().to_path_buf()));

    // Create manager with storage
    let manager1 = InMemorySessionManager::new(Some(storage.clone()));

    // Create and run session
    let config = SessionConfig {
        max_iterations: 10,
        ..Default::default()
    };
    let id = manager1.create_session(config.clone()).await.unwrap();
    manager1.start_session(&id).await.unwrap();

    // Execute some iterations
    for i in 1..=5 {
        manager1
            .record_event(&id, SessionEvent::IterationStarted { number: i })
            .await
            .unwrap();

        let changes = IterationChanges {
            files_modified: vec![std::path::PathBuf::from(format!("file{}.rs", i))],
            lines_added: i as usize * 10,
            lines_removed: i as usize * 5,
            ..Default::default()
        };

        manager1
            .record_event(&id, SessionEvent::IterationCompleted { changes })
            .await
            .unwrap();
    }

    // Save checkpoint
    manager1.save_checkpoint(&id).await.unwrap();

    // Simulate interruption
    manager1
        .record_event(
            &id,
            SessionEvent::Paused {
                reason: "System shutdown".to_string(),
            },
        )
        .await
        .unwrap();
    manager1.save_checkpoint(&id).await.unwrap();

    // Create new manager and restore
    let manager2 = InMemorySessionManager::new(Some(storage));
    manager2.restore_session(&id).await.unwrap();

    // Verify restored state
    let state = manager2.get_state(&id).await.unwrap();
    assert!(matches!(state, SessionState::Paused { .. }));

    let progress = manager2.get_progress(&id).await.unwrap();
    assert_eq!(progress.iterations_completed, 5);
    assert_eq!(progress.files_changed.len(), 5);

    // Resume and complete
    manager2.record_event(&id, SessionEvent::Resumed).await.unwrap();
    manager2.complete_session(&id).await.unwrap();
}

#[tokio::test]
async fn test_event_observers() {
    // Create a test observer that counts events
    struct CountingObserver {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl SessionObserver for CountingObserver {
        async fn on_event(&self, _session_id: &SessionId, _event: &SessionEvent) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    let manager = InMemorySessionManager::new(None);
    let count = Arc::new(AtomicUsize::new(0));
    let observer = Arc::new(CountingObserver {
        count: count.clone(),
    });

    manager.add_observer(observer).await;

    // Create and run session
    let id = manager.create_session(SessionConfig::default()).await.unwrap();
    manager.start_session(&id).await.unwrap(); // 1 event
    manager
        .record_event(&id, SessionEvent::IterationStarted { number: 1 })
        .await
        .unwrap(); // 2 events
    manager.complete_session(&id).await.unwrap(); // 3 events

    // Verify observer was called
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_session_progress_tracking() {
    let manager = InMemorySessionManager::new(None);
    let config = SessionConfig {
        max_iterations: 10,
        ..Default::default()
    };

    let id = manager.create_session(config).await.unwrap();
    manager.start_session(&id).await.unwrap();

    // Execute iterations with different results
    for i in 1..=5 {
        manager
            .record_event(&id, SessionEvent::IterationStarted { number: i })
            .await
            .unwrap();

        // Mix of successful and failed commands
        for j in 1..=3 {
            manager
                .record_event(
                    &id,
                    SessionEvent::CommandExecuted {
                        command: format!("cmd-{}-{}", i, j),
                        success: j != 2, // Middle command fails
                    },
                )
                .await
                .unwrap();
        }

        let changes = IterationChanges {
            files_modified: vec![std::path::PathBuf::from(format!("file{}.rs", i))],
            lines_added: 20,
            lines_removed: 10,
            commands_run: vec!["fmt".to_string(), "clippy".to_string()],
            git_commits: vec![CommitInfo {
                sha: format!("abc{}", i),
                message: format!("Fix {}", i),
                timestamp: chrono::Utc::now(),
            }],
        };

        manager
            .record_event(&id, SessionEvent::IterationCompleted { changes })
            .await
            .unwrap();
    }

    // Check progress
    let progress = manager.get_progress(&id).await.unwrap();
    assert_eq!(progress.iterations_completed, 5);
    assert_eq!(progress.completion_percentage(), 50.0);
    assert_eq!(progress.files_changed.len(), 5);
    assert_eq!(progress.commands_executed.len(), 15);
    assert_eq!(progress.success_rate(), 2.0 / 3.0); // 10 success, 5 failed
    assert_eq!(progress.total_lines_changed(), 150); // 5 * (20 + 10)
    assert_eq!(progress.all_commits().len(), 5);
}

#[tokio::test]
async fn test_storage_operations() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FileSessionStorage::new(temp_dir.path().to_path_buf());

    // Create test session
    let session = PersistedSession {
        id: SessionId::from_string("test-123".to_string()),
        config: SessionConfig::default(),
        state: SessionState::Running { iteration: 3 },
        events: vec![
            TimestampedEvent::new(SessionEvent::Started {
                config: SessionConfig::default(),
            }),
            TimestampedEvent::new(SessionEvent::IterationStarted { number: 1 }),
        ],
        checkpoints: vec![SessionCheckpoint {
            iteration: 3,
            timestamp: chrono::Utc::now(),
            state_snapshot: StateSnapshot {
                files_changed: std::collections::HashSet::new(),
                commands_executed: 5,
                current_phase: Some("testing".to_string()),
            },
            resumable: true,
        }],
    };

    // Save
    storage.save(&session).await.unwrap();

    // List
    let ids = storage.list().await.unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], session.id);

    // Load
    let loaded = storage.load(&session.id).await.unwrap().unwrap();
    assert_eq!(loaded.id, session.id);
    assert_eq!(loaded.events.len(), 2);
    assert!(loaded.is_resumable());

    // Delete
    storage.delete(&session.id).await.unwrap();
    let ids = storage.list().await.unwrap();
    assert_eq!(ids.len(), 0);
}