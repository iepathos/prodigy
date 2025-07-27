use anyhow::{Context, Result};
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
        let base_dir = repo_path.join(".mmm").join("worktrees");
        std::fs::create_dir_all(&base_dir).context("Failed to create worktree base directory")?;

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
                current_path = Some(PathBuf::from(line.trim_start_matches("worktree ")));
            } else if line.starts_with("branch ") {
                current_branch = Some(line.trim_start_matches("branch refs/heads/").to_string());
            } else if line.is_empty() && current_path.is_some() && current_branch.is_some() {
                let path = current_path.take().unwrap();
                let branch = current_branch.take().unwrap();

                if path.starts_with(&self.base_dir) && branch.starts_with("mmm-") {
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

                    sessions.push(WorktreeSession::new(name, branch, path, focus));
                }
            }
        }

        Ok(sessions)
    }

    pub fn merge_session(&self, name: &str, target_branch: Option<&str>) -> Result<()> {
        let target = target_branch.unwrap_or("master");

        let current_branch_output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .context("Failed to get current branch")?;

        if !current_branch_output.status.success() {
            anyhow::bail!("Failed to determine current branch");
        }

        let current_branch = String::from_utf8_lossy(&current_branch_output.stdout)
            .trim()
            .to_string();

        if current_branch != target {
            let checkout_output = Command::new("git")
                .current_dir(&self.repo_path)
                .args(["checkout", target])
                .output()
                .context("Failed to checkout target branch")?;

            if !checkout_output.status.success() {
                let stderr = String::from_utf8_lossy(&checkout_output.stderr);
                anyhow::bail!("Failed to checkout {}: {}", target, stderr);
            }
        }

        let merge_output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["merge", "--no-ff", "-m"])
            .arg(format!("Merge MMM session '{}' into {}", name, target))
            .arg(name)
            .output()
            .context("Failed to execute git merge")?;

        if !merge_output.status.success() {
            let stderr = String::from_utf8_lossy(&merge_output.stderr);
            anyhow::bail!("Failed to merge {}: {}", name, stderr);
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

