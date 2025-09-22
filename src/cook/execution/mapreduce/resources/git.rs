//! Git operations for MapReduce agents

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::orchestrator::ExecutionEnvironment;
use std::path::Path;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Handles git operations for MapReduce agents
pub struct GitOperations {}

impl Default for GitOperations {
    fn default() -> Self {
        Self::new()
    }
}

impl GitOperations {
    /// Create a new git operations handler
    pub fn new() -> Self {
        Self {}
    }

    /// Create a branch for an agent in its worktree
    pub async fn create_agent_branch(
        &self,
        worktree_path: &Path,
        branch_name: &str,
    ) -> MapReduceResult<()> {
        // Create branch from current HEAD
        let output = Command::new("git")
            .args(["checkout", "-b", branch_name])
            .current_dir(worktree_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("create_branch", &e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(self.create_git_error("create_branch", &stderr));
        }

        info!(
            "Created branch {} in worktree at {}",
            branch_name,
            worktree_path.display()
        );
        Ok(())
    }

    /// Merge an agent's branch back to the parent
    pub async fn merge_agent_to_parent(
        &self,
        agent_branch: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        // Get parent worktree path (use working_dir if we're in a parent worktree)
        let parent_path = if env.worktree_name.is_some() {
            &env.working_dir
        } else {
            return Err(self.create_git_error(
                "merge_to_parent",
                "Cannot merge: not running in a worktree context",
            ));
        };

        // First fetch the agent branch
        let output = Command::new("git")
            .args(["fetch", "origin", agent_branch])
            .current_dir(&**parent_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("fetch_agent_branch", &e.to_string()))?;

        if !output.status.success() {
            debug!(
                "Fetch of agent branch {} might not exist (this is ok): {}",
                agent_branch,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Try to merge the agent branch
        let output = Command::new("git")
            .args([
                "merge",
                "--no-ff",
                "-m",
                &format!("Merge agent {}", agent_branch),
                agent_branch,
            ])
            .current_dir(&**parent_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("merge_agent_branch", &e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not something we can merge") {
                // Branch doesn't exist locally, try to fetch and merge from origin
                let output = Command::new("git")
                    .args([
                        "merge",
                        "--no-ff",
                        "-m",
                        &format!("Merge agent {}", agent_branch),
                        &format!("origin/{}", agent_branch),
                    ])
                    .current_dir(&**parent_path)
                    .output()
                    .await
                    .map_err(|e| self.create_git_error("merge_origin_branch", &e.to_string()))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(self.create_git_error("merge_origin_branch", &stderr));
                }
            } else {
                return Err(self.create_git_error("merge_agent_branch", &stderr));
            }
        }

        info!(
            "Successfully merged agent branch {} to parent",
            agent_branch
        );
        Ok(())
    }

    /// Get commits from a worktree
    pub async fn get_worktree_commits(&self, worktree_path: &Path) -> MapReduceResult<Vec<String>> {
        let output = Command::new("git")
            .args(["log", "--format=%H", "HEAD~10..HEAD"])
            .current_dir(worktree_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("get_commits", &e.to_string()))?;

        if !output.status.success() {
            // If there aren't 10 commits, try with just HEAD
            let output = Command::new("git")
                .args(["log", "--format=%H", "HEAD"])
                .current_dir(worktree_path)
                .output()
                .await
                .map_err(|e| self.create_git_error("get_commits_fallback", &e.to_string()))?;

            if !output.status.success() {
                return Ok(Vec::new());
            }

            let commits = String::from_utf8_lossy(&output.stdout)
                .lines()
                .take(10)
                .map(String::from)
                .collect();
            return Ok(commits);
        }

        let commits = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect();

        Ok(commits)
    }

    /// Get modified files in a worktree
    pub async fn get_modified_files(&self, worktree_path: &Path) -> MapReduceResult<Vec<String>> {
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD~1..HEAD"])
            .current_dir(worktree_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("get_modified_files", &e.to_string()))?;

        if !output.status.success() {
            // No previous commit, get all files
            let output = Command::new("git")
                .args(["ls-files"])
                .current_dir(worktree_path)
                .output()
                .await
                .map_err(|e| self.create_git_error("list_files", &e.to_string()))?;

            if !output.status.success() {
                return Ok(Vec::new());
            }

            let files = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(String::from)
                .collect();
            return Ok(files);
        }

        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect();

        Ok(files)
    }

    /// Check if a branch exists
    pub async fn branch_exists(&self, branch_name: &str, worktree_path: &Path) -> bool {
        let output = Command::new("git")
            .args(["rev-parse", "--verify", branch_name])
            .current_dir(worktree_path)
            .output()
            .await
            .ok();

        output.map(|o| o.status.success()).unwrap_or(false)
    }

    /// Delete a branch
    pub async fn delete_branch(
        &self,
        branch_name: &str,
        worktree_path: &Path,
    ) -> MapReduceResult<()> {
        let output = Command::new("git")
            .args(["branch", "-D", branch_name])
            .current_dir(worktree_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("delete_branch", &e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // It's ok if the branch doesn't exist
            if !stderr.contains("not found") {
                warn!("Failed to delete branch {}: {}", branch_name, stderr);
            }
        }

        Ok(())
    }

    /// Create a standardized git error
    fn create_git_error(&self, operation: &str, message: &str) -> MapReduceError {
        MapReduceError::General {
            message: format!("Git operation '{}' failed: {}", operation, message),
            source: None,
        }
    }
}
