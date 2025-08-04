//! Granular git operations layer
//!
//! This module provides fine-grained, highly testable git operations
//! that enable comprehensive testing with minimal setup.

pub mod error;
pub mod parsers;
pub mod scenario;
pub mod types;

pub use error::GitError;
pub use parsers::*;
pub use scenario::*;
pub use types::*;

use crate::subprocess::{ProcessCommandBuilder, ProcessRunner};
use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Git read operations
#[async_trait]
pub trait GitReader: Send + Sync {
    /// Check if a directory is a git repository
    async fn is_repository(&self, path: &Path) -> Result<bool>;

    /// Get git status for the repository
    async fn get_status(&self, path: &Path) -> Result<GitStatus>;

    /// Get the current branch name
    async fn get_current_branch(&self, path: &Path) -> Result<String>;

    /// Get commit message by reference
    async fn get_commit_message(&self, path: &Path, ref_: &str) -> Result<String>;

    /// List all files tracked by git
    async fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>>;

    /// Get diff between two references
    async fn get_diff(&self, path: &Path, from: &str, to: &str) -> Result<GitDiff>;

    /// Get the last commit message
    async fn get_last_commit_message(&self, path: &Path) -> Result<String>;

    /// Check if working directory is clean
    async fn is_clean(&self, path: &Path) -> Result<bool>;
}

/// Git write operations
#[async_trait]
pub trait GitWriter: Send + Sync {
    /// Initialize a new git repository
    async fn init_repository(&self, path: &Path) -> Result<()>;

    /// Stage specific files
    async fn stage_files(&self, path: &Path, files: &[PathBuf]) -> Result<()>;

    /// Stage all changes
    async fn stage_all(&self, path: &Path) -> Result<()>;

    /// Create a commit with message
    async fn commit(&self, path: &Path, message: &str) -> Result<CommitId>;

    /// Create a new branch
    async fn create_branch(&self, path: &Path, name: &str) -> Result<()>;

    /// Switch to a branch
    async fn switch_branch(&self, path: &Path, name: &str) -> Result<()>;

    /// Delete a branch
    async fn delete_branch(&self, path: &Path, name: &str) -> Result<()>;
}

/// Git worktree operations
#[async_trait]
pub trait GitWorktree: Send + Sync {
    /// Create a new worktree
    async fn create_worktree(&self, repo: &Path, name: &str, path: &Path) -> Result<()>;

    /// Remove a worktree
    async fn remove_worktree(&self, repo: &Path, name: &str) -> Result<()>;

    /// List all worktrees
    async fn list_worktrees(&self, repo: &Path) -> Result<Vec<WorktreeInfo>>;

    /// Prune worktrees (remove stale references)
    async fn prune_worktrees(&self, repo: &Path) -> Result<()>;
}

/// Combined trait for all git operations
pub trait GitOperations: GitReader + GitWriter + GitWorktree {}

/// Production implementation of git operations
pub struct GitCommandRunner {
    process_runner: Arc<dyn ProcessRunner>,
}

impl GitCommandRunner {
    /// Create a new GitCommandRunner
    pub fn new(process_runner: Arc<dyn ProcessRunner>) -> Self {
        Self { process_runner }
    }

    /// Execute a git command
    async fn run_git_command(
        &self,
        path: &Path,
        args: &[&str],
    ) -> Result<crate::subprocess::ProcessOutput> {
        let command = ProcessCommandBuilder::new("git")
            .args(args)
            .current_dir(path)
            .build();

        self.process_runner
            .run(command)
            .await
            .map_err(|e| GitError::CommandFailed(format!("Git command failed: {e}")).into())
    }
}

#[async_trait]
impl GitReader for GitCommandRunner {
    async fn is_repository(&self, path: &Path) -> Result<bool> {
        let result = self
            .run_git_command(path, &["rev-parse", "--git-dir"])
            .await;
        Ok(result.is_ok() && result.unwrap().status.success())
    }

    async fn get_status(&self, path: &Path) -> Result<GitStatus> {
        let output = self
            .run_git_command(path, &["status", "--porcelain=v2"])
            .await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git status failed".to_string()).into());
        }

        parsers::parse_status_output(&output.stdout)
    }

    async fn get_current_branch(&self, path: &Path) -> Result<String> {
        let output = self
            .run_git_command(path, &["branch", "--show-current"])
            .await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("Failed to get current branch".to_string()).into());
        }

        let branch = output.stdout.trim();
        if branch.is_empty() {
            return Err(GitError::DetachedHead.into());
        }

        Ok(branch.to_string())
    }

    async fn get_commit_message(&self, path: &Path, ref_: &str) -> Result<String> {
        let output = self
            .run_git_command(path, &["log", "-1", "--pretty=format:%s", ref_])
            .await?;

        if !output.status.success() {
            return Err(GitError::CommitNotFound(ref_.to_string()).into());
        }

        Ok(output.stdout.trim().to_string())
    }

    async fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let output = self.run_git_command(path, &["ls-files"]).await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git ls-files failed".to_string()).into());
        }

        Ok(output
            .stdout
            .lines()
            .map(|line| PathBuf::from(line.trim()))
            .collect())
    }

    async fn get_diff(&self, path: &Path, from: &str, to: &str) -> Result<GitDiff> {
        let range = format!("{from}..{to}");
        let output = self
            .run_git_command(path, &["diff", "--numstat", &range])
            .await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git diff failed".to_string()).into());
        }

        parsers::parse_diff_output(&output.stdout)
    }

    async fn get_last_commit_message(&self, path: &Path) -> Result<String> {
        self.get_commit_message(path, "HEAD").await
    }

    async fn is_clean(&self, path: &Path) -> Result<bool> {
        let status = self.get_status(path).await?;
        Ok(status.is_clean())
    }
}

#[async_trait]
impl GitWriter for GitCommandRunner {
    async fn init_repository(&self, path: &Path) -> Result<()> {
        let output = self.run_git_command(path, &["init"]).await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git init failed".to_string()).into());
        }

        Ok(())
    }

    async fn stage_files(&self, path: &Path, files: &[PathBuf]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let mut args = vec!["add"];
        let file_strs: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let file_refs: Vec<&str> = file_strs.iter().map(|s| s.as_str()).collect();
        args.extend(file_refs);

        let output = self.run_git_command(path, &args).await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git add failed".to_string()).into());
        }

        Ok(())
    }

    async fn stage_all(&self, path: &Path) -> Result<()> {
        let output = self.run_git_command(path, &["add", "."]).await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git add . failed".to_string()).into());
        }

        Ok(())
    }

    async fn commit(&self, path: &Path, message: &str) -> Result<CommitId> {
        let output = self
            .run_git_command(path, &["commit", "-m", message])
            .await?;

        if !output.status.success() {
            if output.stderr.contains("nothing to commit") {
                return Err(GitError::NothingToCommit.into());
            }
            return Err(GitError::CommandFailed("git commit failed".to_string()).into());
        }

        // Get the commit hash
        let hash_output = self.run_git_command(path, &["rev-parse", "HEAD"]).await?;
        let hash = hash_output.stdout.trim().to_string();

        Ok(CommitId::new(hash))
    }

    async fn create_branch(&self, path: &Path, name: &str) -> Result<()> {
        let output = self.run_git_command(path, &["branch", name]).await?;

        if !output.status.success() {
            if output.stderr.contains("already exists") {
                return Err(GitError::BranchExists(name.to_string()).into());
            }
            return Err(GitError::CommandFailed("git branch failed".to_string()).into());
        }

        Ok(())
    }

    async fn switch_branch(&self, path: &Path, name: &str) -> Result<()> {
        let output = self.run_git_command(path, &["checkout", name]).await?;

        if !output.status.success() {
            if output.stderr.contains("did not match any file") {
                return Err(GitError::BranchNotFound(name.to_string()).into());
            }
            if output.stderr.contains("uncommitted changes") {
                return Err(GitError::UncommittedChanges.into());
            }
            return Err(GitError::CommandFailed("git checkout failed".to_string()).into());
        }

        Ok(())
    }

    async fn delete_branch(&self, path: &Path, name: &str) -> Result<()> {
        let output = self.run_git_command(path, &["branch", "-d", name]).await?;

        if !output.status.success() {
            if output.stderr.contains("not found") {
                return Err(GitError::BranchNotFound(name.to_string()).into());
            }
            return Err(GitError::CommandFailed("git branch -d failed".to_string()).into());
        }

        Ok(())
    }
}

#[async_trait]
impl GitWorktree for GitCommandRunner {
    async fn create_worktree(&self, repo: &Path, name: &str, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy();
        let output = self
            .run_git_command(repo, &["worktree", "add", "-b", name, &path_str])
            .await?;

        if !output.status.success() {
            if output.stderr.contains("already exists") {
                return Err(GitError::WorktreeExists(name.to_string()).into());
            }
            return Err(GitError::CommandFailed("git worktree add failed".to_string()).into());
        }

        Ok(())
    }

    async fn remove_worktree(&self, repo: &Path, name: &str) -> Result<()> {
        let output = self
            .run_git_command(repo, &["worktree", "remove", name, "--force"])
            .await?;

        if !output.status.success() {
            if output.stderr.contains("not a working tree") {
                return Err(GitError::WorktreeNotFound(name.to_string()).into());
            }
            return Err(GitError::CommandFailed("git worktree remove failed".to_string()).into());
        }

        Ok(())
    }

    async fn list_worktrees(&self, repo: &Path) -> Result<Vec<WorktreeInfo>> {
        let output = self
            .run_git_command(repo, &["worktree", "list", "--porcelain"])
            .await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git worktree list failed".to_string()).into());
        }

        parsers::parse_worktree_list(&output.stdout)
    }

    async fn prune_worktrees(&self, repo: &Path) -> Result<()> {
        let output = self.run_git_command(repo, &["worktree", "prune"]).await?;

        if !output.status.success() {
            return Err(GitError::CommandFailed("git worktree prune failed".to_string()).into());
        }

        Ok(())
    }
}

// GitCommandRunner is a different abstraction layer that doesn't implement crate::abstractions::git::GitOperations
// It implements its own trait hierarchy: GitReader + GitWriter + GitWorktree

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::MockProcessRunner;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_runner() -> (GitCommandRunner, MockProcessRunner) {
        let mock = MockProcessRunner::new();
        let runner = GitCommandRunner::new(Arc::new(mock.clone()) as Arc<dyn ProcessRunner>);
        (runner, mock)
    }

    #[tokio::test]
    async fn test_is_repository_success() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: ".git".to_string(),
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let result = git.is_repository(temp_dir.path()).await.unwrap();
        assert!(result);

        // Verify the command was called correctly
        let calls = mock.get_calls().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].program, "git");
        assert_eq!(calls[0].args, vec!["rev-parse", "--git-dir"]);
    }

    #[tokio::test]
    async fn test_is_repository_failure() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::failure(1),
                stdout: String::new(),
                stderr: "fatal: not a git repository".to_string(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let result = git.is_repository(temp_dir.path()).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_get_current_branch() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: "main\n".to_string(),
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let branch = git.get_current_branch(temp_dir.path()).await.unwrap();
        assert_eq!(branch, "main");
    }

    #[tokio::test]
    async fn test_get_current_branch_detached_head() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: "\n".to_string(), // Empty output indicates detached HEAD
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let result = git.get_current_branch(temp_dir.path()).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("detached"));
    }

    #[tokio::test]
    async fn test_stage_all() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: String::new(),
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        git.stage_all(temp_dir.path()).await.unwrap();

        let calls = mock.get_calls().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].args, vec!["add", "."]);
    }

    #[tokio::test]
    async fn test_commit_success() {
        let (git, mut mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        // Mock commit command
        mock.expect_command("git")
            .with_args(|args| args.len() >= 2 && args[0] == "commit" && args[1] == "-m")
            .returns_stdout("")
            .finish();

        // Mock rev-parse to get commit hash
        mock.expect_command("git")
            .with_args(|args| args.len() == 2 && args[0] == "rev-parse" && args[1] == "HEAD")
            .returns_stdout("abc1234567890abcdef1234567890abcdef123456\n")
            .finish();

        let commit_id = git.commit(temp_dir.path(), "test commit").await.unwrap();
        assert_eq!(
            commit_id.hash(),
            "abc1234567890abcdef1234567890abcdef123456",
            "Expected commit hash to match, got: {}",
            commit_id.hash()
        );

        let calls = mock.get_calls().await;
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].args, vec!["commit", "-m", "test commit"]);
        assert_eq!(calls[1].args, vec!["rev-parse", "HEAD"]);
    }

    #[tokio::test]
    async fn test_commit_nothing_to_commit() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::failure(1),
                stdout: String::new(),
                stderr: "nothing to commit, working tree clean".to_string(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let result = git.commit(temp_dir.path(), "test commit").await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Nothing to commit"));
    }

    #[tokio::test]
    async fn test_get_status() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: "1 .M N... 100644 100644 100644 abc123 def456 test.txt\n".to_string(),
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let status = git.get_status(temp_dir.path()).await.unwrap();
        assert_eq!(status.added.len(), 0);
        assert_eq!(status.modified.len(), 1);
        assert_eq!(status.untracked.len(), 0);
    }

    #[tokio::test]
    async fn test_list_files() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: "src/main.rs\nsrc/lib.rs\nCargo.toml\n".to_string(),
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let files = git.list_files(temp_dir.path()).await.unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0], PathBuf::from("src/main.rs"));
        assert_eq!(files[1], PathBuf::from("src/lib.rs"));
        assert_eq!(files[2], PathBuf::from("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_is_clean() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: String::new(), // Empty output means clean
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        let is_clean = git.is_clean(temp_dir.path()).await.unwrap();
        assert!(is_clean);
    }

    #[tokio::test]
    async fn test_create_branch() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: String::new(),
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        git.create_branch(temp_dir.path(), "feature-branch")
            .await
            .unwrap();

        let calls = mock.get_calls().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].args, vec!["branch", "feature-branch"]);
    }

    #[tokio::test]
    async fn test_switch_branch() {
        let (git, mock) = create_test_runner();
        let temp_dir = TempDir::new().unwrap();

        mock.add_response(
            "git",
            Ok(crate::subprocess::ProcessOutput {
                status: crate::subprocess::ExitStatusHelper::success(),
                stdout: String::new(),
                stderr: String::new(),
                duration: std::time::Duration::from_millis(10),
            }),
        )
        .await;

        git.switch_branch(temp_dir.path(), "main").await.unwrap();

        let calls = mock.get_calls().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].args, vec!["checkout", "main"]);
    }
}
