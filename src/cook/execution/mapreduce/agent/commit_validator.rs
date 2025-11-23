//! Commit validation for MapReduce agent execution
//!
//! This module provides commit validation functionality for MapReduce agents,
//! ensuring that commands with `commit_required: true` actually create git commits.
//! Validation happens in the agent's isolated worktree to ensure correctness
//! across parallel execution.

use crate::abstractions::git::GitOperations;
use crate::cook::error::ResultExt;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// Result of commit validation
#[derive(Debug, Clone, PartialEq)]
pub enum CommitValidationResult {
    /// Commits were created
    Valid { commits: Vec<CommitInfo> },
    /// No commits were created
    NoCommits,
}

/// Information about a commit
#[derive(Debug, Clone, PartialEq)]
pub struct CommitInfo {
    /// Commit SHA
    pub sha: String,
    /// Commit message (first line)
    pub message: String,
}

/// Validates that commits were created when required
pub struct CommitValidator {
    git_ops: Arc<dyn GitOperations>,
}

impl CommitValidator {
    /// Create a new commit validator
    pub fn new(git_ops: Arc<dyn GitOperations>) -> Self {
        Self { git_ops }
    }

    /// Get the current HEAD SHA in a worktree
    pub async fn get_head(&self, worktree_path: &Path) -> Result<String> {
        let output = self
            .git_ops
            .git_command_in_dir(&["rev-parse", "HEAD"], "get HEAD", worktree_path)
            .await
            .context("Failed to get HEAD")
            .map_err(|e| anyhow::Error::msg(e.to_string()))?;

        if !output.status.success() {
            anyhow::bail!(
                "git rev-parse HEAD failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Verify that commits were created between two HEAD references
    pub async fn verify_commits_created(
        &self,
        worktree_path: &Path,
        head_before: &str,
        head_after: &str,
    ) -> Result<CommitValidationResult> {
        // If HEADs are the same, no commits were created
        if head_before == head_after {
            return Ok(CommitValidationResult::NoCommits);
        }

        // Get commits between the two references
        let commits = self
            .get_commits_between(worktree_path, head_before, head_after)
            .await?;

        Ok(CommitValidationResult::Valid { commits })
    }

    /// Get list of commits between two references
    pub async fn get_commits_between(
        &self,
        worktree_path: &Path,
        from_ref: &str,
        to_ref: &str,
    ) -> Result<Vec<CommitInfo>> {
        let output = self
            .git_ops
            .git_command_in_dir(
                &[
                    "log",
                    "--format=%H%n%s",
                    &format!("{}..{}", from_ref, to_ref),
                ],
                "get commits",
                worktree_path,
            )
            .await
            .context("Failed to get commits")
            .map_err(|e| anyhow::Error::msg(e.to_string()))?;

        if !output.status.success() {
            anyhow::bail!(
                "git log failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();

        // Parse commit info (format is: SHA\nMessage\nSHA\nMessage...)
        let mut commits = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if i + 1 < lines.len() {
                let sha = lines[i].trim().to_string();
                let message = lines[i + 1].trim().to_string();
                commits.push(CommitInfo { sha, message });
                i += 2;
            } else {
                // Odd number of lines, skip the last one
                break;
            }
        }

        Ok(commits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abstractions::git::GitOperations;
    use async_trait::async_trait;
    use std::path::PathBuf;
    use std::process::Output;
    use std::sync::Mutex;

    /// Mock git operations for testing
    struct MockGitOps {
        head_responses: Mutex<Vec<String>>,
        log_response: Mutex<Option<String>>,
    }

    impl MockGitOps {
        fn new() -> Self {
            Self {
                head_responses: Mutex::new(Vec::new()),
                log_response: Mutex::new(None),
            }
        }

        fn set_head_responses(&self, responses: Vec<String>) {
            *self.head_responses.lock().unwrap() = responses;
        }

        fn set_log_response(&self, response: String) {
            *self.log_response.lock().unwrap() = Some(response);
        }
    }

    #[async_trait]
    impl GitOperations for MockGitOps {
        async fn git_command(&self, _args: &[&str], _desc: &str) -> Result<Output> {
            Ok(Output {
                status: std::process::ExitStatus::default(),
                stdout: vec![],
                stderr: vec![],
            })
        }

        async fn git_command_in_dir(
            &self,
            args: &[&str],
            _desc: &str,
            _dir: &Path,
        ) -> Result<Output> {
            if args[0] == "rev-parse" && args[1] == "HEAD" {
                let mut responses = self.head_responses.lock().unwrap();
                if responses.is_empty() {
                    return Ok(Output {
                        status: std::process::ExitStatus::default(),
                        stdout: b"abc123\n".to_vec(),
                        stderr: vec![],
                    });
                }
                let response = responses.remove(0);
                Ok(Output {
                    status: std::process::ExitStatus::default(),
                    stdout: format!("{}\n", response).into_bytes(),
                    stderr: vec![],
                })
            } else if args[0] == "log" {
                let response = self
                    .log_response
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_default();
                Ok(Output {
                    status: std::process::ExitStatus::default(),
                    stdout: response.into_bytes(),
                    stderr: vec![],
                })
            } else {
                Ok(Output {
                    status: std::process::ExitStatus::default(),
                    stdout: vec![],
                    stderr: vec![],
                })
            }
        }

        async fn get_last_commit_message(&self) -> Result<String> {
            Ok("test commit".to_string())
        }

        async fn check_git_status(&self) -> Result<String> {
            Ok("".to_string())
        }

        async fn stage_all_changes(&self) -> Result<()> {
            Ok(())
        }

        async fn create_commit(&self, _message: &str) -> Result<()> {
            Ok(())
        }

        async fn is_git_repo(&self) -> bool {
            true
        }

        async fn create_worktree(&self, _name: &str, _path: &Path) -> Result<()> {
            Ok(())
        }

        async fn get_current_branch(&self) -> Result<String> {
            Ok("main".to_string())
        }

        async fn switch_branch(&self, _branch: &str) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_get_head() {
        let mock_ops = Arc::new(MockGitOps::new());
        mock_ops.set_head_responses(vec!["abc123def456".to_string()]);

        let validator = CommitValidator::new(mock_ops);
        let head = validator.get_head(&PathBuf::from("/test")).await.unwrap();

        assert_eq!(head, "abc123def456");
    }

    #[tokio::test]
    async fn test_verify_commits_created_no_commits() {
        let mock_ops = Arc::new(MockGitOps::new());
        let validator = CommitValidator::new(mock_ops);

        let result = validator
            .verify_commits_created(&PathBuf::from("/test"), "abc123", "abc123")
            .await
            .unwrap();

        assert_eq!(result, CommitValidationResult::NoCommits);
    }

    #[tokio::test]
    async fn test_verify_commits_created_with_commits() {
        let mock_ops = Arc::new(MockGitOps::new());
        mock_ops.set_log_response("def456\nAdd feature X\nabc123\nFix bug Y\n".to_string());

        let validator = CommitValidator::new(mock_ops);

        let result = validator
            .verify_commits_created(&PathBuf::from("/test"), "old123", "new456")
            .await
            .unwrap();

        match result {
            CommitValidationResult::Valid { commits } => {
                assert_eq!(commits.len(), 2);
                assert_eq!(commits[0].sha, "def456");
                assert_eq!(commits[0].message, "Add feature X");
                assert_eq!(commits[1].sha, "abc123");
                assert_eq!(commits[1].message, "Fix bug Y");
            }
            _ => panic!("Expected Valid result"),
        }
    }

    #[tokio::test]
    async fn test_get_commits_between() {
        let mock_ops = Arc::new(MockGitOps::new());
        mock_ops.set_log_response("commit1\nFirst commit\ncommit2\nSecond commit\n".to_string());

        let validator = CommitValidator::new(mock_ops);

        let commits = validator
            .get_commits_between(&PathBuf::from("/test"), "old", "new")
            .await
            .unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "commit1");
        assert_eq!(commits[0].message, "First commit");
        assert_eq!(commits[1].sha, "commit2");
        assert_eq!(commits[1].message, "Second commit");
    }

    #[tokio::test]
    async fn test_get_commits_between_empty() {
        let mock_ops = Arc::new(MockGitOps::new());
        mock_ops.set_log_response("".to_string());

        let validator = CommitValidator::new(mock_ops);

        let commits = validator
            .get_commits_between(&PathBuf::from("/test"), "old", "new")
            .await
            .unwrap();

        assert_eq!(commits.len(), 0);
    }
}
