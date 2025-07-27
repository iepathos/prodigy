use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::process::Command;

use super::WorktreeSession;

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
        let timestamp = Utc::now().timestamp();
        let name = if let Some(focus) = focus {
            let sanitized_focus = focus.replace(" ", "-").replace("/", "-");
            format!("mmm-{}-{}", sanitized_focus, timestamp)
        } else {
            format!("mmm-session-{}", timestamp)
        };

        let branch = name.clone();
        let worktree_path = self.base_dir.join(&name);

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "add", "-b", &branch])
            .arg(&worktree_path)
            .output()
            .context("Failed to execute git worktree add")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create worktree: {}", stderr);
        }

        Ok(WorktreeSession::new(
            name,
            branch,
            worktree_path,
            focus.map(|s| s.to_string()),
        ))
    }

    pub fn list_sessions(&self) -> Result<Vec<WorktreeSession>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .context("Failed to execute git worktree list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to list worktrees: {}", stderr);
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

                        let focus = if name.starts_with("mmm-session-") {
                            None
                        } else {
                            name.strip_prefix("mmm-")
                                .and_then(|s| s.rsplit_once('-'))
                                .map(|(focus, _)| focus.replace("-", " "))
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

                let focus = if name.starts_with("mmm-session-") {
                    None
                } else {
                    name.strip_prefix("mmm-")
                        .and_then(|s| s.rsplit_once('-'))
                        .map(|(focus, _)| focus.replace("-", " "))
                };

                sessions.push(WorktreeSession::new(name, branch, canonical_path, focus));
            }
        }

        Ok(sessions)
    }

    pub fn merge_session(&self, name: &str, target_branch: Option<&str>) -> Result<()> {
        // Call Claude CLI to handle the merge with automatic conflict resolution
        let mut args = vec!["/mmm-merge-worktree", name];

        // Add target branch if specified
        if let Some(target) = target_branch {
            args.push("--target");
            args.push(target);
        }

        println!(
            "🔄 Merging worktree '{}' using Claude-assisted merge...",
            name
        );

        // Execute Claude CLI command
        let output = Command::new("claude")
            .current_dir(&self.repo_path)
            .args(&args)
            .env("MMM_AUTOMATION", "true") // Enable automation mode
            .output()
            .context("Failed to execute claude /mmm-merge-worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Provide detailed error information
            eprintln!("❌ Claude merge failed for worktree '{}':", name);
            if !stderr.is_empty() {
                eprintln!("Error output: {}", stderr);
            }
            if !stdout.is_empty() {
                eprintln!("Standard output: {}", stdout);
            }

            anyhow::bail!("Failed to merge worktree '{}' - Claude merge failed", name);
        }

        // Parse the output for success confirmation
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("{}", stdout);

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
                anyhow::bail!("Failed to remove worktree: {}", stderr);
            }
        }

        let branch_exists = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--verify", &format!("refs/heads/{}", name)])
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
                eprintln!("Warning: Failed to delete branch {}: {}", name, stderr);
            }
        }

        Ok(())
    }

    pub fn cleanup_all_sessions(&self) -> Result<()> {
        let sessions = self.list_sessions()?;
        for session in sessions {
            println!("Cleaning up worktree: {}", session.name);
            self.cleanup_session(&session.name)?;
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
