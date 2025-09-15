//! Git commit tracking and verification
//!
//! This module provides comprehensive commit tracking functionality for workflows,
//! including automatic commit creation, metadata collection, and verification.

use crate::abstractions::GitOperations;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for commit creation and tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitConfig {
    /// Commit message template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_template: Option<String>,

    /// Commit message validation regex
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_pattern: Option<String>,

    /// Whether to sign commits
    #[serde(default)]
    pub sign: bool,

    /// Author override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Files to include (glob patterns)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_files: Option<Vec<String>>,

    /// Files to exclude (glob patterns)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_files: Option<Vec<String>>,

    /// Squash commits at end of workflow
    #[serde(default)]
    pub squash: bool,
}

/// Metadata for a tracked commit
#[derive(Debug, Clone, Serialize)]
pub struct TrackedCommit {
    /// The commit hash
    pub hash: String,

    /// The commit message
    pub message: String,

    /// The commit author
    pub author: String,

    /// The commit timestamp
    pub timestamp: DateTime<Utc>,

    /// Files changed in this commit
    pub files_changed: Vec<PathBuf>,

    /// Number of insertions
    pub insertions: usize,

    /// Number of deletions
    pub deletions: usize,

    /// The step name that created this commit
    pub step_name: String,

    /// The agent ID if this was created by a MapReduce agent
    pub agent_id: Option<String>,
}

/// Tracks commits created during workflow execution
pub struct CommitTracker {
    /// Git operations interface
    git_ops: Arc<dyn GitOperations>,

    /// Working directory for git operations
    working_dir: PathBuf,

    /// Initial HEAD commit when tracking started
    initial_head: Option<String>,

    /// All tracked commits
    tracked_commits: Arc<RwLock<Vec<TrackedCommit>>>,
}

impl CommitTracker {
    /// Create a new commit tracker
    pub fn new(git_ops: Arc<dyn GitOperations>, working_dir: PathBuf) -> Self {
        Self {
            git_ops,
            working_dir,
            initial_head: None,
            tracked_commits: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize tracking by recording the current HEAD
    pub async fn initialize(&mut self) -> Result<()> {
        let output = self
            .git_ops
            .git_command_in_dir(&["rev-parse", "HEAD"], "get HEAD", &self.working_dir)
            .await?;

        self.initial_head = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        Ok(())
    }

    /// Get the current HEAD commit
    pub async fn get_current_head(&self) -> Result<String> {
        let output = self
            .git_ops
            .git_command_in_dir(&["rev-parse", "HEAD"], "get HEAD", &self.working_dir)
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if there are uncommitted changes
    pub async fn has_changes(&self) -> Result<bool> {
        let output = self
            .git_ops
            .git_command_in_dir(
                &["status", "--porcelain"],
                "check status",
                &self.working_dir,
            )
            .await?;

        Ok(!output.stdout.is_empty())
    }

    /// Get commits between two refs
    pub async fn get_commits_between(&self, from: &str, to: &str) -> Result<Vec<TrackedCommit>> {
        let output = self
            .git_ops
            .git_command_in_dir(
                &[
                    "log",
                    &format!("{from}..{to}"),
                    "--pretty=format:%H|%s|%an|%aI",
                    "--name-only",
                ],
                "get commit log",
                &self.working_dir,
            )
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
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
                    commit.files_changed.push(PathBuf::from(line));
                }
            }
        }

        if let Some(commit) = current_commit {
            commits.push(commit);
        }

        // Get diff stats for each commit
        for commit in &mut commits {
            if let Ok(output) = self
                .git_ops
                .git_command_in_dir(
                    &[
                        "diff",
                        "--shortstat",
                        &format!("{}^", commit.hash),
                        &commit.hash,
                    ],
                    "get diff stats",
                    &self.working_dir,
                )
                .await
            {
                let stats = String::from_utf8_lossy(&output.stdout);
                // Parse stats like "2 files changed, 10 insertions(+), 3 deletions(-)"
                if let Some(insertions) = stats
                    .split_whitespace()
                    .position(|w| w == "insertions(+)" || w == "insertion(+)")
                    .and_then(|i| stats.split_whitespace().nth(i.saturating_sub(1)))
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    commit.insertions = insertions;
                }

                if let Some(deletions) = stats
                    .split_whitespace()
                    .position(|w| w == "deletions(-)" || w == "deletion(-)")
                    .and_then(|i| stats.split_whitespace().nth(i.saturating_sub(1)))
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    commit.deletions = deletions;
                }
            }
        }

        Ok(commits)
    }

    /// Create an auto-commit with the given configuration
    pub async fn create_auto_commit(
        &self,
        step_name: &str,
        message_template: Option<&str>,
        variables: &HashMap<String, String>,
        commit_config: Option<&CommitConfig>,
    ) -> Result<TrackedCommit> {
        // Check for changes
        if !self.has_changes().await? {
            return Err(anyhow!("No changes to commit"));
        }

        // Stage all changes
        self.git_ops
            .git_command_in_dir(&["add", "."], "stage changes", &self.working_dir)
            .await?;

        // Generate commit message
        let message = if let Some(template) = message_template {
            self.interpolate_template(template, step_name, variables)?
        } else {
            format!("Auto-commit: {step_name}")
        };

        // Create the commit with optional author override and signing
        let mut commit_args = vec!["commit", "-m", &message];

        // Add author override if specified from commit_config
        let author_string;
        if let Some(config) = commit_config {
            if let Some(author) = &config.author {
                author_string = format!("--author={}", author);
                commit_args.push(&author_string);
            }

            // Add GPG signing if enabled
            if config.sign {
                commit_args.push("-S");
            }
        }

        self.git_ops
            .git_command_in_dir(
                &commit_args,
                "create commit",
                &self.working_dir,
            )
            .await?;

        // Get the new HEAD
        let new_head = self.get_current_head().await?;

        // Get commit details
        let mut commits = self
            .get_commits_between(&format!("{new_head}^"), &new_head)
            .await?;

        if let Some(mut commit) = commits.pop() {
            commit.step_name = step_name.to_string();
            Ok(commit)
        } else {
            Err(anyhow!("Failed to retrieve created commit"))
        }
    }

    /// Track commits created during step execution
    pub async fn track_step_commits(
        &self,
        step_name: &str,
        before_head: &str,
        after_head: &str,
    ) -> Result<Vec<TrackedCommit>> {
        if before_head == after_head {
            return Ok(Vec::new());
        }

        let mut commits = self.get_commits_between(before_head, after_head).await?;

        // Set the step name for all commits
        for commit in &mut commits {
            commit.step_name = step_name.to_string();
        }

        // Add to tracked commits
        let mut tracked = self.tracked_commits.write().await;
        tracked.extend(commits.clone());

        Ok(commits)
    }

    /// Get all tracked commits
    pub async fn get_all_commits(&self) -> Vec<TrackedCommit> {
        self.tracked_commits.read().await.clone()
    }

    /// Interpolate variables in a message template
    fn interpolate_template(
        &self,
        template: &str,
        step_name: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        let mut message = template.to_string();

        // Replace ${step.name}
        message = message.replace("${step.name}", step_name);

        // Replace other variables
        for (key, value) in variables {
            message = message.replace(&format!("${{{key}}}"), value);
            message = message.replace(&format!("${key}"), value);
        }

        Ok(message)
    }

    /// Validate a commit message against a pattern
    pub fn validate_message(&self, message: &str, pattern: &str) -> Result<()> {
        let re = regex::Regex::new(pattern).map_err(|e| anyhow!("Invalid message pattern: {e}"))?;

        if !re.is_match(message) {
            return Err(anyhow!(
                "Commit message '{}' does not match required pattern '{}'",
                message,
                pattern
            ));
        }

        Ok(())
    }

    /// Squash commits into a single commit
    pub async fn squash_commits(&self, commits: &[TrackedCommit], message: &str) -> Result<String> {
        if commits.is_empty() {
            return Err(anyhow!("No commits to squash"));
        }

        // Get the parent of the first commit
        let first_hash = &commits[0].hash;
        let parent_output = self
            .git_ops
            .git_command_in_dir(
                &["rev-parse", &format!("{first_hash}^")],
                "get parent",
                &self.working_dir,
            )
            .await?;

        let parent = String::from_utf8_lossy(&parent_output.stdout)
            .trim()
            .to_string();

        // Reset to parent
        self.git_ops
            .git_command_in_dir(
                &["reset", "--soft", &parent],
                "reset for squash",
                &self.working_dir,
            )
            .await?;

        // Create new squashed commit
        self.git_ops
            .git_command_in_dir(
                &["commit", "-m", message],
                "create squashed commit",
                &self.working_dir,
            )
            .await?;

        // Get the new commit hash
        self.get_current_head().await
    }
}

/// Result of commit tracking for a step
#[derive(Debug, Clone, Serialize)]
pub struct CommitTrackingResult {
    /// Commits created during the step
    pub commits: Vec<TrackedCommit>,

    /// Total files modified across all commits
    pub total_files_changed: usize,

    /// Total insertions across all commits
    pub total_insertions: usize,

    /// Total deletions across all commits
    pub total_deletions: usize,
}

impl CommitTrackingResult {
    /// Create from a list of commits
    pub fn from_commits(commits: Vec<TrackedCommit>) -> Self {
        let total_files_changed = commits
            .iter()
            .flat_map(|c| &c.files_changed)
            .collect::<std::collections::HashSet<_>>()
            .len();

        let total_insertions = commits.iter().map(|c| c.insertions).sum();
        let total_deletions = commits.iter().map(|c| c.deletions).sum();

        Self {
            commits,
            total_files_changed,
            total_insertions,
            total_deletions,
        }
    }
}

#[cfg(test)]
#[path = "commit_tracker_tests.rs"]
mod commit_tracker_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abstractions::MockGitOperations;

    #[tokio::test]
    async fn test_has_changes() {
        let mock_git = Arc::new(MockGitOperations::new());
        mock_git.add_success_response("M  src/main.rs\n").await;

        let tracker = CommitTracker::new(mock_git.clone(), PathBuf::from("/test"));
        assert!(tracker.has_changes().await.unwrap());

        // Test with no changes
        mock_git.add_success_response("").await;
        assert!(!tracker.has_changes().await.unwrap());
    }

    #[tokio::test]
    async fn test_get_current_head() {
        let mock_git = Arc::new(MockGitOperations::new());
        mock_git.add_success_response("abc123def456\n").await;

        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));
        let head = tracker.get_current_head().await.unwrap();
        assert_eq!(head, "abc123def456");
    }

    #[tokio::test]
    async fn test_interpolate_template() {
        let mock_git = Arc::new(MockGitOperations::new());
        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));

        let mut variables = HashMap::new();
        variables.insert("item".to_string(), "user.py".to_string());

        let result = tracker
            .interpolate_template(
                "feat: modernize ${item} in ${step.name}",
                "refactor-step",
                &variables,
            )
            .unwrap();

        assert_eq!(result, "feat: modernize user.py in refactor-step");
    }

    #[tokio::test]
    async fn test_validate_message() {
        let mock_git = Arc::new(MockGitOperations::new());
        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));

        // Valid conventional commit
        tracker
            .validate_message(
                "feat: add new feature",
                r"^(feat|fix|docs|style|refactor|test|chore):",
            )
            .unwrap();

        // Invalid message
        let result = tracker.validate_message(
            "bad message",
            r"^(feat|fix|docs|style|refactor|test|chore):",
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_commit_tracking_result() {
        let commits = vec![
            TrackedCommit {
                hash: "abc123".to_string(),
                message: "commit 1".to_string(),
                author: "test".to_string(),
                timestamp: Utc::now(),
                files_changed: vec![PathBuf::from("file1.rs"), PathBuf::from("file2.rs")],
                insertions: 10,
                deletions: 5,
                step_name: "step1".to_string(),
                agent_id: None,
            },
            TrackedCommit {
                hash: "def456".to_string(),
                message: "commit 2".to_string(),
                author: "test".to_string(),
                timestamp: Utc::now(),
                files_changed: vec![PathBuf::from("file2.rs"), PathBuf::from("file3.rs")],
                insertions: 20,
                deletions: 3,
                step_name: "step2".to_string(),
                agent_id: None,
            },
        ];

        let result = CommitTrackingResult::from_commits(commits);
        assert_eq!(result.total_files_changed, 3);
        assert_eq!(result.total_insertions, 30);
        assert_eq!(result.total_deletions, 8);
    }
}
