//! Session tracking implementation

use super::{
    SessionInfo, SessionManager, SessionState, SessionStatus, SessionSummary, SessionUpdate,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::fs;

/// Default implementation of session tracking
pub struct SessionTrackerImpl {
    state: Mutex<SessionState>,
    base_path: PathBuf,
}

impl SessionTrackerImpl {
    /// Create a new session tracker
    pub fn new(session_id: String, working_directory: std::path::PathBuf) -> Self {
        let base_path = working_directory.join(".prodigy");
        Self {
            state: Mutex::new(SessionState::new(session_id, working_directory)),
            base_path,
        }
    }

    /// Get the session state file path
    fn get_session_file_path(&self, session_id: &str) -> PathBuf {
        self.base_path.join(format!("{}.json", session_id))
    }

    /// Set worktree name
    pub fn set_worktree(&self, name: String) {
        self.state.lock().unwrap().worktree_name = Some(name);
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
            SessionUpdate::StartWorkflow => {
                // Start workflow timing
                if let Ok(mut state) = self.state.lock() {
                    state.workflow_started_at = Some(chrono::Utc::now());
                }
            }
            SessionUpdate::StartIteration(iteration_number) => {
                // Start iteration timing
                if let Ok(mut state) = self.state.lock() {
                    state.current_iteration_started_at = Some(chrono::Utc::now());
                    state.current_iteration_number = Some(iteration_number);
                }
            }
            SessionUpdate::CompleteIteration => {
                // Complete iteration timing
                if let Ok(mut state) = self.state.lock() {
                    if let Some(start_time) = state.current_iteration_started_at.take() {
                        if let Some(iteration_number) = state.current_iteration_number.take() {
                            let end_time = chrono::Utc::now();
                            let duration = end_time
                                .signed_duration_since(start_time)
                                .to_std()
                                .unwrap_or_default();
                            state.iteration_timings.push((iteration_number, duration));
                        }
                    }
                }
            }
            SessionUpdate::RecordCommandTiming(command, duration) => {
                // Record command timing
                if let Ok(mut state) = self.state.lock() {
                    state.command_timings.push((command, duration));
                }
            }
            SessionUpdate::UpdateWorkflowState(workflow_state) => {
                if let Ok(mut state) = self.state.lock() {
                    state.update_workflow_state(workflow_state);
                }
            }
            SessionUpdate::MarkInterrupted => {
                if let Ok(mut state) = self.state.lock() {
                    state.interrupt();
                }
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
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Use atomic write to prevent corruption
        let temp_path = path.with_extension(format!(
            "tmp.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));

        let json = serde_json::to_string_pretty(&*self.state.lock().unwrap())?;

        // Write to temp file first
        fs::write(&temp_path, json).await?;

        // Atomic rename
        fs::rename(&temp_path, path).await.inspect_err(|_| {
            // Clean up temp file on failure
            let _ = std::fs::remove_file(&temp_path);
        })?;

        Ok(())
    }

    async fn load_state(&self, path: &Path) -> Result<()> {
        let json = fs::read_to_string(path).await?;
        *self.state.lock().unwrap() = serde_json::from_str(&json)?;
        Ok(())
    }

    async fn load_session(&self, session_id: &str) -> Result<SessionState> {
        // Try multiple locations for the session state
        let locations = vec![
            // 1. Standard session_state.json in current .prodigy
            self.base_path.join("session_state.json"),
            // 2. Session-specific file in current .prodigy
            self.get_session_file_path(session_id),
            // 3. Worktree's .prodigy directory if it exists
            PathBuf::from(format!(
                "{}/.prodigy/worktrees/{}/{}/{}.prodigy/session_state.json",
                std::env::var("HOME").unwrap_or_else(|_| "~".to_string()),
                self.base_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                session_id,
                session_id
            )),
            // 4. Global worktree metadata directory
            PathBuf::from(format!(
                "{}/.prodigy/worktrees/{}/{}/.prodigy/session_state.json",
                std::env::var("HOME").unwrap_or_else(|_| "~".to_string()),
                self.base_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                session_id
            )),
        ];

        // Try each location
        for location in locations {
            if location.exists() {
                if let Ok(json) = fs::read_to_string(&location).await {
                    if let Ok(state) = serde_json::from_str::<SessionState>(&json) {
                        // Verify this is the right session
                        if state.session_id == session_id {
                            return Ok(state);
                        }
                    }
                }
            }
        }

        Err(anyhow!(
            "Session {} not found in any known location",
            session_id
        ))
    }

    async fn save_checkpoint(&self, state: &SessionState) -> Result<()> {
        // Create a copy with status set to Interrupted for checkpoint
        let mut checkpoint_state = state.clone();
        checkpoint_state.status = SessionStatus::Interrupted;

        // Update the internal state with Interrupted status
        *self.state.lock().unwrap() = checkpoint_state.clone();

        // Save to both standard location and session-specific file
        let session_file = self.base_path.join("session_state.json");
        self.save_state(&session_file).await?;

        // Also save a session-specific backup with Interrupted status
        let specific_file = self.get_session_file_path(&checkpoint_state.session_id);
        let json = serde_json::to_string_pretty(&checkpoint_state)?;

        // Ensure directory exists
        if let Some(parent) = specific_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&specific_file, json).await?;
        Ok(())
    }

    async fn list_resumable(&self) -> Result<Vec<SessionInfo>> {
        let mut sessions = Vec::new();

        // Check the main session file
        let session_file = self.base_path.join("session_state.json");
        if session_file.exists() {
            if let Ok(json) = fs::read_to_string(&session_file).await {
                if let Ok(state) = serde_json::from_str::<SessionState>(&json) {
                    if state.is_resumable() {
                        sessions.push(SessionInfo {
                            session_id: state.session_id.clone(),
                            status: state.status.clone(),
                            started_at: state.started_at,
                            workflow_path: state
                                .workflow_state
                                .as_ref()
                                .map(|ws| ws.workflow_path.clone())
                                .unwrap_or_default(),
                            progress: state.get_resume_info().unwrap_or_default(),
                        });
                    }
                }
            }
        }

        // Check for session-specific files (both old cook-* and new session-* formats)
        if let Ok(entries) = fs::read_dir(&self.base_path).await {
            let mut entries = entries;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    // Support both old cook-{timestamp} and new session-{uuid} formats
                    if (name_str.starts_with("cook-") || name_str.starts_with("session-"))
                        && name_str.ends_with(".json")
                        && name_str != "session_state.json"
                    {
                        if let Ok(json) = fs::read_to_string(&path).await {
                            if let Ok(state) = serde_json::from_str::<SessionState>(&json) {
                                if state.is_resumable()
                                    && !sessions.iter().any(|s| s.session_id == state.session_id)
                                {
                                    sessions.push(SessionInfo {
                                        session_id: state.session_id.clone(),
                                        status: state.status.clone(),
                                        started_at: state.started_at,
                                        workflow_path: state
                                            .workflow_state
                                            .as_ref()
                                            .map(|ws| ws.workflow_path.clone())
                                            .unwrap_or_default(),
                                        progress: state.get_resume_info().unwrap_or_default(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(sessions)
    }

    async fn get_last_interrupted(&self) -> Result<Option<String>> {
        let sessions = self.list_resumable().await?;

        // Find the most recent interrupted session
        let interrupted = sessions
            .into_iter()
            .filter(|s| s.status == SessionStatus::Interrupted)
            .max_by_key(|s| s.started_at);

        Ok(interrupted.map(|s| s.session_id))
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

    #[tokio::test]
    async fn test_atomic_save_prevents_corruption() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        // Create multiple trackers that will write concurrently
        let mut handles = vec![];

        for i in 0..10 {
            let path = state_path.clone();
            let handle = tokio::spawn(async move {
                let tracker =
                    SessionTrackerImpl::new(format!("concurrent-{i}"), PathBuf::from("/tmp"));

                // Update and save state multiple times
                for j in 0..5 {
                    tracker
                        .update_session(SessionUpdate::IncrementIteration)
                        .await
                        .unwrap();
                    tracker
                        .update_session(SessionUpdate::AddFilesChanged(j))
                        .await
                        .unwrap();

                    // Save state - should use atomic write
                    tracker.save_state(&path).await.unwrap();

                    // Small delay to increase chance of concurrent writes
                    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all concurrent saves to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify the final state file is valid JSON
        let final_content = tokio::fs::read_to_string(&state_path).await.unwrap();
        let parsed: Result<SessionState, _> = serde_json::from_str(&final_content);
        assert!(
            parsed.is_ok(),
            "State file should contain valid JSON after concurrent writes"
        );

        // Check no temp files are left behind
        let mut entries = tokio::fs::read_dir(temp_dir.path()).await.unwrap();
        let mut file_count = 0;
        while let Some(entry) = entries.next_entry().await.unwrap() {
            if entry.file_name().to_string_lossy().ends_with(".json") {
                file_count += 1;
            }
        }
        assert_eq!(
            file_count, 1,
            "Only one state.json file should exist, no temp files"
        );
    }
}
