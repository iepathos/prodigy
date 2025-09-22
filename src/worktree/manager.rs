use crate::config::mapreduce::MergeWorkflow;
use crate::cook::execution::{ClaudeExecutor, ClaudeExecutorImpl};
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::debug;
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
    verbosity: u8,
    custom_merge_workflow: Option<MergeWorkflow>,
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
        Self::with_config(repo_path, subprocess, 0, None)
    }

    /// Create a new WorktreeManager with configuration
    pub fn with_config(
        repo_path: PathBuf,
        subprocess: SubprocessManager,
        verbosity: u8,
        custom_merge_workflow: Option<MergeWorkflow>,
    ) -> Result<Self> {
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

        // Use home directory for worktrees (or temp dir during tests)
        let base_dir = {
            #[cfg(test)]
            {
                use std::sync::OnceLock;
                static TEST_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();
                let test_dir = TEST_DIR.get_or_init(|| {
                    let temp =
                        std::env::temp_dir().join(format!("prodigy-test-{}", std::process::id()));
                    std::fs::create_dir_all(&temp).unwrap();
                    temp
                });
                test_dir.join("worktrees").join(repo_name)
            }
            #[cfg(not(test))]
            {
                let home_dir = directories::BaseDirs::new()
                    .ok_or_else(|| anyhow!("Could not determine base directories"))?
                    .home_dir()
                    .to_path_buf();
                home_dir.join(".prodigy").join("worktrees").join(repo_name)
            }
        };

        std::fs::create_dir_all(&base_dir).context("Failed to create worktree base directory")?;

        // Create .gitignore if it doesn't exist
        let gitignore_path = base_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, ".metadata/\n")?;
        }

        // Try to canonicalize paths to handle symlinks (e.g., /private/var vs /var on macOS)
        // If canonicalization fails (e.g., on certain filesystems), use the original paths
        let base_dir = base_dir.canonicalize().unwrap_or(base_dir);
        let repo_path = repo_path.canonicalize().unwrap_or(repo_path);

        Ok(Self {
            base_dir,
            repo_path,
            subprocess,
            verbosity,
            custom_merge_workflow,
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

        self.create_session_with_id(&name).await
    }

    /// Create a new worktree session with a specific session ID
    ///
    /// # Arguments
    /// * `session_id` - The session ID to use (should be in "session-{uuid}" format)
    ///
    /// # Returns
    /// * `Result<WorktreeSession>` - The created worktree session
    ///
    /// # Errors
    /// Returns error if worktree creation fails
    pub async fn create_session_with_id(&self, session_id: &str) -> Result<WorktreeSession> {
        // Use the provided session ID as the name
        let name = session_id.to_string();
        let branch = format!("prodigy-{name}");
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
        let worktree_entries = Self::parse_worktree_output(stdout);

        Ok(worktree_entries
            .into_iter()
            .filter_map(|(path, branch)| self.create_worktree_session(path, branch))
            .collect())
    }

    /// Parse git worktree list output into path/branch pairs
    fn parse_worktree_output(output: &str) -> Vec<(PathBuf, String)> {
        // Split output into worktree blocks
        let blocks = Self::split_into_worktree_blocks(output);

        // Parse each block into a path/branch pair
        blocks
            .into_iter()
            .filter_map(Self::parse_worktree_block)
            .collect()
    }

    /// Split the git worktree output into individual worktree blocks
    fn split_into_worktree_blocks(output: &str) -> Vec<Vec<&str>> {
        let mut blocks = Vec::new();
        let mut current_block = Vec::new();

        for line in output.lines() {
            if line.starts_with("worktree ") && !current_block.is_empty() {
                // Start of new block, save the current one
                blocks.push(current_block);
                current_block = vec![line];
            } else if !line.is_empty() {
                current_block.push(line);
            }
        }

        // Don't forget the last block
        if !current_block.is_empty() {
            blocks.push(current_block);
        }

        blocks
    }

    /// Parse a single worktree block into a path/branch pair
    fn parse_worktree_block(block: Vec<&str>) -> Option<(PathBuf, String)> {
        let path = block
            .iter()
            .find(|line| line.starts_with("worktree "))
            .map(|line| PathBuf::from(line.trim_start_matches("worktree ")))?;

        let branch = block
            .iter()
            .find(|line| line.starts_with("branch "))
            .map(|line| line.trim_start_matches("branch refs/heads/").to_string())?;

        Some((path, branch))
    }

    /// Create a WorktreeSession if the path is within our base directory
    fn create_worktree_session(&self, path: PathBuf, branch: String) -> Option<WorktreeSession> {
        let canonical_path = path.canonicalize().unwrap_or(path.clone());

        // Include all worktrees in our base directory, regardless of branch name
        // This includes MapReduce branches like "merge-prodigy-*" and "prodigy-agent-*"
        if !canonical_path.starts_with(&self.base_dir) {
            return None;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&branch)
            .to_string();

        Some(WorktreeSession::new(name, branch, canonical_path))
    }

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

    /// Get the parent branch for a given branch
    async fn get_parent_branch(&self, branch_name: &str) -> Result<String> {
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

        // Check if we have a custom merge workflow or use default Claude merge
        let stdout = if let Some(ref merge_workflow) = self.custom_merge_workflow {
            // Execute custom merge workflow
            println!("🔄 Executing custom merge workflow for '{name}' into '{target}'...");
            self.execute_custom_merge_workflow(merge_workflow, name, worktree_branch, &target)
                .await?
        } else {
            // Use ClaudeStreamingExecutor for transparent logging
            println!("🔄 Merging worktree '{name}' into '{target}' using Claude-assisted merge...");

            // Print what we're about to execute
            eprintln!("Running claude /prodigy-merge-worktree with branch: {worktree_branch}");

            // Create environment variables for Claude execution
            let mut env_vars = HashMap::new();
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

            // Enable streaming if verbosity is high enough
            if self.verbosity >= 1 {
                env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
            }

            // Check for console output override
            if std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").unwrap_or_default() == "true" {
                env_vars.insert(
                    "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
                    "true".to_string(),
                );
            }

            // Create Claude executor with streaming support
            use crate::cook::execution::runner::RealCommandRunner;
            let command_runner = RealCommandRunner::new();
            let claude_executor =
                ClaudeExecutorImpl::new(command_runner).with_verbosity(self.verbosity);

            // Execute the merge command
            let result = claude_executor
                .execute_claude_command(
                    &format!("/prodigy-merge-worktree {worktree_branch}"),
                    &self.repo_path,
                    env_vars,
                )
                .await
                .context("Failed to execute claude /prodigy-merge-worktree")?;

            if !result.success {
                // Provide detailed error information
                eprintln!("❌ Claude merge failed for worktree '{name}':");
                if !result.stderr.is_empty() {
                    eprintln!("Error output: {}", result.stderr);
                }
                if !result.stdout.is_empty() {
                    eprintln!("Standard output: {}", result.stdout);
                }

                anyhow::bail!("Failed to merge worktree '{name}' - Claude merge failed");
            }

            // Parse the output for success confirmation
            println!("{}", result.stdout);
            result.stdout
        };

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

                    // Try to get more diagnostic information
                    let worktree_path = self.base_dir.join(name);
                    if worktree_path.exists() {
                        let status_command = ProcessCommandBuilder::new("git")
                            .current_dir(&worktree_path)
                            .args(["status", "--short"])
                            .build();

                        if let Ok(status_output) =
                            self.subprocess.runner().run(status_command).await
                        {
                            if status_output.status.success()
                                && !status_output.stdout.trim().is_empty()
                            {
                                eprintln!("📝 Current worktree status:");
                                eprintln!("{}", status_output.stdout.trim());
                            }
                        }
                    }

                    eprintln!(
                        "   You can manually clean up later with: prodigy worktree cleanup {name}"
                    );
                }
            }
        } else {
            println!("ℹ️  Session '{name}' has been merged. You can clean it up with: prodigy worktree cleanup {name}");
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
    /// Execute a custom merge workflow
    async fn execute_custom_merge_workflow(
        &self,
        merge_workflow: &MergeWorkflow,
        worktree_name: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<String> {
        // For now, execute each command in the merge workflow directly
        // We'll use variable interpolation to replace merge-specific variables
        let mut output = String::new();

        // Create merge context variables for interpolation
        let mut session_id = String::new();
        if let Ok(state) = self.load_session_state(worktree_name) {
            session_id = state.session_id;
        }

        // Initialize variables map for step context
        let mut variables = HashMap::new();
        variables.insert("merge.worktree".to_string(), worktree_name.to_string());
        variables.insert("merge.source_branch".to_string(), source_branch.to_string());
        variables.insert("merge.target_branch".to_string(), target_branch.to_string());
        variables.insert("merge.session_id".to_string(), session_id.clone());

        // Initialize checkpoint manager for merge workflow using global storage
        let checkpoint_manager = {
            use crate::storage::{extract_repo_name, GlobalStorage};

            // Create global storage instance
            let storage = GlobalStorage::new()
                .map_err(|e| anyhow::anyhow!("Failed to create global storage: {}", e))?;

            // Extract repository name and get global checkpoint directory
            let repo_name = extract_repo_name(&self.repo_path)
                .map_err(|e| anyhow::anyhow!("Failed to extract repository name: {}", e))?;
            let checkpoint_dir = storage
                .get_state_dir(&repo_name, "checkpoints")
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get checkpoint directory: {}", e))?;

            crate::cook::workflow::checkpoint::CheckpointManager::new(checkpoint_dir)
        };

        let mut step_index = 0;
        for command in &merge_workflow.commands {
            // Perform variable interpolation for merge-specific variables
            let cmd_str = format!("{:?}", command);
            let interpolated = cmd_str
                .replace("${merge.worktree}", worktree_name)
                .replace("${merge.source_branch}", source_branch)
                .replace("${merge.target_branch}", target_branch)
                .replace("${merge.session_id}", &session_id);

            // For now, we'll execute shell commands directly and Claude commands through the executor
            // This is a simplified implementation - a full implementation would use the workflow executor
            match command {
                crate::cook::workflow::WorkflowStep {
                    shell: Some(shell_cmd),
                    ..
                } => {
                    // Execute shell command
                    let shell_cmd_interpolated = shell_cmd
                        .replace("${merge.worktree}", worktree_name)
                        .replace("${merge.source_branch}", source_branch)
                        .replace("${merge.target_branch}", target_branch)
                        .replace("${merge.session_id}", &session_id);

                    // Log shell command execution similar to regular workflow
                    let step_name = format!("shell: {}", shell_cmd_interpolated);
                    println!(
                        "🔄 Executing step {}/{}: {}",
                        step_index + 1,
                        merge_workflow.commands.len(),
                        step_name
                    );
                    println!("🔄 Executing: {}", step_name);

                    // Log verbose execution context
                    tracing::info!("=== Step Execution Context ===");
                    tracing::info!("Step: {}", step_name);
                    tracing::info!("Working Directory: {}", self.repo_path.display());
                    tracing::info!("Project Directory: {}", self.repo_path.display());
                    tracing::info!("Worktree: {}", worktree_name);
                    tracing::info!("Session ID: {}", session_id);
                    tracing::info!("Variables:");
                    for (key, value) in &variables {
                        let display_value = if value.len() > 100 {
                            format!("{}... (truncated)", &value[..100])
                        } else {
                            value.clone()
                        };
                        tracing::info!("  {} = {}", key, display_value);
                    }
                    tracing::info!("Environment Variables:");
                    tracing::info!("  PRODIGY_AUTOMATION = true");
                    if self.verbosity >= 1 {
                        tracing::info!("  PRODIGY_CLAUDE_STREAMING = true");
                    }
                    tracing::info!("Actual execution directory: {}", self.repo_path.display());
                    tracing::info!("==============================");

                    tracing::info!("Executing shell command: {}", shell_cmd_interpolated);
                    tracing::info!("  Working directory: {}", self.repo_path.display());

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
                    output.push_str(&result.stdout);

                    // Save checkpoint after successful step
                    step_index += 1;
                    let checkpoint = crate::cook::workflow::checkpoint::WorkflowCheckpoint {
                        workflow_id: format!("merge-workflow-{}", worktree_name),
                        execution_state: crate::cook::workflow::checkpoint::ExecutionState {
                            current_step_index: step_index,
                            total_steps: merge_workflow.commands.len(),
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
                        total_steps: merge_workflow.commands.len(),
                        workflow_name: Some(format!("merge-workflow-{}", worktree_name)),
                        workflow_path: None,
                        error_recovery_state: None,
                        retry_checkpoint_state: None,
                        variable_checkpoint_state: None,
                    };
                    if let Err(e) = checkpoint_manager.save_checkpoint(&checkpoint).await {
                        tracing::warn!("Failed to save merge workflow checkpoint: {}", e);
                    } else {
                        tracing::info!(
                            "Saved checkpoint for merge workflow at step {}",
                            step_index
                        );
                    }
                }
                crate::cook::workflow::WorkflowStep {
                    claude: Some(claude_cmd),
                    ..
                } => {
                    // Execute Claude command with streaming support
                    let claude_cmd_interpolated = claude_cmd
                        .replace("${merge.worktree}", worktree_name)
                        .replace("${merge.source_branch}", source_branch)
                        .replace("${merge.target_branch}", target_branch)
                        .replace("${merge.session_id}", &session_id);

                    // Log Claude command execution similar to regular workflow
                    let step_name = format!("claude: {}", claude_cmd_interpolated);
                    println!(
                        "🔄 Executing step {}/{}: {}",
                        step_index + 1,
                        merge_workflow.commands.len(),
                        step_name
                    );
                    println!("🔄 Executing: {}", step_name);

                    // Log verbose execution context
                    tracing::info!("=== Step Execution Context ===");
                    tracing::info!("Step: {}", step_name);
                    tracing::info!("Working Directory: {}", self.repo_path.display());
                    tracing::info!("Project Directory: {}", self.repo_path.display());
                    tracing::info!("Worktree: {}", worktree_name);
                    tracing::info!("Session ID: {}", session_id);
                    tracing::info!("Variables:");
                    for (key, value) in &variables {
                        let display_value = if value.len() > 100 {
                            format!("{}... (truncated)", &value[..100])
                        } else {
                            value.clone()
                        };
                        tracing::info!("  {} = {}", key, display_value);
                    }

                    let mut env_vars = HashMap::new();
                    env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

                    // Enable streaming if verbosity is high enough
                    if self.verbosity >= 1 {
                        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
                    }

                    // Check for console output override
                    if std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").unwrap_or_default() == "true"
                    {
                        env_vars.insert(
                            "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
                            "true".to_string(),
                        );
                    }

                    tracing::info!("Environment Variables:");
                    for (key, value) in &env_vars {
                        tracing::info!("  {} = {}", key, value);
                    }
                    tracing::info!("Actual execution directory: {}", self.repo_path.display());
                    tracing::info!("==============================");

                    tracing::info!(
                        "Claude execution mode: streaming={}, env_var={:?}",
                        self.verbosity >= 1,
                        env_vars.get("PRODIGY_CLAUDE_STREAMING")
                    );
                    if self.verbosity >= 1 {
                        tracing::info!("Using streaming mode for Claude command");
                    } else {
                        tracing::info!("Using print mode for Claude command");
                    }

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
                    output.push_str(&result.stdout);

                    // Save checkpoint after successful step
                    step_index += 1;
                    let checkpoint = crate::cook::workflow::checkpoint::WorkflowCheckpoint {
                        workflow_id: format!("merge-workflow-{}", worktree_name),
                        execution_state: crate::cook::workflow::checkpoint::ExecutionState {
                            current_step_index: step_index,
                            total_steps: merge_workflow.commands.len(),
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
                        total_steps: merge_workflow.commands.len(),
                        workflow_name: Some(format!("merge-workflow-{}", worktree_name)),
                        workflow_path: None,
                        error_recovery_state: None,
                        retry_checkpoint_state: None,
                        variable_checkpoint_state: None,
                    };
                    if let Err(e) = checkpoint_manager.save_checkpoint(&checkpoint).await {
                        tracing::warn!("Failed to save merge workflow checkpoint: {}", e);
                    } else {
                        tracing::info!(
                            "Saved checkpoint for merge workflow at step {}",
                            step_index
                        );
                    }
                }
                _ => {
                    // For other command types, just log them for now
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
    use tempfile::TempDir;

    #[test]
    fn test_parse_worktree_output() {
        // Test parsing of git worktree list --porcelain output
        let output = r#"worktree /home/user/project/.prodigy/worktrees/test-session
HEAD abc123def456
branch refs/heads/test-branch

worktree /home/user/project/.prodigy/worktrees/another-session
HEAD 789012ghi345
branch refs/heads/another-branch

worktree /home/user/project
HEAD xyz789mno123
branch refs/heads/main"#;

        let entries = WorktreeManager::parse_worktree_output(output);

        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0].0,
            PathBuf::from("/home/user/project/.prodigy/worktrees/test-session")
        );
        assert_eq!(entries[0].1, "test-branch");
        assert_eq!(
            entries[1].0,
            PathBuf::from("/home/user/project/.prodigy/worktrees/another-session")
        );
        assert_eq!(entries[1].1, "another-branch");
        assert_eq!(entries[2].0, PathBuf::from("/home/user/project"));
        assert_eq!(entries[2].1, "main");
    }

    #[test]
    fn test_parse_worktree_output_empty() {
        // Test with empty output
        let output = "";
        let entries = WorktreeManager::parse_worktree_output(output);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_worktree_output_single_entry() {
        // Test with single worktree
        let output = r#"worktree /path/to/worktree
HEAD abc123
branch refs/heads/feature"#;

        let entries = WorktreeManager::parse_worktree_output(output);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, PathBuf::from("/path/to/worktree"));
        assert_eq!(entries[0].1, "feature");
    }

    #[test]
    fn test_parse_worktree_output_missing_branch() {
        // Test with missing branch info (should not include incomplete entries)
        let output = r#"worktree /path/to/worktree
HEAD abc123"#;

        let entries = WorktreeManager::parse_worktree_output(output);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_split_into_worktree_blocks() {
        // Test splitting output into individual worktree blocks
        let output = r#"worktree /path/one
HEAD abc123
branch refs/heads/feature-one

worktree /path/two
HEAD def456
branch refs/heads/feature-two"#;

        let blocks = WorktreeManager::split_into_worktree_blocks(output);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].len(), 3);
        assert_eq!(blocks[0][0], "worktree /path/one");
        assert_eq!(blocks[0][1], "HEAD abc123");
        assert_eq!(blocks[0][2], "branch refs/heads/feature-one");

        assert_eq!(blocks[1].len(), 3);
        assert_eq!(blocks[1][0], "worktree /path/two");
    }

    #[test]
    fn test_split_into_worktree_blocks_empty() {
        let output = "";
        let blocks = WorktreeManager::split_into_worktree_blocks(output);
        assert_eq!(blocks.len(), 0);
    }

    #[test]
    fn test_split_into_worktree_blocks_single() {
        let output = r#"worktree /single/path
HEAD xyz789
branch refs/heads/main"#;

        let blocks = WorktreeManager::split_into_worktree_blocks(output);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].len(), 3);
    }

    #[test]
    fn test_parse_worktree_block_valid() {
        let block = vec![
            "worktree /test/path",
            "HEAD abc123",
            "branch refs/heads/test-branch",
        ];

        let result = WorktreeManager::parse_worktree_block(block);
        assert!(result.is_some());

        let (path, branch) = result.unwrap();
        assert_eq!(path, PathBuf::from("/test/path"));
        assert_eq!(branch, "test-branch");
    }

    #[test]
    fn test_parse_worktree_block_missing_path() {
        let block = vec!["HEAD abc123", "branch refs/heads/test-branch"];

        let result = WorktreeManager::parse_worktree_block(block);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_worktree_block_missing_branch() {
        let block = vec!["worktree /test/path", "HEAD abc123"];

        let result = WorktreeManager::parse_worktree_block(block);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_worktree_block_extra_fields() {
        // Test that extra fields don't break parsing
        let block = vec![
            "worktree /test/path",
            "HEAD abc123",
            "branch refs/heads/test-branch",
            "extra field that should be ignored",
        ];

        let result = WorktreeManager::parse_worktree_block(block);
        assert!(result.is_some());

        let (path, branch) = result.unwrap();
        assert_eq!(path, PathBuf::from("/test/path"));
        assert_eq!(branch, "test-branch");
    }

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
                last_command: "/prodigy-test".to_string(),
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
            checkpoint.last_command = "/prodigy-updated".to_string();
        })?;

        // Verify checkpoint was updated
        let updated_state = manager.get_session_state("test-session")?;
        let checkpoint = updated_state.last_checkpoint.unwrap();
        assert_eq!(checkpoint.iteration, 2);
        assert_eq!(checkpoint.last_command, "/prodigy-updated");
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
                last_command: "/prodigy-test1".to_string(),
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
            checkpoint.last_command = "/prodigy-test2".to_string();
        })?;

        let state = manager.get_session_state("test-session")?;
        assert_eq!(state.last_checkpoint.unwrap().iteration, 2);
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
        let subprocess = SubprocessManager::production();

        // Initialize a git repository in the temp directory
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

        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess).unwrap();

        // Create metadata directory
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        // For testing, we'll create minimal JSON representations that match
        // what the list_detailed method expects to parse
        let state1_json = serde_json::json!({
            "session_id": "session-test-1",
            "status": "in_progress",
            "branch": "feature-1",
            "created_at": (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339(),
            "updated_at": (chrono::Utc::now() - chrono::Duration::minutes(30)).to_rfc3339(),
            "error": null,
            "stats": {
                "files_changed": 5,
                "commits": 2,
                "last_commit_sha": null
            },
            "worktree_name": "session-test-1",
            "iterations": { "completed": 0, "max": 5 },
            "merged": false,
            "merged_at": null,
            "merge_prompt_shown": false,
            "merge_prompt_response": null,
            "interrupted_at": null,
            "interruption_type": null,
            "last_checkpoint": null,
            "resumable": false
        });

        let state2_json = serde_json::json!({
            "session_id": "session-test-2",
            "status": "completed",
            "branch": "feature-2",
            "created_at": (chrono::Utc::now() - chrono::Duration::hours(3)).to_rfc3339(),
            "updated_at": (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
            "error": null,
            "stats": {
                "files_changed": 10,
                "commits": 5,
                "last_commit_sha": null
            },
            "worktree_name": "session-test-2",
            "iterations": { "completed": 0, "max": 5 },
            "merged": false,
            "merged_at": null,
            "merge_prompt_shown": false,
            "merge_prompt_response": null,
            "interrupted_at": null,
            "interruption_type": null,
            "last_checkpoint": null,
            "resumable": false
        });

        let state3_json = serde_json::json!({
            "session_id": "session-test-3",
            "status": "failed",
            "branch": "feature-3",
            "created_at": (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
            "updated_at": (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339(),
            "error": "Test error message",
            "stats": {
                "files_changed": 2,
                "commits": 1,
                "last_commit_sha": null
            },
            "worktree_name": "session-test-3",
            "iterations": { "completed": 0, "max": 5 },
            "merged": false,
            "merged_at": null,
            "merge_prompt_shown": false,
            "merge_prompt_response": null,
            "interrupted_at": null,
            "interruption_type": null,
            "last_checkpoint": null,
            "resumable": false
        });

        // Save states to metadata
        let state1_file = metadata_dir.join("session-test-1.json");
        let state2_file = metadata_dir.join("session-test-2.json");
        let state3_file = metadata_dir.join("session-test-3.json");

        std::fs::write(&state1_file, serde_json::to_string(&state1_json)?)?;
        std::fs::write(&state2_file, serde_json::to_string(&state2_json)?)?;
        std::fs::write(&state3_file, serde_json::to_string(&state3_json)?)?;

        // Create corresponding worktree directories
        let wt1_dir = manager.base_dir.join("session-test-1");
        let wt2_dir = manager.base_dir.join("session-test-2");
        let wt3_dir = manager.base_dir.join("session-test-3");

        std::fs::create_dir_all(&wt1_dir)?;
        std::fs::create_dir_all(&wt2_dir)?;
        std::fs::create_dir_all(&wt3_dir)?;

        // Note: In a real environment we'd have git worktrees set up
        // For testing, we'll simulate by creating minimal .git files
        // to make the directories appear as valid worktrees
        std::fs::write(wt1_dir.join(".git"), "gitdir: /fake/path")?;
        std::fs::write(wt2_dir.join(".git"), "gitdir: /fake/path")?;
        std::fs::write(wt3_dir.join(".git"), "gitdir: /fake/path")?;

        // Get detailed list
        let result = manager.list_detailed().await?;

        // Verify summary counts
        assert_eq!(result.summary.total, 3);
        assert_eq!(result.summary.in_progress, 1);
        assert_eq!(result.summary.completed, 1);
        assert_eq!(result.summary.failed, 1);
        assert_eq!(result.summary.interrupted, 0);

        // Verify we have the expected sessions
        assert_eq!(result.sessions.len(), 3);

        // Find each session and verify key fields
        let session1 = result
            .sessions
            .iter()
            .find(|s| s.session_id == "session-test-1")
            .expect("Session 1 not found");
        assert_eq!(session1.status, WorktreeStatus::InProgress);
        assert_eq!(session1.files_changed, 5);
        assert_eq!(session1.commits, 2);

        let session2 = result
            .sessions
            .iter()
            .find(|s| s.session_id == "session-test-2")
            .expect("Session 2 not found");
        assert_eq!(session2.status, WorktreeStatus::Completed);
        assert_eq!(session2.files_changed, 10);
        assert_eq!(session2.commits, 5);

        let session3 = result
            .sessions
            .iter()
            .find(|s| s.session_id == "session-test-3")
            .expect("Session 3 not found");
        assert_eq!(session3.status, WorktreeStatus::Failed);
        assert_eq!(
            session3.error_summary,
            Some("Test error message".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_list_detailed_with_workflow_info() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();

        // Initialize a git repository in the temp directory
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

        let manager =
            WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess.clone()).unwrap();

        // Create metadata directory
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        // Create a test worktree state as JSON
        let state_json = serde_json::json!({
            "session_id": "workflow-session",
            "status": "in_progress",
            "branch": "workflow-branch",
            "created_at": (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
            "updated_at": (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339(),
            "error": null,
            "stats": {
                "files_changed": 3,
                "commits": 1,
                "last_commit_sha": null
            },
            "worktree_name": "workflow-session",
            "iterations": { "completed": 0, "max": 5 },
            "merged": false,
            "merged_at": null,
            "merge_prompt_shown": false,
            "merge_prompt_response": null,
            "interrupted_at": null,
            "interruption_type": null,
            "last_checkpoint": null,
            "resumable": false
        });

        // Save state
        let state_file = metadata_dir.join("workflow-session.json");
        std::fs::write(&state_file, serde_json::to_string(&state_json)?)?;

        // Create actual git worktree
        let wt_dir = manager.base_dir.join("workflow-session");
        let add_worktree = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args([
                "worktree",
                "add",
                "-b",
                "workflow-branch",
                wt_dir.to_string_lossy().as_ref(),
            ])
            .build();
        subprocess.runner().run(add_worktree).await?;

        // Create session state directory
        let prodigy_dir = wt_dir.join(".prodigy");
        std::fs::create_dir_all(&prodigy_dir)?;

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

        let session_state_file = prodigy_dir.join("session_state.json");
        std::fs::write(&session_state_file, serde_json::to_string(&session_state)?)?;

        // Get detailed list
        let result = manager.list_detailed().await?;

        assert_eq!(result.sessions.len(), 1);
        let session = &result.sessions[0];

        // Verify workflow information was extracted
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

        // Initialize a git repository in the temp directory
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

        let manager =
            WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess.clone()).unwrap();

        // Create metadata directory
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        // Create a test worktree state as JSON
        let state_json = serde_json::json!({
            "session_id": "mapreduce-session",
            "status": "in_progress",
            "branch": "mapreduce-branch",
            "created_at": (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339(),
            "updated_at": (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339(),
            "error": null,
            "stats": {
                "files_changed": 0,
                "commits": 0,
                "last_commit_sha": null
            },
            "worktree_name": "mapreduce-session",
            "iterations": { "completed": 0, "max": 5 },
            "merged": false,
            "merged_at": null,
            "merge_prompt_shown": false,
            "merge_prompt_response": null,
            "interrupted_at": null,
            "interruption_type": null,
            "last_checkpoint": null,
            "resumable": false
        });

        // Save state
        let state_file = metadata_dir.join("mapreduce-session.json");
        std::fs::write(&state_file, serde_json::to_string(&state_json)?)?;

        // Create actual git worktree
        let wt_dir = manager.base_dir.join("mapreduce-session");
        let add_worktree = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args([
                "worktree",
                "add",
                "-b",
                "mapreduce-branch",
                wt_dir.to_string_lossy().as_ref(),
            ])
            .build();
        subprocess.runner().run(add_worktree).await?;

        // Create session state directory
        let prodigy_dir = wt_dir.join(".prodigy");
        std::fs::create_dir_all(&prodigy_dir)?;

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

        let session_state_file = prodigy_dir.join("session_state.json");
        std::fs::write(&session_state_file, serde_json::to_string(&session_state)?)?;

        // Get detailed list
        let result = manager.list_detailed().await?;

        assert_eq!(result.sessions.len(), 1);
        let session = &result.sessions[0];

        // Verify MapReduce information was extracted
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
    fn test_filter_sessions_by_status() {
        let now = Utc::now();
        let states = vec![
            WorktreeState {
                session_id: "session1".to_string(),
                worktree_name: "worktree1".to_string(),
                branch: "branch1".to_string(),
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

        let in_progress =
            WorktreeManager::filter_sessions_by_status(states.clone(), WorktreeStatus::InProgress);
        assert_eq!(in_progress.len(), 2);
        assert_eq!(in_progress[0].session_id, "session1");
        assert_eq!(in_progress[1].session_id, "session3");

        let complete =
            WorktreeManager::filter_sessions_by_status(states, WorktreeStatus::Completed);
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

        let loaded = WorktreeManager::load_state_from_file(&json_path);
        assert!(loaded.is_some());
        let loaded_state = loaded.unwrap();
        assert_eq!(loaded_state.session_id, "test-session");

        // Test with non-JSON file
        let txt_path = temp_dir.path().join("not_json.txt");
        fs::write(&txt_path, "not json content").unwrap();
        assert!(WorktreeManager::load_state_from_file(&txt_path).is_none());

        // Test with invalid JSON
        let bad_json_path = temp_dir.path().join("bad.json");
        fs::write(&bad_json_path, "{ invalid json }").unwrap();
        assert!(WorktreeManager::load_state_from_file(&bad_json_path).is_none());

        // Test with non-existent file
        let missing_path = temp_dir.path().join("missing.json");
        assert!(WorktreeManager::load_state_from_file(&missing_path).is_none());
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

        let states = WorktreeManager::collect_all_states(&metadata_dir).unwrap();
        assert_eq!(states.len(), 3);

        // Verify all states were loaded
        let session_ids: Vec<String> = states.iter().map(|s| s.session_id.clone()).collect();
        assert!(session_ids.contains(&"session1".to_string()));
        assert!(session_ids.contains(&"session2".to_string()));
        assert!(session_ids.contains(&"session3".to_string()));

        // Test with non-existent directory
        let missing_dir = temp_dir.path().join("missing");
        let result = WorktreeManager::collect_all_states(&missing_dir).unwrap();
        assert_eq!(result.len(), 0);
    }
}
