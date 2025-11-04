//! Git operations support for workflow execution
//!
//! This module provides git-related operations abstracted behind the GitOperations trait.
//! It separates git interaction concerns from workflow orchestration.

use crate::abstractions::git::GitOperations;
use crate::cook::commit_tracker::TrackedCommit;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::Arc;

/// Helper for git operations in workflow execution
pub struct GitOperationsHelper {
    git_operations: Arc<dyn GitOperations>,
}

impl GitOperationsHelper {
    /// Create a new GitOperationsHelper
    pub fn new(git_operations: Arc<dyn GitOperations>) -> Self {
        Self { git_operations }
    }

    /// Get current git HEAD
    pub async fn get_current_head(&self, working_dir: &Path) -> Result<String> {
        // We need to run git commands in the correct working directory (especially for worktrees)
        let output = self
            .git_operations
            .git_command_in_dir(&["rev-parse", "HEAD"], "get HEAD", working_dir)
            .await
            .context("Failed to get git HEAD")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if there are uncommitted changes
    pub async fn check_for_changes(&self, working_dir: &Path) -> Result<bool> {
        let output = self
            .git_operations
            .git_command_in_dir(&["status", "--porcelain"], "check status", working_dir)
            .await
            .context("Failed to check git status")?;

        Ok(!output.stdout.is_empty())
    }

    /// Get commits between two refs
    pub async fn get_commits_between(
        &self,
        working_dir: &Path,
        from: &str,
        to: &str,
    ) -> Result<Vec<TrackedCommit>> {
        let output = self
            .git_operations
            .git_command_in_dir(
                &[
                    "log",
                    &format!("{from}..{to}"),
                    "--pretty=format:%H|%s|%an|%aI",
                    "--name-only",
                ],
                "get commit log",
                working_dir,
            )
            .await
            .context("Failed to get commit log")?;

        parse_commit_log(&String::from_utf8_lossy(&output.stdout))
    }
}

/// Parse git log output into TrackedCommits (pure function)
fn parse_commit_log(stdout: &str) -> Result<Vec<TrackedCommit>> {
    let mut commits = Vec::new();
    let mut current_commit: Option<TrackedCommit> = None;

    for line in stdout.lines() {
        if line.contains('|') {
            // This is a commit header line
            if let Some(commit) = current_commit.take() {
                commits.push(commit);
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                current_commit = Some(TrackedCommit {
                    hash: parts[0].to_string(),
                    message: parts[1].to_string(),
                    author: parts[2].to_string(),
                    timestamp: parts[3]
                        .parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    files_changed: Vec::new(),
                    insertions: 0,
                    deletions: 0,
                    step_name: String::new(),
                    agent_id: None,
                });
            }
        } else if !line.is_empty() {
            // This is a file name
            if let Some(ref mut commit) = current_commit {
                commit.files_changed.push(std::path::PathBuf::from(line));
            }
        }
    }

    if let Some(commit) = current_commit {
        commits.push(commit);
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commit_log_single_commit() {
        let log = "abc123|feat: add feature|Alice|2024-01-01T12:00:00Z\nsrc/main.rs\nsrc/lib.rs";
        let commits = parse_commit_log(log).expect("Failed to parse");

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].hash, "abc123");
        assert_eq!(commits[0].message, "feat: add feature");
        assert_eq!(commits[0].author, "Alice");
        assert_eq!(commits[0].files_changed.len(), 2);
    }

    #[test]
    fn test_parse_commit_log_multiple_commits() {
        let log = "abc123|feat: add feature|Alice|2024-01-01T12:00:00Z\nsrc/main.rs\ndef456|fix: bug fix|Bob|2024-01-02T12:00:00Z\nsrc/lib.rs";
        let commits = parse_commit_log(log).expect("Failed to parse");

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].hash, "abc123");
        assert_eq!(commits[1].hash, "def456");
    }

    #[test]
    fn test_parse_commit_log_empty() {
        let log = "";
        let commits = parse_commit_log(log).expect("Failed to parse");
        assert_eq!(commits.len(), 0);
    }
}
