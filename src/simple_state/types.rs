//! Type definitions for simple state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Main state structure - simplified to essentials only
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct State {
    pub version: String,
    pub project_id: String,
    pub current_score: f32,
    pub last_run: Option<DateTime<Utc>>,
    pub total_runs: u32,
}

/// Session record for a single improvement run - simplified
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionRecord {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub initial_score: f32,
    pub final_score: Option<f32>,
    pub summary: String, // Simple description of what was done
}

/// Project analysis cache - kept for expensive operations
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectAnalysis {
    pub language: String,
    pub framework: Option<String>,
    pub health_score: f32,
    pub focus_areas: Vec<String>,
    pub analyzed_at: DateTime<Utc>,
}

impl State {
    pub fn new(project_id: String) -> Self {
        Self {
            version: "1.0".to_string(),
            project_id,
            current_score: 0.0,
            last_run: None,
            total_runs: 0,
        }
    }
}

impl SessionRecord {
    pub fn new(initial_score: f32) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            started_at: Utc::now(),
            completed_at: None,
            initial_score,
            final_score: None,
            summary: String::new(),
        }
    }

    pub fn complete(&mut self, final_score: f32, summary: String) {
        self.completed_at = Some(Utc::now());
        self.final_score = Some(final_score);
        self.summary = summary;
    }
}
