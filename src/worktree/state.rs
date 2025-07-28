use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorktreeState {
    pub session_id: String,
    pub worktree_name: String,
    pub branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: WorktreeStatus,
    pub focus: Option<String>,
    pub iterations: IterationInfo,
    pub stats: WorktreeStats,
    pub merged: bool,
    pub merged_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    InProgress,
    Completed,
    Failed,
    Abandoned,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IterationInfo {
    pub completed: u32,
    pub max: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WorktreeStats {
    pub files_changed: u32,
    pub commits: u32,
    pub last_commit_sha: Option<String>,
}
