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

    /// Validate that we're in a worktree context
    ///
    /// Pure function that checks if the execution environment has a worktree name.
    /// Returns the working directory path if valid, error message otherwise.
    fn validate_worktree_context(
        env: &ExecutionEnvironment,
    ) -> Result<&std::sync::Arc<std::path::PathBuf>, &'static str> {
        if env.worktree_name.is_some() {
            Ok(&env.working_dir)
        } else {
            Err("Cannot merge: not running in a worktree context")
        }
    }

    /// Check if there's an incomplete merge in progress
    ///
    /// Pure function that checks for the existence of .git/MERGE_HEAD file.
    /// Returns true if an incomplete merge exists.
    fn has_incomplete_merge(parent_path: &Path) -> bool {
        parent_path.join(".git/MERGE_HEAD").exists()
    }

    /// Determine action based on git status output
    ///
    /// Pure function that parses git status --porcelain output
    /// to decide whether to commit staged changes or abort.
    fn should_commit_staged_changes(status_output: &str) -> bool {
        !status_output.trim().is_empty()
    }

    /// Check git status in a repository
    ///
    /// Runs `git status --porcelain` and returns the output.
    async fn check_git_status(&self, repo_path: &Path) -> MapReduceResult<String> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("git_status", &e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(self.create_git_error("git_status", &stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Commit staged changes without prompting for a message
    ///
    /// Uses --no-edit to commit with the existing merge message.
    async fn commit_staged_changes(&self, repo_path: &Path) -> MapReduceResult<()> {
        let output = Command::new("git")
            .args(["commit", "--no-edit"])
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("git_commit", &e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(self.create_git_error("git_commit", &stderr));
        }

        Ok(())
    }

    /// Abort an in-progress merge
    ///
    /// Best-effort operation that ignores errors.
    async fn abort_merge(&self, repo_path: &Path) {
        let _ = Command::new("git")
            .args(["merge", "--abort"])
            .current_dir(repo_path)
            .output()
            .await;
    }

    /// Recover from an incomplete merge
    ///
    /// Handles the merge recovery logic by either committing staged changes
    /// or aborting the incomplete merge.
    async fn recover_incomplete_merge(&self, parent_path: &Path, agent_branch: &str) -> MapReduceResult<()> {
        warn!(
            "Detected incomplete merge state (MERGE_HEAD exists), cleaning up before merging {}",
            agent_branch
        );

        // Get git status to decide action
        let status = self.check_git_status(parent_path).await?;

        // Decide action based on status (pure function)
        if Self::should_commit_staged_changes(&status) {
            warn!("Committing staged changes from incomplete merge");

            // Try to commit, abort on failure
            if self.commit_staged_changes(parent_path).await.is_err() {
                warn!("Failed to commit staged changes, aborting merge");
                self.abort_merge(parent_path).await;
            }
        } else {
            // No staged changes, just abort
            warn!("No staged changes, aborting incomplete merge");
            self.abort_merge(parent_path).await;
        }

        Ok(())
    }

    /// Execute a git merge
    ///
    /// Performs the actual merge with --no-ff to always create a merge commit.
    async fn execute_merge(&self, parent_path: &Path, agent_branch: &str) -> MapReduceResult<()> {
        let output = Command::new("git")
            .args([
                "merge",
                "--no-ff",
                "-m",
                &format!("Merge agent {}", agent_branch),
                agent_branch,
            ])
            .current_dir(parent_path)
            .output()
            .await
            .map_err(|e| self.create_git_error("merge_agent_branch", &e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(self.create_git_error("merge_agent_branch", &stderr));
        }

        Ok(())
    }

    /// Merge an agent's branch back to the parent
    pub async fn merge_agent_to_parent(
        &self,
        agent_branch: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        // Validate worktree context (pure function)
        let parent_path = Self::validate_worktree_context(env)
            .map_err(|msg| self.create_git_error("merge_to_parent", msg))?;

        // Recover from incomplete merge if needed (extracted function)
        if Self::has_incomplete_merge(parent_path) {
            self.recover_incomplete_merge(parent_path, agent_branch).await?;
        }

        // Execute the merge (extracted function)
        self.execute_merge(parent_path, agent_branch).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::process::Command as TokioCommand;

    /// Helper to create a temporary git repository
    async fn create_test_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        let init_output = TokioCommand::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to run git init");
        assert!(init_output.status.success(), "git init failed");

        // Configure git user
        TokioCommand::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to config user.name");

        TokioCommand::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to config user.email");

        // Create initial commit on main branch
        fs::write(repo_path.join("README.md"), "# Test Repo").expect("Failed to write README");
        TokioCommand::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to git add");

        let commit_output = TokioCommand::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to commit");
        assert!(commit_output.status.success(), "initial commit failed");

        (temp_dir, repo_path)
    }

    /// Helper to create a worktree from the parent repo
    async fn create_test_worktree(
        parent_path: &Path,
        worktree_name: &str,
    ) -> std::path::PathBuf {
        // Create worktree inside the parent directory to avoid conflicts between concurrent tests
        let worktree_path = parent_path.join(worktree_name);

        let output = TokioCommand::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                worktree_name,
            ])
            .current_dir(parent_path)
            .output()
            .await
            .expect("Failed to create worktree");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "Failed to create worktree: {}\nStdout: {}\nStderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                stderr
            );
        }

        worktree_path
    }

    /// Helper to create a commit in a worktree
    async fn create_commit_in_worktree(worktree_path: &Path, file_name: &str, content: &str) {
        fs::write(worktree_path.join(file_name), content).expect("Failed to write file");

        TokioCommand::new("git")
            .args(["add", "."])
            .current_dir(worktree_path)
            .output()
            .await
            .expect("Failed to git add");

        let output = TokioCommand::new("git")
            .args(["commit", "-m", &format!("Add {}", file_name)])
            .current_dir(worktree_path)
            .output()
            .await
            .expect("Failed to commit");

        assert!(output.status.success(), "Failed to create commit");
    }

    /// Helper to create MERGE_HEAD file to simulate incomplete merge
    async fn create_merge_head(repo_path: &Path, commit_sha: &str) {
        let merge_head_path = repo_path.join(".git/MERGE_HEAD");
        fs::write(&merge_head_path, format!("{}\n", commit_sha))
            .expect("Failed to create MERGE_HEAD");
    }

    /// Helper to get current commit SHA
    async fn get_current_commit_sha(repo_path: &Path) -> String {
        let output = TokioCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .await
            .expect("Failed to get commit SHA");

        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    // Tests for pure decision logic functions
    #[test]
    fn test_validate_worktree_context_with_worktree() {
        let env = ExecutionEnvironment {
            working_dir: Arc::new(std::path::PathBuf::from("/tmp/test")),
            project_dir: Arc::new(std::path::PathBuf::from("/tmp/project")),
            worktree_name: Some(Arc::from("test-worktree")),
            session_id: Arc::from("test-session"),
        };

        let result = GitOperations::validate_worktree_context(&env);
        assert!(result.is_ok());
        assert_eq!(**result.unwrap(), std::path::PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_validate_worktree_context_without_worktree() {
        let env = ExecutionEnvironment {
            working_dir: Arc::new(std::path::PathBuf::from("/tmp/test")),
            project_dir: Arc::new(std::path::PathBuf::from("/tmp/project")),
            worktree_name: None,
            session_id: Arc::from("test-session"),
        };

        let result = GitOperations::validate_worktree_context(&env);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot merge: not running in a worktree context"
        );
    }

    #[tokio::test]
    async fn test_has_incomplete_merge_when_merge_head_exists() {
        let (_temp_dir, repo_path) = create_test_repo().await;
        let commit_sha = get_current_commit_sha(&repo_path).await;
        create_merge_head(&repo_path, &commit_sha).await;

        assert!(GitOperations::has_incomplete_merge(&repo_path));
    }

    #[tokio::test]
    async fn test_has_incomplete_merge_when_merge_head_absent() {
        let (_temp_dir, repo_path) = create_test_repo().await;

        assert!(!GitOperations::has_incomplete_merge(&repo_path));
    }

    #[test]
    fn test_should_commit_staged_changes_with_changes() {
        let status_with_changes = "M  some_file.txt\nA  new_file.txt\n";
        assert!(GitOperations::should_commit_staged_changes(
            status_with_changes
        ));
    }

    #[test]
    fn test_should_commit_staged_changes_without_changes() {
        let status_empty = "";
        assert!(!GitOperations::should_commit_staged_changes(
            status_empty
        ));

        let status_whitespace = "   \n  \n";
        assert!(!GitOperations::should_commit_staged_changes(
            status_whitespace
        ));
    }

    #[tokio::test]
    async fn test_merge_agent_to_parent_not_in_worktree_context() {
        let git_ops = GitOperations::new();

        // Create ExecutionEnvironment without worktree_name (not in worktree context)
        let env = ExecutionEnvironment {
            working_dir: Arc::new(std::path::PathBuf::from("/tmp")),
            project_dir: Arc::new(std::path::PathBuf::from("/tmp")),
            worktree_name: None,
            session_id: Arc::from("test-session"),
        };

        // Should fail because we're not in a worktree context
        let result = git_ops
            .merge_agent_to_parent("agent-branch", &env)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            MapReduceError::General { message, .. } => {
                assert!(message.contains("not running in a worktree context"));
            }
            _ => panic!("Expected General error"),
        }
    }

    #[tokio::test]
    async fn test_merge_agent_to_parent_clean_merge_success() {
        let (_temp_dir, parent_path) = create_test_repo().await;
        let worktree_path = create_test_worktree(&parent_path, "agent-worktree").await;

        // Create a commit in the worktree
        create_commit_in_worktree(&worktree_path, "feature.txt", "New feature").await;

        // Create ExecutionEnvironment with worktree context
        let env = ExecutionEnvironment {
            working_dir: Arc::new(parent_path.clone()),
            project_dir: Arc::new(parent_path.clone()),
            worktree_name: Some(Arc::from("agent-worktree")),
            session_id: Arc::from("test-session"),
        };

        let git_ops = GitOperations::new();

        // Perform the merge
        let result = git_ops
            .merge_agent_to_parent("agent-worktree", &env)
            .await;

        assert!(result.is_ok());

        // Verify the merge was successful by checking if the file exists in parent
        let merged_file = parent_path.join("feature.txt");
        assert!(merged_file.exists(), "Merged file should exist in parent");
    }

    #[tokio::test]
    async fn test_merge_agent_to_parent_with_merge_head_and_staged_changes_commit_succeeds() {
        let (_temp_dir, parent_path) = create_test_repo().await;

        // Get current commit SHA for MERGE_HEAD
        let commit_sha = get_current_commit_sha(&parent_path).await;

        // Create MERGE_HEAD to simulate incomplete merge
        create_merge_head(&parent_path, &commit_sha).await;

        // Create a staged change
        fs::write(parent_path.join("staged.txt"), "staged content")
            .expect("Failed to write staged file");
        let add_output = TokioCommand::new("git")
            .args(["add", "staged.txt"])
            .current_dir(&parent_path)
            .output()
            .await
            .expect("Failed to stage file");
        assert!(add_output.status.success());

        // Create a worktree and commit for the actual merge
        let worktree_path = create_test_worktree(&parent_path, "agent-worktree").await;
        create_commit_in_worktree(&worktree_path, "feature.txt", "New feature").await;

        let env = ExecutionEnvironment {
            working_dir: Arc::new(parent_path.clone()),
            project_dir: Arc::new(parent_path.clone()),
            worktree_name: Some(Arc::from("agent-worktree")),
            session_id: Arc::from("test-session"),
        };

        let git_ops = GitOperations::new();

        // Should recover from incomplete merge and then perform new merge
        let result = git_ops
            .merge_agent_to_parent("agent-worktree", &env)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_merge_agent_to_parent_with_merge_head_no_staged_changes_abort() {
        let (_temp_dir, parent_path) = create_test_repo().await;

        // Get current commit SHA for MERGE_HEAD
        let commit_sha = get_current_commit_sha(&parent_path).await;

        // Create MERGE_HEAD without staged changes
        create_merge_head(&parent_path, &commit_sha).await;

        // Create a worktree for actual merge
        let worktree_path = create_test_worktree(&parent_path, "agent-worktree").await;
        create_commit_in_worktree(&worktree_path, "feature.txt", "New feature").await;

        let env = ExecutionEnvironment {
            working_dir: Arc::new(parent_path.clone()),
            project_dir: Arc::new(parent_path.clone()),
            worktree_name: Some(Arc::from("agent-worktree")),
            session_id: Arc::from("test-session"),
        };

        let git_ops = GitOperations::new();

        // Should abort incomplete merge and proceed with new merge
        let result = git_ops
            .merge_agent_to_parent("agent-worktree", &env)
            .await;

        assert!(result.is_ok());

        // MERGE_HEAD should be gone after merge
        assert!(!parent_path.join(".git/MERGE_HEAD").exists());
    }

    #[tokio::test]
    async fn test_merge_agent_to_parent_invalid_branch() {
        let (_temp_dir, parent_path) = create_test_repo().await;

        let env = ExecutionEnvironment {
            working_dir: Arc::new(parent_path.clone()),
            project_dir: Arc::new(parent_path.clone()),
            worktree_name: Some(Arc::from("test-worktree")),
            session_id: Arc::from("test-session"),
        };

        let git_ops = GitOperations::new();

        // Try to merge a non-existent branch
        let result = git_ops
            .merge_agent_to_parent("non-existent-branch", &env)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            MapReduceError::General { message, .. } => {
                assert!(message.contains("merge_agent_branch"));
            }
            _ => panic!("Expected General error"),
        }
    }
}
