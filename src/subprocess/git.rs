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

/// Parse a git status branch line (format: "## branch...upstream")
/// Returns the local branch name if the line is a branch marker.
#[inline]
fn parse_branch_line(line: &str) -> Option<String> {
    line.strip_prefix("## ")
        .and_then(|branch_info| branch_info.split("...").next())
        .map(|s| s.to_string())
}

/// Parse a git status untracked file line (format: "?? filename")
/// Returns the filename if the line marks an untracked file.
#[inline]
fn parse_untracked_line(line: &str) -> Option<String> {
    line.strip_prefix("?? ").map(|file| file.to_string())
}

/// Parse a git status modified file line (any status code except untracked)
/// Returns the filename if the line is a valid file status line.
#[inline]
fn parse_modified_line(line: &str) -> Option<String> {
    if line.len() > 2 {
        Some(line[3..].to_string())
    } else {
        None
    }
}

/// Check if a command completed successfully, returning an error for non-zero exit codes.
/// This is a pure function that translates exit status into a Result.
#[inline]
fn check_command_success(status: &super::runner::ExitStatus) -> Result<(), ProcessError> {
    if status.success() {
        Ok(())
    } else {
        Err(ProcessError::ExitCode(status.code().unwrap_or(1)))
    }
}

/// Parse git status --porcelain output into structured data.
/// Returns a tuple of (branch_name, untracked_files, modified_files).
/// This is a pure function that performs no I/O.
fn parse_git_status_output(output: &str) -> (Option<String>, Vec<String>, Vec<String>) {
    let mut branch = None;
    let mut untracked_files = Vec::new();
    let mut modified_files = Vec::new();

    for line in output.lines() {
        if let Some(branch_name) = parse_branch_line(line) {
            branch = Some(branch_name);
            continue;
        }

        if let Some(file) = parse_untracked_line(line) {
            untracked_files.push(file);
            continue;
        }

        if let Some(file) = parse_modified_line(line) {
            modified_files.push(file);
        }
    }

    (branch, untracked_files, modified_files)
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

        check_command_success(&output.status)?;

        let (branch, untracked_files, modified_files) = parse_git_status_output(&output.stdout);

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

        check_command_success(&output.status)?;

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

        check_command_success(&output.status)?;

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

        check_command_success(&output.status)?;

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

        check_command_success(&output.status)?;

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

        check_command_success(&output.status)?;

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

        check_command_success(&output.status)?;

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

    #[tokio::test]
    async fn test_status_only_branch_no_files() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main...origin/main\n")
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
    async fn test_status_files_without_branch() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("?? untracked.rs\n M modified.rs\n")
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
        assert_eq!(status.modified_files.len(), 1);
    }

    #[tokio::test]
    async fn test_status_comprehensive_all_scenarios() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## feature/test...origin/feature/test [ahead 1, behind 2]\n?? new1.rs\n?? new2.rs\n M modified1.rs\nAM modified2.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("feature/test".to_string()));
        assert!(!status.clean);
        assert_eq!(status.untracked_files.len(), 2);
        assert_eq!(status.untracked_files[0], "new1.rs");
        assert_eq!(status.untracked_files[1], "new2.rs");
        assert_eq!(status.modified_files.len(), 2);
        assert_eq!(status.modified_files[0], "modified1.rs");
        assert_eq!(status.modified_files[1], "modified2.rs");
    }

    // Phase 1: Edge Case Coverage - Line Length Boundaries

    #[tokio::test]
    async fn test_status_line_length_exactly_two() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\nM \n M file.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert_eq!(status.modified_files.len(), 1);
        assert_eq!(status.modified_files[0], "file.rs");
    }

    #[tokio::test]
    async fn test_status_line_length_one() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\nM\n M file.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert_eq!(status.modified_files.len(), 1);
        assert_eq!(status.modified_files[0], "file.rs");
    }

    #[tokio::test]
    async fn test_status_empty_lines() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n\n\n M file.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert_eq!(status.modified_files.len(), 1);
        assert_eq!(status.modified_files[0], "file.rs");
    }

    #[tokio::test]
    async fn test_status_whitespace_only_lines() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n   \n\t\n M file.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        // Whitespace lines > 2 chars get parsed as modified files (extracting from char 3 onward)
        assert_eq!(status.modified_files.len(), 2);
        assert_eq!(status.modified_files[0], "");
        assert_eq!(status.modified_files[1], "file.rs");
    }

    // Phase 2: Git Status Code Coverage - Deleted, Renamed, Copied, Dual-Status

    #[tokio::test]
    async fn test_status_deleted_files() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n D deleted1.rs\n D deleted2.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert!(!status.clean);
        assert_eq!(status.modified_files.len(), 2);
        assert_eq!(status.modified_files[0], "deleted1.rs");
        assert_eq!(status.modified_files[1], "deleted2.rs");
    }

    #[tokio::test]
    async fn test_status_renamed_files() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n R old.rs -> new.rs\nR  another_old.rs -> another_new.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert!(!status.clean);
        assert_eq!(status.modified_files.len(), 2);
        assert_eq!(status.modified_files[0], "old.rs -> new.rs");
        assert_eq!(status.modified_files[1], "another_old.rs -> another_new.rs");
    }

    #[tokio::test]
    async fn test_status_copied_files() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\n C original.rs -> copy.rs\nC  file1.rs -> file2.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert!(!status.clean);
        assert_eq!(status.modified_files.len(), 2);
        assert_eq!(status.modified_files[0], "original.rs -> copy.rs");
        assert_eq!(status.modified_files[1], "file1.rs -> file2.rs");
    }

    #[tokio::test]
    async fn test_status_dual_status_files() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## main\nMM staged_and_modified.rs\nAM added_and_modified.rs\nMA modified_and_added.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("main".to_string()));
        assert!(!status.clean);
        assert_eq!(status.modified_files.len(), 3);
        assert_eq!(status.modified_files[0], "staged_and_modified.rs");
        assert_eq!(status.modified_files[1], "added_and_modified.rs");
        assert_eq!(status.modified_files[2], "modified_and_added.rs");
    }

    // Phase 3: Branch Parsing Edge Cases

    #[tokio::test]
    async fn test_status_branch_with_spaces_no_upstream() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## feature/branch with spaces\n M file.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(
            status.branch,
            Some("feature/branch with spaces".to_string())
        );
        assert!(!status.clean);
    }

    #[tokio::test]
    async fn test_status_branch_with_special_characters() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## feature/foo-bar.baz_123\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some("feature/foo-bar.baz_123".to_string()));
        assert!(status.clean);
    }

    #[tokio::test]
    async fn test_status_branch_very_long_name() {
        let long_branch = "feature/very-long-branch-name-that-exceeds-typical-limits-but-is-still-technically-valid-in-git-repositories-with-many-nested-components";
        let output = format!("## {}\n", long_branch);
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout(&output)
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.branch, Some(long_branch.to_string()));
        assert!(status.clean);
    }

    #[tokio::test]
    async fn test_status_branch_multiple_separators() {
        let mut mock_runner = MockProcessRunner::new();
        mock_runner
            .expect_command("git")
            .with_args(|args| args == ["status", "--porcelain", "--branch"])
            .returns_stdout("## feature...origin...malformed\n M file.rs\n")
            .returns_success()
            .finish();

        let git = GitRunnerImpl::new(Arc::new(mock_runner));
        let temp_dir = TempDir::new().unwrap();
        let result = git.status(temp_dir.path()).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        // split("...").next() takes the first part before any "..."
        assert_eq!(status.branch, Some("feature".to_string()));
        assert!(!status.clean);
    }
}

#[cfg(test)]
mod parse_git_status_output_tests {
    use super::parse_git_status_output;

    #[test]
    fn test_empty_output() {
        let (branch, untracked, modified) = parse_git_status_output("");
        assert_eq!(branch, None);
        assert!(untracked.is_empty());
        assert!(modified.is_empty());
    }

    #[test]
    fn test_branch_only() {
        let (branch, untracked, modified) = parse_git_status_output("## main\n");
        assert_eq!(branch, Some("main".to_string()));
        assert!(untracked.is_empty());
        assert!(modified.is_empty());
    }

    #[test]
    fn test_branch_with_upstream() {
        let (branch, untracked, modified) =
            parse_git_status_output("## feature/test...origin/feature/test\n");
        assert_eq!(branch, Some("feature/test".to_string()));
        assert!(untracked.is_empty());
        assert!(modified.is_empty());
    }

    #[test]
    fn test_untracked_files_only() {
        let (branch, untracked, modified) =
            parse_git_status_output("?? file1.rs\n?? file2.txt\n?? dir/file3.rs\n");
        assert_eq!(branch, None);
        assert_eq!(untracked.len(), 3);
        assert_eq!(untracked[0], "file1.rs");
        assert_eq!(untracked[1], "file2.txt");
        assert_eq!(untracked[2], "dir/file3.rs");
        assert!(modified.is_empty());
    }

    #[test]
    fn test_modified_files_only() {
        let (branch, untracked, modified) =
            parse_git_status_output(" M modified.rs\n A added.rs\n D deleted.rs\n");
        assert_eq!(branch, None);
        assert!(untracked.is_empty());
        assert_eq!(modified.len(), 3);
        assert_eq!(modified[0], "modified.rs");
        assert_eq!(modified[1], "added.rs");
        assert_eq!(modified[2], "deleted.rs");
    }

    #[test]
    fn test_mixed_status() {
        let output = "## main\n?? untracked.rs\n M modified.rs\n A added.rs\n";
        let (branch, untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("main".to_string()));
        assert_eq!(untracked.len(), 1);
        assert_eq!(untracked[0], "untracked.rs");
        assert_eq!(modified.len(), 2);
        assert_eq!(modified[0], "modified.rs");
        assert_eq!(modified[1], "added.rs");
    }

    #[test]
    fn test_comprehensive_all_scenarios() {
        let output = concat!(
            "## feature/test...origin/feature/test [ahead 1, behind 2]\n",
            "?? new1.rs\n",
            "?? new2.rs\n",
            " M modified1.rs\n",
            "AM modified2.rs\n"
        );
        let (branch, untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("feature/test".to_string()));
        assert_eq!(untracked.len(), 2);
        assert_eq!(untracked[0], "new1.rs");
        assert_eq!(untracked[1], "new2.rs");
        assert_eq!(modified.len(), 2);
        assert_eq!(modified[0], "modified1.rs");
        assert_eq!(modified[1], "modified2.rs");
    }

    #[test]
    fn test_empty_lines() {
        let output = "## main\n\n\n M file.rs\n";
        let (branch, _untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("main".to_string()));
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0], "file.rs");
    }

    #[test]
    fn test_whitespace_only_lines() {
        let output = "## main\n   \n\t\n M file.rs\n";
        let (branch, _untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("main".to_string()));
        // Whitespace lines > 2 chars get parsed as modified files
        assert_eq!(modified.len(), 2);
        assert_eq!(modified[0], "");
        assert_eq!(modified[1], "file.rs");
    }

    #[test]
    fn test_line_length_boundaries() {
        // Lines with length exactly 2 should not be parsed as modified
        let output = "## main\nM \n M file.rs\n";
        let (branch, _untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("main".to_string()));
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0], "file.rs");
    }

    #[test]
    fn test_line_length_one() {
        // Lines with length 1 should not be parsed as modified
        let output = "## main\nM\n M file.rs\n";
        let (branch, _untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("main".to_string()));
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0], "file.rs");
    }

    #[test]
    fn test_renamed_files() {
        let output = "## main\n R old.rs -> new.rs\nR  another_old.rs -> another_new.rs\n";
        let (branch, _untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("main".to_string()));
        assert_eq!(modified.len(), 2);
        assert_eq!(modified[0], "old.rs -> new.rs");
        assert_eq!(modified[1], "another_old.rs -> another_new.rs");
    }

    #[test]
    fn test_dual_status_files() {
        let output = "## main\nMM staged_and_modified.rs\nAM added_and_modified.rs\n";
        let (branch, _untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("main".to_string()));
        assert_eq!(modified.len(), 2);
        assert_eq!(modified[0], "staged_and_modified.rs");
        assert_eq!(modified[1], "added_and_modified.rs");
    }

    #[test]
    fn test_branch_with_spaces() {
        let output = "## feature/branch with spaces\n M file.rs\n";
        let (branch, _untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("feature/branch with spaces".to_string()));
        assert_eq!(modified.len(), 1);
    }

    #[test]
    fn test_malformed_branch_line() {
        let output = "## \n?? file.txt\n";
        let (branch, untracked, _modified) = parse_git_status_output(output);
        assert_eq!(branch, Some("".to_string()));
        assert_eq!(untracked.len(), 1);
    }

    #[test]
    fn test_no_branch_with_files() {
        let output = "?? untracked.rs\n M modified.rs\n";
        let (branch, untracked, modified) = parse_git_status_output(output);
        assert_eq!(branch, None);
        assert_eq!(untracked.len(), 1);
        assert_eq!(untracked[0], "untracked.rs");
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0], "modified.rs");
    }
}
