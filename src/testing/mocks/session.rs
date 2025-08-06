//! Mock session manager for testing

use crate::cook::session::{
    SessionManager, SessionState, SessionStatus, SessionSummary, SessionUpdate,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Mock session manager for testing
pub struct MockSessionManager {
    state: Arc<Mutex<SessionState>>,
    pub start_called: Arc<Mutex<bool>>,
    pub update_calls: Arc<Mutex<Vec<SessionUpdate>>>,
    pub complete_called: Arc<Mutex<bool>>,
    pub save_path: Arc<Mutex<Option<PathBuf>>>,
    pub load_path: Arc<Mutex<Option<PathBuf>>>,
    pub should_fail: bool,
}

impl MockSessionManager {
    /// Create new mock session manager
    pub fn new() -> Self {
        let state = SessionState::new("test-session".to_string(), PathBuf::from("/test"));
        Self {
            state: Arc::new(Mutex::new(state)),
            start_called: Arc::new(Mutex::new(false)),
            update_calls: Arc::new(Mutex::new(Vec::new())),
            complete_called: Arc::new(Mutex::new(false)),
            save_path: Arc::new(Mutex::new(None)),
            load_path: Arc::new(Mutex::new(None)),
            should_fail: false,
        }
    }

    /// Create a failing mock
    pub fn failing() -> Self {
        let mut mock = Self::new();
        mock.should_fail = true;
        mock
    }

    /// Get update calls for verification
    pub fn get_update_calls(&self) -> Vec<SessionUpdate> {
        self.update_calls.lock().unwrap().clone()
    }

    /// Check if start was called
    pub fn was_start_called(&self) -> bool {
        *self.start_called.lock().unwrap()
    }

    /// Check if complete was called
    pub fn was_complete_called(&self) -> bool {
        *self.complete_called.lock().unwrap()
    }

    /// Set the internal state
    pub fn set_state(&self, state: SessionState) {
        *self.state.lock().unwrap() = state;
    }
}

#[async_trait]
impl SessionManager for MockSessionManager {
    async fn start_session(&self, session_id: &str) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        *self.start_called.lock().unwrap() = true;
        self.state.lock().unwrap().session_id = session_id.to_string();
        Ok(())
    }

    async fn update_session(&self, update: SessionUpdate) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }

        self.update_calls.lock().unwrap().push(update.clone());

        let mut state = self.state.lock().unwrap();
        match update {
            SessionUpdate::IncrementIteration => {
                state.increment_iteration();
            }
            SessionUpdate::AddFilesChanged(count) => {
                state.add_files_changed(count);
            }
            SessionUpdate::UpdateStatus(status) => {
                state.status = status;
            }
            SessionUpdate::AddError(error) => {
                state.errors.push(error);
            }
            _ => {} // Other updates not relevant for this mock
        }
        Ok(())
    }

    async fn complete_session(&self) -> Result<SessionSummary> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        *self.complete_called.lock().unwrap() = true;
        self.state.lock().unwrap().complete();

        Ok(SessionSummary {
            iterations: self.state.lock().unwrap().iterations_completed,
            files_changed: self.state.lock().unwrap().files_changed,
        })
    }

    fn get_state(&self) -> SessionState {
        self.state.lock().unwrap().clone()
    }

    async fn save_state(&self, path: &Path) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        *self.save_path.lock().unwrap() = Some(path.to_path_buf());
        Ok(())
    }

    async fn load_state(&self, path: &Path) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        *self.load_path.lock().unwrap() = Some(path.to_path_buf());
        Ok(())
    }
}
