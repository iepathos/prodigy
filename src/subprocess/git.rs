use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use super::builder::ProcessCommandBuilder;
use super::error::ProcessError;
use super::runner::ProcessRunner;
use crate::abstractions::exit_status::ExitStatusExt;

#[derive(Debug, Clone)]
pub struct GitStatus {
    pub branch: Option<String>,
    pub clean: bool,
    pub untracked_files: Vec<String>,
    pub modified_files: Vec<String>,
}

#[async_trait]
pub trait GitRunner: Send + Sync {
    async fn status(&self, path: &Path) -> Result<GitStatus, ProcessError>;
    async fn commit(&self, path: &Path, message: &str) -> Result<String, ProcessError>;
    async fn add(&self, path: &Path, files: &[&str]) -> Result<(), ProcessError>;
    async fn create_worktree(
        &self,
        path: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<(), ProcessError>;
    async fn remove_worktree(&self, path: &Path, worktree_name: &str) -> Result<(), ProcessError>;
    async fn current_branch(&self, path: &Path) -> Result<String, ProcessError>;
    async fn log(
        &self,
        path: &Path,
        format: &str,
        max_count: usize,
    ) -> Result<String, ProcessError>;
    async fn run_command(&self, args: &[&str]) -> Result<std::process::Output, ProcessError>;
}

pub struct GitRunnerImpl {
    runner: Arc<dyn ProcessRunner>,
}

impl GitRunnerImpl {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl GitRunner for GitRunnerImpl {
    async fn status(&self, path: &Path) -> Result<GitStatus, ProcessError> {
        let output = self
            .runner
            .run(
                ProcessCommandBuilder::new("git")
                    .args(["status", "--porcelain", "--branch"])
                    .current_dir(path)
                    .build(),
            )
            .await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        let mut branch = None;
        let mut untracked_files = Vec::new();
        let mut modified_files = Vec::new();

        for line in output.stdout.lines() {
            if line.starts_with("## ") {
                if let Some(branch_info) = line.strip_prefix("## ") {
                    branch = branch_info.split("...").next().map(|s| s.to_string());
                }
            } else if line.starts_with("??") {
                if let Some(file) = line.strip_prefix("?? ") {
                    untracked_files.push(file.to_string());
                }
            } else if line.len() > 2 {
                let file = line[3..].to_string();
                modified_files.push(file);
            }
        }

        Ok(GitStatus {
            branch,
            clean: untracked_files.is_empty() && modified_files.is_empty(),
            untracked_files,
            modified_files,
        })
    }

    async fn commit(&self, path: &Path, message: &str) -> Result<String, ProcessError> {
        let output = self
            .runner
            .run(
                ProcessCommandBuilder::new("git")
                    .args(["commit", "-m", message])
                    .current_dir(path)
                    .build(),
            )
            .await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        // Extract commit hash from output
        for line in output.stdout.lines() {
            if line.contains("commit") {
                if let Some(hash) = line.split_whitespace().nth(1) {
                    return Ok(hash.to_string());
                }
            }
        }

        Ok(String::new())
    }

    async fn add(&self, path: &Path, files: &[&str]) -> Result<(), ProcessError> {
        let mut args = vec!["add"];
        args.extend(files);

        let output = self
            .runner
            .run(
                ProcessCommandBuilder::new("git")
                    .args(&args)
                    .current_dir(path)
                    .build(),
            )
            .await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        Ok(())
    }

    async fn create_worktree(
        &self,
        path: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<(), ProcessError> {
        let output = self
            .runner
            .run(
                ProcessCommandBuilder::new("git")
                    .args(["worktree", "add", "-b", branch])
                    .arg(worktree_path.to_string_lossy().as_ref())
                    .current_dir(path)
                    .build(),
            )
            .await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        Ok(())
    }

    async fn remove_worktree(&self, path: &Path, worktree_name: &str) -> Result<(), ProcessError> {
        let output = self
            .runner
            .run(
                ProcessCommandBuilder::new("git")
                    .args(["worktree", "remove", worktree_name, "--force"])
                    .current_dir(path)
                    .build(),
            )
            .await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        Ok(())
    }

    async fn current_branch(&self, path: &Path) -> Result<String, ProcessError> {
        let output = self
            .runner
            .run(
                ProcessCommandBuilder::new("git")
                    .args(["branch", "--show-current"])
                    .current_dir(path)
                    .build(),
            )
            .await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        Ok(output.stdout.trim().to_string())
    }

    async fn log(
        &self,
        path: &Path,
        format: &str,
        max_count: usize,
    ) -> Result<String, ProcessError> {
        let output = self
            .runner
            .run(
                ProcessCommandBuilder::new("git")
                    .args([
                        "log",
                        &format!("--pretty=format:{format}"),
                        &format!("--max-count={max_count}"),
                    ])
                    .current_dir(path)
                    .build(),
            )
            .await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        Ok(output.stdout)
    }

    async fn run_command(&self, args: &[&str]) -> Result<std::process::Output, ProcessError> {
        let command = ProcessCommandBuilder::new("git").args(args).build();

        let output = self.runner.run(command).await?;

        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(output.status.code().unwrap_or(1)),
            stdout: output.stdout.into_bytes(),
            stderr: output.stderr.into_bytes(),
        })
    }
}

#[cfg(test)]
mod git_error_tests {
    use super::*;
    use crate::subprocess::mock::MockProcessRunner;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_git_command_failure() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stderr("fatal: not a git repository")
            .returns_exit_code(128)
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ProcessError::ExitCode(_) => (),
            _ => panic!("Expected ExitCode error"),
        }
    }

    #[tokio::test]
    async fn test_git_parse_errors() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args.len() >= 2 && args[0] == "log")
            .returns_stdout("invalid log format")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.log(temp_dir.path(), "%H", 10).await;

        assert!(result.is_ok()); // Log returns Ok with the output
        assert_eq!(result.unwrap(), "invalid log format");
    }

    #[tokio::test]
    async fn test_status_clean_repository() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert!(status.clean);
        assert!(status.untracked_files.is_empty());
        assert!(status.modified_files.is_empty());
    }

    #[tokio::test]
    async fn test_status_with_branch_information() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## feature/test...origin/feature/test\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("feature/test".to_string()));
        assert!(status.clean);
    }

    #[tokio::test]
    async fn test_status_exit_code_error() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stderr("fatal: not a git repository")
            .returns_exit_code(128)
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ProcessError::ExitCode(code) => assert_eq!(code, 128),
            _ => panic!("Expected ExitCode error"),
        }
    }

    #[tokio::test]
    async fn test_status_with_untracked_files() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n?? new_file.rs\n?? another.txt\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(!status.clean);
        assert_eq!(status.untracked_files.len(), 2);
        assert_eq!(status.untracked_files[0], "new_file.rs");
        assert_eq!(status.untracked_files[1], "another.txt");
        assert!(status.modified_files.is_empty());
    }

    #[tokio::test]
    async fn test_status_with_modified_files() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n M src/lib.rs\n A src/new.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(!status.clean);
        assert_eq!(status.modified_files.len(), 2);
        assert_eq!(status.modified_files[0], "src/lib.rs");
        assert_eq!(status.modified_files[1], "src/new.rs");
        assert!(status.untracked_files.is_empty());
    }

    #[tokio::test]
    async fn test_status_with_mixed_status() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n?? untracked.rs\n M modified.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(!status.clean);
        assert_eq!(status.untracked_files.len(), 1);
        assert_eq!(status.untracked_files[0], "untracked.rs");
        assert_eq!(status.modified_files.len(), 1);
        assert_eq!(status.modified_files[0], "modified.rs");
    }

    #[tokio::test]
    async fn test_status_with_empty_output() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.clean);
        assert!(status.branch.is_none());
        assert!(status.untracked_files.is_empty());
        assert!(status.modified_files.is_empty());
    }

    #[tokio::test]
    async fn test_status_branch_with_upstream() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## feature/my-branch...origin/feature/my-branch [ahead 2]\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("feature/my-branch".to_string()));
        assert!(status.clean);
    }

    #[tokio::test]
    async fn test_status_branch_without_upstream() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## develop\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("develop".to_string()));
        assert!(status.clean);
    }

    #[tokio::test]
    async fn test_status_detached_head() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("?? file.txt\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.branch.is_none());
        assert!(!status.clean);
        assert_eq!(status.untracked_files.len(), 1);
    }

    #[tokio::test]
    async fn test_status_malformed_branch_line() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## \n?? file.txt\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("".to_string()));
        assert!(!status.clean);
    }
}
