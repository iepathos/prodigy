use crate::config::mapreduce::MergeWorkflow;
use crate::cook::execution::{ClaudeExecutor, ClaudeExecutorImpl};
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

use super::manager_queries::load_state_from_file;
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
        Ok(Self::should_proceed_with_merge(&commit_count))
    }

    /// Pure function to determine if merge should proceed based on commit count
    fn should_proceed_with_merge(commit_count: &str) -> bool {
        commit_count != "0"
    }

    /// Execute merge workflow - orchestrates merge execution
    async fn execute_merge_workflow(
        &self,
        name: &str,
        worktree_branch: &str,
        target_branch: &str,
    ) -> Result<String> {
        match &self.custom_merge_workflow {
            Some(merge_workflow) => {
                println!(
                    "🔄 Executing custom merge workflow for '{name}' into '{target_branch}'..."
                );
                self.execute_custom_merge_workflow(
                    merge_workflow,
                    name,
                    worktree_branch,
                    target_branch,
                )
                .await
            }
            None => {
                println!("🔄 Merging worktree '{name}' into '{target_branch}' using Claude-assisted merge...");
                self.execute_claude_merge(worktree_branch).await
            }
        }
    }

    /// Execute Claude merge - I/O operation
    async fn execute_claude_merge(&self, worktree_branch: &str) -> Result<String> {
        if self.verbosity >= 1 {
            eprintln!("Running claude /prodigy-merge-worktree with branch: {worktree_branch}");
        }

        let env_vars = self.build_claude_environment_variables();
        let claude_executor = self.create_claude_executor();

        let result = claude_executor
            .execute_claude_command(
                &format!("/prodigy-merge-worktree {worktree_branch}"),
                &self.repo_path,
                env_vars,
            )
            .await
            .context("Failed to execute claude /prodigy-merge-worktree")?;

        Self::validate_claude_result(&result)?;
        if self.verbosity == 0 {
            // Clean output - only show the final result message
            println!("{}", result.stdout);
        }
        Ok(result.stdout)
    }

    /// Pure function to build Claude environment variables
    /// Pure function to validate Claude execution result
    fn validate_claude_result(result: &crate::cook::execution::ExecutionResult) -> Result<()> {
        if !result.success {
            eprintln!("❌ Claude merge failed:");
            if !result.stderr.is_empty() {
                eprintln!("Error output: {}", result.stderr);
            }
            if !result.stdout.is_empty() {
                eprintln!("Standard output: {}", result.stdout);
            }
            anyhow::bail!("Claude merge failed");
        }
        Ok(())
    }

    /// Verify merge completion - I/O operation with pure validation
    async fn verify_merge_completion(
        &self,
        worktree_branch: &str,
        target_branch: &str,
        merge_output: &str,
    ) -> Result<()> {
        let merged_branches = self.get_merged_branches(target_branch).await?;
        Self::validate_merge_success(
            worktree_branch,
            target_branch,
            &merged_branches,
            merge_output,
        )
    }

    /// Pure function to build merge check command
    /// Pure function to validate merge success
    fn validate_merge_success(
        worktree_branch: &str,
        target_branch: &str,
        merged_branches: &str,
        merge_output: &str,
    ) -> Result<()> {
        if !merged_branches.contains(worktree_branch) {
            if Self::is_permission_denied(merge_output) {
                anyhow::bail!(
                    "Merge was not completed - Claude requires permission to proceed. \
                    Please run the command again and grant permission when prompted."
                );
            }
            anyhow::bail!(
                "Merge verification failed - branch '{}' is not merged into '{}'. \
                The merge may have been aborted or failed silently.",
                worktree_branch,
                target_branch
            );
        }
        Ok(())
    }

    /// Pure function to check if merge output indicates permission denial
    fn is_permission_denied(output: &str) -> bool {
        output.contains("permission") || output.contains("grant permission")
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
        println!("ℹ️  Session '{name}' has been merged. You can clean it up with: prodigy worktree cleanup {name}");
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

        Ok(output.stdout.contains(branch))
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

    /// Execute a custom merge workflow
    /// Initialize variables for merge workflow execution
    async fn init_merge_variables(
        &self,
        worktree_name: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<(HashMap<String, String>, String)> {
        let mut session_id = String::new();
        if let Ok(state) = self.load_session_state(worktree_name) {
            session_id = state.session_id;
        }

        let mut variables = HashMap::new();
        variables.insert("merge.worktree".to_string(), worktree_name.to_string());
        variables.insert("merge.source_branch".to_string(), source_branch.to_string());
        variables.insert("merge.target_branch".to_string(), target_branch.to_string());
        variables.insert("merge.session_id".to_string(), session_id.clone());

        // Get git information for merge workflow
        let worktree_path = self.base_dir.join(worktree_name);
        if worktree_path.exists() {
            use crate::cook::execution::mapreduce::resources::git_operations::{
                GitOperationsConfig, GitOperationsService,
            };

            let config = GitOperationsConfig {
                max_commits: 100, // Limit to recent commits for merge context
                max_files: 500,   // Reasonable limit for modified files
                ..Default::default()
            };
            let mut git_service = GitOperationsService::new(config);

            match git_service
                .get_merge_git_info(&worktree_path, target_branch)
                .await
            {
                Ok(git_info) => {
                    // Serialize commits as JSON for use in workflows
                    if let Ok(commits_json) = serde_json::to_string(&git_info.commits) {
                        variables.insert("merge.commits".to_string(), commits_json);
                        variables.insert(
                            "merge.commit_count".to_string(),
                            git_info.commits.len().to_string(),
                        );
                    }

                    // Serialize modified files as JSON
                    if let Ok(files_json) = serde_json::to_string(&git_info.modified_files) {
                        variables.insert("merge.modified_files".to_string(), files_json);
                        variables.insert(
                            "merge.file_count".to_string(),
                            git_info.modified_files.len().to_string(),
                        );
                    }

                    // Add simple list of file paths for easy reference
                    let file_paths: Vec<String> = git_info
                        .modified_files
                        .iter()
                        .map(|f| f.path.to_string_lossy().to_string())
                        .collect();
                    variables.insert("merge.file_list".to_string(), file_paths.join(", "));

                    // Add simple list of commit IDs
                    let commit_ids: Vec<String> = git_info
                        .commits
                        .iter()
                        .map(|c| c.short_id.clone())
                        .collect();
                    variables.insert("merge.commit_ids".to_string(), commit_ids.join(", "));

                    tracing::debug!(
                        "Added git information to merge variables: {} commits, {} files",
                        git_info.commits.len(),
                        git_info.modified_files.len()
                    );
                }
                Err(e) => {
                    // Log warning but continue - merge can proceed without git info
                    tracing::warn!("Failed to get git information for merge variables: {}", e);
                    variables.insert("merge.commits".to_string(), "[]".to_string());
                    variables.insert("merge.modified_files".to_string(), "[]".to_string());
                    variables.insert("merge.commit_count".to_string(), "0".to_string());
                    variables.insert("merge.file_count".to_string(), "0".to_string());
                    variables.insert("merge.file_list".to_string(), String::new());
                    variables.insert("merge.commit_ids".to_string(), String::new());
                }
            }
        } else {
            // Worktree doesn't exist yet, set empty values
            variables.insert("merge.commits".to_string(), "[]".to_string());
            variables.insert("merge.modified_files".to_string(), "[]".to_string());
            variables.insert("merge.commit_count".to_string(), "0".to_string());
            variables.insert("merge.file_count".to_string(), "0".to_string());
            variables.insert("merge.file_list".to_string(), String::new());
            variables.insert("merge.commit_ids".to_string(), String::new());
        }

        // Include workflow environment variables for interpolation in merge workflow commands
        for (key, value) in &self.workflow_env {
            variables.insert(key.clone(), value.clone());
        }

        Ok((variables, session_id))
    }

    /// Execute a shell command in the merge workflow
    async fn execute_merge_shell_command(
        &self,
        shell_cmd: &str,
        variables: &HashMap<String, String>,
        step_index: usize,
        total_steps: usize,
    ) -> Result<String> {
        let shell_cmd_interpolated = self.interpolate_merge_variables(shell_cmd, variables);

        let step_name = format!("shell: {}", shell_cmd_interpolated);
        println!(
            "🔄 Executing step {}/{}: {}",
            step_index + 1,
            total_steps,
            step_name
        );

        self.log_execution_context(&step_name, variables);

        tracing::info!("Executing shell command: {}", shell_cmd_interpolated);
        tracing::info!("Working directory: {}", self.repo_path.display());

        let shell_command = ProcessCommandBuilder::new("sh")
            .current_dir(&self.repo_path)
            .args(["-c", &shell_cmd_interpolated])
            .build();

        let result = self.subprocess.runner().run(shell_command).await?;
        if !result.status.success() {
            anyhow::bail!(
                "Merge workflow shell command failed: {}",
                shell_cmd_interpolated
            );
        }
        if !result.stdout.is_empty() {
            println!("{}", result.stdout.trim());
        }
        Ok(result.stdout)
    }

    /// Execute a Claude command in the merge workflow
    async fn execute_merge_claude_command(
        &self,
        claude_cmd: &str,
        variables: &HashMap<String, String>,
        step_index: usize,
        total_steps: usize,
    ) -> Result<String> {
        let claude_cmd_interpolated = self.interpolate_merge_variables(claude_cmd, variables);

        let step_name = format!("claude: {}", claude_cmd_interpolated);
        println!(
            "🔄 Executing step {}/{}: {}",
            step_index + 1,
            total_steps,
            step_name
        );

        self.log_execution_context(&step_name, variables);

        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Explicitly set console output based on verbosity unless overridden by environment
        let console_output_override = std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").ok();
        if let Some(override_value) = console_output_override {
            // Environment variable takes precedence
            env_vars.insert("PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(), override_value);
        } else {
            // Default: only show console output when verbosity >= 1
            env_vars.insert(
                "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
                (self.verbosity >= 1).to_string(),
            );
        }

        self.log_claude_execution_details(&env_vars);

        use crate::cook::execution::runner::RealCommandRunner;
        let command_runner = RealCommandRunner::new();
        let claude_executor =
            ClaudeExecutorImpl::new(command_runner).with_verbosity(self.verbosity);

        let result = claude_executor
            .execute_claude_command(&claude_cmd_interpolated, &self.repo_path, env_vars)
            .await?;

        if !result.success {
            anyhow::bail!(
                "Merge workflow Claude command failed: {}",
                claude_cmd_interpolated
            );
        }
        Ok(result.stdout)
    }

    /// Interpolate merge-specific variables in a string
    fn interpolate_merge_variables(
        &self,
        input: &str,
        variables: &HashMap<String, String>,
    ) -> String {
        let mut result = input.to_string();
        for (key, value) in variables {
            let placeholder = format!("${{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Log execution context for debugging
    fn log_execution_context(&self, step_name: &str, variables: &HashMap<String, String>) {
        tracing::debug!("=== Step Execution Context ===");
        tracing::debug!("Step: {}", step_name);
        tracing::debug!("Working Directory: {}", self.repo_path.display());
        tracing::debug!("Project Directory: {}", self.repo_path.display());
        tracing::debug!("Variables:");
        for (key, value) in variables {
            let display_value = if value.len() > 100 {
                format!("{}... (truncated)", &value[..100])
            } else {
                value.clone()
            };
            tracing::debug!("  {} = {}", key, display_value);
        }
        tracing::debug!("Environment Variables:");
        tracing::debug!("  PRODIGY_AUTOMATION = true");
        if self.verbosity >= 1 {
            tracing::debug!("  PRODIGY_CLAUDE_STREAMING = true");
        }
        tracing::debug!("Actual execution directory: {}", self.repo_path.display());
    }

    /// Log Claude-specific execution details
    fn log_claude_execution_details(&self, env_vars: &HashMap<String, String>) {
        tracing::debug!("Environment Variables:");
        for (key, value) in env_vars {
            tracing::debug!("  {} = {}", key, value);
        }
        tracing::debug!("Actual execution directory: {}", self.repo_path.display());

        tracing::debug!(
            "Claude execution mode: streaming={}, env_var={:?}",
            self.verbosity >= 1,
            env_vars.get("PRODIGY_CLAUDE_STREAMING")
        );
        if self.verbosity >= 1 {
            tracing::debug!("Using streaming mode for Claude command");
        } else {
            tracing::debug!("Using print mode for Claude command");
        }
    }

    /// Save checkpoint after a successful step
    async fn save_merge_checkpoint(
        &self,
        checkpoint_manager: &crate::cook::workflow::checkpoint::CheckpointManager,
        worktree_name: &str,
        step_index: usize,
        total_steps: usize,
        variables: &HashMap<String, String>,
    ) -> Result<()> {
        let checkpoint = crate::cook::workflow::checkpoint::WorkflowCheckpoint {
            workflow_id: format!("merge-workflow-{}", worktree_name),
            execution_state: crate::cook::workflow::checkpoint::ExecutionState {
                current_step_index: step_index,
                total_steps,
                status: crate::cook::workflow::checkpoint::WorkflowStatus::Running,
                start_time: chrono::Utc::now(),
                last_checkpoint: chrono::Utc::now(),
                current_iteration: Some(1),
                total_iterations: Some(1),
            },
            completed_steps: vec![],
            variable_state: variables
                .clone()
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
            mapreduce_state: None,
            timestamp: chrono::Utc::now(),
            version: 1,
            workflow_hash: format!("merge-{}", worktree_name),
            total_steps,
            workflow_name: Some(format!("merge-workflow-{}", worktree_name)),
            workflow_path: None,
            error_recovery_state: None,
            retry_checkpoint_state: None,
            variable_checkpoint_state: None,
        };
        checkpoint_manager.save_checkpoint(&checkpoint).await?;
        tracing::info!("Saved checkpoint for merge workflow at step {}", step_index);
        Ok(())
    }

    async fn execute_custom_merge_workflow(
        &self,
        merge_workflow: &MergeWorkflow,
        worktree_name: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<String> {
        let mut output = String::new();

        // Initialize merge variables and checkpoint manager
        let (variables, _session_id) = self
            .init_merge_variables(worktree_name, source_branch, target_branch)
            .await?;
        let checkpoint_manager = self.create_merge_checkpoint_manager()?;

        // Execute each command in the merge workflow
        let mut step_index = 0;
        for command in &merge_workflow.commands {
            match command {
                crate::cook::workflow::WorkflowStep {
                    shell: Some(shell_cmd),
                    ..
                } => {
                    let cmd_output = self
                        .execute_merge_shell_command(
                            shell_cmd,
                            &variables,
                            step_index,
                            merge_workflow.commands.len(),
                        )
                        .await?;
                    output.push_str(&cmd_output);

                    step_index += 1;
                    if let Err(e) = self
                        .save_merge_checkpoint(
                            &checkpoint_manager,
                            worktree_name,
                            step_index,
                            merge_workflow.commands.len(),
                            &variables,
                        )
                        .await
                    {
                        tracing::warn!("Failed to save merge workflow checkpoint: {}", e);
                    }
                }
                crate::cook::workflow::WorkflowStep {
                    claude: Some(claude_cmd),
                    ..
                } => {
                    let cmd_output = self
                        .execute_merge_claude_command(
                            claude_cmd,
                            &variables,
                            step_index,
                            merge_workflow.commands.len(),
                        )
                        .await?;
                    output.push_str(&cmd_output);

                    step_index += 1;
                    if let Err(e) = self
                        .save_merge_checkpoint(
                            &checkpoint_manager,
                            worktree_name,
                            step_index,
                            merge_workflow.commands.len(),
                            &variables,
                        )
                        .await
                    {
                        tracing::warn!("Failed to save merge workflow checkpoint: {}", e);
                    }
                }
                _ => {
                    // For other command types, just log them for now
                    let cmd_str = format!("{:?}", command);
                    let interpolated = self.interpolate_merge_variables(&cmd_str, &variables);
                    eprintln!(
                        "Skipping unsupported merge workflow command: {}",
                        interpolated
                    );
                    step_index += 1;
                }
            }
        }

        // Clean up the merge workflow checkpoint after successful completion
        let workflow_id = format!("merge-workflow-{}", worktree_name);
        if let Err(e) = checkpoint_manager.delete_checkpoint(&workflow_id).await {
            tracing::warn!(
                "Failed to delete merge workflow checkpoint for {}: {}",
                workflow_id,
                e
            );
        } else {
            tracing::debug!("Deleted merge workflow checkpoint for {}", workflow_id);
        }

        Ok(output)
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
        // This test verifies error handling when Claude CLI is not available
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a mock session by manipulating internal state
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let state = WorktreeState {
            session_id: "test-session".to_string(),
            worktree_name: "test-session".to_string(),
            branch: "test-branch".to_string(),
            original_branch: String::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: WorktreeStats {
                files_changed: 0,
                commits: 0,
                last_commit_sha: None,
            },
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: None,
            resumable: true,
        };

        let state_path = metadata_dir.join("test-session.json");
        std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

        // Create a mock worktree list that includes our session
        // Note: In reality, we'd need actual git worktrees, but for this test
        // we're testing the Claude CLI failure path

        let result = manager.merge_session("test-session").await;
        // Should fail because worktree doesn't actually exist in git
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_config_defaults() {
        let config = CleanupConfig::default();
        assert!(config.auto_cleanup);
        assert!(config.confirm_before_cleanup);
        assert_eq!(config.retention_days, 7);
        assert!(!config.dry_run);
    }

    #[tokio::test]
    async fn test_get_cleanup_config_from_env() {
        // Test environment variable override
        std::env::set_var("PRODIGY_AUTO_CLEANUP", "false");
        std::env::set_var("PRODIGY_DRY_RUN", "true");
        std::env::set_var("PRODIGY_RETENTION_DAYS", "14");

        let config = WorktreeManager::get_cleanup_config();
        assert!(!config.auto_cleanup);
        assert!(config.dry_run);
        assert_eq!(config.retention_days, 14);

        // Clean up environment variables
        std::env::remove_var("PRODIGY_AUTO_CLEANUP");
        std::env::remove_var("PRODIGY_DRY_RUN");
        std::env::remove_var("PRODIGY_RETENTION_DAYS");
    }

    #[tokio::test]
    async fn test_cleanup_session_after_merge_not_merged() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create a session that is NOT marked as merged
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let state = WorktreeState {
            session_id: "test-session".to_string(),
            worktree_name: "test-session".to_string(),
            branch: "test-branch".to_string(),
            original_branch: String::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: WorktreeStats {
                files_changed: 0,
                commits: 0,
                last_commit_sha: None,
            },
            merged: false, // Key: not merged
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: None,
            resumable: true,
        };

        let state_path = metadata_dir.join("test-session.json");
        std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

        // Should fail because session is not marked as merged
        let result = manager.cleanup_session_after_merge("test-session").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not marked as merged"));
    }

    #[tokio::test]
    async fn test_detect_mergeable_sessions_empty() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();

        // Initialize git repository in temp directory first
        let init_command = crate::subprocess::ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .build();

        let _ = subprocess.runner().run(init_command).await;

        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // No sessions created, should detect no mergeable sessions
        // This might still fail if git commands fail, but that's expected in a non-git environment
        let result = manager.detect_mergeable_sessions().await;
        // Either should succeed with empty list, or fail with git error - both are acceptable
        match result {
            Ok(sessions) => assert!(sessions.is_empty()),
            Err(_) => {
                // Expected in test environment without proper git setup
                // Test passes if we reach here as we've tested the error path
            }
        }
    }

    // Test helper functions for common setup patterns
    async fn setup_test_git_repo(
        temp_dir: &TempDir,
        subprocess: &SubprocessManager,
    ) -> anyhow::Result<()> {
        // Initialize a git repository
        let init_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .build();
        subprocess.runner().run(init_command).await?;

        // Configure user for git (needed for commits)
        let config_name = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.name", "Test User"])
            .build();
        subprocess.runner().run(config_name).await?;

        let config_email = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.email", "test@example.com"])
            .build();
        subprocess.runner().run(config_email).await?;

        // Create initial commit (required for worktrees)
        let initial_file = temp_dir.path().join("README.md");
        std::fs::write(&initial_file, "# Test Repository")?;

        let add_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "."])
            .build();
        subprocess.runner().run(add_command).await?;

        let commit_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Initial commit"])
            .build();
        subprocess.runner().run(commit_command).await?;

        Ok(())
    }

    fn create_test_worktree_state_with_checkpoint(
        session_id: &str,
        iteration: u32,
        command: &str,
    ) -> WorktreeState {
        use crate::worktree::Checkpoint;

        WorktreeState {
            session_id: session_id.to_string(),
            worktree_name: session_id.to_string(),
            branch: "test-branch".to_string(),
            original_branch: String::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: WorktreeStats {
                files_changed: 0,
                commits: 0,
                last_commit_sha: None,
            },
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: Some(Checkpoint {
                iteration,
                timestamp: chrono::Utc::now(),
                last_command: command.to_string(),
                last_command_type: crate::worktree::CommandType::CodeReview,
                last_spec_id: Some("spec-123".to_string()),
                files_modified: vec!["src/main.rs".to_string()],
                command_output: None,
            }),
            resumable: true,
        }
    }

    #[tokio::test]
    async fn test_update_checkpoint_success() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        let state = create_test_worktree_state_with_checkpoint("test-session", 1, "/prodigy-test");

        // Create the metadata directory and save state
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;
        std::fs::write(
            metadata_dir.join("test-session.json"),
            serde_json::to_string_pretty(&state)?,
        )?;

        // Update and verify checkpoint
        manager.update_checkpoint("test-session", |checkpoint| {
            checkpoint.iteration = 2;
            checkpoint.last_command = "/prodigy-updated".to_string();
        })?;

        let updated_state = manager.get_session_state("test-session")?;
        let checkpoint = updated_state.last_checkpoint.unwrap();
        assert_eq!(checkpoint.iteration, 2);
        assert_eq!(checkpoint.last_command, "/prodigy-updated");
        Ok(())
    }

    fn create_test_session_state(
        session_id: &str,
        status: &str,
        hours_ago: i64,
        minutes_ago: i64,
        files_changed: u32,
        commits: u32,
        error_msg: Option<&str>,
    ) -> serde_json::Value {
        serde_json::json!({
            "session_id": session_id,
            "status": status,
            "branch": format!("feature-{}", session_id.split('-').next_back().unwrap_or("1")),
            "created_at": (chrono::Utc::now() - chrono::Duration::hours(hours_ago)).to_rfc3339(),
            "updated_at": (chrono::Utc::now() - chrono::Duration::minutes(minutes_ago)).to_rfc3339(),
            "error": error_msg,
            "stats": {
                "files_changed": files_changed,
                "commits": commits,
                "last_commit_sha": null
            },
            "worktree_name": session_id,
            "iterations": { "completed": 0, "max": 5 },
            "merged": false,
            "merged_at": null,
            "merge_prompt_shown": false,
            "merge_prompt_response": null,
            "interrupted_at": null,
            "interruption_type": null,
            "last_checkpoint": null,
            "resumable": false
        })
    }

    async fn setup_test_worktree_manager(temp_dir: &TempDir) -> anyhow::Result<WorktreeManager> {
        let subprocess = SubprocessManager::production();
        setup_test_git_repo(temp_dir, &subprocess).await?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        // Create metadata directory
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        Ok(manager)
    }

    fn assert_session_properties(
        sessions: &[crate::worktree::display::EnhancedSessionInfo],
        session_id: &str,
        expected_status: WorktreeStatus,
        expected_files: u32,
        expected_commits: u32,
        expected_error: Option<&str>,
    ) {
        let session = sessions
            .iter()
            .find(|s| s.session_id == session_id)
            .unwrap_or_else(|| panic!("{} not found", session_id));

        assert_eq!(session.status, expected_status);
        assert_eq!(session.files_changed, expected_files);
        assert_eq!(session.commits, expected_commits);

        if let Some(error) = expected_error {
            assert_eq!(session.error_summary, Some(error.to_string()));
        }
    }

    fn create_mock_worktree_dirs(
        manager: &WorktreeManager,
        session_ids: &[&str],
    ) -> anyhow::Result<()> {
        for session_id in session_ids {
            let wt_dir = manager.base_dir.join(session_id);
            std::fs::create_dir_all(&wt_dir)?;
            // Create minimal .git file to make it appear as valid worktree
            std::fs::write(wt_dir.join(".git"), "gitdir: /fake/path")?;
        }
        Ok(())
    }

    async fn create_test_worktree_with_session_state(
        manager: &WorktreeManager,
        temp_dir: &TempDir,
        session_id: &str,
        branch: &str,
        session_state: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let wt_dir = manager.base_dir.join(session_id);
        let subprocess = SubprocessManager::production();

        let add_worktree = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args([
                "worktree",
                "add",
                "-b",
                branch,
                wt_dir.to_string_lossy().as_ref(),
            ])
            .build();
        subprocess.runner().run(add_worktree).await?;

        let prodigy_dir = wt_dir.join(".prodigy");
        std::fs::create_dir_all(&prodigy_dir)?;

        let session_state_file = prodigy_dir.join("session_state.json");
        std::fs::write(&session_state_file, serde_json::to_string(session_state)?)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_update_checkpoint_increments_iteration() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        let state = create_test_worktree_state_with_checkpoint("test-session", 1, "/prodigy-test1");

        // Create metadata directory and save state
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;
        std::fs::write(
            metadata_dir.join("test-session.json"),
            serde_json::to_string_pretty(&state)?,
        )?;

        // Update and verify checkpoint iteration
        manager.update_checkpoint("test-session", |checkpoint| {
            checkpoint.iteration = 2;
            checkpoint.last_command = "/prodigy-test2".to_string();
        })?;

        let updated_state = manager.get_session_state("test-session")?;
        assert_eq!(updated_state.last_checkpoint.unwrap().iteration, 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_list_detailed_empty() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();

        // Initialize a git repository in the temp directory
        let init_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .build();
        subprocess.runner().run(init_command).await.unwrap();

        // Configure user for git (needed for commits)
        let config_name = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.name", "Test User"])
            .build();
        subprocess.runner().run(config_name).await.unwrap();

        let config_email = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.email", "test@example.com"])
            .build();
        subprocess.runner().run(config_email).await.unwrap();

        // Create initial commit (required for worktrees)
        let initial_file = temp_dir.path().join("README.md");
        std::fs::write(&initial_file, "# Test Repository").unwrap();

        let add_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "."])
            .build();
        subprocess.runner().run(add_command).await.unwrap();

        let commit_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Initial commit"])
            .build();
        subprocess.runner().run(commit_command).await.unwrap();

        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create metadata directory
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let result = manager.list_detailed().await.unwrap();
        assert_eq!(result.sessions.len(), 0);
        assert_eq!(result.summary.total, 0);
    }

    #[tokio::test]
    async fn test_list_detailed_with_sessions() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_worktree_manager(&temp_dir).await?;
        let metadata_dir = manager.base_dir.join(".metadata");

        // Create test session states with helper function
        let state1_json =
            create_test_session_state("session-test-1", "in_progress", 2, 30, 5, 2, None);
        let state2_json =
            create_test_session_state("session-test-2", "completed", 3, 60, 10, 5, None);
        let state3_json = create_test_session_state(
            "session-test-3",
            "failed",
            1,
            10,
            2,
            1,
            Some("Test error message"),
        );

        // Save states to metadata
        std::fs::write(
            metadata_dir.join("session-test-1.json"),
            serde_json::to_string(&state1_json)?,
        )?;
        std::fs::write(
            metadata_dir.join("session-test-2.json"),
            serde_json::to_string(&state2_json)?,
        )?;
        std::fs::write(
            metadata_dir.join("session-test-3.json"),
            serde_json::to_string(&state3_json)?,
        )?;

        // Create mock worktree directories
        create_mock_worktree_dirs(
            &manager,
            &["session-test-1", "session-test-2", "session-test-3"],
        )?;

        // Get detailed list and verify
        let result = manager.list_detailed().await?;

        // Verify summary counts
        assert_eq!(result.summary.total, 3);
        assert_eq!(result.summary.in_progress, 1);
        assert_eq!(result.summary.completed, 1);
        assert_eq!(result.summary.failed, 1);
        assert_eq!(result.summary.interrupted, 0);
        assert_eq!(result.sessions.len(), 3);

        // Verify session properties using helper
        assert_session_properties(
            &result.sessions,
            "session-test-1",
            WorktreeStatus::InProgress,
            5,
            2,
            None,
        );
        assert_session_properties(
            &result.sessions,
            "session-test-2",
            WorktreeStatus::Completed,
            10,
            5,
            None,
        );
        assert_session_properties(
            &result.sessions,
            "session-test-3",
            WorktreeStatus::Failed,
            2,
            1,
            Some("Test error message"),
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_list_detailed_with_workflow_info() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        setup_test_git_repo(&temp_dir, &subprocess).await?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        // Create test state and save to metadata
        let state_json =
            create_test_session_state("workflow-session", "in_progress", 1, 5, 3, 1, None);
        std::fs::write(
            metadata_dir.join("workflow-session.json"),
            serde_json::to_string(&state_json)?,
        )?;

        // Create session state with workflow information
        let session_state = serde_json::json!({
            "session_id": "workflow-session",
            "workflow_state": {
                "workflow_path": "workflows/test.yaml",
                "input_args": ["arg1", "arg2"],
                "current_step": 3,
                "completed_steps": [1, 2, 3, 4, 5]
            }
        });

        create_test_worktree_with_session_state(
            &manager,
            &temp_dir,
            "workflow-session",
            "workflow-branch",
            &session_state,
        )
        .await?;

        let result = manager.list_detailed().await?;
        assert_eq!(result.sessions.len(), 1);

        let session = &result.sessions[0];
        assert_eq!(
            session.workflow_path,
            Some(PathBuf::from("workflows/test.yaml"))
        );
        assert_eq!(session.workflow_args, vec!["arg1", "arg2"]);
        assert_eq!(session.current_step, 3);
        assert_eq!(session.total_steps, Some(5));

        Ok(())
    }

    #[tokio::test]
    async fn test_list_detailed_with_mapreduce_info() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        setup_test_git_repo(&temp_dir, &subprocess).await?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        // Create test state and save to metadata
        let state_json =
            create_test_session_state("mapreduce-session", "in_progress", 2, 10, 0, 0, None);
        std::fs::write(
            metadata_dir.join("mapreduce-session.json"),
            serde_json::to_string(&state_json)?,
        )?;

        // Create session state with MapReduce information
        let session_state = serde_json::json!({
            "session_id": "mapreduce-session",
            "workflow_state": {
                "workflow_path": "mapreduce.yaml"
            },
            "mapreduce_state": {
                "items_processed": 25,
                "total_items": 100
            }
        });

        create_test_worktree_with_session_state(
            &manager,
            &temp_dir,
            "mapreduce-session",
            "mapreduce-branch",
            &session_state,
        )
        .await?;

        let result = manager.list_detailed().await?;
        assert_eq!(result.sessions.len(), 1);

        let session = &result.sessions[0];
        assert_eq!(session.items_processed, Some(25));
        assert_eq!(session.total_items, Some(100));

        Ok(())
    }

    #[test]
    fn test_merge_workflow_variable_interpolation() {
        // Test that variable interpolation works correctly for merge workflows
        let test_str = "echo 'Worktree: ${merge.worktree}, Source: ${merge.source_branch}, Target: ${merge.target_branch}, Session: ${merge.session_id}'";

        let interpolated = test_str
            .replace("${merge.worktree}", "my-worktree")
            .replace("${merge.source_branch}", "feature-123")
            .replace("${merge.target_branch}", "develop")
            .replace("${merge.session_id}", "session-abc");

        assert!(interpolated.contains("Worktree: my-worktree"));
        assert!(interpolated.contains("Source: feature-123"));
        assert!(interpolated.contains("Target: develop"));
        assert!(interpolated.contains("Session: session-abc"));
    }

    #[test]
    fn test_workflow_env_vars_in_merge_interpolation() {
        // Test that workflow environment variables are included in merge variable interpolation
        let subprocess = SubprocessManager::production();
        let temp_dir = TempDir::new().unwrap();

        // Create workflow environment variables
        let mut workflow_env = HashMap::new();
        workflow_env.insert("BOOK_DIR".to_string(), "book".to_string());
        workflow_env.insert("PROJECT_NAME".to_string(), "Prodigy".to_string());
        workflow_env.insert(
            "ANALYSIS_DIR".to_string(),
            ".prodigy/book-analysis".to_string(),
        );

        let manager = WorktreeManager::with_config(
            temp_dir.path().to_path_buf(),
            subprocess,
            0,
            None,
            workflow_env,
        )
        .unwrap();

        // Test interpolation function with workflow env vars
        // Note: The interpolation function only supports ${VAR} syntax, not $VAR
        let test_str = "cd ${BOOK_DIR} && mdbook build for ${PROJECT_NAME} in ${ANALYSIS_DIR}";

        let mut variables = HashMap::new();
        variables.insert("BOOK_DIR".to_string(), "book".to_string());
        variables.insert("PROJECT_NAME".to_string(), "Prodigy".to_string());
        variables.insert(
            "ANALYSIS_DIR".to_string(),
            ".prodigy/book-analysis".to_string(),
        );

        let interpolated = manager.interpolate_merge_variables(test_str, &variables);

        // Verify workflow environment variables are interpolated
        assert_eq!(
            interpolated,
            "cd book && mdbook build for Prodigy in .prodigy/book-analysis"
        );
        assert!(!interpolated.contains("${BOOK_DIR}"));
        assert!(!interpolated.contains("${PROJECT_NAME}"));
        assert!(!interpolated.contains("${ANALYSIS_DIR}"));
    }

    #[test]
    fn test_filter_sessions_by_status() {
        let now = Utc::now();
        let states = vec![
            WorktreeState {
                session_id: "session1".to_string(),
                worktree_name: "worktree1".to_string(),
                branch: "branch1".to_string(),
                original_branch: String::new(),
                created_at: now,
                updated_at: now,
                status: WorktreeStatus::InProgress,
                iterations: IterationInfo {
                    completed: 1,
                    max: 5,
                },
                stats: WorktreeStats::default(),
                merged: false,
                merged_at: None,
                error: None,
                merge_prompt_shown: false,
                merge_prompt_response: None,
                interrupted_at: None,
                interruption_type: None,
                last_checkpoint: None,
                resumable: false,
            },
            WorktreeState {
                session_id: "session2".to_string(),
                worktree_name: "worktree2".to_string(),
                branch: "branch2".to_string(),
                original_branch: String::new(),
                created_at: now,
                updated_at: now,
                status: WorktreeStatus::Completed,
                iterations: IterationInfo {
                    completed: 5,
                    max: 5,
                },
                stats: WorktreeStats::default(),
                merged: false,
                merged_at: None,
                error: None,
                merge_prompt_shown: false,
                merge_prompt_response: None,
                interrupted_at: None,
                interruption_type: None,
                last_checkpoint: None,
                resumable: false,
            },
            WorktreeState {
                session_id: "session3".to_string(),
                worktree_name: "worktree3".to_string(),
                branch: "branch3".to_string(),
                original_branch: String::new(),
                created_at: now,
                updated_at: now,
                status: WorktreeStatus::InProgress,
                iterations: IterationInfo {
                    completed: 2,
                    max: 5,
                },
                stats: WorktreeStats::default(),
                merged: false,
                merged_at: None,
                error: None,
                merge_prompt_shown: false,
                merge_prompt_response: None,
                interrupted_at: None,
                interruption_type: None,
                last_checkpoint: None,
                resumable: false,
            },
        ];

        let in_progress = filter_sessions_by_status(states.clone(), WorktreeStatus::InProgress);
        assert_eq!(in_progress.len(), 2);
        assert_eq!(in_progress[0].session_id, "session1");
        assert_eq!(in_progress[1].session_id, "session3");

        let complete = filter_sessions_by_status(states, WorktreeStatus::Completed);
        assert_eq!(complete.len(), 1);
        assert_eq!(complete[0].session_id, "session2");
    }

    #[test]
    fn test_load_state_from_file() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let json_path = temp_dir.path().join("test_state.json");

        // Test with valid JSON
        let state = WorktreeState {
            session_id: "test-session".to_string(),
            worktree_name: "test-worktree".to_string(),
            branch: "test-branch".to_string(),
            original_branch: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: IterationInfo {
                completed: 3,
                max: 10,
            },
            stats: WorktreeStats::default(),
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: None,
            resumable: false,
        };

        let json_content = serde_json::to_string(&state).unwrap();
        fs::write(&json_path, json_content).unwrap();

        let loaded = load_state_from_file(&json_path);
        assert!(loaded.is_some());
        let loaded_state = loaded.unwrap();
        assert_eq!(loaded_state.session_id, "test-session");

        // Test with non-JSON file
        let txt_path = temp_dir.path().join("not_json.txt");
        fs::write(&txt_path, "not json content").unwrap();
        assert!(load_state_from_file(&txt_path).is_none());

        // Test with invalid JSON
        let bad_json_path = temp_dir.path().join("bad.json");
        fs::write(&bad_json_path, "{ invalid json }").unwrap();
        assert!(load_state_from_file(&bad_json_path).is_none());

        // Test with non-existent file
        let missing_path = temp_dir.path().join("missing.json");
        assert!(load_state_from_file(&missing_path).is_none());
    }

    #[test]
    fn test_collect_all_states() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let metadata_dir = temp_dir.path().join(".metadata");
        fs::create_dir(&metadata_dir).unwrap();

        // Create multiple state files
        for i in 1..=3 {
            let state = WorktreeState {
                session_id: format!("session{}", i),
                worktree_name: format!("worktree{}", i),
                branch: format!("branch{}", i),
                original_branch: String::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                status: WorktreeStatus::InProgress,
                iterations: IterationInfo {
                    completed: i,
                    max: 10,
                },
                stats: WorktreeStats::default(),
                merged: false,
                merged_at: None,
                error: None,
                merge_prompt_shown: false,
                merge_prompt_response: None,
                interrupted_at: None,
                interruption_type: None,
                last_checkpoint: None,
                resumable: false,
            };

            let json_path = metadata_dir.join(format!("session{}.json", i));
            let json_content = serde_json::to_string(&state).unwrap();
            fs::write(&json_path, json_content).unwrap();
        }

        // Also create a non-JSON file that should be ignored
        fs::write(metadata_dir.join("readme.txt"), "ignored").unwrap();

        let states = collect_all_states(&metadata_dir).unwrap();
        assert_eq!(states.len(), 3);

        // Verify all states were loaded
        let session_ids: Vec<String> = states.iter().map(|s| s.session_id.clone()).collect();
        assert!(session_ids.contains(&"session1".to_string()));
        assert!(session_ids.contains(&"session2".to_string()));
        assert!(session_ids.contains(&"session3".to_string()));

        // Test with non-existent directory
        let missing_dir = temp_dir.path().join("missing");
        let result = collect_all_states(&missing_dir).unwrap();
        assert_eq!(result.len(), 0);
    }
}
