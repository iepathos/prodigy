//! Session manager implementation

use super::{
    SessionConfig, SessionEvent, SessionId, SessionInfo, SessionObserver, SessionProgress,
    SessionState, SessionStorage, SessionSummary, TimestampedEvent,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Trait for managing cook sessions
#[async_trait]
pub trait SessionManager: Send + Sync {
    /// Create a new session
    async fn create_session(&self, config: SessionConfig) -> Result<SessionId>;

    /// Start a session
    async fn start_session(&self, id: &SessionId) -> Result<()>;

    /// Record an event
    async fn record_event(&self, id: &SessionId, event: SessionEvent) -> Result<()>;

    /// Get current state
    async fn get_state(&self, id: &SessionId) -> Result<SessionState>;

    /// Get session progress
    async fn get_progress(&self, id: &SessionId) -> Result<SessionProgress>;

    /// Complete a session
    async fn complete_session(&self, id: &SessionId) -> Result<SessionSummary>;

    /// List active sessions
    async fn list_active_sessions(&self) -> Result<Vec<SessionInfo>>;

    /// Save checkpoint
    async fn save_checkpoint(&self, id: &SessionId) -> Result<()>;

    /// Restore session
    async fn restore_session(&self, id: &SessionId) -> Result<()>;
}

/// Session data
struct SessionData {
    config: SessionConfig,
    state: SessionState,
    events: Vec<TimestampedEvent>,
    progress: SessionProgress,
    started_at: Instant,
}

/// In-memory session manager implementation
pub struct InMemorySessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, SessionData>>>,
    storage: Option<Arc<dyn SessionStorage>>,
    observers: Arc<RwLock<Vec<Arc<dyn SessionObserver>>>>,
}

impl InMemorySessionManager {
    /// Create new session manager
    pub fn new(storage: Option<Arc<dyn SessionStorage>>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            storage,
            observers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add an observer
    pub async fn add_observer(&self, observer: Arc<dyn SessionObserver>) {
        self.observers.write().await.push(observer);
    }

    /// Notify observers of an event
    async fn notify_observers(&self, session_id: &SessionId, event: &SessionEvent) {
        let observers = self.observers.read().await;
        for observer in observers.iter() {
            observer.on_event(session_id, event).await;
        }
    }

    /// Apply event to session state
    async fn apply_event(&self, id: &SessionId, event: &SessionEvent) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let data = sessions
            .get_mut(id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        // Update state based on event
        match event {
            SessionEvent::Started { .. } => {
                data.state = SessionState::Running { iteration: 0 };
            }
            SessionEvent::IterationStarted { number } => {
                data.state = SessionState::Running { iteration: *number };
                data.progress.current_phase = Some(format!("Iteration {number}"));
            }
            SessionEvent::IterationCompleted { changes } => {
                data.progress.iterations_completed += 1;
                data.progress.iteration_changes.push(changes.clone());
                for file in &changes.files_modified {
                    data.progress.files_changed.insert(file.clone());
                }
            }
            SessionEvent::AnalysisCompleted { results: _ } => {
                data.progress.current_phase = Some("Analysis complete".to_string());
            }
            SessionEvent::CommandExecuted { command, success } => {
                data.progress
                    .commands_executed
                    .push(super::ExecutedCommand {
                        command: command.clone(),
                        success: *success,
                        duration: std::time::Duration::from_secs(1), // Would be tracked properly
                        output_size: 0,
                    });
            }
            SessionEvent::Paused { reason } => {
                data.state = SessionState::Paused {
                    reason: reason.clone(),
                };
            }
            SessionEvent::Resumed => {
                if let SessionState::Paused { .. } = &data.state {
                    data.state = SessionState::Running {
                        iteration: data.progress.iterations_completed,
                    };
                }
            }
            SessionEvent::Completed => {
                let summary = SessionSummary {
                    total_iterations: data.progress.iterations_completed,
                    files_changed: data.progress.files_changed.len(),
                    total_commits: data.progress.all_commits().len(),
                    duration: data.started_at.elapsed(),
                    success_rate: data.progress.success_rate(),
                };
                data.state = SessionState::Completed { summary };
            }
            SessionEvent::Failed { error } => {
                data.state = SessionState::Failed {
                    error: error.clone(),
                };
            }
        }

        // Record event
        data.events.push(TimestampedEvent::new(event.clone()));

        // Update progress duration
        data.progress.duration = data.started_at.elapsed();
        data.progress.state = data.state.clone();

        Ok(())
    }
}

#[async_trait]
impl SessionManager for InMemorySessionManager {
    async fn create_session(&self, config: SessionConfig) -> Result<SessionId> {
        let id = SessionId::new();
        let data = SessionData {
            state: SessionState::Created,
            progress: SessionProgress::new(config.max_iterations),
            config,
            events: Vec::new(),
            started_at: Instant::now(),
        };

        self.sessions.write().await.insert(id.clone(), data);
        Ok(id)
    }

    async fn start_session(&self, id: &SessionId) -> Result<()> {
        let sessions = self.sessions.read().await;
        let data = sessions
            .get(id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;
        let config = data.config.clone();
        drop(sessions);

        let event = SessionEvent::Started { config };
        self.record_event(id, event).await
    }

    async fn record_event(&self, id: &SessionId, event: SessionEvent) -> Result<()> {
        // Apply event
        self.apply_event(id, &event).await?;

        // Notify observers
        self.notify_observers(id, &event).await;

        // Save to storage if available
        if let Some(storage) = &self.storage {
            // Would save to persistent storage here
            let _ = storage;
        }

        Ok(())
    }

    async fn get_state(&self, id: &SessionId) -> Result<SessionState> {
        let sessions = self.sessions.read().await;
        let data = sessions
            .get(id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;
        Ok(data.state.clone())
    }

    async fn get_progress(&self, id: &SessionId) -> Result<SessionProgress> {
        let sessions = self.sessions.read().await;
        let data = sessions
            .get(id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;
        Ok(data.progress.clone())
    }

    async fn complete_session(&self, id: &SessionId) -> Result<SessionSummary> {
        self.record_event(id, SessionEvent::Completed).await?;

        let sessions = self.sessions.read().await;
        let data = sessions
            .get(id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        match &data.state {
            SessionState::Completed { summary } => Ok(summary.clone()),
            _ => Err(anyhow!("Session not in completed state")),
        }
    }

    async fn list_active_sessions(&self) -> Result<Vec<SessionInfo>> {
        let sessions = self.sessions.read().await;
        let mut active = Vec::new();

        for (id, data) in sessions.iter() {
            if !data.state.is_terminal() {
                active.push(SessionInfo {
                    id: id.clone(),
                    state: data.state.clone(),
                    config: data.config.clone(),
                    progress: data.progress.clone(),
                });
            }
        }

        Ok(active)
    }

    async fn save_checkpoint(&self, id: &SessionId) -> Result<()> {
        if let Some(storage) = &self.storage {
            let sessions = self.sessions.read().await;
            let data = sessions
                .get(id)
                .ok_or_else(|| anyhow!("Session not found: {}", id))?;

            // Create persisted session
            let persisted = super::PersistedSession {
                id: id.clone(),
                config: data.config.clone(),
                state: data.state.clone(),
                events: data.events.clone(),
                checkpoints: vec![super::SessionCheckpoint {
                    iteration: data.progress.iterations_completed,
                    timestamp: chrono::Utc::now(),
                    state_snapshot: super::StateSnapshot {
                        files_changed: data.progress.files_changed.clone(),
                        commands_executed: data.progress.commands_executed.len(),
                        current_phase: data.progress.current_phase.clone(),
                    },
                    resumable: !data.state.is_terminal(),
                }],
            };

            storage.save(&persisted).await?;
        }

        Ok(())
    }

    async fn restore_session(&self, id: &SessionId) -> Result<()> {
        if let Some(storage) = &self.storage {
            if let Some(persisted) = storage.load(id).await? {
                // Reconstruct session data
                let mut progress = SessionProgress::new(persisted.config.max_iterations);

                // Replay events to rebuild state
                for timestamped_event in &persisted.events {
                    match &timestamped_event.event {
                        SessionEvent::IterationCompleted { changes } => {
                            progress.iterations_completed += 1;
                            progress.iteration_changes.push(changes.clone());
                            for file in &changes.files_modified {
                                progress.files_changed.insert(file.clone());
                            }
                        }
                        SessionEvent::CommandExecuted { command, success } => {
                            progress.commands_executed.push(super::ExecutedCommand {
                                command: command.clone(),
                                success: *success,
                                duration: std::time::Duration::from_secs(1),
                                output_size: 0,
                            });
                        }
                        _ => {}
                    }
                }

                let data = SessionData {
                    config: persisted.config,
                    state: persisted.state,
                    events: persisted.events,
                    progress,
                    started_at: Instant::now(),
                };

                self.sessions.write().await.insert(id.clone(), data);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::session::IterationChanges;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_session_lifecycle() {
        let manager = InMemorySessionManager::new(None);

        // Create session
        let config = SessionConfig {
            project_path: PathBuf::from("/test"),
            workflow: crate::config::workflow::WorkflowConfig { commands: vec![] },
            execution_mode: crate::session::ExecutionMode::Direct,
            max_iterations: 5,
            focus: None,
            options: Default::default(),
        };

        let id = manager.create_session(config).await.unwrap();

        // Start session
        manager.start_session(&id).await.unwrap();
        let state = manager.get_state(&id).await.unwrap();
        assert!(matches!(state, SessionState::Running { iteration: 0 }));

        // Record iteration
        manager
            .record_event(&id, SessionEvent::IterationStarted { number: 1 })
            .await
            .unwrap();

        let changes = IterationChanges {
            files_modified: vec![PathBuf::from("test.rs")],
            lines_added: 10,
            lines_removed: 5,
            commands_run: vec!["cargo fmt".to_string()],
            git_commits: vec![],
        };

        manager
            .record_event(&id, SessionEvent::IterationCompleted { changes })
            .await
            .unwrap();

        // Complete session
        let summary = manager.complete_session(&id).await.unwrap();
        assert_eq!(summary.total_iterations, 1);
        assert_eq!(summary.files_changed, 1);

        // Verify terminal state
        let state = manager.get_state(&id).await.unwrap();
        assert!(state.is_terminal());
    }

    #[tokio::test]
    async fn test_concurrent_sessions() {
        let manager = InMemorySessionManager::new(None);

        // Create multiple sessions
        let config = SessionConfig::default();
        let id1 = manager.create_session(config.clone()).await.unwrap();
        let id2 = manager.create_session(config).await.unwrap();

        // Start both
        manager.start_session(&id1).await.unwrap();
        manager.start_session(&id2).await.unwrap();

        // List active
        let active = manager.list_active_sessions().await.unwrap();
        assert_eq!(active.len(), 2);

        // Complete one
        manager.complete_session(&id1).await.unwrap();

        // List active again
        let active = manager.list_active_sessions().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id2);
    }
}
