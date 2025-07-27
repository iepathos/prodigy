//! State manager for JSON-based persistence

use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use super::types::{SessionRecord, State};

/// Manages the main state file - simplified to essentials
pub struct StateManager {
    root: PathBuf,
    state: State,
}

impl StateManager {
    /// Create a new state manager
    pub fn new() -> Result<Self> {
        let root = PathBuf::from(".mmm");

        // Create directory if needed
        fs::create_dir_all(&root).context("Failed to create .mmm directory")?;

        // Load or create state
        let state = Self::load_or_create(&root)?;

        Ok(Self { root, state })
    }

    /// Create state manager with custom root directory
    pub fn with_root(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root).context("Failed to create state directory")?;

        let state = Self::load_or_create(&root)?;

        Ok(Self { root, state })
    }

    /// Get current state
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Get mutable state
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    /// Save state to disk
    pub fn save(&self) -> Result<()> {
        // Save with atomic write - use unique temp file for concurrent access
        let temp_file = self.root.join(format!(
            "state.json.tmp.{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let final_file = self.root.join("state.json");

        // Write to temp file
        let json =
            serde_json::to_string_pretty(&self.state).context("Failed to serialize state")?;
        fs::write(&temp_file, json).context("Failed to write temp state file")?;

        // Atomic rename - last writer wins
        fs::rename(&temp_file, &final_file).map_err(|e| {
            // Clean up temp file on failure
            let _ = fs::remove_file(&temp_file);
            anyhow::anyhow!("Failed to rename state file: {}", e)
        })?;

        Ok(())
    }

    /// Record a completed session - simplified
    pub fn record_session(&mut self, session: SessionRecord) -> Result<()> {
        // Create history directory if needed
        let history_dir = self.root.join("history");
        fs::create_dir_all(&history_dir).context("Failed to create history directory")?;

        // Save session with timestamp in filename
        let timestamp = session.started_at.format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("{timestamp}_{}.json", &session.session_id[..8]);
        let session_file = history_dir.join(filename);

        let json = serde_json::to_string_pretty(&session).context("Failed to serialize session")?;
        fs::write(&session_file, json).context("Failed to write session file")?;

        // Update main state - essentials only
        if let Some(completed_at) = session.completed_at {
            self.state.last_run = Some(completed_at);
        }
        self.state.total_runs += 1;

        self.save()?;
        Ok(())
    }

    /// Load state from disk or create default
    fn load_or_create(root: &Path) -> Result<State> {
        let state_file = root.join("state.json");

        if state_file.exists() {
            // Try to load, with fallback
            match fs::read_to_string(&state_file) {
                Ok(contents) => {
                    match serde_json::from_str(&contents) {
                        Ok(state) => Ok(state),
                        Err(e) => {
                            // Backup corrupted file - handle concurrent access
                            let backup = root.join(format!(
                                "state.json.corrupted.{}",
                                Utc::now().timestamp_nanos_opt().unwrap_or(0)
                            ));
                            match fs::rename(&state_file, &backup) {
                                Ok(_) => {
                                    eprintln!("⚠️  State file corrupted, backed up to {backup:?}");
                                    eprintln!("   Error: {e}");
                                }
                                Err(rename_err) => {
                                    // File might already be renamed by another thread
                                    eprintln!(
                                        "⚠️  State file corrupted, backup failed: {rename_err}"
                                    );
                                    eprintln!("   Parse error: {e}");
                                }
                            }
                            Ok(State::new(uuid::Uuid::new_v4().to_string()))
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Cannot read state file: {e}");
                    Ok(State::new(uuid::Uuid::new_v4().to_string()))
                }
            }
        } else {
            Ok(State::new(uuid::Uuid::new_v4().to_string()))
        }
    }

    /// Get history - simplified
    pub fn get_history(&self) -> Result<Vec<SessionRecord>> {
        let history_dir = self.root.join("history");
        let mut sessions: Vec<SessionRecord> = Vec::new();

        if history_dir.exists() {
            for entry in fs::read_dir(&history_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                if let Some(name_str) = name.to_str() {
                    if name_str.ends_with(".json") {
                        if let Ok(contents) = fs::read_to_string(entry.path()) {
                            if let Ok(session) = serde_json::from_str(&contents) {
                                sessions.push(session);
                            }
                        }
                    }
                }
            }
        }

        // Sort by start time
        sessions.sort_by_key(|s| s.started_at);
        Ok(sessions)
    }
}
