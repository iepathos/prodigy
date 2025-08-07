//! Mock Git operations implementation for testing

use crate::abstractions::exit_status::ExitStatusExt;
use crate::abstractions::git::GitOperations;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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
        // Simulate different git commands based on args
        let output = if args.starts_with(&["status", "--porcelain"]) {
            let mut status_responses = self.status_responses.lock().unwrap();
            status_responses.pop_front().unwrap_or_default()
        } else if args.starts_with(&["log", "-1"]) {
            let messages = self.commit_messages.lock().unwrap();
            messages
                .last()
                .cloned()
                .unwrap_or_else(|| "Initial commit".to_string())
        } else if args.starts_with(&["add"]) {
            let mut staged = self.staged_files.lock().unwrap();
            staged.push("all files".to_string());
            String::new()
        } else if args.starts_with(&["commit"]) {
            let message = args.get(2).unwrap_or(&"commit");
            let mut commits = self.commits.lock().unwrap();
            commits.push(message.to_string());
            String::new()
        } else {
            self.next_response()?
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
}
