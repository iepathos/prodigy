//! Effect-based session update operations
//!
//! This module provides Stillwater Effect abstractions for session I/O operations,
//! following the "pure core, imperative shell" pattern. Session updates are
//! encapsulated in Effects that compose pure transformations with I/O operations.
//!
//! # Architecture
//!
//! - **Pure logic** lives in `core::session::updates` (apply_session_update, apply_updates)
//! - **I/O effects** live here (load, save session operations)
//! - **Environment** provides storage via dependency injection
//!
//! # Example
//!
//! ```ignore
//! use prodigy::unified_session::effects::{update_session_effect, SessionEnv};
//! use prodigy::core::session::updates::SessionUpdate;
//!
//! let env = SessionEnv::new(storage);
//! let update = SessionUpdate::Status(SessionStatus::Running);
//!
//! let effect = update_session_effect(session_id, update);
//! let updated_session = effect.run(&env).await?;
//! ```

use crate::core::session::updates::{apply_session_update, apply_updates, SessionUpdate};
use crate::unified_session::{SessionId, UnifiedSession};
use async_trait::async_trait;
use std::sync::Arc;
use stillwater::Effect;

/// Error type for session effects
#[derive(Debug, Clone)]
pub enum SessionEffectError {
    /// Session not found
    NotFound { id: String },
    /// Failed to load session
    LoadFailed { message: String },
    /// Failed to save session
    SaveFailed { message: String },
    /// Invalid session update
    InvalidUpdate { message: String },
}

impl std::fmt::Display for SessionEffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionEffectError::NotFound { id } => {
                write!(f, "Session not found: {}", id)
            }
            SessionEffectError::LoadFailed { message } => {
                write!(f, "Failed to load session: {}", message)
            }
            SessionEffectError::SaveFailed { message } => {
                write!(f, "Failed to save session: {}", message)
            }
            SessionEffectError::InvalidUpdate { message } => {
                write!(f, "Invalid session update: {}", message)
            }
        }
    }
}

impl std::error::Error for SessionEffectError {}

/// Trait for async session storage operations
#[async_trait]
pub trait SessionStorage: Send + Sync {
    /// Load a session by ID
    async fn load_session(&self, id: &SessionId) -> anyhow::Result<UnifiedSession>;

    /// Save a session
    async fn save_session(&self, session: &UnifiedSession) -> anyhow::Result<()>;
}

/// Environment for session effect execution
///
/// Provides dependencies for session I/O operations.
#[derive(Clone)]
pub struct SessionEnv {
    /// Storage implementation for session persistence
    pub storage: Arc<dyn SessionStorage>,
}

impl SessionEnv {
    /// Create a new session environment
    pub fn new(storage: Arc<dyn SessionStorage>) -> Self {
        Self { storage }
    }
}

/// Effect: Update session with I/O
///
/// This effect composes I/O (load, save) with pure transformations
/// (apply_session_update) using the Effect pattern.
///
/// # Arguments
///
/// * `id` - Session ID to update
/// * `update` - The update to apply
///
/// # Returns
///
/// An Effect that, when run, will:
/// 1. I/O: Load the session from storage
/// 2. Pure: Apply the update transformation
/// 3. I/O: Save the updated session
/// 4. Return the updated session
///
/// # Example
///
/// ```ignore
/// let update = SessionUpdate::Status(SessionStatus::Running);
/// let effect = update_session_effect(session_id, update);
/// let session = effect.run(&env).await?;
/// ```
pub fn update_session_effect(
    id: SessionId,
    update: SessionUpdate,
) -> Effect<UnifiedSession, SessionEffectError, SessionEnv> {
    Effect::from_async(move |env: &SessionEnv| {
        let id = id.clone();
        let update = update.clone();
        let storage = env.storage.clone();

        async move {
            // I/O: Load session
            let session =
                storage
                    .load_session(&id)
                    .await
                    .map_err(|e| SessionEffectError::LoadFailed {
                        message: e.to_string(),
                    })?;

            // Pure: Apply update
            let updated = apply_session_update(session, update).map_err(|e| {
                SessionEffectError::InvalidUpdate {
                    message: e.to_string(),
                }
            })?;

            // I/O: Save session
            storage
                .save_session(&updated)
                .await
                .map_err(|e| SessionEffectError::SaveFailed {
                    message: e.to_string(),
                })?;

            Ok(updated)
        }
    })
}

/// Effect: Batch update session (multiple updates atomically)
///
/// Applies multiple updates to a session in sequence, loading once
/// and saving once for efficiency.
///
/// # Arguments
///
/// * `id` - Session ID to update
/// * `updates` - Vector of updates to apply in order
///
/// # Returns
///
/// An Effect that applies all updates and returns the final session state.
///
/// # Example
///
/// ```ignore
/// let updates = vec![
///     SessionUpdate::Status(SessionStatus::Running),
///     SessionUpdate::Progress(ProgressUpdate { completed_steps: 5, .. }),
/// ];
///
/// let effect = batch_update_session_effect(session_id, updates);
/// let session = effect.run(&env).await?;
/// ```
pub fn batch_update_session_effect(
    id: SessionId,
    updates: Vec<SessionUpdate>,
) -> Effect<UnifiedSession, SessionEffectError, SessionEnv> {
    Effect::from_async(move |env: &SessionEnv| {
        let id = id.clone();
        let updates = updates.clone();
        let storage = env.storage.clone();

        async move {
            // I/O: Load once
            let session =
                storage
                    .load_session(&id)
                    .await
                    .map_err(|e| SessionEffectError::LoadFailed {
                        message: e.to_string(),
                    })?;

            // Pure: Apply all updates
            let updated =
                apply_updates(session, updates).map_err(|e| SessionEffectError::InvalidUpdate {
                    message: e.to_string(),
                })?;

            // I/O: Save once
            storage
                .save_session(&updated)
                .await
                .map_err(|e| SessionEffectError::SaveFailed {
                    message: e.to_string(),
                })?;

            Ok(updated)
        }
    })
}

/// Effect: Load session without modification
///
/// Pure I/O effect that loads a session from storage.
pub fn load_session_effect(
    id: SessionId,
) -> Effect<UnifiedSession, SessionEffectError, SessionEnv> {
    Effect::from_async(move |env: &SessionEnv| {
        let id = id.clone();
        let storage = env.storage.clone();

        async move {
            storage
                .load_session(&id)
                .await
                .map_err(|e| SessionEffectError::LoadFailed {
                    message: e.to_string(),
                })
        }
    })
}

/// Effect: Save session to storage
///
/// Pure I/O effect that saves a session to storage.
pub fn save_session_effect(session: UnifiedSession) -> Effect<(), SessionEffectError, SessionEnv> {
    Effect::from_async(move |env: &SessionEnv| {
        let session = session.clone();
        let storage = env.storage.clone();

        async move {
            storage
                .save_session(&session)
                .await
                .map_err(|e| SessionEffectError::SaveFailed {
                    message: e.to_string(),
                })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::session::updates::ProgressUpdate;
    use crate::unified_session::SessionStatus;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockSessionStorage {
        sessions: Arc<Mutex<HashMap<String, UnifiedSession>>>,
    }

    impl MockSessionStorage {
        fn new() -> Self {
            Self {
                sessions: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn add_session(&self, session: UnifiedSession) {
            self.sessions
                .lock()
                .unwrap()
                .insert(session.id.as_str().to_string(), session);
        }

        fn get_session(&self, id: &str) -> Option<UnifiedSession> {
            self.sessions.lock().unwrap().get(id).cloned()
        }
    }

    #[async_trait]
    impl SessionStorage for MockSessionStorage {
        async fn load_session(&self, id: &SessionId) -> anyhow::Result<UnifiedSession> {
            self.sessions
                .lock()
                .unwrap()
                .get(id.as_str())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id.as_str()))
        }

        async fn save_session(&self, session: &UnifiedSession) -> anyhow::Result<()> {
            self.sessions
                .lock()
                .unwrap()
                .insert(session.id.as_str().to_string(), session.clone());
            Ok(())
        }
    }

    fn create_test_session() -> UnifiedSession {
        let mut session =
            UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
        if let Some(ref mut wd) = session.workflow_data {
            wd.total_steps = 10;
        }
        session
    }

    #[tokio::test]
    async fn test_update_session_effect_status() {
        let storage = Arc::new(MockSessionStorage::new());
        let session = create_test_session();
        let session_id = session.id.clone();
        storage.add_session(session);

        let env = SessionEnv::new(storage.clone());

        let effect = update_session_effect(
            session_id.clone(),
            SessionUpdate::Status(SessionStatus::Running),
        );
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Running);

        // Verify storage was updated
        let stored = storage.get_session(session_id.as_str()).unwrap();
        assert_eq!(stored.status, SessionStatus::Running);
    }

    #[tokio::test]
    async fn test_update_session_effect_progress() {
        let storage = Arc::new(MockSessionStorage::new());
        let mut session = create_test_session();
        session.status = SessionStatus::Running;
        let session_id = session.id.clone();
        storage.add_session(session);

        let env = SessionEnv::new(storage.clone());

        let progress = ProgressUpdate {
            completed_steps: 3,
            failed_steps: 0,
            current_step: Some("step-4".to_string()),
        };

        let effect = update_session_effect(session_id.clone(), SessionUpdate::Progress(progress));
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        let wd = updated.workflow_data.unwrap();
        assert_eq!(wd.completed_steps.len(), 3);
    }

    #[tokio::test]
    async fn test_update_session_effect_not_found() {
        let storage = Arc::new(MockSessionStorage::new());
        let env = SessionEnv::new(storage);

        let fake_id = SessionId::from_string("nonexistent".to_string());
        let effect = update_session_effect(fake_id, SessionUpdate::Status(SessionStatus::Running));
        let result = effect.run(&env).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SessionEffectError::LoadFailed { .. }));
    }

    #[tokio::test]
    async fn test_update_session_effect_invalid_transition() {
        let storage = Arc::new(MockSessionStorage::new());
        let session = create_test_session(); // Status is Initializing
        let session_id = session.id.clone();
        storage.add_session(session);

        let env = SessionEnv::new(storage);

        // Invalid: Can't go directly from Initializing to Completed
        let effect =
            update_session_effect(session_id, SessionUpdate::Status(SessionStatus::Completed));
        let result = effect.run(&env).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SessionEffectError::InvalidUpdate { .. }));
    }

    #[tokio::test]
    async fn test_batch_update_session_effect() {
        let storage = Arc::new(MockSessionStorage::new());
        let session = create_test_session();
        let session_id = session.id.clone();
        storage.add_session(session);

        let env = SessionEnv::new(storage.clone());

        let updates = vec![
            SessionUpdate::Status(SessionStatus::Running),
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 5,
                failed_steps: 0,
                current_step: Some("step-6".to_string()),
            }),
        ];

        let effect = batch_update_session_effect(session_id.clone(), updates);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SessionStatus::Running);

        let wd = updated.workflow_data.unwrap();
        assert_eq!(wd.completed_steps.len(), 5);
    }

    #[tokio::test]
    async fn test_batch_update_session_effect_empty_updates() {
        let storage = Arc::new(MockSessionStorage::new());
        let session = create_test_session();
        let session_id = session.id.clone();
        let _original_updated_at = session.updated_at;
        storage.add_session(session);

        let env = SessionEnv::new(storage);

        // Small delay to ensure timestamp would change
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let effect = batch_update_session_effect(session_id, vec![]);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        // Empty updates should still work (no-op)
    }

    #[tokio::test]
    async fn test_batch_update_session_effect_stops_on_error() {
        let storage = Arc::new(MockSessionStorage::new());
        let session = create_test_session();
        let session_id = session.id.clone();
        storage.add_session(session);

        let env = SessionEnv::new(storage);

        let updates = vec![
            SessionUpdate::Status(SessionStatus::Running),
            // This should fail - can't go from Running to Initializing
            SessionUpdate::Status(SessionStatus::Initializing),
            SessionUpdate::Progress(ProgressUpdate::default()),
        ];

        let effect = batch_update_session_effect(session_id, updates);
        let result = effect.run(&env).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_session_effect() {
        let storage = Arc::new(MockSessionStorage::new());
        let session = create_test_session();
        let session_id = session.id.clone();
        storage.add_session(session);

        let env = SessionEnv::new(storage);

        let effect = load_session_effect(session_id.clone());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert_eq!(loaded.id, session_id);
    }

    #[tokio::test]
    async fn test_load_session_effect_not_found() {
        let storage = Arc::new(MockSessionStorage::new());
        let env = SessionEnv::new(storage);

        let fake_id = SessionId::from_string("nonexistent".to_string());
        let effect = load_session_effect(fake_id);
        let result = effect.run(&env).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_save_session_effect() {
        let storage = Arc::new(MockSessionStorage::new());
        let env = SessionEnv::new(storage.clone());

        let session = create_test_session();
        let session_id = session.id.clone();

        let effect = save_session_effect(session);
        let result = effect.run(&env).await;

        assert!(result.is_ok());

        // Verify it was saved
        let stored = storage.get_session(session_id.as_str());
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_effect_composition() {
        let storage = Arc::new(MockSessionStorage::new());
        let session = create_test_session();
        let session_id = session.id.clone();
        storage.add_session(session);

        let env = SessionEnv::new(storage.clone());

        // Compose effects: update status, then update progress
        let effect1 = update_session_effect(
            session_id.clone(),
            SessionUpdate::Status(SessionStatus::Running),
        );
        let result1 = effect1.run(&env).await;
        assert!(result1.is_ok());

        let effect2 = update_session_effect(
            session_id.clone(),
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 2,
                failed_steps: 0,
                current_step: None,
            }),
        );
        let result2 = effect2.run(&env).await;
        assert!(result2.is_ok());

        let final_session = result2.unwrap();
        assert_eq!(final_session.status, SessionStatus::Running);
        let wd = final_session.workflow_data.unwrap();
        assert_eq!(wd.completed_steps.len(), 2);
    }
}
