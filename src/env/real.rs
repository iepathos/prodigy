//! Real environment implementations that interact with the actual system
//!
//! These implementations are used in production code and perform actual I/O operations.

use super::traits::{DbEnv, FileEnv, GitEnv, ProcessEnv};
use anyhow::{Context, Result};
use std::fs::{self, Metadata};
use std::path::Path;
use std::process::{Child, Command, Output};

/// Real file system implementation
///
/// Delegates all operations to the standard library's `std::fs` module.
#[derive(Debug, Clone, Default)]
pub struct RealFileEnv;

impl RealFileEnv {
    pub fn new() -> Self {
        Self
    }
}

impl FileEnv for RealFileEnv {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        fs::read_to_string(path)
    }

    fn write(&self, path: &Path, content: &str) -> std::io::Result<()> {
        fs::write(path, content)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn metadata(&self, path: &Path) -> std::io::Result<Metadata> {
        fs::metadata(path)
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()> {
        fs::remove_dir_all(path)
    }
}

/// Real process execution implementation
///
/// Delegates to standard library's `std::process::Command`.
#[derive(Debug, Clone, Default)]
pub struct RealProcessEnv;

impl RealProcessEnv {
    pub fn new() -> Self {
        Self
    }
}

impl ProcessEnv for RealProcessEnv {
    fn spawn(&self, cmd: &mut Command) -> std::io::Result<Child> {
        cmd.spawn()
    }

    fn run(&self, cmd: &mut Command) -> std::io::Result<Output> {
        cmd.output()
    }
}

/// Real git operations implementation
///
/// Uses the `git2` crate for git operations where possible, falls back to shell commands.
#[derive(Debug, Clone)]
pub struct RealGitEnv {
    /// Working directory for git operations
    working_dir: std::path::PathBuf,
}

impl RealGitEnv {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
        }
    }

    /// Run a git command in the working directory
    fn run_git(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| format!("Failed to run git {:?}", args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git command failed: {:?}\n{}", args, stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

impl GitEnv for RealGitEnv {
    fn worktree_add(&self, path: &Path, branch: &str) -> Result<()> {
        self.run_git(&[
            "worktree",
            "add",
            path.to_str().context("Invalid path")?,
            branch,
        ])?;
        Ok(())
    }

    fn worktree_remove(&self, path: &Path) -> Result<()> {
        self.run_git(&["worktree", "remove", path.to_str().context("Invalid path")?])?;
        Ok(())
    }

    fn worktree_list(&self) -> Result<Vec<String>> {
        let output = self.run_git(&["worktree", "list", "--porcelain"])?;
        Ok(output.lines().map(String::from).collect())
    }

    fn merge(&self, branch: &str) -> Result<()> {
        self.run_git(&["merge", branch])?;
        Ok(())
    }

    fn commit(&self, message: &str) -> Result<String> {
        self.run_git(&["commit", "-m", message])?;
        self.head_sha()
    }

    fn head_sha(&self) -> Result<String> {
        self.run_git(&["rev-parse", "HEAD"])
    }

    fn create_branch(&self, name: &str) -> Result<()> {
        self.run_git(&["branch", name])?;
        Ok(())
    }

    fn checkout(&self, branch: &str) -> Result<()> {
        self.run_git(&["checkout", branch])?;
        Ok(())
    }

    fn current_branch(&self) -> Result<String> {
        self.run_git(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    fn is_clean(&self) -> Result<bool> {
        let output = self.run_git(&["status", "--porcelain"])?;
        Ok(output.is_empty())
    }
}

/// Real database implementation (placeholder)
///
/// This will be implemented when we refactor storage operations.
#[derive(Debug, Clone, Default)]
pub struct RealDbEnv;

impl RealDbEnv {
    pub fn new() -> Self {
        Self
    }
}

impl DbEnv for RealDbEnv {
    // Placeholder implementation
}
