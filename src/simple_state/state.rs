//! State manager for JSON-based persistence

use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use super::types::{SessionRecord, State};

/// Manages the main state file
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
        // Save with atomic write
        let temp_file = self.root.join("state.json.tmp");
        let final_file = self.root.join("state.json");

        // Write to temp file
        let json =
            serde_json::to_string_pretty(&self.state).context("Failed to serialize state")?;
        fs::write(&temp_file, json).context("Failed to write temp state file")?;

        // Atomic rename
        fs::rename(temp_file, final_file).context("Failed to rename state file")?;

        Ok(())
    }

    /// Record a completed session
    pub fn record_session(&mut self, session: SessionRecord) -> Result<()> {
        // Create date directory
        let date_str = Utc::now().format("%Y-%m-%d").to_string();
        let date_dir = self.root.join("history").join(&date_str);
        fs::create_dir_all(&date_dir).context("Failed to create history directory")?;

        // Find next session number
        let session_num = self.next_session_number(&date_dir)?;
        let filename = format!("{:03}-improve.json", session_num);

        // Save session
        let session_file = date_dir.join(filename);
        let json = serde_json::to_string_pretty(&session).context("Failed to serialize session")?;
        fs::write(&session_file, json).context("Failed to write session file")?;

        // Update main state
        self.state.last_run = Some(session.completed_at);
        self.state.current_score = session.final_score;
        self.state.sessions.last_completed = Some(session.session_id);
        self.state.stats.total_runs += 1;
        self.state.stats.total_improvements += session.improvements.len() as u32;

        // Update average improvement
        let total = self.state.stats.total_runs as f32;
        let prev_avg = self.state.stats.average_improvement;
        let improvement = session.final_score - session.initial_score;
        self.state.stats.average_improvement = (prev_avg * (total - 1.0) + improvement) / total;

        // Update favorite improvements
        for imp in &session.improvements {
            if !self
                .state
                .stats
                .favorite_improvements
                .contains(&imp.improvement_type)
            {
                self.state
                    .stats
                    .favorite_improvements
                    .push(imp.improvement_type.clone());
            }
        }

        self.save()?;

        // Also save a summary for the day
        self.update_daily_summary(&date_dir, &date_str)?;

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
                            // Backup corrupted file
                            let backup = root
                                .join(format!("state.json.corrupted.{}", Utc::now().timestamp()));
                            fs::rename(&state_file, &backup)?;
                            eprintln!("⚠️  State file corrupted, backed up to {:?}", backup);
                            eprintln!("   Error: {}", e);
                            Ok(State::new(uuid::Uuid::new_v4().to_string()))
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Cannot read state file: {}", e);
                    Ok(State::new(uuid::Uuid::new_v4().to_string()))
                }
            }
        } else {
            Ok(State::new(uuid::Uuid::new_v4().to_string()))
        }
    }

    /// Find the next session number for today
    fn next_session_number(&self, date_dir: &Path) -> Result<u32> {
        let mut max_num = 0;

        if date_dir.exists() {
            for entry in fs::read_dir(date_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                if let Some(name_str) = name.to_str() {
                    if name_str.ends_with("-improve.json") {
                        if let Ok(num) = name_str[..3].parse::<u32>() {
                            max_num = max_num.max(num);
                        }
                    }
                }
            }
        }

        Ok(max_num + 1)
    }

    /// Update daily summary
    fn update_daily_summary(&self, date_dir: &Path, date_str: &str) -> Result<()> {
        let summary_file = date_dir.join("summary.json");

        // Count sessions and improvements for the day
        let mut total_sessions = 0;
        let mut total_improvements = 0;
        let mut total_score_change = 0.0;

        for entry in fs::read_dir(date_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if let Some(name_str) = name.to_str() {
                if name_str.ends_with("-improve.json") && name_str != "summary.json" {
                    total_sessions += 1;

                    // Load session to count improvements
                    if let Ok(contents) = fs::read_to_string(entry.path()) {
                        if let Ok(session) = serde_json::from_str::<SessionRecord>(&contents) {
                            total_improvements += session.improvements.len();
                            total_score_change += session.final_score - session.initial_score;
                        }
                    }
                }
            }
        }

        let summary = serde_json::json!({
            "date": date_str,
            "total_sessions": total_sessions,
            "total_improvements": total_improvements,
            "average_score_change": if total_sessions > 0 {
                total_score_change / total_sessions as f32
            } else {
                0.0
            },
            "updated_at": Utc::now()
        });

        let json = serde_json::to_string_pretty(&summary)?;
        fs::write(summary_file, json)?;

        Ok(())
    }

    /// Get history for a specific date
    pub fn get_history(&self, date: Option<&str>) -> Result<Vec<SessionRecord>> {
        let history_dir = self.root.join("history");
        let mut sessions = Vec::new();

        if let Some(date_str) = date {
            // Get specific date
            let date_dir = history_dir.join(date_str);
            if date_dir.exists() {
                sessions.extend(self.load_sessions_from_dir(&date_dir)?);
            }
        } else {
            // Get all history
            if history_dir.exists() {
                for entry in fs::read_dir(&history_dir)? {
                    let entry = entry?;
                    if entry.path().is_dir() {
                        sessions.extend(self.load_sessions_from_dir(&entry.path())?);
                    }
                }
            }
        }

        // Sort by start time
        sessions.sort_by_key(|s| s.started_at);

        Ok(sessions)
    }

    /// Load sessions from a directory
    fn load_sessions_from_dir(&self, dir: &Path) -> Result<Vec<SessionRecord>> {
        let mut sessions = Vec::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if let Some(name_str) = name.to_str() {
                if name_str.ends_with("-improve.json") && name_str != "summary.json" {
                    if let Ok(contents) = fs::read_to_string(entry.path()) {
                        if let Ok(session) = serde_json::from_str(&contents) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }

        Ok(sessions)
    }
}
