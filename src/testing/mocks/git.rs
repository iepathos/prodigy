//! Mock Git operations implementation for testing

use crate::abstractions::exit_status::ExitStatusExt;
use crate::abstractions::git::GitOperations;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, PartialEq)]
enum GitCommandType {
    Status,
    Log,
    Add,
    Commit,
    Other,
}

impl GitCommandType {
    /// Classify git command based on arguments - pure function for testability
    fn classify(args: &[&str]) -> Self {
        match args.first() {
            Some(&"status") if args.get(1) == Some(&"--porcelain") => Self::Status,
            Some(&"log") if args.get(1) == Some(&"-1") => Self::Log,
            Some(&"add") => Self::Add,
            Some(&"commit") => Self::Commit,
            _ => Self::Other,
        }
    }
}

/// Builder for creating configured mock Git operations
pub struct MockGitOperationsBuilder {
    is_repo: bool,
    responses: VecDeque<Result<String>>,
    status_responses: VecDeque<String>,
    commit_messages: Vec<String>,
}

impl Default for MockGitOperationsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGitOperationsBuilder {
    pub fn new() -> Self {
        Self {
            is_repo: true,
            responses: VecDeque::new(),
            status_responses: VecDeque::new(),
            commit_messages: Vec::new(),
        }
    }

    pub fn is_repo(mut self, is_repo: bool) -> Self {
        self.is_repo = is_repo;
        self
    }

    pub fn with_status(mut self, status: &str) -> Self {
        self.status_responses.push_back(status.to_string());
        self
    }

    pub fn with_clean_status(mut self) -> Self {
        self.status_responses.push_back(String::new());
        self
    }

    pub fn with_dirty_status(mut self, files: Vec<&str>) -> Self {
        let status = files.join("\n");
        self.status_responses.push_back(status);
        self
    }

    pub fn with_commit_message(mut self, message: &str) -> Self {
        self.commit_messages.push(message.to_string());
        self
    }

    pub fn with_response(mut self, response: Result<String>) -> Self {
        self.responses.push_back(response);
        self
    }

    pub fn build(self) -> MockGitOperations {
        MockGitOperations {
            is_repo: self.is_repo,
            responses: Arc::new(Mutex::new(self.responses)),
            status_responses: Arc::new(Mutex::new(self.status_responses)),
            commit_messages: Arc::new(Mutex::new(self.commit_messages)),
            staged_files: Arc::new(Mutex::new(Vec::new())),
            commits: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Mock implementation of GitOperations for testing
pub struct MockGitOperations {
    is_repo: bool,
    responses: Arc<Mutex<VecDeque<Result<String>>>>,
    status_responses: Arc<Mutex<VecDeque<String>>>,
    commit_messages: Arc<Mutex<Vec<String>>>,
    staged_files: Arc<Mutex<Vec<String>>>,
    commits: Arc<Mutex<Vec<String>>>,
}

impl Default for MockGitOperations {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGitOperations {
    pub fn new() -> Self {
        Self {
            is_repo: true,
            responses: Arc::new(Mutex::new(VecDeque::new())),
            status_responses: Arc::new(Mutex::new(VecDeque::new())),
            commit_messages: Arc::new(Mutex::new(Vec::new())),
            staged_files: Arc::new(Mutex::new(Vec::new())),
            commits: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn builder() -> MockGitOperationsBuilder {
        MockGitOperationsBuilder::new()
    }

    pub fn get_staged_files(&self) -> Vec<String> {
        self.staged_files.lock().unwrap().clone()
    }

    pub fn get_commits(&self) -> Vec<String> {
        self.commits.lock().unwrap().clone()
    }

    fn next_response(&self) -> Result<String> {
        let mut responses = self.responses.lock().unwrap();
        responses
            .pop_front()
            .unwrap_or(Ok("Mock response".to_string()))
    }
}

#[async_trait]
impl GitOperations for MockGitOperations {
    async fn git_command(&self, args: &[&str], _description: &str) -> Result<std::process::Output> {
        // Use the pure classifier function to determine command type
        let output = match GitCommandType::classify(args) {
            GitCommandType::Status => {
                let mut status_responses = self.status_responses.lock().unwrap();
                status_responses.pop_front().unwrap_or_default()
            }
            GitCommandType::Log => {
                let messages = self.commit_messages.lock().unwrap();
                messages
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "Initial commit".to_string())
            }
            GitCommandType::Add => {
                let mut staged = self.staged_files.lock().unwrap();
                staged.push("all files".to_string());
                String::new()
            }
            GitCommandType::Commit => {
                let message = args.get(2).unwrap_or(&"commit");
                let mut commits = self.commits.lock().unwrap();
                commits.push(message.to_string());
                String::new()
            }
            GitCommandType::Other => self.next_response()?,
        };

        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: output.into_bytes(),
            stderr: Vec::new(),
        })
    }

    async fn git_command_in_dir(
        &self,
        args: &[&str],
        description: &str,
        _working_dir: &std::path::Path,
    ) -> Result<std::process::Output> {
        // Delegate to git_command for simplicity in mocks
        self.git_command(args, description).await
    }

    async fn get_last_commit_message(&self) -> Result<String> {
        let messages = self.commit_messages.lock().unwrap();
        Ok(messages
            .last()
            .cloned()
            .unwrap_or_else(|| "Initial commit".to_string()))
    }

    async fn check_git_status(&self) -> Result<String> {
        let mut status_responses = self.status_responses.lock().unwrap();
        Ok(status_responses.pop_front().unwrap_or_default())
    }

    async fn stage_all_changes(&self) -> Result<()> {
        let mut staged = self.staged_files.lock().unwrap();
        staged.push("all files".to_string());
        Ok(())
    }

    async fn create_commit(&self, message: &str) -> Result<()> {
        let mut commits = self.commits.lock().unwrap();
        commits.push(message.to_string());
        Ok(())
    }

    async fn is_git_repo(&self) -> bool {
        self.is_repo
    }

    async fn create_worktree(&self, _name: &str, _path: &std::path::Path) -> Result<()> {
        Ok(())
    }

    async fn get_current_branch(&self) -> Result<String> {
        Ok("main".to_string())
    }

    async fn switch_branch(&self, _branch: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_command_type_classify_status() {
        assert_eq!(
            GitCommandType::classify(&["status", "--porcelain"]),
            GitCommandType::Status
        );
        assert_eq!(GitCommandType::classify(&["status"]), GitCommandType::Other);
    }

    #[test]
    fn test_git_command_type_classify_log() {
        assert_eq!(
            GitCommandType::classify(&["log", "-1"]),
            GitCommandType::Log
        );
        assert_eq!(
            GitCommandType::classify(&["log", "--oneline"]),
            GitCommandType::Other
        );
        assert_eq!(GitCommandType::classify(&["log"]), GitCommandType::Other);
    }

    #[test]
    fn test_git_command_type_classify_add() {
        assert_eq!(GitCommandType::classify(&["add", "."]), GitCommandType::Add);
        assert_eq!(
            GitCommandType::classify(&["add", "file.rs"]),
            GitCommandType::Add
        );
        assert_eq!(GitCommandType::classify(&["add"]), GitCommandType::Add);
    }

    #[test]
    fn test_git_command_type_classify_commit() {
        assert_eq!(
            GitCommandType::classify(&["commit", "-m", "message"]),
            GitCommandType::Commit
        );
        assert_eq!(
            GitCommandType::classify(&["commit", "--amend"]),
            GitCommandType::Commit
        );
        assert_eq!(
            GitCommandType::classify(&["commit"]),
            GitCommandType::Commit
        );
    }

    #[test]
    fn test_git_command_type_classify_other() {
        assert_eq!(GitCommandType::classify(&["push"]), GitCommandType::Other);
        assert_eq!(GitCommandType::classify(&["pull"]), GitCommandType::Other);
        assert_eq!(
            GitCommandType::classify(&["checkout", "branch"]),
            GitCommandType::Other
        );
        assert_eq!(GitCommandType::classify(&[]), GitCommandType::Other);
    }

    #[test]
    fn test_git_command_type_classify_edge_cases() {
        // Empty args
        assert_eq!(GitCommandType::classify(&[]), GitCommandType::Other);

        // Partial matches that shouldn't match
        assert_eq!(
            GitCommandType::classify(&["status", "something"]),
            GitCommandType::Other
        );

        // Case sensitive check
        assert_eq!(
            GitCommandType::classify(&["STATUS", "--porcelain"]),
            GitCommandType::Other
        );
    }

    #[tokio::test]
    async fn test_mock_git_builder() {
        let mock = MockGitOperationsBuilder::new()
            .is_repo(true)
            .with_clean_status()
            .with_commit_message("feat: add new feature")
            .build();

        assert!(mock.is_git_repo().await);
        assert_eq!(mock.check_git_status().await.unwrap(), "");
        assert_eq!(
            mock.get_last_commit_message().await.unwrap(),
            "feat: add new feature"
        );
    }

    #[tokio::test]
    async fn test_mock_git_dirty_status() {
        let mock = MockGitOperationsBuilder::new()
            .with_dirty_status(vec!["M  src/main.rs", "A  src/new.rs"])
            .build();

        let status = mock.check_git_status().await.unwrap();
        assert!(status.contains("src/main.rs"));
        assert!(status.contains("src/new.rs"));
    }

    #[tokio::test]
    async fn test_mock_git_staging() {
        let mock = MockGitOperations::new();

        mock.stage_all_changes().await.unwrap();

        let staged = mock.get_staged_files();
        assert_eq!(staged.len(), 1);
        assert!(staged.contains(&"all files".to_string()));
    }

    #[tokio::test]
    async fn test_mock_git_commits() {
        let mock = MockGitOperations::new();

        mock.create_commit("Initial commit").await.unwrap();
        mock.create_commit("Add feature").await.unwrap();

        let commits = mock.get_commits();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0], "Initial commit");
        assert_eq!(commits[1], "Add feature");
    }

    #[tokio::test]
    async fn test_git_command_with_classifier() {
        let mock = MockGitOperationsBuilder::new()
            .with_clean_status()
            .with_commit_message("test commit")
            .build();

        // Test status command
        let output = mock
            .git_command(&["status", "--porcelain"], "status")
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");

        // Test log command
        let output = mock.git_command(&["log", "-1"], "log").await.unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "test commit");

        // Test add command
        let output = mock.git_command(&["add", "."], "add").await.unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        assert_eq!(mock.get_staged_files().len(), 1);

        // Test commit command
        let output = mock
            .git_command(&["commit", "-m", "new commit"], "commit")
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        assert_eq!(mock.get_commits().len(), 1);
        assert_eq!(mock.get_commits()[0], "new commit");
    }
}
