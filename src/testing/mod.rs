//! Testing utilities and fixtures
//!
//! This module provides test helpers, fixtures, and utilities for
//! comprehensive testing of the mmm codebase.

use crate::abstractions::{ClaudeClient, GitOperations, MockClaudeClient, MockGitOperations};
use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test context containing all necessary mocks and utilities
pub struct TestContext {
    /// Mock git operations
    pub git_ops: Box<dyn GitOperations>,
    /// Mock Claude client
    pub claude_client: Box<dyn ClaudeClient>,
    /// Temporary directory for test files
    pub temp_dir: TempDir,
}

impl TestContext {
    /// Create a new test context with default mocks
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let git_ops = Box::new(MockGitOperations::new());
        let claude_client = Box::new(MockClaudeClient::new());

        Ok(Self {
            git_ops,
            claude_client,
            temp_dir,
        })
    }

    /// Create a test context with custom mocks
    pub fn with_mocks(
        git_ops: Box<dyn GitOperations>,
        claude_client: Box<dyn ClaudeClient>,
    ) -> Result<Self> {
        let temp_dir = TempDir::new()?;

        Ok(Self {
            git_ops,
            claude_client,
            temp_dir,
        })
    }

    /// Get the path to the temporary directory
    pub fn temp_path(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    /// Create a test file in the temporary directory
    pub fn create_test_file(&self, name: &str, content: &str) -> Result<PathBuf> {
        use std::fs;
        let file_path = self.temp_dir.path().join(name);
        fs::write(&file_path, content)?;
        Ok(file_path)
    }
}

/// Builder for creating mock git operations with predefined responses
pub struct MockGitBuilder {
    mock: MockGitOperations,
}

impl Default for MockGitBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGitBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            mock: MockGitOperations::new(),
        }
    }

    /// Set whether this is a git repository
    pub fn is_repo(mut self, is_repo: bool) -> Self {
        self.mock.is_repo = is_repo;
        self
    }

    /// Add a successful git command response
    pub async fn with_success(self, stdout: &str) -> Self {
        self.mock.add_success_response(stdout).await;
        self
    }

    /// Add an error response
    pub async fn with_error(self, error: &str) -> Self {
        self.mock.add_error_response(error).await;
        self
    }

    /// Build the mock
    pub fn build(self) -> MockGitOperations {
        self.mock
    }
}

/// Builder for creating mock Claude client with predefined responses
pub struct MockClaudeBuilder {
    mock: MockClaudeClient,
}

impl Default for MockClaudeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockClaudeBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            mock: MockClaudeClient::new(),
        }
    }

    /// Set whether Claude CLI is available
    pub fn is_available(mut self, available: bool) -> Self {
        self.mock.is_available = available;
        self
    }

    /// Add a successful command response
    pub async fn with_success(self, stdout: &str) -> Self {
        self.mock.add_success_response(stdout).await;
        self
    }

    /// Add an error response
    pub async fn with_error(self, stderr: &str, exit_code: i32) -> Self {
        self.mock.add_error_response(stderr, exit_code).await;
        self
    }

    /// Build the mock
    pub fn build(self) -> MockClaudeClient {
        self.mock
    }
}

/// Test fixture for common scenarios
pub struct TestFixtures;

impl TestFixtures {
    /// Create a mock git operations that simulates a clean repository
    pub async fn clean_repo_git() -> MockGitOperations {
        MockGitBuilder::new()
            .is_repo(true)
            .with_success("") // Empty status means clean
            .await
            .with_success("Initial commit") // Last commit message
            .await
            .build()
    }

    /// Create a mock git operations that simulates a dirty repository
    pub async fn dirty_repo_git() -> MockGitOperations {
        MockGitBuilder::new()
            .is_repo(true)
            .with_success("M  src/main.rs\nA  src/new.rs") // Modified and added files
            .await
            .with_success("Previous commit")
            .await
            .build()
    }

    /// Create a mock Claude client that always succeeds
    pub async fn successful_claude() -> MockClaudeClient {
        MockClaudeBuilder::new()
            .is_available(true)
            .with_success("Review completed successfully")
            .await
            .with_success("Implementation completed")
            .await
            .with_success("Linting completed")
            .await
            .build()
    }

    /// Create a mock Claude client that simulates rate limiting
    pub async fn rate_limited_claude() -> MockClaudeClient {
        MockClaudeBuilder::new()
            .is_available(true)
            .with_error("Error: rate limit exceeded", 1)
            .await
            .build()
    }

    /// Create a mock Claude client that is not installed
    pub fn unavailable_claude() -> MockClaudeClient {
        let mut mock = MockClaudeClient::new();
        mock.is_available = false;
        mock
    }
}

/// Common test helpers for context modules
#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Standard imports that context test modules typically need
    pub use tempfile::TempDir;

    /// Creates a test project structure with common directories
    pub fn setup_test_project(temp_dir: &TempDir) -> PathBuf {
        let project_path = temp_dir.path().to_path_buf();

        // Create standard project structure
        fs::create_dir_all(project_path.join("src")).expect("Failed to create src dir");
        fs::create_dir_all(project_path.join("tests")).expect("Failed to create tests dir");
        fs::create_dir_all(project_path.join("benches")).expect("Failed to create benches dir");

        project_path
    }

    /// Creates a test file with the given content
    pub fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let file_path = dir.join(name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent directory");
        }
        fs::write(&file_path, content).expect("Failed to write test file");
        file_path
    }

    /// Creates multiple test files from a list of (path, content) tuples
    pub fn create_test_files(dir: &Path, files: &[(&str, &str)]) {
        for (path, content) in files {
            create_test_file(dir, path, content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_creation() {
        let context = TestContext::new().unwrap();
        assert!(context.temp_dir.path().exists());
    }

    #[tokio::test]
    async fn test_create_test_file() {
        let context = TestContext::new().unwrap();
        let file_path = context
            .create_test_file("test.txt", "Hello, world!")
            .unwrap();

        assert!(file_path.exists());
        let content = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_mock_builders() {
        let git_mock = MockGitBuilder::new()
            .is_repo(true)
            .with_success("test output")
            .await
            .build();

        assert!(git_mock.is_repo);

        let claude_mock = MockClaudeBuilder::new()
            .is_available(true)
            .with_success("test response")
            .await
            .build();

        assert!(claude_mock.is_available);
    }

    #[tokio::test]
    async fn test_fixtures() {
        let clean_git = TestFixtures::clean_repo_git().await;
        assert!(clean_git.is_repo);

        let dirty_git = TestFixtures::dirty_repo_git().await;
        assert!(dirty_git.is_repo);

        let successful_claude = TestFixtures::successful_claude().await;
        assert!(successful_claude.is_available);

        let unavailable_claude = TestFixtures::unavailable_claude();
        assert!(!unavailable_claude.is_available);
    }
}
