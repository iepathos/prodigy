//! Git operations abstraction layer
//!
//! Provides trait-based abstraction for git commands to enable
//! testing without actual git repository access.

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Trait for git operations
#[async_trait]
pub trait GitOperations: Send + Sync {
    /// Execute a git command with exclusive access
    async fn git_command(&self, args: &[&str], description: &str) -> Result<std::process::Output>;

    /// Get the last commit message
    async fn get_last_commit_message(&self) -> Result<String>;

    /// Check git status
    async fn check_git_status(&self) -> Result<String>;

    /// Stage all changes
    async fn stage_all_changes(&self) -> Result<()>;

    /// Create a commit
    async fn create_commit(&self, message: &str) -> Result<()>;

    /// Check if we're in a git repository
    async fn is_git_repo(&self) -> bool;

    /// Create a worktree
    async fn create_worktree(&self, name: &str, path: &Path) -> Result<()>;

    /// Get current branch name
    async fn get_current_branch(&self) -> Result<String>;

    /// Switch to a branch
    async fn switch_branch(&self, branch: &str) -> Result<()>;
}

/// Real implementation of GitOperations
pub struct RealGitOperations {
    /// Mutex for thread-safe git operations
    git_mutex: Arc<Mutex<()>>,
}

impl RealGitOperations {
    /// Create a new RealGitOperations instance
    pub fn new() -> Self {
        Self {
            git_mutex: Arc::new(Mutex::new(())),
        }
    }
}

impl Default for RealGitOperations {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitOperations for RealGitOperations {
    async fn git_command(&self, args: &[&str], description: &str) -> Result<std::process::Output> {
        // Acquire the mutex to ensure exclusive access
        let _guard = self.git_mutex.lock().await;

        let output = tokio::process::Command::new("git")
            .args(args)
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute git {}: {}", description, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Git {} failed: {}",
                description,
                stderr.trim()
            ));
        }

        Ok(output)
    }

    async fn get_last_commit_message(&self) -> Result<String> {
        let output = self
            .git_command(&["log", "-1", "--pretty=format:%s"], "log")
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn check_git_status(&self) -> Result<String> {
        let output = self
            .git_command(&["status", "--porcelain"], "status")
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn stage_all_changes(&self) -> Result<()> {
        self.git_command(&["add", "."], "add").await?;
        Ok(())
    }

    async fn create_commit(&self, message: &str) -> Result<()> {
        self.git_command(&["commit", "-m", message], "commit")
            .await?;
        Ok(())
    }

    async fn is_git_repo(&self) -> bool {
        tokio::process::Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    async fn create_worktree(&self, name: &str, path: &Path) -> Result<()> {
        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        self.git_command(&["worktree", "add", path_str, "-b", name], "worktree add")
            .await?;
        Ok(())
    }

    async fn get_current_branch(&self) -> Result<String> {
        let output = self
            .git_command(&["rev-parse", "--abbrev-ref", "HEAD"], "get current branch")
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn switch_branch(&self, branch: &str) -> Result<()> {
        self.git_command(&["checkout", branch], "checkout").await?;
        Ok(())
    }
}

/// Mock implementation of GitOperations for testing
pub struct MockGitOperations {
    /// Predefined responses for git commands
    pub command_responses: Arc<Mutex<Vec<Result<std::process::Output>>>>,
    /// Predefined response for is_git_repo
    pub is_repo: bool,
    /// Track called commands for verification
    pub called_commands: Arc<Mutex<Vec<Vec<String>>>>,
}

use crate::abstractions::exit_status::ExitStatusExt;

impl MockGitOperations {
    /// Create a new MockGitOperations instance
    pub fn new() -> Self {
        Self {
            command_responses: Arc::new(Mutex::new(Vec::new())),
            is_repo: true,
            called_commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a response for the next git command
    pub async fn add_response(&self, response: Result<std::process::Output>) {
        self.command_responses.lock().await.push(response);
    }

    /// Add a successful response with stdout content
    pub async fn add_success_response(&self, stdout: &str) {
        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        };
        self.add_response(Ok(output)).await;
    }

    /// Add an error response
    pub async fn add_error_response(&self, error: &str) {
        let error_string = error.to_string();
        self.add_response(Err(anyhow::anyhow!(error_string))).await;
    }

    /// Get the list of called commands
    pub async fn get_called_commands(&self) -> Vec<Vec<String>> {
        self.called_commands.lock().await.clone()
    }
}

impl Default for MockGitOperations {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitOperations for MockGitOperations {
    async fn git_command(&self, args: &[&str], _description: &str) -> Result<std::process::Output> {
        // Track the called command
        let cmd_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        self.called_commands.lock().await.push(cmd_vec);

        // Return the next predefined response
        let mut responses = self.command_responses.lock().await;
        if responses.is_empty() {
            return Err(anyhow::anyhow!("No mock response configured"));
        }
        responses.remove(0)
    }

    async fn get_last_commit_message(&self) -> Result<String> {
        let output = self
            .git_command(&["log", "-1", "--pretty=format:%s"], "log")
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn check_git_status(&self) -> Result<String> {
        let output = self
            .git_command(&["status", "--porcelain"], "status")
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn stage_all_changes(&self) -> Result<()> {
        self.git_command(&["add", "."], "add").await?;
        Ok(())
    }

    async fn create_commit(&self, message: &str) -> Result<()> {
        self.git_command(&["commit", "-m", message], "commit")
            .await?;
        Ok(())
    }

    async fn is_git_repo(&self) -> bool {
        self.is_repo
    }

    async fn create_worktree(&self, name: &str, path: &Path) -> Result<()> {
        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        self.git_command(&["worktree", "add", path_str, "-b", name], "worktree add")
            .await?;
        Ok(())
    }

    async fn get_current_branch(&self) -> Result<String> {
        let output = self
            .git_command(&["rev-parse", "--abbrev-ref", "HEAD"], "get current branch")
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn switch_branch(&self, branch: &str) -> Result<()> {
        self.git_command(&["checkout", branch], "checkout").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_git_operations() {
        let mock = MockGitOperations::new();

        // Add responses
        mock.add_success_response("test commit message").await;
        mock.add_success_response("M  src/main.rs\nA  src/new.rs")
            .await;

        // Test get_last_commit_message
        let msg = mock.get_last_commit_message().await.unwrap();
        assert_eq!(msg, "test commit message");

        // Test check_git_status
        let status = mock.check_git_status().await.unwrap();
        assert!(status.contains("M  src/main.rs"));

        // Verify called commands
        let commands = mock.get_called_commands().await;
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], vec!["log", "-1", "--pretty=format:%s"]);
        assert_eq!(commands[1], vec!["status", "--porcelain"]);
    }

    #[tokio::test]
    async fn test_mock_git_error() {
        let mock = MockGitOperations::new();

        // Add error response
        mock.add_error_response("fatal: not a git repository").await;

        // Test error handling
        let result = mock.get_last_commit_message().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not a git repository"));
    }

    #[tokio::test]
    async fn test_real_git_operations_is_git_repo() {
        let real = RealGitOperations::new();

        // This test will pass or fail depending on whether it's run in a git repo
        // Just verify it doesn't panic
        let _ = real.is_git_repo().await;
    }
}
