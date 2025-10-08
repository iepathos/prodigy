//! Pure data access and query functions for WorktreeManager
//!
//! This module contains pure functions for querying and filtering worktree data.
//! These functions have no side effects and are easily testable in isolation.

use anyhow::{Context, Result};
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

use super::{WorktreeManager, WorktreeState, WorktreeStatus};
use crate::subprocess::ProcessCommandBuilder;

// ============================================================================
// Pure Data Filtering Functions
// ============================================================================

/// Filter session states by a specific status
///
/// This is a pure function that can be tested in isolation
pub(crate) fn filter_sessions_by_status(
    states: Vec<WorktreeState>,
    target_status: WorktreeStatus,
) -> Vec<WorktreeState> {
    states
        .into_iter()
        .filter(|state| state.status == target_status)
        .collect()
}

/// Load and parse a worktree state from a JSON file path
///
/// Returns None if the file cannot be read or parsed
pub(crate) fn load_state_from_file(path: &Path) -> Option<WorktreeState> {
    if path.extension().and_then(|s| s.to_str()) != Some("json") {
        return None;
    }

    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<WorktreeState>(&content).ok())
}

/// Collect all worktree states from a metadata directory
pub(crate) fn collect_all_states(metadata_dir: &Path) -> Result<Vec<WorktreeState>> {
    if !metadata_dir.exists() {
        return Ok(Vec::new());
    }

    let mut states = Vec::new();

    for entry in fs::read_dir(metadata_dir)? {
        let path = entry?.path();
        if let Some(state) = load_state_from_file(&path) {
            states.push(state);
        }
    }

    Ok(states)
}

// ============================================================================
// Session State Access Methods
// ============================================================================

impl WorktreeManager {
    /// Get session state by name
    pub fn get_session_state(&self, name: &str) -> Result<WorktreeState> {
        let state_file = self.base_dir.join(".metadata").join(format!("{name}.json"));
        let state_json = fs::read_to_string(&state_file)?;
        let state: WorktreeState = serde_json::from_str(&state_json)?;
        Ok(state)
    }

    /// List all interrupted sessions
    pub fn list_interrupted_sessions(&self) -> Result<Vec<WorktreeState>> {
        let metadata_dir = self.base_dir.join(".metadata");
        let all_states = collect_all_states(&metadata_dir)?;
        Ok(filter_sessions_by_status(
            all_states,
            WorktreeStatus::Interrupted,
        ))
    }

    /// Load session state by session ID (name)
    pub fn load_session_state(&self, session_id: &str) -> Result<WorktreeState> {
        self.get_session_state(session_id)
    }
}

// ============================================================================
// Git Query Methods
// ============================================================================

impl WorktreeManager {
    /// Get the parent branch for a given branch
    pub(crate) async fn get_parent_branch(&self, branch_name: &str) -> Result<String> {
        let command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["config", "--get", &format!("branch.{}.merge", branch_name)])
            .build();

        let output = self.subprocess.runner().run(command).await?;

        if output.status.success() && !output.stdout.is_empty() {
            // Extract branch name from refs/heads/main format
            let parent = output.stdout.trim();
            if let Some(name) = parent.strip_prefix("refs/heads/") {
                return Ok(name.to_string());
            }
        }

        // Default to main or master if we can't determine
        Ok("main".to_string())
    }

    /// Get the current branch name from the repository
    ///
    /// Returns the name of the currently checked-out branch, or "HEAD"
    /// if in detached HEAD state.
    pub(crate) async fn get_current_branch(&self) -> Result<String> {
        let command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .build();

        let output = self.subprocess.runner().run(command).await?;

        if !output.status.success() {
            anyhow::bail!("Failed to get current branch");
        }

        Ok(output.stdout.trim().to_string())
    }

    /// Determine the merge target for a worktree session
    ///
    /// Returns the original branch from session state, with fallbacks:
    /// 1. If original_branch is empty (old session): use main/master
    /// 2. If original_branch is "HEAD" (detached): use main/master
    /// 3. If original_branch was deleted: warn and use main/master
    /// 4. Otherwise: use original_branch
    pub async fn get_merge_target(&self, session_name: &str) -> Result<String> {
        let state = self.get_session_state(session_name)?;

        // Handle old sessions or edge cases
        if state.original_branch.is_empty() || state.original_branch == "HEAD" {
            if !state.original_branch.is_empty() {
                warn!("Detached HEAD state detected, using default branch");
            }
            return self.determine_default_branch().await;
        }

        // Verify original branch still exists
        if !self.check_branch_exists(&state.original_branch).await? {
            warn!(
                "Original branch '{}' no longer exists, using default branch",
                state.original_branch
            );
            return self.determine_default_branch().await;
        }

        Ok(state.original_branch.clone())
    }

    /// Check if a branch exists - I/O operation
    pub(crate) async fn check_branch_exists(&self, branch: &str) -> Result<bool> {
        let command = Self::build_branch_check_command(&self.repo_path, branch);
        Ok(self
            .subprocess
            .runner()
            .run(command)
            .await
            .map(|o| o.status.success())
            .unwrap_or(false))
    }

    /// Get commit count between branches - I/O operation
    pub(crate) async fn get_commit_count_between_branches(
        &self,
        target_branch: &str,
        worktree_branch: &str,
    ) -> Result<String> {
        let command =
            Self::build_commit_diff_command(&self.repo_path, target_branch, worktree_branch);
        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context("Failed to check for new commits")?;

        if output.status.success() {
            Ok(output.stdout.trim().to_string())
        } else {
            Err(anyhow::anyhow!("Failed to get commit count"))
        }
    }

    /// Get merged branches - I/O operation
    pub(crate) async fn get_merged_branches(&self, target_branch: &str) -> Result<String> {
        // Use current directory's git root instead of self.repo_path
        // This ensures we check merges in the correct location when running from a worktree
        let check_path = self
            .get_git_root_path()
            .await
            .unwrap_or_else(|_| self.repo_path.clone());
        let command = Self::build_merge_check_command(&check_path, target_branch);
        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context("Failed to check merged branches")?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(anyhow::anyhow!("Failed to check merged branches"))
        }
    }

    /// Get the git root path for the current working directory
    pub(crate) async fn get_git_root_path(&self) -> Result<PathBuf> {
        let command = ProcessCommandBuilder::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .build();
        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context("Failed to get git root path")?;

        if output.status.success() {
            Ok(PathBuf::from(output.stdout.trim()))
        } else {
            Err(anyhow::anyhow!("Failed to get git root path"))
        }
    }

    /// Get worktree path for a specific branch
    pub async fn get_worktree_for_branch(&self, branch: &str) -> Result<Option<PathBuf>> {
        let sessions = self.list_sessions().await?;
        Ok(sessions
            .into_iter()
            .find(|s| s.branch == branch)
            .map(|s| s.path))
    }

    /// Determine default branch (main or master) - I/O operation separated
    pub(crate) async fn determine_default_branch(&self) -> Result<String> {
        let main_exists = self.check_branch_exists("main").await?;
        Ok(Self::select_default_branch(main_exists))
    }

    /// Pure function to select default branch based on main existence
    fn select_default_branch(main_exists: bool) -> String {
        if main_exists {
            "main".to_string()
        } else {
            "master".to_string()
        }
    }
}

// ============================================================================
// Configuration Access
// ============================================================================

use super::CleanupConfig;

impl WorktreeManager {
    /// Get cleanup configuration from environment or defaults
    pub fn get_cleanup_config() -> CleanupConfig {
        CleanupConfig {
            auto_cleanup: std::env::var("PRODIGY_AUTO_CLEANUP")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            confirm_before_cleanup: std::env::var("PRODIGY_CONFIRM_CLEANUP")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(std::env::var("PRODIGY_AUTOMATION").is_err()),
            retention_days: std::env::var("PRODIGY_RETENTION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7),
            dry_run: std::env::var("PRODIGY_DRY_RUN")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }
}
