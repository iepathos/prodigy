use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// State information for a worktree session
///
/// Tracks the progress and status of improvements in a git worktree
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
    pub merge_prompt_shown: bool,
    pub merge_prompt_response: Option<String>,
}

/// Status of a worktree session
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    InProgress,
    Completed,
    Merged,
    Failed,
    Abandoned,
}

/// Information about iterations completed in a worktree session
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IterationInfo {
    pub completed: u32,
    pub max: u32,
}

/// Statistics about a worktree session
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WorktreeStats {
    pub files_changed: u32,
    pub commits: u32,
    pub last_commit_sha: Option<String>,
}
