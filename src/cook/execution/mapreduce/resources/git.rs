//! Git operations for MapReduce agents

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::orchestrator::ExecutionEnvironment;
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

use super::git_operations::{GitOperationsConfig, GitOperationsService, GitResultExt};

/// Handles git operations for MapReduce agents
pub struct GitOperations {
    service: GitOperationsService,
}

impl Default for GitOperations {
    fn default() -> Self {
        Self::new()
    }
}

impl GitOperations {
    /// Create a new git operations handler
    pub fn new() -> Self {
        Self {
            service: GitOperationsService::new(GitOperationsConfig::default()),
        }
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

        // Check if there's an existing merge in progress and clean it up
        let merge_head_path = parent_path.join(".git/MERGE_HEAD");
        if merge_head_path.exists() {
            warn!(
                "Detected incomplete merge state (MERGE_HEAD exists), cleaning up before merging {}",
                agent_branch
            );

            // First, try to complete the merge by committing staged changes
            let status_output = Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&**parent_path)
                .output()
                .await
                .map_err(|e| self.create_git_error("git_status", &e.to_string()))?;

            if status_output.status.success() {
                let status = String::from_utf8_lossy(&status_output.stdout);

                // If there are staged changes, commit them
                if !status.trim().is_empty() {
                    warn!("Committing staged changes from incomplete merge");
                    let commit_output = Command::new("git")
                        .args(["commit", "--no-edit"])
                        .current_dir(&**parent_path)
                        .output()
                        .await
                        .map_err(|e| self.create_git_error("git_commit", &e.to_string()))?;

                    if !commit_output.status.success() {
                        // If commit fails, abort the merge
                        warn!("Failed to commit staged changes, aborting merge");
                        let _ = Command::new("git")
                            .args(["merge", "--abort"])
                            .current_dir(&**parent_path)
                            .output()
                            .await;
                    }
                } else {
                    // No staged changes, just abort
                    warn!("No staged changes, aborting incomplete merge");
                    let _ = Command::new("git")
                        .args(["merge", "--abort"])
                        .current_dir(&**parent_path)
                        .output()
                        .await;
                }
            }
        }

        // Merge directly - no fetch needed since worktrees share the same object database
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
            return Err(self.create_git_error("merge_agent_branch", &stderr));
        }

        info!(
            "Successfully merged agent branch {} to parent",
            agent_branch
        );
        Ok(())
    }

    /// Get commits from a worktree
    pub async fn get_worktree_commits(
        &mut self,
        worktree_path: &Path,
    ) -> MapReduceResult<Vec<String>> {
        let commit_infos = self
            .service
            .get_worktree_commits(worktree_path, None, None)
            .await?;
        Ok(commit_infos.to_string_list())
    }

    /// Get modified files in a worktree
    pub async fn get_modified_files(
        &mut self,
        worktree_path: &Path,
    ) -> MapReduceResult<Vec<String>> {
        let file_infos = self
            .service
            .get_worktree_modified_files(worktree_path, None)
            .await?;
        Ok(file_infos.to_string_list())
    }

    /// Get modified files in a worktree (non-mutable version for backward compatibility)
    pub async fn get_worktree_modified_files(
        &mut self,
        worktree_path: &Path,
    ) -> MapReduceResult<Vec<String>> {
        self.get_modified_files(worktree_path).await
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
