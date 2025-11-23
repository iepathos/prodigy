//! Environment trait definitions for dependency injection and testing
//!
//! This module defines trait abstractions for all I/O operations, enabling:
//! - Pure business logic separation from side effects
//! - Easy mocking and testing
//! - Explicit dependency tracking
//! - Composable effects through stillwater's Effect type

use anyhow::Result;
use std::fs::Metadata;
use std::path::Path;
use std::process::{Child, Command, Output};

/// File system operations trait
///
/// Abstracts all file system interactions to enable testing with mock implementations.
///
/// # Examples
///
/// ```
/// use prodigy::env::FileEnv;
/// use std::path::Path;
///
/// fn read_config<E: FileEnv>(env: &E, path: &Path) -> Result<String, std::io::Error> {
///     env.read_to_string(path)
/// }
/// ```
pub trait FileEnv: Send + Sync {
    /// Read a file's contents as a string
    fn read_to_string(&self, path: &Path) -> std::io::Result<String>;

    /// Write a string to a file
    fn write(&self, path: &Path, content: &str) -> std::io::Result<()>;

    /// Check if a path exists
    fn exists(&self, path: &Path) -> bool;

    /// Get metadata for a path
    fn metadata(&self, path: &Path) -> std::io::Result<Metadata>;

    /// Create a directory (and all parent directories)
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;

    /// Remove a file
    fn remove_file(&self, path: &Path) -> std::io::Result<()>;

    /// Remove a directory and all its contents
    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()>;
}

/// Process execution trait
///
/// Abstracts process spawning and execution for testing and isolation.
///
/// # Examples
///
/// ```no_run
/// use prodigy::env::ProcessEnv;
/// use std::process::Command;
///
/// fn run_test<E: ProcessEnv>(env: &E) -> Result<(), std::io::Error> {
///     let mut cmd = Command::new("cargo");
///     cmd.arg("test");
///     let output = env.run(&cmd)?;
///     Ok(())
/// }
/// ```
pub trait ProcessEnv: Send + Sync {
    /// Spawn a child process
    fn spawn(&self, cmd: &mut Command) -> std::io::Result<Child>;

    /// Run a command and wait for output
    fn run(&self, cmd: &mut Command) -> std::io::Result<Output>;
}

/// Git operations trait
///
/// Abstracts git operations for testing without actual git repositories.
///
/// # Examples
///
/// ```no_run
/// use prodigy::env::GitEnv;
/// use std::path::Path;
///
/// fn create_branch<E: GitEnv>(env: &E, name: &str) -> Result<(), anyhow::Error> {
///     env.create_branch(name)
/// }
/// ```
pub trait GitEnv: Send + Sync {
    /// Add a git worktree
    fn worktree_add(&self, path: &Path, branch: &str) -> Result<()>;

    /// Remove a git worktree
    fn worktree_remove(&self, path: &Path) -> Result<()>;

    /// List all worktrees
    fn worktree_list(&self) -> Result<Vec<String>>;

    /// Merge a branch
    fn merge(&self, branch: &str) -> Result<()>;

    /// Create a commit
    fn commit(&self, message: &str) -> Result<String>;

    /// Get current HEAD SHA
    fn head_sha(&self) -> Result<String>;

    /// Create a new branch
    fn create_branch(&self, name: &str) -> Result<()>;

    /// Checkout a branch
    fn checkout(&self, branch: &str) -> Result<()>;

    /// Get current branch name
    fn current_branch(&self) -> Result<String>;

    /// Check if working directory is clean
    fn is_clean(&self) -> Result<bool>;
}

/// Database operations trait (placeholder for future storage abstraction)
///
/// This trait will be implemented when we refactor storage operations.
pub trait DbEnv: Send + Sync {
    // Placeholder for future database operations
    // Example methods that will be added:
    // fn save_workflow(&self, workflow: &Workflow) -> Result<()>;
    // fn fetch_workflow(&self, id: &str) -> Result<Workflow>;
    // fn save_event(&self, event: &Event) -> Result<()>;
}
