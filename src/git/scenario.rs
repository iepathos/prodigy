//! Scenario-based git mocking for comprehensive testing

use super::{error::GitError, types::*, GitOperations, GitReader, GitWorktree, GitWriter};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Scenario-based mock for git operations
pub struct GitScenarioMock {
    scenarios: Arc<Mutex<HashMap<PathBuf, GitScenario>>>,
    default_scenario: GitScenario,
    command_log: Arc<Mutex<Vec<GitCommand>>>,
}

/// Git scenario definition
#[derive(Debug, Clone)]
pub struct GitScenario {
    /// Initial repository state
    pub initial_state: GitRepoState,
    /// Responses for specific commands
    pub responses: HashMap<String, ScenarioResponse>,
    /// Whether the path is a git repository
    pub is_repository: bool,
}

/// Response configuration for a specific git command
#[derive(Debug, Clone)]
pub enum ScenarioResponse {
    /// Return a success result with optional output
    Success(Option<String>),
    /// Return an error
    Error(GitError),
}

/// Logged git command for verification
#[derive(Debug, Clone)]
pub struct GitCommand {
    /// Command arguments
    pub args: Vec<String>,
    /// Working directory
    pub working_dir: PathBuf,
    /// Timestamp
    pub timestamp: std::time::Instant,
}

impl GitScenarioMock {
    /// Create a new scenario mock
    pub fn new() -> Self {
        Self {
            scenarios: Arc::new(Mutex::new(HashMap::new())),
            default_scenario: GitScenario::default(),
            command_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Configure scenario for a specific path
    pub async fn set_scenario(&self, path: &Path, scenario: GitScenario) {
        self.scenarios
            .lock()
            .await
            .insert(path.to_path_buf(), scenario);
    }

    /// Create a clean repository scenario
    pub async fn with_clean_repo(&mut self, path: &Path) {
        let scenario = GitScenario {
            initial_state: GitRepoState {
                current_branch: Some("main".to_string()),
                current_commit: CommitId::new("abc123def456".to_string()),
                status: GitStatus::new(),
                branches: vec!["main".to_string()],
                tags: vec![],
                remotes: vec!["origin".to_string()],
            },
            responses: HashMap::new(),
            is_repository: true,
        };

        self.set_scenario(path, scenario).await;
    }

    /// Create a dirty repository scenario
    pub async fn with_dirty_repo(&mut self, path: &Path) {
        let mut status = GitStatus::new();
        status.modified.push(PathBuf::from("modified.rs"));
        status.untracked.push(PathBuf::from("untracked.txt"));

        let scenario = GitScenario {
            initial_state: GitRepoState {
                current_branch: Some("main".to_string()),
                current_commit: CommitId::new("abc123def456".to_string()),
                status,
                branches: vec!["main".to_string()],
                tags: vec![],
                remotes: vec!["origin".to_string()],
            },
            responses: HashMap::new(),
            is_repository: true,
        };

        self.set_scenario(path, scenario).await;
    }

    /// Create a merge conflict scenario
    pub async fn with_merge_conflict(&mut self, path: &Path) {
        let mut status = GitStatus::new();
        status.conflicts.push(PathBuf::from("conflicted.rs"));
        status.in_merge = true;

        let scenario = GitScenario {
            initial_state: GitRepoState {
                current_branch: Some("main".to_string()),
                current_commit: CommitId::new("abc123def456".to_string()),
                status,
                branches: vec!["main".to_string(), "feature".to_string()],
                tags: vec![],
                remotes: vec!["origin".to_string()],
            },
            responses: HashMap::new(),
            is_repository: true,
        };

        self.set_scenario(path, scenario).await;
    }

    /// Create a detached HEAD scenario
    pub async fn with_detached_head(&mut self, path: &Path) {
        let scenario = GitScenario {
            initial_state: GitRepoState {
                current_branch: None,
                current_commit: CommitId::new("abc123def456".to_string()),
                status: GitStatus::new(),
                branches: vec!["main".to_string()],
                tags: vec!["v1.0.0".to_string()],
                remotes: vec!["origin".to_string()],
            },
            responses: HashMap::new(),
            is_repository: true,
        };

        self.set_scenario(path, scenario).await;
    }

    /// Create a non-repository scenario
    pub async fn with_non_repository(&mut self, path: &Path) {
        let scenario = GitScenario {
            initial_state: GitRepoState {
                current_branch: None,
                current_commit: CommitId::new("".to_string()),
                status: GitStatus::new(),
                branches: vec![],
                tags: vec![],
                remotes: vec![],
            },
            responses: HashMap::new(),
            is_repository: false,
        };

        self.set_scenario(path, scenario).await;
    }

    /// Add a custom response for a specific command
    pub async fn when_command(&mut self, path: &Path, command: &str, response: ScenarioResponse) {
        let mut scenarios = self.scenarios.lock().await;
        let scenario = scenarios
            .entry(path.to_path_buf())
            .or_insert_with(GitScenario::default);
        scenario.responses.insert(command.to_string(), response);
    }

    /// Get logged commands for verification
    pub async fn get_command_log(&self) -> Vec<GitCommand> {
        self.command_log.lock().await.clone()
    }

    /// Clear command log
    pub async fn clear_command_log(&self) {
        self.command_log.lock().await.clear();
    }

    /// Get scenario for a path
    async fn get_scenario(&self, path: &Path) -> GitScenario {
        self.scenarios
            .lock()
            .await
            .get(path)
            .cloned()
            .unwrap_or_else(|| self.default_scenario.clone())
    }

    /// Log a command execution
    async fn log_command(&self, working_dir: &Path, args: &[&str]) {
        let command = GitCommand {
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: working_dir.to_path_buf(),
            timestamp: std::time::Instant::now(),
        };

        self.command_log.lock().await.push(command);
    }

    /// Execute a mock command
    async fn execute_command(&self, path: &Path, args: &[&str]) -> Result<String> {
        self.log_command(path, args).await;

        let scenario = self.get_scenario(path).await;
        let command_key = args.join(" ");

        if let Some(response) = scenario.responses.get(&command_key) {
            match response {
                ScenarioResponse::Success(output) => Ok(output.clone().unwrap_or_default()),
                ScenarioResponse::Error(error) => Err(error.clone().into()),
            }
        } else {
            // Default responses based on command
            self.default_response(path, args).await
        }
    }

    /// Provide default responses for common commands
    async fn default_response(&self, path: &Path, args: &[&str]) -> Result<String> {
        let scenario = self.get_scenario(path).await;

        match args {
            ["rev-parse", "--git-dir"] => {
                if scenario.is_repository {
                    Ok(".git".to_string())
                } else {
                    Err(GitError::NotARepository.into())
                }
            }
            ["branch", "--show-current"] => {
                if let Some(branch) = &scenario.initial_state.current_branch {
                    Ok(branch.clone())
                } else {
                    Ok("".to_string()) // Detached HEAD
                }
            }
            ["status", "--porcelain=v2"] => {
                let mut status = scenario.initial_state.status.clone();
                status.branch = scenario.initial_state.current_branch.clone();
                Ok(format_status_output(&status))
            }
            ["log", "-1", "--pretty=format:%s", ref_] => {
                if *ref_ == "HEAD" || *ref_ == scenario.initial_state.current_commit.hash() {
                    Ok("test commit message".to_string())
                } else {
                    Err(GitError::CommitNotFound(ref_.to_string()).into())
                }
            }
            ["ls-files"] => Ok("src/main.rs\nsrc/lib.rs\nCargo.toml\n".to_string()),
            ["add", ".."] => Ok("".to_string()),
            ["commit", "-m", _] => {
                if scenario.initial_state.status.is_clean() {
                    Err(GitError::NothingToCommit.into())
                } else {
                    Ok("[main abc1234] test commit".to_string())
                }
            }
            ["rev-parse", "HEAD"] => Ok(scenario.initial_state.current_commit.hash().to_string()),
            _ => {
                // Unknown command - return empty success
                Ok("".to_string())
            }
        }
    }
}

impl Default for GitScenarioMock {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for GitScenario {
    fn default() -> Self {
        Self {
            initial_state: GitRepoState {
                current_branch: Some("main".to_string()),
                current_commit: CommitId::new("abc123def456".to_string()),
                status: GitStatus::new(),
                branches: vec!["main".to_string()],
                tags: vec![],
                remotes: vec![],
            },
            responses: HashMap::new(),
            is_repository: true,
        }
    }
}

/// Format git status for porcelain v2 output
fn format_status_output(status: &GitStatus) -> String {
    let mut output = String::new();

    if let Some(branch) = &status.branch {
        output.push_str(&format!("# branch.head {branch}\n"));
    } else {
        output.push_str("# branch.head (detached)\n");
    }

    // Add merge state indicator if in merge
    if status.in_merge {
        output.push_str("# merge.in-progress true\n");
    }

    for file in &status.added {
        output.push_str(&format!(
            "1 A. N... 000000 100644 100644 000000 abc123 {}\n",
            file.display()
        ));
    }

    for file in &status.modified {
        output.push_str(&format!(
            "1 .M N... 100644 100644 100644 abc123 def456 {}\n",
            file.display()
        ));
    }

    for file in &status.deleted {
        output.push_str(&format!(
            "1 .D N... 100644 000000 000000 abc123 000000 {}\n",
            file.display()
        ));
    }

    for file in &status.untracked {
        output.push_str(&format!("? {}\n", file.display()));
    }

    for file in &status.conflicts {
        output.push_str(&format!(
            "u UU N... 100644 100644 100644 100644 abc123 def456 ghi789 {}\n",
            file.display()
        ));
    }

    for (old, new) in &status.renamed {
        output.push_str(&format!(
            "2 R. N... 100644 100644 100644 abc123 def456 R100 {}\t{}\n",
            new.display(),
            old.display()
        ));
    }

    output
}

#[async_trait]
impl GitReader for GitScenarioMock {
    async fn is_repository(&self, path: &Path) -> Result<bool> {
        let scenario = self.get_scenario(path).await;
        Ok(scenario.is_repository)
    }

    async fn get_status(&self, path: &Path) -> Result<GitStatus> {
        let output = self
            .execute_command(path, &["status", "--porcelain=v2"])
            .await?;
        super::parsers::parse_status_output(&output)
    }

    async fn get_current_branch(&self, path: &Path) -> Result<String> {
        let output = self
            .execute_command(path, &["branch", "--show-current"])
            .await?;
        let branch = output.trim();
        if branch.is_empty() {
            Err(GitError::DetachedHead.into())
        } else {
            Ok(branch.to_string())
        }
    }

    async fn get_commit_message(&self, path: &Path, ref_: &str) -> Result<String> {
        let output = self
            .execute_command(path, &["log", "-1", "--pretty=format:%s", ref_])
            .await?;
        Ok(output.trim().to_string())
    }

    async fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let output = self.execute_command(path, &["ls-files"]).await?;
        Ok(output
            .lines()
            .map(|line| PathBuf::from(line.trim()))
            .collect())
    }

    async fn get_diff(&self, path: &Path, from: &str, to: &str) -> Result<GitDiff> {
        let range = format!("{from}..{to}");
        let output = self
            .execute_command(path, &["diff", "--numstat", &range])
            .await?;
        super::parsers::parse_diff_output(&output)
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
impl GitWriter for GitScenarioMock {
    async fn init_repository(&self, path: &Path) -> Result<()> {
        self.execute_command(path, &["init"]).await?;
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

        self.execute_command(path, &args).await?;
        Ok(())
    }

    async fn stage_all(&self, path: &Path) -> Result<()> {
        self.execute_command(path, &["add", "."]).await?;
        Ok(())
    }

    async fn commit(&self, path: &Path, message: &str) -> Result<CommitId> {
        let result = self.execute_command(path, &["commit", "-m", message]).await;
        match result {
            Err(e) if e.to_string().contains("nothing to commit") => {
                return Err(GitError::NothingToCommit.into());
            }
            Err(e) => return Err(e),
            Ok(_) => {}
        }

        let hash_output = self.execute_command(path, &["rev-parse", "HEAD"]).await?;
        Ok(CommitId::new(hash_output.trim().to_string()))
    }

    async fn create_branch(&self, path: &Path, name: &str) -> Result<()> {
        self.execute_command(path, &["branch", name]).await?;
        Ok(())
    }

    async fn switch_branch(&self, path: &Path, name: &str) -> Result<()> {
        self.execute_command(path, &["checkout", name]).await?;
        Ok(())
    }

    async fn delete_branch(&self, path: &Path, name: &str) -> Result<()> {
        self.execute_command(path, &["branch", "-d", name]).await?;
        Ok(())
    }
}

#[async_trait]
impl GitWorktree for GitScenarioMock {
    async fn create_worktree(&self, repo: &Path, name: &str, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy();
        self.execute_command(repo, &["worktree", "add", "-b", name, &path_str])
            .await?;
        Ok(())
    }

    async fn remove_worktree(&self, repo: &Path, name: &str) -> Result<()> {
        self.execute_command(repo, &["worktree", "remove", name, "--force"])
            .await?;
        Ok(())
    }

    async fn list_worktrees(&self, repo: &Path) -> Result<Vec<WorktreeInfo>> {
        let output = self
            .execute_command(repo, &["worktree", "list", "--porcelain"])
            .await?;
        super::parsers::parse_worktree_list(&output)
    }

    async fn prune_worktrees(&self, repo: &Path) -> Result<()> {
        self.execute_command(repo, &["worktree", "prune"]).await?;
        Ok(())
    }
}

impl GitOperations for GitScenarioMock {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_scenario_mock_clean_repo() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();
        mock.with_clean_repo(temp_dir.path()).await;

        let is_repo = mock.is_repository(temp_dir.path()).await.unwrap();
        assert!(is_repo);

        let status = mock.get_status(temp_dir.path()).await.unwrap();
        assert!(status.is_clean());
        assert_eq!(status.branch, Some("main".to_string()));

        let branch = mock.get_current_branch(temp_dir.path()).await.unwrap();
        assert_eq!(branch, "main");

        let is_clean = mock.is_clean(temp_dir.path()).await.unwrap();
        assert!(is_clean);
    }

    #[tokio::test]
    async fn test_scenario_mock_dirty_repo() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();
        mock.with_dirty_repo(temp_dir.path()).await;

        let status = mock.get_status(temp_dir.path()).await.unwrap();
        assert!(!status.is_clean());
        assert_eq!(status.modified.len(), 1);
        assert_eq!(status.untracked.len(), 1);

        let is_clean = mock.is_clean(temp_dir.path()).await.unwrap();
        assert!(!is_clean);
    }

    #[tokio::test]
    async fn test_scenario_mock_merge_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();
        mock.with_merge_conflict(temp_dir.path()).await;

        let status = mock.get_status(temp_dir.path()).await.unwrap();
        assert!(status.has_conflicts());
        assert_eq!(status.conflicts.len(), 1);
        assert!(status.in_merge);
    }

    #[tokio::test]
    async fn test_scenario_mock_detached_head() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();
        mock.with_detached_head(temp_dir.path()).await;

        let result = mock.get_current_branch(temp_dir.path()).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("detached"));
    }

    #[tokio::test]
    async fn test_scenario_mock_non_repository() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();
        mock.with_non_repository(temp_dir.path()).await;

        let is_repo = mock.is_repository(temp_dir.path()).await.unwrap();
        assert!(!is_repo);
    }

    #[tokio::test]
    async fn test_scenario_mock_custom_response() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();

        mock.when_command(
            temp_dir.path(),
            "log -1 --pretty=format:%s HEAD",
            ScenarioResponse::Success(Some("Custom commit message".to_string())),
        )
        .await;

        let message = mock.get_last_commit_message(temp_dir.path()).await.unwrap();
        assert_eq!(message, "Custom commit message");
    }

    #[tokio::test]
    async fn test_scenario_mock_command_logging() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();
        mock.with_clean_repo(temp_dir.path()).await;

        let _ = mock.is_repository(temp_dir.path()).await;
        let _ = mock.get_current_branch(temp_dir.path()).await;

        let log = mock.get_command_log().await;
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].args, vec!["rev-parse", "--git-dir"]);
        assert_eq!(log[1].args, vec!["branch", "--show-current"]);
    }

    #[tokio::test]
    async fn test_scenario_mock_commit_nothing_to_commit() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock = GitScenarioMock::new();
        mock.with_clean_repo(temp_dir.path()).await;

        let result = mock.commit(temp_dir.path(), "test message").await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("nothing to commit"));
    }
}
