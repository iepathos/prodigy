//! Type definitions for simple state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Main state structure - simplified to essentials only
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct State {
    pub version: String,
    pub project_id: String,
    pub last_run: Option<DateTime<Utc>>,
    pub total_runs: u32,
}

/// Session record for a single improvement run - simplified
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionRecord {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub iterations: u32,
    pub files_changed: u32,
    pub summary: String, // Simple description of what was done
}

impl State {
    #[must_use]
    pub fn new(project_id: String) -> Self {
        Self {
            version: "1.0".to_string(),
            project_id,
            last_run: None,
            total_runs: 0,
        }
    }
}

impl SessionRecord {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            started_at: Utc::now(),
            completed_at: None,
            iterations: 0,
            files_changed: 0,
            summary: String::new(),
        }
    }

    pub fn complete(&mut self, iterations: u32, files_changed: u32, summary: String) {
        self.completed_at = Some(Utc::now());
        self.iterations = iterations;
        self.files_changed = files_changed;
        self.summary = summary;
    }
}

impl Default for SessionRecord {
    fn default() -> Self {
        Self::new()
    }
}
