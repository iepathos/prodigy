//! Type definitions for simple state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main state structure
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct State {
    pub version: String,
    pub project_id: String,
    pub last_run: Option<DateTime<Utc>>,
    pub current_score: f32,
    pub sessions: SessionInfo,
    pub stats: Statistics,
}

/// Session information
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct SessionInfo {
    pub active: Option<String>,
    pub last_completed: Option<String>,
}

/// Project statistics
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Statistics {
    pub total_runs: u32,
    pub total_improvements: u32,
    pub average_improvement: f32,
    pub favorite_improvements: Vec<String>,
}

/// Session record for a single improvement run
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionRecord {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub initial_score: f32,
    pub final_score: f32,
    pub improvements: Vec<Improvement>,
    pub files_changed: Vec<String>,
    pub metrics: SessionMetrics,
}

/// Individual improvement made during a session
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Improvement {
    #[serde(rename = "type")]
    pub improvement_type: String,
    pub file: String,
    pub line: Option<u32>,
    pub description: String,
    pub impact: f32,
}

/// Metrics for a session
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionMetrics {
    pub duration_seconds: u64,
    pub claude_calls: u32,
    pub tokens_used: u32,
}

/// Learning data structure
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Learning {
    pub patterns: PatternStats,
    pub preferences: Preferences,
}

/// Pattern statistics
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct PatternStats {
    pub successful_improvements: HashMap<String, PatternInfo>,
    pub failed_patterns: HashMap<String, FailureInfo>,
}

/// Information about a successful pattern
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct PatternInfo {
    pub success_rate: f32,
    pub average_impact: f32,
    pub examples: Vec<String>,
    pub total_attempts: u32,
    pub successful: u32,
    pub impacts: Vec<f32>,
}

/// Information about a failed pattern
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FailureInfo {
    pub failure_rate: f32,
    pub avoid: bool,
}

/// User preferences
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Preferences {
    pub focus_areas: Vec<String>,
    pub skip_patterns: Vec<String>,
    pub style_hints: HashMap<String, String>,
}

/// Project analysis cache
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
            last_run: None,
            current_score: 0.0,
            sessions: SessionInfo::default(),
            stats: Statistics::default(),
        }
    }
}

impl SessionRecord {
    pub fn new(initial_score: f32) -> Self {
        let now = Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            started_at: now,
            completed_at: now,
            initial_score,
            final_score: initial_score,
            improvements: Vec::new(),
            files_changed: Vec::new(),
            metrics: SessionMetrics {
                duration_seconds: 0,
                claude_calls: 0,
                tokens_used: 0,
            },
        }
    }

    pub fn complete(&mut self, final_score: f32) {
        self.completed_at = Utc::now();
        self.final_score = final_score;
        self.metrics.duration_seconds = (self.completed_at - self.started_at).num_seconds() as u64;
    }
}
