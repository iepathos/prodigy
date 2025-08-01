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
