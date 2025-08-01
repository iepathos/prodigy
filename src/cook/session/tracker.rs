//! Session tracking implementation

use super::{SessionManager, SessionState, SessionStatus, SessionSummary, SessionUpdate};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Mutex;
use tokio::fs;

/// Default implementation of session tracking
pub struct SessionTrackerImpl {
    state: Mutex<SessionState>,
}

impl SessionTrackerImpl {
    /// Create a new session tracker
    pub fn new(session_id: String, working_directory: std::path::PathBuf) -> Self {
        Self {
            state: Mutex::new(SessionState::new(session_id, working_directory)),
        }
    }

    /// Set worktree name
    pub fn set_worktree(&self, name: String) {
        self.state.lock().unwrap().worktree_name = Some(name);
    }

    /// Set focus area
    pub fn set_focus(&self, focus: String) {
        self.state.lock().unwrap().focus = Some(focus);
    }
}

#[async_trait]
impl SessionManager for SessionTrackerImpl {
    async fn start_session(&self, session_id: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.session_id = session_id.to_string();
        state.status = SessionStatus::InProgress;
        Ok(())
    }

    async fn update_session(&self, update: SessionUpdate) -> Result<()> {
        match update {
            SessionUpdate::IncrementIteration => {
                self.state.lock().unwrap().increment_iteration();
            }
            SessionUpdate::AddFilesChanged(count) => {
                self.state.lock().unwrap().add_files_changed(count);
            }
            SessionUpdate::UpdateStatus(status) => {
                self.state.lock().unwrap().status = status;
            }
            SessionUpdate::AddError(error) => {
                self.state.lock().unwrap().errors.push(error);
            }
        }
        Ok(())
    }

    async fn complete_session(&self) -> Result<SessionSummary> {
        let mut state = self.state.lock().unwrap();
        state.complete();
        Ok(SessionSummary {
            iterations: state.iterations_completed,
            files_changed: state.files_changed,
        })
    }

    fn get_state(&self) -> SessionState {
        self.state.lock().unwrap().clone()
    }

    async fn save_state(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(&*self.state.lock().unwrap())?;
        fs::write(path, json).await?;
        Ok(())
    }

    async fn load_state(&self, path: &Path) -> Result<()> {
        let json = fs::read_to_string(path).await?;
        *self.state.lock().unwrap() = serde_json::from_str(&json)?;
        Ok(())
    }
}

/// Trait for session tracking operations
#[async_trait]
pub trait SessionTracker: Send + Sync {
    /// Track iteration progress
    async fn track_iteration(&mut self, iteration: usize, files_changed: usize) -> Result<()>;

    /// Track command execution
    async fn track_command(&mut self, command: &str, success: bool) -> Result<()>;

    /// Get progress report
    fn get_progress(&self) -> String;
}

#[async_trait]
impl SessionTracker for SessionTrackerImpl {
    async fn track_iteration(&mut self, _iteration: usize, files_changed: usize) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.increment_iteration();
        state.add_files_changed(files_changed);
        Ok(())
    }

    async fn track_command(&mut self, command: &str, success: bool) -> Result<()> {
        if !success {
            self.state
                .lock()
                .unwrap()
                .errors
                .push(format!("Command failed: {command}"));
        }
        Ok(())
    }

    fn get_progress(&self) -> String {
        let state = self.state.lock().unwrap();
        format!(
            "Session {} - Iterations: {}, Files changed: {}",
            state.session_id, state.iterations_completed, state.files_changed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_session_tracker_basic() {
        let mut tracker =
            SessionTrackerImpl::new("test-session".to_string(), PathBuf::from("/tmp"));

        // Test initial state
        let state = tracker.get_state();
        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.status, SessionStatus::InProgress);
        assert_eq!(state.iterations_completed, 0);
        assert_eq!(state.files_changed, 0);

        // Test iteration tracking
        tracker.track_iteration(1, 5).await.unwrap();
        let state = tracker.get_state();
        assert_eq!(state.iterations_completed, 1);
        assert_eq!(state.files_changed, 5);

        // Test session completion
        let summary = tracker.complete_session().await.unwrap();
        assert_eq!(summary.iterations, 1);
        assert_eq!(summary.files_changed, 5);
        let state = tracker.get_state();
        assert_eq!(state.status, SessionStatus::Completed);
    }

    #[tokio::test]
    async fn test_session_state_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("session.json");

        let mut tracker =
            SessionTrackerImpl::new("persist-test".to_string(), PathBuf::from("/tmp"));
        tracker.set_worktree("test-worktree".to_string());
        tracker.set_focus("performance".to_string());

        // Update state
        tracker.track_iteration(1, 3).await.unwrap();

        // Save state
        tracker.save_state(&state_path).await.unwrap();

        // Load into new tracker
        let new_tracker = SessionTrackerImpl::new("dummy".to_string(), PathBuf::from("/tmp"));
        new_tracker.load_state(&state_path).await.unwrap();

        // Verify loaded state
        let state = new_tracker.get_state();
        assert_eq!(state.session_id, "persist-test");
        assert_eq!(state.iterations_completed, 1);
        assert_eq!(state.files_changed, 3);
        assert_eq!(state.worktree_name, Some("test-worktree".to_string()));
        assert_eq!(state.focus, Some("performance".to_string()));
    }

    #[tokio::test]
    async fn test_session_updates() {
        let tracker = SessionTrackerImpl::new("update-test".to_string(), PathBuf::from("/tmp"));

        // Test various updates
        tracker
            .update_session(SessionUpdate::IncrementIteration)
            .await
            .unwrap();
        assert_eq!(tracker.get_state().iterations_completed, 1);

        tracker
            .update_session(SessionUpdate::AddFilesChanged(10))
            .await
            .unwrap();
        assert_eq!(tracker.get_state().files_changed, 10);

        tracker
            .update_session(SessionUpdate::AddError("Test error".to_string()))
            .await
            .unwrap();
        let state = tracker.get_state();
        assert_eq!(state.errors.len(), 1);
        assert_eq!(state.errors[0], "Test error");

        tracker
            .update_session(SessionUpdate::UpdateStatus(SessionStatus::Failed))
            .await
            .unwrap();
        assert_eq!(tracker.get_state().status, SessionStatus::Failed);
    }
}
