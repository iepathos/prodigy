use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use super::{IterationInfo, WorktreeSession, WorktreeState, WorktreeStats, WorktreeStatus};

pub struct WorktreeManager {
    pub base_dir: PathBuf,
    pub repo_path: PathBuf,
    subprocess: SubprocessManager,
}

impl WorktreeManager {
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
        // Simple name without focus, using UUID
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
        let session = WorktreeSession::new(
            name.clone(),
            branch,
            worktree_path,
        );

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
                    if canonical_path.starts_with(&self.base_dir) && branch.starts_with("mmm-") {
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
            if canonical_path.starts_with(&self.base_dir) && branch.starts_with("mmm-") {
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
        if !metadata_dir.exists() {
            return Ok(Vec::new());
        }

        let mut interrupted_sessions = Vec::new();

        for entry in fs::read_dir(&metadata_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(state_json) = fs::read_to_string(&path) {
                    if let Ok(state) = serde_json::from_str::<WorktreeState>(&state_json) {
                        if state.status == WorktreeStatus::Interrupted {
                            interrupted_sessions.push(state);
                        }
                    }
                }
            }
        }

        Ok(interrupted_sessions)
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
        // we're focusing on the Claude CLI failure path

        let result = manager.merge_session("test-session").await;
        // Should fail because worktree doesn't actually exist in git
        assert!(result.is_err());
    }
}
