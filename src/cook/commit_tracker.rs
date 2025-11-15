//! Git commit tracking and verification
//!
//! This module provides comprehensive commit tracking functionality for workflows,
//! including automatic commit creation, metadata collection, and verification.

use crate::abstractions::GitOperations;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use glob::Pattern;
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

/// Strategy for staging files before commit
#[derive(Debug, Clone, PartialEq)]
enum StagingStrategy {
    /// Stage all changes
    All,
    /// Stage only files matching the patterns
    Selective,
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
                    .position(|w| w.starts_with("insertions(+)") || w.starts_with("insertion(+)"))
                    .and_then(|i| stats.split_whitespace().nth(i.saturating_sub(1)))
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    commit.insertions = insertions;
                }

                if let Some(deletions) = stats
                    .split_whitespace()
                    .position(|w| w.starts_with("deletions(-)") || w.starts_with("deletion(-)"))
                    .and_then(|i| stats.split_whitespace().nth(i.saturating_sub(1)))
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    commit.deletions = deletions;
                }
            }
        }

        Ok(commits)
    }

    /// Check if GPG signing is properly configured
    async fn check_gpg_config(&self) -> Result<bool> {
        // Check if GPG signing is configured in git
        let output = self
            .git_ops
            .git_command_in_dir(
                &["config", "--get", "commit.gpgsign"],
                "check GPG config",
                &self.working_dir,
            )
            .await
            .ok();

        if let Some(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim() == "true" {
                // Check if a signing key is configured
                let key_output = self
                    .git_ops
                    .git_command_in_dir(
                        &["config", "--get", "user.signingkey"],
                        "check signing key",
                        &self.working_dir,
                    )
                    .await
                    .ok();

                if let Some(key_output) = key_output {
                    let key_stdout = String::from_utf8_lossy(&key_output.stdout);
                    if !key_stdout.trim().is_empty() {
                        // Verify GPG is available and the key exists
                        let gpg_check = self
                            .git_ops
                            .git_command_in_dir(
                                &["config", "--get", "gpg.program"],
                                "check GPG program",
                                &self.working_dir,
                            )
                            .await
                            .ok();

                        let gpg_program = if let Some(gpg_output) = gpg_check {
                            String::from_utf8_lossy(&gpg_output.stdout)
                                .trim()
                                .to_string()
                        } else {
                            "gpg".to_string()
                        };

                        // Try to list the key to verify it exists
                        let key = key_stdout.trim();
                        let check_key_cmd = format!("{} --list-secret-keys {}", gpg_program, key);

                        // Run the GPG check using shell command
                        let key_exists = std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&check_key_cmd)
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false);

                        return Ok(key_exists);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Parse a git status line to extract the filename
    ///
    /// Returns Some(filename) if the line is valid (length > 3), None otherwise
    fn parse_git_status_line(line: &str) -> Option<String> {
        if line.len() > 3 {
            Some(line[3..].trim().to_string())
        } else {
            None
        }
    }

    /// Check if a file should be included based on include patterns
    ///
    /// Returns false if include_patterns is empty (no patterns = exclude all)
    /// Returns true if any pattern matches the file
    /// Handles invalid patterns gracefully by skipping them
    fn should_include_file(file: &str, include_patterns: &[String]) -> bool {
        if include_patterns.is_empty() {
            return false;
        }

        for pattern_str in include_patterns {
            if let Ok(pattern) = Pattern::new(pattern_str) {
                if pattern.matches(file) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a file should be excluded based on exclude patterns
    ///
    /// Returns false if exclude_patterns is empty (no patterns = exclude nothing)
    /// Returns true if any pattern matches the file
    /// Handles invalid patterns gracefully by skipping them
    fn should_exclude_file(file: &str, exclude_patterns: &[String]) -> bool {
        if exclude_patterns.is_empty() {
            return false;
        }

        for pattern_str in exclude_patterns {
            if let Ok(pattern) = Pattern::new(pattern_str) {
                if pattern.matches(file) {
                    return true;
                }
            }
        }

        false
    }

    /// Determine if a file should be staged based on commit configuration
    ///
    /// Returns true if the file should be staged, considering both include and exclude patterns
    /// - If config is None, returns true (stage all files)
    /// - Otherwise, checks include patterns first, then exclude patterns
    fn should_stage_file(file: &str, config: Option<&CommitConfig>) -> bool {
        match config {
            None => true, // No config means stage all files
            Some(cfg) => {
                // Check include patterns
                let passes_include = match &cfg.include_files {
                    Some(patterns) => Self::should_include_file(file, patterns),
                    None => true, // No include patterns means include all
                };

                // If file doesn't pass include check, exclude it
                if !passes_include {
                    return false;
                }

                // Check exclude patterns
                match &cfg.exclude_files {
                    Some(patterns) => !Self::should_exclude_file(file, patterns),
                    None => true, // No exclude patterns means exclude none
                }
            }
        }
    }

    /// Filter files based on include/exclude patterns
    async fn get_files_to_stage(
        &self,
        commit_config: Option<&CommitConfig>,
    ) -> Result<Vec<String>> {
        // Get all changed files
        let output = self
            .git_ops
            .git_command_in_dir(&["status", "--porcelain"], "get status", &self.working_dir)
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();
        for line in stdout.lines() {
            if let Some(file) = Self::parse_git_status_line(line) {
                if Self::should_stage_file(&file, commit_config) {
                    files.push(file);
                }
            }
        }

        Ok(files)
    }

    /// Determine the staging strategy based on commit configuration (pure function)
    ///
    /// Returns `StagingStrategy::Selective` if include or exclude patterns are specified,
    /// otherwise returns `StagingStrategy::All` for default behavior.
    fn determine_staging_strategy(commit_config: Option<&CommitConfig>) -> StagingStrategy {
        match commit_config {
            Some(config)
                if config.include_files.is_some() || config.exclude_files.is_some() =>
            {
                StagingStrategy::Selective
            }
            _ => StagingStrategy::All,
        }
    }

    /// Stage files based on the staging strategy
    ///
    /// For `StagingStrategy::All`, stages all changes with `git add .`.
    /// For `StagingStrategy::Selective`, stages only files matching include/exclude patterns.
    async fn stage_files(
        &self,
        strategy: StagingStrategy,
        commit_config: Option<&CommitConfig>,
    ) -> Result<()> {
        match strategy {
            StagingStrategy::All => {
                self.git_ops
                    .git_command_in_dir(&["add", "."], "stage all changes", &self.working_dir)
                    .await?;
            }
            StagingStrategy::Selective => {
                let files_to_stage = self.get_files_to_stage(commit_config).await?;

                if files_to_stage.is_empty() {
                    return Err(anyhow!("No files match the specified patterns"));
                }

                for file in files_to_stage {
                    self.git_ops
                        .git_command_in_dir(&["add", &file], "stage file", &self.working_dir)
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Generate commit message from template or step name (pure function)
    ///
    /// Interpolates variables in the template using the provided step name and variables map.
    /// If no template is provided, returns a default message format.
    fn generate_commit_message(
        step_name: &str,
        template: Option<&str>,
        variables: &HashMap<String, String>,
    ) -> String {
        match template {
            Some(tmpl) => {
                let mut message = tmpl.to_string();

                // Replace ${step.name}
                message = message.replace("${step.name}", step_name);

                // Replace other variables
                for (key, value) in variables {
                    message = message.replace(&format!("${{{key}}}"), value);
                    message = message.replace(&format!("${key}"), value);
                }

                message
            }
            None => format!("Auto-commit: {step_name}"),
        }
    }

    /// Validate commit message against a regex pattern (pure function)
    ///
    /// Returns Ok(()) if the message matches the pattern, or an error with details if it doesn't.
    fn validate_commit_message(message: &str, pattern: &str) -> Result<()> {
        let re = regex::Regex::new(pattern)
            .map_err(|e| anyhow!("Invalid message pattern: {e}"))?;

        if !re.is_match(message) {
            return Err(anyhow!(
                "Commit message '{}' does not match required pattern '{}'",
                message,
                pattern
            ));
        }

        Ok(())
    }

    /// Prepare and validate commit message (combines generation and validation)
    ///
    /// Generates the message from template/step name, then validates it against the pattern
    /// if one is configured in commit_config.
    fn prepare_commit_message(
        step_name: &str,
        template: Option<&str>,
        variables: &HashMap<String, String>,
        commit_config: Option<&CommitConfig>,
    ) -> Result<String> {
        let message = Self::generate_commit_message(step_name, template, variables);

        if let Some(config) = commit_config {
            if let Some(pattern) = &config.message_pattern {
                Self::validate_commit_message(&message, pattern)?;
            }
        }

        Ok(message)
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

        // Determine staging strategy and stage files
        let strategy = Self::determine_staging_strategy(commit_config);
        self.stage_files(strategy, commit_config).await?;

        // Prepare and validate commit message
        let message = Self::prepare_commit_message(step_name, message_template, variables, commit_config)?;

        // Create the commit with optional author override and signing
        let mut commit_args = vec!["commit", "-m", &message];

        // Add author override if specified from commit_config
        let author_string;
        if let Some(config) = commit_config {
            if let Some(author) = &config.author {
                author_string = format!("--author={}", author);
                commit_args.push(&author_string);
            }

            // Add GPG signing if enabled and properly configured
            if config.sign {
                if self.check_gpg_config().await? {
                    commit_args.push("-S");
                } else {
                    log::warn!(
                        "GPG signing requested but not properly configured, skipping signing"
                    );
                }
            }
        }

        self.git_ops
            .git_command_in_dir(&commit_args, "create commit", &self.working_dir)
            .await?;

        // Get the new HEAD
        let new_head = self.get_current_head().await?;

        // Get commit details
        let mut commits = self
            .get_commits_between(&format!("{new_head}^"), &new_head)
            .await?;

        if let Some(mut commit) = commits.pop() {
            commit.step_name = step_name.to_string();

            // Add to tracked commits
            let mut tracked = self.tracked_commits.write().await;
            tracked.push(commit.clone());

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

    /// Interpolate variables in a message template (delegates to pure function)
    ///
    /// This method exists for backward compatibility with tests.
    /// New code should use `generate_commit_message` directly.
    #[cfg(test)]
    fn interpolate_template(
        &self,
        template: &str,
        step_name: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        Ok(Self::generate_commit_message(step_name, Some(template), variables))
    }

    /// Validate a commit message against a pattern (delegates to pure function)
    pub fn validate_message(&self, message: &str, pattern: &str) -> Result<()> {
        Self::validate_commit_message(message, pattern)
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

    #[tokio::test]
    async fn test_step_commits_variable_format() {
        // Create test commits with known values
        let timestamp = DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let commits = vec![
            TrackedCommit {
                hash: "abc123def456789".to_string(),
                message: "feat: implement new feature".to_string(),
                author: "Test Author <test@example.com>".to_string(),
                timestamp,
                files_changed: vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")],
                insertions: 42,
                deletions: 17,
                step_name: "implement-feature".to_string(),
                agent_id: Some("agent-001".to_string()),
            },
            TrackedCommit {
                hash: "fedcba987654321".to_string(),
                message: "fix: resolve bug in parser".to_string(),
                author: "Bug Fixer <fix@example.com>".to_string(),
                timestamp: timestamp + chrono::Duration::minutes(30),
                files_changed: vec![PathBuf::from("src/parser.rs")],
                insertions: 5,
                deletions: 3,
                step_name: "fix-bug".to_string(),
                agent_id: None,
            },
        ];

        // Serialize to JSON (mimicking what executor.rs does)
        let json_str = serde_json::to_string(&commits).unwrap();

        // Parse back to verify structure
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify it's an array
        assert!(parsed.is_array());
        let commits_array = parsed.as_array().unwrap();
        assert_eq!(commits_array.len(), 2);

        // Verify first commit structure
        let first_commit = &commits_array[0];
        assert_eq!(first_commit["hash"], "abc123def456789");
        assert_eq!(first_commit["message"], "feat: implement new feature");
        assert_eq!(first_commit["author"], "Test Author <test@example.com>");
        assert_eq!(first_commit["step_name"], "implement-feature");
        assert_eq!(first_commit["agent_id"], "agent-001");
        assert_eq!(first_commit["insertions"], 42);
        assert_eq!(first_commit["deletions"], 17);

        // Verify files_changed is an array
        assert!(first_commit["files_changed"].is_array());
        let files = first_commit["files_changed"].as_array().unwrap();
        assert_eq!(files.len(), 2);

        // Verify timestamp is ISO 8601 format
        assert!(first_commit["timestamp"].is_string());
        let timestamp_str = first_commit["timestamp"].as_str().unwrap();
        assert!(timestamp_str.contains("2024-01-01"));
        assert!(timestamp_str.contains("T"));
        assert!(timestamp_str.ends_with("Z"));

        // Verify second commit has null agent_id
        assert!(commits_array[1]["agent_id"].is_null());

        // Verify the format can be used in variable interpolation
        // This is what would be available as ${step.commits}
        assert!(json_str.contains("hash"));
        assert!(json_str.contains("message"));
        assert!(json_str.contains("files_changed"));
        assert!(json_str.contains("insertions"));
        assert!(json_str.contains("deletions"));
    }
}
