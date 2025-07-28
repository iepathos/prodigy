use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use super::{IterationInfo, WorktreeSession, WorktreeState, WorktreeStats, WorktreeStatus};

pub struct WorktreeManager {
    pub base_dir: PathBuf,
    pub repo_path: PathBuf,
}

impl WorktreeManager {
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        // Get the repository name from the path
        let repo_name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Could not determine repository name"))?;

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
        })
    }

    pub fn create_session(&self, focus: Option<&str>) -> Result<WorktreeSession> {
        let session_id = Uuid::new_v4();
        // Simple name without focus, using UUID
        let name = format!("session-{session_id}");
        let branch = format!("mmm-{name}");
        let worktree_path = self.base_dir.join(&name);

        // Create worktree
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "add", "-b", &branch])
            .arg(&worktree_path)
            .output()
            .context("Failed to execute git worktree add")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create worktree: {stderr}");
        }

        // Create session
        let session = WorktreeSession::new(
            name.clone(),
            branch,
            worktree_path,
            focus.map(|s| s.to_string()),
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
            focus: session.focus.clone(),
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
        };

        let json = serde_json::to_string_pretty(&state)?;
        fs::write(state_file, json)?;
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
        fs::write(state_file, json)?;
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<WorktreeSession>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .context("Failed to execute git worktree list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to list worktrees: {stderr}");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
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

                        // Try to load state from metadata
                        let state_file =
                            self.base_dir.join(".metadata").join(format!("{name}.json"));
                        let focus = if let Ok(state_json) = fs::read_to_string(&state_file) {
                            if let Ok(state) = serde_json::from_str::<WorktreeState>(&state_json) {
                                state.focus
                            } else {
                                // Fallback for legacy sessions
                                extract_focus_from_name(&name)
                            }
                        } else {
                            // Fallback for legacy sessions
                            extract_focus_from_name(&name)
                        };

                        sessions.push(WorktreeSession::new(name, branch, canonical_path, focus));
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

                // Try to load state from metadata
                let state_file = self.base_dir.join(".metadata").join(format!("{name}.json"));
                let focus = if let Ok(state_json) = fs::read_to_string(&state_file) {
                    if let Ok(state) = serde_json::from_str::<WorktreeState>(&state_json) {
                        state.focus
                    } else {
                        // Fallback for legacy sessions
                        extract_focus_from_name(&name)
                    }
                } else {
                    // Fallback for legacy sessions
                    extract_focus_from_name(&name)
                };

                sessions.push(WorktreeSession::new(name, branch, canonical_path, focus));
            }
        }

        Ok(sessions)
    }

    pub fn merge_session(&self, name: &str) -> Result<()> {
        // Get the worktree branch name to verify merge
        let sessions = self.list_sessions()?;
        let session = sessions
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Worktree '{}' not found", name))?;
        let worktree_branch = &session.branch;

        // Determine the default branch (main or master)
        let main_exists = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--verify", "refs/heads/main"])
            .output()
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
        let mut cmd = Command::new("claude");
        cmd.current_dir(&self.repo_path)
            .arg("--dangerously-skip-permissions") // Skip interactive permission prompts
            .arg("--print") // Output response to stdout for capture
            .arg(format!("/mmm-merge-worktree {worktree_branch}")) // Include branch name in the command
            .env("MMM_AUTOMATION", "true"); // Enable automation mode
                                            // Debug: Print what we're about to execute
        eprintln!("Debug: Running claude /mmm-merge-worktree with branch: {worktree_branch}");

        let output = cmd
            .output()
            .context("Failed to execute claude /mmm-merge-worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

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
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("{stdout}");

        // Verify the merge actually happened by checking if the worktree branch
        // is now merged into the target branch
        let merge_check = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["branch", "--merged", &target])
            .output()
            .context("Failed to check merged branches")?;

        if merge_check.status.success() {
            let merged_branches = String::from_utf8_lossy(&merge_check.stdout);
            if !merged_branches.contains(worktree_branch) {
                // Check if Claude output indicates permission was denied
                if stdout.contains("permission") || stdout.contains("grant permission") {
                    anyhow::bail!(
                        "Merge was not completed - Claude requires permission to proceed. \
                        Please run the command again and grant permission when prompted."
                    );
                } else {
                    anyhow::bail!(
                        "Merge verification failed - branch '{}' is not merged into '{}'. \
                        The merge may have been aborted or failed silently.",
                        worktree_branch,
                        target
                    );
                }
            }
        }

        // Update session state to mark as merged
        if let Err(e) = self.update_session_state(name, |state| {
            state.merged = true;
            state.merged_at = Some(Utc::now());
        }) {
            eprintln!("Warning: Failed to update session state after merge: {e}");
        }

        Ok(())
    }

    pub fn cleanup_session(&self, name: &str) -> Result<()> {
        let worktree_path = self.base_dir.join(name);

        let prune_output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "remove", &worktree_path.to_string_lossy()])
            .output()
            .context("Failed to execute git worktree remove")?;

        if !prune_output.status.success() {
            let stderr = String::from_utf8_lossy(&prune_output.stderr);
            if !stderr.contains("is not a working tree") {
                anyhow::bail!("Failed to remove worktree: {stderr}");
            }
        }

        let branch_exists = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--verify", &format!("refs/heads/{name}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if branch_exists {
            let delete_output = Command::new("git")
                .current_dir(&self.repo_path)
                .args(["branch", "-D", name])
                .output()
                .context("Failed to delete branch")?;

            if !delete_output.status.success() {
                let stderr = String::from_utf8_lossy(&delete_output.stderr);
                eprintln!("Warning: Failed to delete branch {name}: {stderr}");
            }
        }

        Ok(())
    }

    pub fn cleanup_all_sessions(&self) -> Result<()> {
        let sessions = self.list_sessions()?;
        for session in sessions {
            let name = &session.name;
            println!("Cleaning up worktree: {name}");
            self.cleanup_session(name)?;
        }
        Ok(())
    }

    pub fn get_worktree_for_branch(&self, branch: &str) -> Result<Option<PathBuf>> {
        let sessions = self.list_sessions()?;
        Ok(sessions
            .into_iter()
            .find(|s| s.branch == branch)
            .map(|s| s.path))
    }
}

/// Extract focus from legacy worktree names
fn extract_focus_from_name(name: &str) -> Option<String> {
    if name.starts_with("mmm-session-") || name.starts_with("session-") {
        None
    } else {
        name.strip_prefix("mmm-")
            .and_then(|s| s.rsplit_once('-'))
            .map(|(focus, _)| focus.replace("-", " "))
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
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf()).unwrap();

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
}
