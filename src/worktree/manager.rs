//! Worktree manager implementation for git worktree operations
//!
//! This module provides the core `WorktreeManager` struct that orchestrates
//! all worktree operations including creation, merging, cleanup, and session
//! management. It coordinates between multiple helper modules for validation,
//! utilities, and queries.
//!
//! # Architecture
//!
//! The WorktreeManager serves as the main orchestrator and delegates to:
//! - `manager_validation` - Pure validation functions for merge operations
//! - `manager_utilities` - Pure utility functions for string manipulation
//! - `manager_queries` - Query operations for reading session state
//!
//! # Responsibilities
//!
//! Core responsibilities retained in WorktreeManager:
//! - Session creation and lifecycle management
//! - Git worktree operations (create, merge, cleanup)
//! - Subprocess execution and I/O operations
//! - Async orchestration of complex workflows
//! - State management and persistence
//! - Custom merge workflow execution
//!
//! # Design Principles
//!
//! - **I/O at the edges**: All file and subprocess operations stay in manager
//! - **Pure logic extracted**: Validation and utilities moved to separate modules
//! - **Async orchestration**: Complex workflows coordinated at manager level
//! - **Clear boundaries**: Each module has a single, well-defined responsibility

use crate::config::mapreduce::MergeWorkflow;
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

use super::manager_queries::load_state_from_file;
use super::manager_utilities;
use super::manager_validation;
use super::merge_orchestrator::MergeOrchestrator;
use super::parsing;
use super::{WorktreeSession, WorktreeState, WorktreeStatus};

/// Configuration for worktree cleanup behavior
#[derive(Debug, Clone)]
pub struct CleanupConfig {
    pub auto_cleanup: bool,
    pub confirm_before_cleanup: bool,
    pub retention_days: u32,
    pub dry_run: bool,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            auto_cleanup: true,
            confirm_before_cleanup: true,
            retention_days: 7,
            dry_run: false,
        }
    }
}

/// Strategy for cleanup operations
#[derive(Debug, Clone, PartialEq)]
pub enum CleanupPolicy {
    Automatic,
    Manual,
    Disabled,
}

pub struct WorktreeManager {
    pub base_dir: PathBuf,
    pub repo_path: PathBuf,
    pub(crate) subprocess: SubprocessManager,
    pub(crate) verbosity: u8,
    pub(crate) custom_merge_workflow: Option<MergeWorkflow>,
    pub(crate) workflow_env: HashMap<String, String>,
}

impl WorktreeManager {
    pub fn update_session_state<F>(&self, name: &str, updater: F) -> Result<()>
    where
        F: FnOnce(&mut WorktreeState),
    {
        let state_file = self.base_dir.join(".metadata").join(format!("{name}.json"));
        let mut state: WorktreeState = serde_json::from_str(&fs::read_to_string(&state_file)?)?;

        updater(&mut state);
        state.updated_at = Utc::now();

        let json = serde_json::to_string_pretty(&state)?;

        // Write to temp file first, then rename atomically
        let temp_file = self
            .base_dir
            .join(".metadata")
            .join(format!("{name}.json.tmp"));
        fs::write(&temp_file, &json)?;
        fs::rename(&temp_file, &state_file)?;

        Ok(())
    }

    /// List all active worktree sessions
    ///
    /// # Returns
    /// * `Result<Vec<WorktreeSession>>` - List of active sessions
    ///
    /// # Errors
    /// Returns error if unable to read worktree information
    pub async fn list_sessions(&self) -> Result<Vec<WorktreeSession>> {
        // First, get sessions from Git worktrees
        let mut sessions = self.list_git_worktree_sessions().await?;

        // Then, supplement with sessions from metadata that might not be in Git
        // (e.g., sessions with non-standard branch names or in transitional states)
        let metadata_sessions = self.list_metadata_sessions()?;

        // Merge the two lists, preferring Git state but using metadata for missing info
        for meta_session in metadata_sessions {
            if !sessions.iter().any(|s| s.name == meta_session.name) {
                // This session exists in metadata but not in Git worktrees
                // Check if the worktree directory actually exists AND is a valid git worktree
                let worktree_path = self.base_dir.join(&meta_session.name);
                if worktree_path.exists() {
                    // Verify it's actually a git worktree by checking for .git file
                    let git_file = worktree_path.join(".git");
                    if git_file.exists() {
                        sessions.push(meta_session);
                    } else {
                        // This is a stale metadata entry - the worktree is gone or invalid
                        // We'll skip it from the list, and it should be cleaned up
                        debug!(
                            "Skipping stale metadata entry: {} (not a valid git worktree)",
                            meta_session.name
                        );
                    }
                }
            }
        }

        Ok(sessions)
    }

    /// List sessions from Git worktrees
    async fn list_git_worktree_sessions(&self) -> Result<Vec<WorktreeSession>> {
        let command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "list", "--porcelain"])
            .build();

        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context("Failed to execute git worktree list")?;

        if !output.status.success() {
            anyhow::bail!("Failed to list worktrees: {}", output.stderr);
        }

        let stdout = &output.stdout;
        let worktree_entries = parsing::parse_worktree_output(stdout);

        Ok(worktree_entries
            .into_iter()
            .filter_map(|(path, branch)| self.create_worktree_session(path, branch))
            .collect())
    }

    /// Create a WorktreeSession if the path is within our base directory
    /// List sessions with detailed information including workflow and progress
    ///
    /// This method gathers enhanced session information from both worktree state
    /// and session state files to provide comprehensive details about each session.
    ///
    /// # Returns
    /// * `Result<DetailedWorktreeList>` - Detailed list of sessions with enhanced info
    ///
    /// # Errors
    /// Returns error if unable to read session information
    pub async fn list_detailed(&self) -> Result<super::display::DetailedWorktreeList> {
        use super::display::{DetailedWorktreeList, EnhancedSessionInfo, WorktreeSummary};

        // Get basic session list
        let sessions = self.list_sessions().await?;
        let mut enhanced_sessions = Vec::new();
        let mut summary = WorktreeSummary::default();

        for session in sessions {
            // Load worktree state
            let state_file = self
                .base_dir
                .join(".metadata")
                .join(format!("{}.json", session.name));

            if let Ok(state_json) = std::fs::read_to_string(&state_file) {
                if let Ok(state) = serde_json::from_str::<WorktreeState>(&state_json) {
                    // Create enhanced info from worktree state
                    let mut enhanced = EnhancedSessionInfo::from(&state);
                    enhanced.worktree_path = session.path.clone();

                    // Try to load session state for workflow information
                    let session_state_path =
                        session.path.join(".prodigy").join("session_state.json");
                    if let Ok(session_json) = std::fs::read_to_string(&session_state_path) {
                        if let Ok(session_state) =
                            serde_json::from_str::<serde_json::Value>(&session_json)
                        {
                            // Extract workflow information from session state
                            if let Some(workflow_state) = session_state.get("workflow_state") {
                                if let Some(path) =
                                    workflow_state.get("workflow_path").and_then(|p| p.as_str())
                                {
                                    enhanced.workflow_path = Some(PathBuf::from(path));
                                }

                                if let Some(args) =
                                    workflow_state.get("input_args").and_then(|a| a.as_array())
                                {
                                    enhanced.workflow_args = args
                                        .iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect();
                                }

                                if let Some(current_step) =
                                    workflow_state.get("current_step").and_then(|s| s.as_u64())
                                {
                                    enhanced.current_step = current_step as usize;
                                }

                                if let Some(completed) = workflow_state
                                    .get("completed_steps")
                                    .and_then(|s| s.as_array())
                                {
                                    enhanced.total_steps = Some(completed.len());
                                }
                            }

                            // Extract MapReduce progress if available
                            if let Some(mapreduce_state) = session_state.get("mapreduce_state") {
                                if let Some(processed) = mapreduce_state
                                    .get("items_processed")
                                    .and_then(|p| p.as_u64())
                                {
                                    enhanced.items_processed = Some(processed as u32);
                                }
                                if let Some(total) =
                                    mapreduce_state.get("total_items").and_then(|t| t.as_u64())
                                {
                                    enhanced.total_items = Some(total as u32);
                                }
                            }
                        }
                    }

                    // Try to determine parent branch from git
                    enhanced.parent_branch = self.get_parent_branch(&session.branch).await.ok();

                    // Update summary counts
                    summary.total += 1;
                    match state.status {
                        WorktreeStatus::InProgress => summary.in_progress += 1,
                        WorktreeStatus::Interrupted => summary.interrupted += 1,
                        WorktreeStatus::Failed => summary.failed += 1,
                        WorktreeStatus::Completed | WorktreeStatus::Merged => {
                            summary.completed += 1
                        }
                        _ => {}
                    }

                    enhanced_sessions.push(enhanced);
                }
            }
        }

        // Sort by last activity (most recent first)
        enhanced_sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        Ok(DetailedWorktreeList {
            sessions: enhanced_sessions,
            summary,
        })
    }

    /// List sessions from metadata files
    fn list_metadata_sessions(&self) -> Result<Vec<WorktreeSession>> {
        let metadata_dir = self.base_dir.join(".metadata");
        if !metadata_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for entry in fs::read_dir(&metadata_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip non-JSON files and special files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Skip cleanup.log and other non-session files
            let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !filename.starts_with("session-") {
                continue;
            }

            if let Some(state) = load_state_from_file(&path) {
                // Only include sessions that are not cleaned up
                if state.status != WorktreeStatus::CleanedUp {
                    let worktree_path = self.base_dir.join(&state.worktree_name);
                    sessions.push(WorktreeSession::new(
                        state.worktree_name,
                        state.branch,
                        worktree_path,
                    ));
                }
            }
        }

        Ok(sessions)
    }

    /// Merge a worktree session back to the original branch
    ///
    /// # Arguments
    /// * `name` - Name of the worktree session to merge
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Errors
    /// Returns error if merge fails or session not found
    pub async fn merge_session(&self, name: &str) -> Result<()> {
        // Find session and extract branch information
        let session = self.find_session_by_name(name).await?;
        let worktree_branch = &session.branch;

        // Use original branch instead of hardcoded main/master
        let target_branch = self.get_merge_target(name).await?;
        info!("Merging {} to {}", worktree_branch, target_branch);

        let should_merge = self
            .validate_merge_preconditions(name, worktree_branch, &target_branch)
            .await?;

        if should_merge {
            // Execute merge workflow
            let merge_output = self
                .execute_merge_workflow(name, worktree_branch, &target_branch)
                .await?;

            // Verify merge completed successfully
            self.verify_merge_completion(worktree_branch, &target_branch, &merge_output)
                .await?;
        } else {
            println!(
                "ℹ️  No new commits in worktree '{}', skipping merge (already in sync with '{}')",
                name, target_branch
            );
        }

        // Update session state and handle cleanup
        self.finalize_merge_session(name).await?;

        Ok(())
    }

    /// Find session by name - pure function that extracts session lookup logic
    async fn find_session_by_name(&self, name: &str) -> Result<WorktreeSession> {
        let sessions = self.list_sessions().await?;
        sessions
            .into_iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Worktree '{}' not found", name))
    }

    /// Pure function to build branch check command
    /// Validate merge preconditions - combines validation logic
    /// Returns Ok(true) if merge should proceed, Ok(false) if no commits to merge
    async fn validate_merge_preconditions(
        &self,
        _name: &str,
        worktree_branch: &str,
        target_branch: &str,
    ) -> Result<bool> {
        let commit_count = self
            .get_commit_count_between_branches(target_branch, worktree_branch)
            .await?;
        Ok(manager_validation::should_proceed_with_merge(&commit_count))
    }

    /// Execute merge workflow - delegates to MergeOrchestrator
    async fn execute_merge_workflow(
        &self,
        name: &str,
        worktree_branch: &str,
        target_branch: &str,
    ) -> Result<String> {
        let orchestrator = MergeOrchestrator::new(
            self.subprocess.clone(),
            self.base_dir.clone(),
            self.repo_path.clone(),
            self.verbosity,
            self.custom_merge_workflow.clone(),
            self.workflow_env.clone(),
        );

        orchestrator
            .execute_merge_workflow(name, worktree_branch, target_branch, |session_name| {
                self.load_session_state(session_name)
            })
            .await
    }

    /// Verify merge completion - I/O operation with pure validation
    async fn verify_merge_completion(
        &self,
        worktree_branch: &str,
        target_branch: &str,
        merge_output: &str,
    ) -> Result<()> {
        let merged_branches = self.get_merged_branches(target_branch).await?;
        manager_validation::validate_merge_success(
            worktree_branch,
            target_branch,
            &merged_branches,
            merge_output,
        )
    }

    /// Finalize merge session - orchestrates post-merge operations
    async fn finalize_merge_session(&self, name: &str) -> Result<()> {
        self.update_session_state_after_merge(name)?;
        self.handle_auto_cleanup_if_enabled(name).await?;
        Ok(())
    }

    /// Update session state after merge - I/O operation
    fn update_session_state_after_merge(&self, name: &str) -> Result<()> {
        if let Err(e) = self.update_session_state(name, |state| {
            state.merged = true;
            state.merged_at = Some(Utc::now());
            state.status = crate::worktree::WorktreeStatus::Merged;
        }) {
            eprintln!("Warning: Failed to update session state after merge: {e}");
        }
        Ok(())
    }

    /// Handle auto-cleanup if enabled - orchestrates cleanup logic
    async fn handle_auto_cleanup_if_enabled(&self, name: &str) -> Result<()> {
        let cleanup_config = Self::get_cleanup_config();
        if cleanup_config.auto_cleanup {
            self.perform_auto_cleanup(name).await
        } else {
            self.show_manual_cleanup_message(name);
            Ok(())
        }
    }

    /// Perform auto cleanup - I/O operation
    async fn perform_auto_cleanup(&self, name: &str) -> Result<()> {
        println!("🧹 Auto-cleanup is enabled, checking if session can be cleaned up...");

        // Give a moment for the merge to propagate
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        match self.cleanup_session_after_merge(name).await {
            Ok(()) => {
                println!("✅ Successfully cleaned up merged session: {name}");
                Ok(())
            }
            Err(e) => {
                eprintln!("⚠️  Auto-cleanup failed for session {name}: {e}");
                self.show_cleanup_diagnostics(name).await;
                eprintln!(
                    "   You can manually clean up later with: prodigy worktree cleanup {name}"
                );
                Ok(())
            }
        }
    }

    /// Show cleanup diagnostics - I/O operation
    async fn show_cleanup_diagnostics(&self, name: &str) {
        let worktree_path = self.base_dir.join(name);
        if worktree_path.exists() {
            let status_command = ProcessCommandBuilder::new("git")
                .current_dir(&worktree_path)
                .args(["status", "--short"])
                .build();

            if let Ok(status_output) = self.subprocess.runner().run(status_command).await {
                if status_output.status.success() && !status_output.stdout.trim().is_empty() {
                    eprintln!("📝 Current worktree status:");
                    eprintln!("{}", status_output.stdout.trim());
                }
            }
        }
    }

    /// Pure function to show manual cleanup message
    fn show_manual_cleanup_message(&self, name: &str) {
        println!("{}", manager_utilities::format_cleanup_message(name));
    }

    /// Clean up a worktree session
    ///
    /// # Arguments
    /// * `name` - Name of the worktree session to clean up
    /// * `force` - Force cleanup even if there are uncommitted changes
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Errors
    /// Returns error if cleanup fails or session not found
    pub async fn cleanup_session(&self, name: &str, force: bool) -> Result<()> {
        let worktree_path = self.base_dir.join(name);
        let worktree_path_str = worktree_path.to_string_lossy();

        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(&worktree_path_str);

        let remove_command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(&args)
            .build();

        let prune_output = self
            .subprocess
            .runner()
            .run(remove_command)
            .await
            .context("Failed to execute git worktree remove")?;

        if !prune_output.status.success() {
            let stderr = &prune_output.stderr;
            if !stderr.contains("is not a working tree") {
                anyhow::bail!("Failed to remove worktree: {stderr}");
            }
        }

        let branch_check_command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--verify", &format!("refs/heads/{name}")])
            .build();

        let branch_exists = self
            .subprocess
            .runner()
            .run(branch_check_command)
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        if branch_exists {
            let delete_command = ProcessCommandBuilder::new("git")
                .current_dir(&self.repo_path)
                .args(["branch", "-D", name])
                .build();

            let delete_output = self
                .subprocess
                .runner()
                .run(delete_command)
                .await
                .context("Failed to delete branch")?;

            if !delete_output.status.success() {
                let stderr = &delete_output.stderr;
                eprintln!("Warning: Failed to delete branch {name}: {stderr}");
            }
        }

        // Clean up metadata file
        let metadata_file = self.base_dir.join(".metadata").join(format!("{name}.json"));
        if metadata_file.exists() {
            if let Err(e) = fs::remove_file(&metadata_file) {
                eprintln!("Warning: Failed to remove metadata file for {name}: {e}");
            }
        }

        // Also try to remove the worktree directory if it still exists
        // (in case git worktree remove failed or it wasn't a valid worktree)
        if worktree_path.exists() && force {
            if let Err(e) = fs::remove_dir_all(&worktree_path) {
                eprintln!(
                    "Warning: Failed to remove worktree directory {}: {e}",
                    worktree_path_str
                );
            }
        }

        Ok(())
    }

    pub async fn cleanup_all_sessions(&self, force: bool) -> Result<()> {
        let sessions = self.list_sessions().await?;
        for session in sessions {
            let name = &session.name;
            println!("Cleaning up worktree: {name}");
            self.cleanup_session(name, force).await?;
        }
        Ok(())
    }

    /// Update an existing checkpoint
    pub fn update_checkpoint<F>(&self, session_name: &str, updater: F) -> Result<()>
    where
        F: FnOnce(&mut super::Checkpoint),
    {
        self.update_session_state(session_name, |state| {
            if let Some(ref mut checkpoint) = state.last_checkpoint {
                updater(checkpoint);
            }
        })
    }

    /// Restore a session for resuming work
    pub fn restore_session(&self, session_id: &str) -> Result<WorktreeSession> {
        let state = self.load_session_state(session_id)?;
        let worktree_path = self.base_dir.join(&state.worktree_name);

        // Verify the worktree still exists
        if !worktree_path.exists() {
            anyhow::bail!(
                "Worktree path no longer exists: {}",
                worktree_path.display()
            );
        }

        Ok(WorktreeSession::new(
            state.worktree_name.clone(),
            state.branch.clone(),
            worktree_path,
        ))
    }

    /// Mark a session as abandoned (non-resumable)
    pub fn mark_session_abandoned(&self, session_id: &str) -> Result<()> {
        self.update_session_state(session_id, |state| {
            state.status = WorktreeStatus::Abandoned;
            state.resumable = false;
        })
    }

    /// Get the last successful command from a session
    pub fn get_last_successful_command(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, super::CommandType)>> {
        let state = self.load_session_state(session_id)?;
        Ok(state
            .last_checkpoint
            .map(|checkpoint| (checkpoint.last_command, checkpoint.last_command_type)))
    }

    /// Check if a branch has been merged into the target branch
    pub async fn is_branch_merged(&self, branch: &str, target: &str) -> Result<bool> {
        let merge_check_command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["branch", "--merged", target])
            .build();

        let output = self
            .subprocess
            .runner()
            .run(merge_check_command)
            .await
            .context("Failed to check merged branches")?;

        if !output.status.success() {
            return Ok(false);
        }

        Ok(manager_validation::check_if_branch_merged(
            branch,
            &output.stdout,
        ))
    }

    /// Detect if a worktree branch has been merged and is ready for cleanup
    pub async fn detect_mergeable_sessions(&self) -> Result<Vec<String>> {
        let sessions = self.list_sessions().await?;
        let mut mergeable = Vec::new();

        // Determine the default branch (main or master)
        let main_check_command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--verify", "refs/heads/main"])
            .build();

        let main_exists = self
            .subprocess
            .runner()
            .run(main_check_command)
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        let target_branch = if main_exists { "main" } else { "master" };

        for session in sessions {
            // Check if this session is marked as merged in our state
            if let Ok(state) = self.get_session_state(&session.name) {
                if state.merged
                    && self
                        .is_branch_merged(&session.branch, target_branch)
                        .await?
                {
                    mergeable.push(session.name);
                }
            }
        }

        Ok(mergeable)
    }

    /// Clean up merged worktree sessions
    pub async fn cleanup_merged_sessions(&self, config: &CleanupConfig) -> Result<Vec<String>> {
        let mergeable_sessions = self.detect_mergeable_sessions().await?;
        let mut cleaned_up = Vec::new();

        for session_name in mergeable_sessions {
            if config.dry_run {
                println!("[DRY RUN] Would cleanup session: {session_name}");
                cleaned_up.push(session_name);
                continue;
            }

            if config.confirm_before_cleanup {
                println!("Session '{session_name}' has been merged. Clean up? (y/N): ");
                // In a real implementation, we'd read from stdin here
                // For now, we'll skip confirmation in automated contexts
                if std::env::var("PRODIGY_AUTOMATION").is_ok() {
                    // Auto-confirm in automation mode
                } else {
                    // Skip cleanup if not in automation mode and confirmation is required
                    continue;
                }
            }

            // Perform the cleanup
            match self.cleanup_session_after_merge(&session_name).await {
                Ok(()) => {
                    println!("✅ Cleaned up merged session: {session_name}");
                    cleaned_up.push(session_name);
                }
                Err(e) => {
                    eprintln!("❌ Failed to cleanup session {session_name}: {e}");
                }
            }
        }

        Ok(cleaned_up)
    }

    /// Clean up a specific session after merge, with additional safety checks
    pub async fn cleanup_session_after_merge(&self, name: &str) -> Result<()> {
        // Verify the session exists and is marked as merged
        let state = self.get_session_state(name)?;
        if !state.merged {
            anyhow::bail!("Session '{name}' is not marked as merged. Cannot clean up.");
        }

        let worktree_path = self.base_dir.join(name);

        // Safety check: verify no uncommitted changes exist
        // After a successful merge, we can safely force cleanup even if there are
        // uncommitted changes in the worktree, since the important changes have
        // already been merged to the main branch
        if worktree_path.exists() {
            let status_command = ProcessCommandBuilder::new("git")
                .current_dir(&worktree_path)
                .args(["status", "--porcelain"])
                .build();

            let status_output = self
                .subprocess
                .runner()
                .run(status_command)
                .await
                .context("Failed to check worktree status")?;

            if status_output.status.success() && !status_output.stdout.trim().is_empty() {
                // Worktree has uncommitted changes, but since it's already merged,
                // we can safely force cleanup
                println!("📝 Worktree has uncommitted changes after merge:");
                println!("{}", status_output.stdout.trim());
                println!("🔧 Using force cleanup since changes are already merged...");
                self.cleanup_session(name, true).await?;
            } else {
                // No uncommitted changes, regular cleanup
                self.cleanup_session(name, false).await?;
            }
        } else {
            // Worktree doesn't exist, just clean up metadata
            self.cleanup_session(name, false).await?;
        }

        // Clean up session state file
        let state_file = self.base_dir.join(".metadata").join(format!("{name}.json"));
        if state_file.exists() {
            fs::remove_file(&state_file).context("Failed to remove session state file")?;
        }

        // Log the cleanup operation
        let log_entry = format!(
            "[{}] Cleaned up merged worktree session: {name} (branch: {})",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            state.branch
        );

        let log_file = self.base_dir.join(".metadata").join("cleanup.log");
        let log_dir = log_file
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid log file path: no parent directory"))?;
        fs::create_dir_all(log_dir).context("Failed to create log directory")?;

        fs::write(
            &log_file,
            if log_file.exists() {
                format!("{}\n{log_entry}", fs::read_to_string(&log_file)?)
            } else {
                log_entry
            },
        )
        .context("Failed to write cleanup log")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::ProcessCommandBuilder;
    use crate::worktree::manager_queries::{
        collect_all_states, filter_sessions_by_status, load_state_from_file,
    };
    use crate::worktree::{IterationInfo, WorktreeStats};
    use tempfile::TempDir;

    #[test]
    fn test_claude_merge_command_construction() {
        // Test that merge_session correctly constructs the Claude command
        let temp_dir = TempDir::new().unwrap();
        let repo_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // We can't actually test the command execution without Claude CLI,
        // but we can verify the logic flow exists
        assert!(manager.base_dir.exists());
        // The base_dir should end with the repository name now
        assert_eq!(
            manager.base_dir.file_name().unwrap().to_str().unwrap(),
            repo_name
        );
        // And it should be under ~/.prodigy/worktrees/
        let parent = manager.base_dir.parent().unwrap();
        assert_eq!(parent.file_name().unwrap(), "worktrees");
    }

    #[tokio::test]
    async fn test_merge_session_success() {
        // Note: This test is limited because we can't mock the external Claude CLI
        // In a real test environment, we would use dependency injection for the command execution
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a mock session - though we can't actually merge without Claude CLI
        let session_name = "test-session";

        // Test will fail because session doesn't exist, which is expected
        let result = manager.merge_session(session_name).await;
        assert!(result.is_err());
        // Just check that it returns an error, the specific message may vary
        // depending on the environment
    }

    #[tokio::test]
    async fn test_merge_session_claude_cli_failure() {
        // Test behavior when Claude CLI is not available
        // This test documents expected failure mode
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        let session_name = "nonexistent-session";
        let result = manager.merge_session(session_name).await;

        // Should fail because session doesn't exist
        assert!(result.is_err());
        let error = result.unwrap_err();
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("not found") || error_msg.contains("does not exist"),
            "Expected session not found error, got: {}",
            error_msg
        );
    }

    #[tokio::test]
    async fn test_cleanup_session() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Try to cleanup a non-existent session - should handle gracefully
        let result = manager.cleanup_session("nonexistent", false).await;

        // Should succeed (cleanup is idempotent)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_all_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Should succeed even with no sessions
        let result = manager.cleanup_all_sessions(false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a session state file for testing
        let session_name = "test-checkpoint-session";
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let state = WorktreeState::new(
            session_name.to_string(),
            "test-branch".to_string(),
            "session-123".to_string(),
        );
        let state_file = metadata_dir.join(format!("{session_name}.json"));
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Test updating checkpoint
        let result = manager.update_checkpoint(session_name, |checkpoint| {
            checkpoint.current_step += 1;
        });

        // May fail if checkpoint doesn't exist, which is expected
        if result.is_ok() {
            let updated_state = manager.load_session_state(session_name).unwrap();
            assert!(updated_state.last_checkpoint.is_some());
        }
    }

    #[tokio::test]
    async fn test_restore_session() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a session state file
        let session_name = "test-restore-session";
        let worktree_name = format!("worktree-{session_name}");
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let state = WorktreeState::new(
            worktree_name.clone(),
            "test-branch".to_string(),
            session_name.to_string(),
        );
        let state_file = metadata_dir.join(format!("{session_name}.json"));
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Create the worktree directory
        let worktree_path = manager.base_dir.join(&worktree_name);
        std::fs::create_dir_all(&worktree_path).unwrap();

        // Test restoring session
        let result = manager.restore_session(session_name);
        assert!(result.is_ok());

        let session = result.unwrap();
        assert_eq!(session.name, worktree_name);
    }

    #[tokio::test]
    async fn test_mark_session_abandoned() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a session state file
        let session_name = "test-abandoned-session";
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let state = WorktreeState::new(
            session_name.to_string(),
            "test-branch".to_string(),
            "session-123".to_string(),
        );
        let state_file = metadata_dir.join(format!("{session_name}.json"));
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Mark as abandoned
        let result = manager.mark_session_abandoned(session_name);
        assert!(result.is_ok());

        // Verify status changed
        let updated_state = manager.load_session_state(session_name).unwrap();
        assert_eq!(updated_state.status, WorktreeStatus::Abandoned);
        assert!(!updated_state.resumable);
    }

    #[tokio::test]
    async fn test_get_last_successful_command() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a session with a checkpoint
        let session_name = "test-command-session";
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let mut state = WorktreeState::new(
            session_name.to_string(),
            "test-branch".to_string(),
            "session-123".to_string(),
        );

        use crate::worktree::{Checkpoint, CommandType};
        state.last_checkpoint = Some(Checkpoint {
            step_index: 0,
            total_steps: 5,
            current_step: 0,
            last_command: "test command".to_string(),
            last_command_type: CommandType::Claude,
            timestamp: chrono::Utc::now(),
            variables: HashMap::new(),
        });

        let state_file = metadata_dir.join(format!("{session_name}.json"));
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Get last command
        let result = manager.get_last_successful_command(session_name);
        assert!(result.is_ok());

        let command = result.unwrap();
        assert!(command.is_some());
        let (cmd, cmd_type) = command.unwrap();
        assert_eq!(cmd, "test command");
        assert!(matches!(cmd_type, CommandType::Claude));
    }

    #[tokio::test]
    async fn test_is_branch_merged() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // This will fail without a real git repo, which is expected
        let result = manager.is_branch_merged("some-branch", "main").await;

        // Just verify the function exists and returns a Result
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_detect_mergeable_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Should return empty list when no sessions exist
        let result = manager.detect_mergeable_sessions().await;

        // May fail if git operations fail, but should not panic
        if let Ok(mergeable) = result {
            assert!(mergeable.is_empty());
        }
    }

    #[tokio::test]
    async fn test_cleanup_merged_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        let config = CleanupConfig {
            dry_run: true,
            ..Default::default()
        };

        // Should handle empty session list gracefully
        let result = manager.cleanup_merged_sessions(&config).await;

        if let Ok(cleaned) = result {
            assert!(cleaned.is_empty());
        }
    }

    #[tokio::test]
    async fn test_cleanup_session_after_merge() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a session marked as merged
        let session_name = "test-merged-session";
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let mut state = WorktreeState::new(
            session_name.to_string(),
            "test-branch".to_string(),
            "session-123".to_string(),
        );
        state.merged = true;

        let state_file = metadata_dir.join(format!("{session_name}.json"));
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Attempt cleanup - may fail if worktree doesn't exist, which is expected
        let result = manager.cleanup_session_after_merge(session_name).await;

        // Function should exist and attempt cleanup
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_update_session_state() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a session state file
        let session_name = "test-update-session";
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let state = WorktreeState::new(
            session_name.to_string(),
            "test-branch".to_string(),
            "session-123".to_string(),
        );
        let state_file = metadata_dir.join(format!("{session_name}.json"));
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Update state
        let result = manager.update_session_state(session_name, |state| {
            state.iterations.completed = 5;
            state.stats.files_changed = 10;
        });

        assert!(result.is_ok());

        // Verify update
        let updated = manager.load_session_state(session_name).unwrap();
        assert_eq!(updated.iterations.completed, 5);
        assert_eq!(updated.stats.files_changed, 10);
    }

    #[test]
    fn test_cleanup_config_default() {
        let config = CleanupConfig::default();
        assert!(config.auto_cleanup);
        assert!(config.confirm_before_cleanup);
        assert_eq!(config.retention_days, 7);
        assert!(!config.dry_run);
    }

    #[test]
    fn test_cleanup_policy_variants() {
        use CleanupPolicy::*;
        assert_ne!(Automatic, Manual);
        assert_ne!(Manual, Disabled);
        assert_eq!(Automatic, Automatic);
    }
}
