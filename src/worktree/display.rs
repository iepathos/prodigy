use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{WorktreeState, WorktreeStatus};

/// Enhanced session information for display
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnhancedSessionInfo {
    /// Session identifier
    pub session_id: String,
    /// Current session status
    pub status: WorktreeStatus,
    /// Path to the workflow file being executed
    pub workflow_path: Option<PathBuf>,
    /// Arguments passed to the workflow
    pub workflow_args: Vec<String>,
    /// When the session was started
    pub started_at: DateTime<Utc>,
    /// Last time the session was active
    pub last_activity: DateTime<Utc>,
    /// Current step number (0-based)
    pub current_step: usize,
    /// Total number of steps in the workflow
    pub total_steps: Option<usize>,
    /// Error summary if failed
    pub error_summary: Option<String>,
    /// Git branch name for this session
    pub branch_name: String,
    /// Parent branch if applicable
    pub parent_branch: Option<String>,
    /// Path to the worktree
    pub worktree_path: PathBuf,
    /// Number of files changed
    pub files_changed: u32,
    /// Number of commits made
    pub commits: u32,
    /// For MapReduce jobs, items processed
    pub items_processed: Option<u32>,
    /// For MapReduce jobs, total items
    pub total_items: Option<u32>,
}

/// Summary of all worktree sessions
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WorktreeSummary {
    pub total: usize,
    pub in_progress: usize,
    pub interrupted: usize,
    pub failed: usize,
    pub completed: usize,
}

/// Detailed worktree list with enhanced information
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetailedWorktreeList {
    pub sessions: Vec<EnhancedSessionInfo>,
    pub summary: WorktreeSummary,
}

/// Formatting trait for session display
pub trait SessionDisplay {
    fn format_default(&self) -> String;
    fn format_verbose(&self) -> String;
    fn format_json(&self) -> serde_json::Value;
}

impl SessionDisplay for EnhancedSessionInfo {
    fn format_default(&self) -> String {
        let workflow_name = self
            .workflow_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let args = if self.workflow_args.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.workflow_args.join(" "))
        };

        let status_emoji = match self.status {
            WorktreeStatus::InProgress => "🔄",
            WorktreeStatus::Completed => "✅",
            WorktreeStatus::Merged => "🔀",
            WorktreeStatus::CleanedUp => "🧹",
            WorktreeStatus::Failed => "❌",
            WorktreeStatus::Abandoned => "⚠️",
            WorktreeStatus::Interrupted => "⏸️",
        };

        let progress = if let Some(total) = self.total_steps {
            format!("step {}/{}", self.current_step, total)
        } else if let (Some(processed), Some(total)) = (self.items_processed, self.total_items) {
            format!("processed {}/{} items", processed, total)
        } else {
            format!("step {}", self.current_step)
        };

        let time_info = format_time_relative(&self.started_at, &self.last_activity);

        let mut output = format!(
            "📂 {}{}\n  └─ {} [{}{}]\n     Status: {} {} ({}) • {}",
            workflow_name,
            args,
            self.session_id,
            self.branch_name,
            self.parent_branch
                .as_ref()
                .map(|p| format!(" → {}", p))
                .unwrap_or_default(),
            status_emoji,
            format_status(&self.status),
            progress,
            time_info
        );

        if let Some(error) = &self.error_summary {
            output.push_str(&format!("\n     Error: \"{}\"", error));
        }

        output
    }

    fn format_verbose(&self) -> String {
        let mut output = self.format_default();
        output.push_str(&format!(
            "\n     Files changed: {} • Commits: {}",
            self.files_changed, self.commits
        ));
        output.push_str(&format!(
            "\n     Worktree: {}",
            self.worktree_path.display()
        ));
        output
    }

    fn format_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}

impl SessionDisplay for DetailedWorktreeList {
    fn format_default(&self) -> String {
        if self.sessions.is_empty() {
            return "No active Prodigy worktrees found.".to_string();
        }

        let mut output = format!("Active Prodigy worktrees ({} total):\n", self.summary.total);

        for session in &self.sessions {
            output.push_str(&format!("\n{}\n", session.format_default()));
        }

        if self.summary.total > 0 {
            output.push_str(&format!(
                "\nSummary: {} in progress, {} interrupted, {} failed, {} completed",
                self.summary.in_progress,
                self.summary.interrupted,
                self.summary.failed,
                self.summary.completed
            ));
        }

        output
    }

    fn format_verbose(&self) -> String {
        if self.sessions.is_empty() {
            return "No active Prodigy worktrees found.".to_string();
        }

        let mut output = format!("Active Prodigy worktrees ({} total):\n", self.summary.total);

        for session in &self.sessions {
            output.push_str(&format!("\n{}\n", session.format_verbose()));
        }

        if self.summary.total > 0 {
            output.push_str(&format!(
                "\nSummary: {} in progress, {} interrupted, {} failed, {} completed",
                self.summary.in_progress,
                self.summary.interrupted,
                self.summary.failed,
                self.summary.completed
            ));
        }

        output
    }

    fn format_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}

/// Format status for display
fn format_status(status: &WorktreeStatus) -> &str {
    match status {
        WorktreeStatus::InProgress => "InProgress",
        WorktreeStatus::Completed => "Completed",
        WorktreeStatus::Merged => "Merged",
        WorktreeStatus::CleanedUp => "CleanedUp",
        WorktreeStatus::Failed => "Failed",
        WorktreeStatus::Abandoned => "Abandoned",
        WorktreeStatus::Interrupted => "Interrupted",
    }
}

/// Format time information in a human-readable way
fn format_time_relative(started: &DateTime<Utc>, last_activity: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let started_ago = format_duration(now.signed_duration_since(*started));
    let active_ago = format_duration(now.signed_duration_since(*last_activity));

    format!("Started: {} • Last active: {}", started_ago, active_ago)
}

/// Format duration in a human-readable way
fn format_duration(duration: chrono::Duration) -> String {
    let total_seconds = duration.num_seconds().abs();

    if total_seconds < 60 {
        format!("{}s ago", total_seconds)
    } else if total_seconds < 3600 {
        format!("{}m ago", total_seconds / 60)
    } else if total_seconds < 86400 {
        format!("{}h ago", total_seconds / 3600)
    } else {
        format!("{}d ago", total_seconds / 86400)
    }
}

/// Convert WorktreeState to EnhancedSessionInfo
impl From<&WorktreeState> for EnhancedSessionInfo {
    fn from(state: &WorktreeState) -> Self {
        EnhancedSessionInfo {
            session_id: state.session_id.clone(),
            status: state.status.clone(),
            workflow_path: None, // Will be populated from session state
            workflow_args: vec![],
            started_at: state.created_at,
            last_activity: state.updated_at,
            current_step: 0,
            total_steps: None,
            error_summary: state.error.clone(),
            branch_name: state.branch.clone(),
            parent_branch: None,           // Will be detected from git
            worktree_path: PathBuf::new(), // Will be populated
            files_changed: state.stats.files_changed,
            commits: state.stats.commits,
            items_processed: None,
            total_items: None,
        }
    }
}
