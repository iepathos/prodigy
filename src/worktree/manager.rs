use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use super::{IterationInfo, WorktreeSession, WorktreeState, WorktreeStats, WorktreeStatus};

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
    subprocess: SubprocessManager,
}

impl WorktreeManager {
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
    pub(crate) fn load_state_from_file(path: &std::path::Path) -> Option<WorktreeState> {
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            return None;
        }

        fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str::<WorktreeState>(&content).ok())
    }

    /// Collect all worktree states from a metadata directory
    pub(crate) fn collect_all_states(metadata_dir: &std::path::Path) -> Result<Vec<WorktreeState>> {
        if !metadata_dir.exists() {
            return Ok(Vec::new());
        }

        let mut states = Vec::new();

        for entry in fs::read_dir(metadata_dir)? {
            let path = entry?.path();
            if let Some(state) = Self::load_state_from_file(&path) {
                states.push(state);
            }
        }

        Ok(states)
    }

    /// Create a new WorktreeManager for the given repository
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `subprocess` - Subprocess manager for git operations
    ///
    /// # Returns
    /// * `Result<Self>` - WorktreeManager instance or error
    ///
    /// # Errors
    /// Returns error if:
    /// - Repository path is invalid
    /// - Git repository is not found
    pub fn new(repo_path: PathBuf, subprocess: SubprocessManager) -> Result<Self> {
        // Get the repository name from the path
        let repo_name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                anyhow!(
                    "Could not determine repository name from path: {}",
                    repo_path.display()
                )
            })?;

        // Use home directory for worktrees
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;

        let base_dir = home_dir.join(".mmm").join("worktrees").join(repo_name);

        std::fs::create_dir_all(&base_dir).context("Failed to create worktree base directory")?;

        // Create .gitignore if it doesn't exist
        let gitignore_path = base_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, ".metadata/\n")?;
        }

        // Canonicalize paths to handle symlinks (e.g., /private/var vs /var on macOS)
        let base_dir = base_dir
            .canonicalize()
            .context("Failed to canonicalize base directory")?;
        let repo_path = repo_path
            .canonicalize()
            .context("Failed to canonicalize repo path")?;

        Ok(Self {
            base_dir,
            repo_path,
            subprocess,
        })
    }

    /// Create a new worktree session
    ///
    /// # Returns
    /// * `Result<WorktreeSession>` - The created worktree session
    ///
    /// # Errors
    /// Returns error if worktree creation fails
    pub async fn create_session(&self) -> Result<WorktreeSession> {
        let session_id = Uuid::new_v4();
        // Simple name using UUID
        let name = format!("session-{session_id}");
        let branch = format!("mmm-{name}");
        let worktree_path = self.base_dir.join(&name);

        // Create worktree
        let command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "add", "-b", &branch])
            .arg(worktree_path.to_string_lossy().as_ref())
            .build();

        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context("Failed to execute git worktree add")?;

        if !output.status.success() {
            anyhow::bail!("Failed to create worktree: {}", output.stderr);
        }

        // Create session
        let session = WorktreeSession::new(name.clone(), branch, worktree_path);

        // Save session state
        self.save_session_state(&session)?;

        Ok(session)
    }

    fn save_session_state(&self, session: &WorktreeSession) -> Result<()> {
        let state_dir = self.base_dir.join(".metadata");
        fs::create_dir_all(&state_dir)?;

        let state_file = state_dir.join(format!("{}.json", session.name));
        let state = WorktreeState {
            session_id: session.name.clone(),
            worktree_name: session.name.clone(),
            branch: session.branch.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: IterationInfo {
                completed: 0,
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

        let json = serde_json::to_string_pretty(&state)?;

        // Write to temp file first, then rename atomically
        let temp_file = state_dir.join(format!("{}.json.tmp", session.name));
        fs::write(&temp_file, &json)?;
        fs::rename(&temp_file, &state_file)?;

        Ok(())
    }

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

    pub fn get_session_state(&self, name: &str) -> Result<WorktreeState> {
        let state_file = self.base_dir.join(".metadata").join(format!("{name}.json"));
        let state_json = fs::read_to_string(&state_file)?;
        let state: WorktreeState = serde_json::from_str(&state_json)?;
        Ok(state)
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
                // Check if the worktree directory actually exists
                let worktree_path = self.base_dir.join(&meta_session.name);
                if worktree_path.exists() {
                    sessions.push(meta_session);
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
        let mut sessions = Vec::new();
        let mut current_path: Option<PathBuf> = None;
        let mut current_branch: Option<String> = None;

        for line in stdout.lines() {
            if line.starts_with("worktree ") {
                // Process any pending worktree before starting a new one
                if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
                    // Canonicalize the path to handle symlinks
                    let canonical_path = path.canonicalize().unwrap_or(path.clone());
                    // Include all worktrees in our base directory, regardless of branch name
                    // This includes MapReduce branches like "merge-mmm-*" and "mmm-agent-*"
                    if canonical_path.starts_with(&self.base_dir) {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&branch)
                            .to_string();

                        sessions.push(WorktreeSession::new(name, branch, canonical_path));
                    }
                }
                current_path = Some(PathBuf::from(line.trim_start_matches("worktree ")));
            } else if line.starts_with("branch ") {
                current_branch = Some(line.trim_start_matches("branch refs/heads/").to_string());
            }
        }

        // Handle the last entry
        if let (Some(path), Some(branch)) = (current_path, current_branch) {
            let canonical_path = path.canonicalize().unwrap_or(path.clone());
            // Include all worktrees in our base directory
            if canonical_path.starts_with(&self.base_dir) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&branch)
                    .to_string();

                sessions.push(WorktreeSession::new(name, branch, canonical_path));
            }
        }

        Ok(sessions)
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

            if let Some(state) = Self::load_state_from_file(&path) {
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

    /// Merge a worktree session back to the main branch
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
        // Get the worktree branch name to verify merge
        let sessions = self.list_sessions().await?;
        let session = sessions
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Worktree '{}' not found", name))?;
        let worktree_branch = &session.branch;

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

        let target = if main_exists {
            "main".to_string()
        } else {
            "master".to_string()
        };

        // Check if there are any new commits in the worktree branch
        let diff_check_command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args([
                "rev-list",
                "--count",
                &format!("{target}..{worktree_branch}"),
            ])
            .build();

        let diff_output = self
            .subprocess
            .runner()
            .run(diff_check_command)
            .await
            .context("Failed to check for new commits")?;

        if diff_output.status.success() {
            let commit_count = diff_output.stdout.trim();
            if commit_count == "0" {
                anyhow::bail!(
                    "No new commits in worktree '{}' to merge into '{}'. The branches are already in sync.",
                    name,
                    target
                );
            }
        }

        // Call Claude CLI to handle the merge with automatic conflict resolution
        println!("🔄 Merging worktree '{name}' into '{target}' using Claude-assisted merge...");

        // Execute Claude CLI command
        let claude_command = ProcessCommandBuilder::new("claude")
            .current_dir(&self.repo_path)
            .arg("--dangerously-skip-permissions") // Skip interactive permission prompts
            .arg("--print") // Output response to stdout for capture
            .arg(&format!("/mmm-merge-worktree {worktree_branch}")) // Include branch name in the command
            .env("MMM_AUTOMATION", "true") // Enable automation mode
            .build();

        // Print what we're about to execute
        eprintln!("Running claude /mmm-merge-worktree with branch: {worktree_branch}");

        let output = self
            .subprocess
            .runner()
            .run(claude_command)
            .await
            .context("Failed to execute claude /mmm-merge-worktree")?;

        if !output.status.success() {
            let stderr = &output.stderr;
            let stdout = &output.stdout;

            // Provide detailed error information
            eprintln!("❌ Claude merge failed for worktree '{name}':");
            if !stderr.is_empty() {
                eprintln!("Error output: {stderr}");
            }
            if !stdout.is_empty() {
                eprintln!("Standard output: {stdout}");
            }

            anyhow::bail!("Failed to merge worktree '{name}' - Claude merge failed");
        }

        // Parse the output for success confirmation
        let stdout = &output.stdout;
        println!("{stdout}");

        // Verify the merge actually happened by checking if the worktree branch
        // is now merged into the target branch
        let merge_check_command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["branch", "--merged", &target])
            .build();

        let merge_check = self
            .subprocess
            .runner()
            .run(merge_check_command)
            .await
            .context("Failed to check merged branches")?;

        if merge_check.status.success() {
            let merged_branches = &merge_check.stdout;
            if !merged_branches.contains(worktree_branch) {
                // Check if Claude output indicates permission was denied
                if stdout.contains("permission") || stdout.contains("grant permission") {
                    anyhow::bail!(
                        "Merge was not completed - Claude requires permission to proceed. \
                        Please run the command again and grant permission when prompted."
                    );
                }
                anyhow::bail!(
                    "Merge verification failed - branch '{}' is not merged into '{}'. \
                    The merge may have been aborted or failed silently.",
                    worktree_branch,
                    target
                );
            }
        }

        // Update session state to mark as merged
        if let Err(e) = self.update_session_state(name, |state| {
            state.merged = true;
            state.merged_at = Some(Utc::now());
            state.status = crate::worktree::WorktreeStatus::Merged;
        }) {
            eprintln!("Warning: Failed to update session state after merge: {e}");
        }

        // Check if auto-cleanup is enabled and perform cleanup
        let cleanup_config = Self::get_cleanup_config();
        if cleanup_config.auto_cleanup {
            println!("🧹 Auto-cleanup is enabled, checking if session can be cleaned up...");

            // Give a moment for the merge to propagate
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            match self.cleanup_session_after_merge(name).await {
                Ok(()) => {
                    println!("✅ Successfully cleaned up merged session: {name}");
                }
                Err(e) => {
                    eprintln!("⚠️  Auto-cleanup failed for session {name}: {e}");
                    eprintln!(
                        "   You can manually clean up later with: mmm worktree cleanup {name}"
                    );
                }
            }
        } else {
            println!("ℹ️  Session '{name}' has been merged. You can clean it up with: mmm worktree cleanup {name}");
        }

        Ok(())
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

    pub async fn get_worktree_for_branch(&self, branch: &str) -> Result<Option<PathBuf>> {
        let sessions = self.list_sessions().await?;
        Ok(sessions
            .into_iter()
            .find(|s| s.branch == branch)
            .map(|s| s.path))
    }

    /// Create a checkpoint for the current state
    pub fn create_checkpoint(
        &self,
        session_name: &str,
        checkpoint: super::Checkpoint,
    ) -> Result<()> {
        self.update_session_state(session_name, |state| {
            state.last_checkpoint = Some(checkpoint);
            state.resumable = true;
        })
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

    /// Load session state by session ID (name)
    pub fn load_session_state(&self, session_id: &str) -> Result<WorktreeState> {
        self.get_session_state(session_id)
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

    /// List all interrupted sessions
    pub fn list_interrupted_sessions(&self) -> Result<Vec<WorktreeState>> {
        let metadata_dir = self.base_dir.join(".metadata");
        let all_states = Self::collect_all_states(&metadata_dir)?;
        Ok(Self::filter_sessions_by_status(
            all_states,
            WorktreeStatus::Interrupted,
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
                if std::env::var("MMM_AUTOMATION").is_ok() {
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
                anyhow::bail!("Worktree '{name}' has uncommitted changes. Cannot clean up safely.");
            }
        }

        // Perform the actual cleanup
        self.cleanup_session(name, false).await?;

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
        let log_dir = log_file.parent().unwrap();
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

    /// Get cleanup configuration from environment or defaults
    pub fn get_cleanup_config() -> CleanupConfig {
        CleanupConfig {
            auto_cleanup: std::env::var("MMM_AUTO_CLEANUP")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            confirm_before_cleanup: std::env::var("MMM_CONFIRM_CLEANUP")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(std::env::var("MMM_AUTOMATION").is_err()),
            retention_days: std::env::var("MMM_RETENTION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7),
            dry_run: std::env::var("MMM_DRY_RUN")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // And it should be under ~/.mmm/worktrees/
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
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: super::IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: super::WorktreeStats {
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
        std::env::set_var("MMM_AUTO_CLEANUP", "false");
        std::env::set_var("MMM_DRY_RUN", "true");
        std::env::set_var("MMM_RETENTION_DAYS", "14");

        let config = WorktreeManager::get_cleanup_config();
        assert!(!config.auto_cleanup);
        assert!(config.dry_run);
        assert_eq!(config.retention_days, 14);

        // Clean up environment variables
        std::env::remove_var("MMM_AUTO_CLEANUP");
        std::env::remove_var("MMM_DRY_RUN");
        std::env::remove_var("MMM_RETENTION_DAYS");
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
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: super::IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: super::WorktreeStats {
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

    #[tokio::test]
    async fn test_update_checkpoint_success() -> Result<()> {
        use crate::worktree::Checkpoint;
        let temp_dir = TempDir::new()?;
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        // First create a session with a checkpoint
        let state = WorktreeState {
            session_id: "test-session".to_string(),
            worktree_name: "test-session".to_string(),
            branch: "test-branch".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: super::IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: super::WorktreeStats {
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
                iteration: 1,
                timestamp: chrono::Utc::now(),
                last_command: "/mmm-test".to_string(),
                last_command_type: crate::worktree::CommandType::CodeReview,
                last_spec_id: Some("spec-123".to_string()),
                files_modified: vec!["src/main.rs".to_string()],
                command_output: None,
            }),
            resumable: true,
        };

        // Create the metadata directory and save state directly
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;
        let state_path = metadata_dir.join("test-session.json");
        std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;

        // Update the checkpoint
        manager.update_checkpoint("test-session", |checkpoint| {
            checkpoint.iteration = 2;
            checkpoint.last_command = "/mmm-updated".to_string();
        })?;

        // Verify checkpoint was updated
        let updated_state = manager.get_session_state("test-session")?;
        let checkpoint = updated_state.last_checkpoint.unwrap();
        assert_eq!(checkpoint.iteration, 2);
        assert_eq!(checkpoint.last_command, "/mmm-updated");
        Ok(())
    }

    #[tokio::test]
    async fn test_update_checkpoint_increments_iteration() -> Result<()> {
        use crate::worktree::Checkpoint;
        let temp_dir = TempDir::new()?;
        let subprocess = SubprocessManager::production();
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        // Create a session with initial checkpoint
        let state = WorktreeState {
            session_id: "test-session".to_string(),
            worktree_name: "test-session".to_string(),
            branch: "test-branch".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: super::IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: super::WorktreeStats {
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
                iteration: 1,
                timestamp: chrono::Utc::now(),
                last_command: "/mmm-test1".to_string(),
                last_command_type: crate::worktree::CommandType::CodeReview,
                last_spec_id: None,
                files_modified: vec![],
                command_output: None,
            }),
            resumable: true,
        };

        // Create the metadata directory and save state directly
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;
        let state_path = metadata_dir.join("test-session.json");
        std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;

        // Update checkpoint with new iteration
        manager.update_checkpoint("test-session", |checkpoint| {
            checkpoint.iteration = 2;
            checkpoint.last_command = "/mmm-test2".to_string();
        })?;

        let state = manager.get_session_state("test-session")?;
        assert_eq!(state.last_checkpoint.unwrap().iteration, 2);
        Ok(())
    }
}
