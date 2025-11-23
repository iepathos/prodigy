//! Mock environment implementations for testing
//!
//! These implementations use in-memory data structures and provide controlled,
//! predictable behavior for testing without actual I/O operations.

use super::traits::{DbEnv, FileEnv, GitEnv, ProcessEnv};
use anyhow::Result;
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::sync::{Arc, Mutex};

/// Mock file system for testing
///
/// Stores files in memory and provides controlled file system operations.
///
/// # Examples
///
/// ```
/// use prodigy::env::{MockFileEnv, FileEnv};
/// use std::path::Path;
///
/// let env = MockFileEnv::new();
/// env.add_file("config.yml", "name: test");
///
/// let content = env.read_to_string(Path::new("config.yml")).unwrap();
/// assert_eq!(content, "name: test");
/// ```
#[derive(Debug, Clone)]
pub struct MockFileEnv {
    files: Arc<Mutex<HashMap<PathBuf, String>>>,
}

impl MockFileEnv {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a file to the mock file system
    pub fn add_file(&self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.files
            .lock()
            .unwrap()
            .insert(path.into(), content.into());
    }

    /// Get all files in the mock file system
    pub fn files(&self) -> HashMap<PathBuf, String> {
        self.files.lock().unwrap().clone()
    }

    /// Clear all files from the mock file system
    pub fn clear(&self) {
        self.files.lock().unwrap().clear();
    }
}

impl Default for MockFileEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl FileEnv for MockFileEnv {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("File not found: {}", path.display()),
                )
            })
    }

    fn write(&self, path: &Path, content: &str) -> std::io::Result<()> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), content.to_string());
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }

    fn metadata(&self, _path: &Path) -> std::io::Result<Metadata> {
        // For mock purposes, we don't need actual metadata
        // Tests that need metadata should use real file system
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Metadata not supported in mock file system",
        ))
    }

    fn create_dir_all(&self, _path: &Path) -> std::io::Result<()> {
        // In mock, directories are implicit
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        self.files
            .lock()
            .unwrap()
            .remove(path)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("File not found: {}", path.display()),
                )
            })
            .map(|_| ())
    }

    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()> {
        // Remove all files that start with this path
        let mut files = self.files.lock().unwrap();
        files.retain(|p, _| !p.starts_with(path));
        Ok(())
    }
}

/// Mock process environment for testing
///
/// Records command executions and returns predefined outputs.
#[derive(Debug, Clone)]
pub struct MockProcessEnv {
    commands: Arc<Mutex<Vec<String>>>,
    outputs: Arc<Mutex<HashMap<String, Output>>>,
}

impl MockProcessEnv {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            outputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record that a command was executed
    fn record_command(&self, cmd: &Command) {
        let cmd_str = format!("{:?}", cmd);
        self.commands.lock().unwrap().push(cmd_str);
    }

    /// Get all commands that were executed
    pub fn commands(&self) -> Vec<String> {
        self.commands.lock().unwrap().clone()
    }

    /// Set a predefined output for a command
    pub fn set_output(&self, cmd_pattern: impl Into<String>, output: Output) {
        self.outputs
            .lock()
            .unwrap()
            .insert(cmd_pattern.into(), output);
    }
}

impl Default for MockProcessEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessEnv for MockProcessEnv {
    fn spawn(&self, cmd: &mut Command) -> std::io::Result<Child> {
        self.record_command(cmd);
        // Mock spawn is not supported - use run() instead
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Spawn not supported in mock environment, use run() instead",
        ))
    }

    fn run(&self, cmd: &mut Command) -> std::io::Result<Output> {
        self.record_command(cmd);

        // Return predefined output if available
        let cmd_str = format!("{:?}", cmd);
        if let Some(output) = self.outputs.lock().unwrap().get(&cmd_str) {
            return Ok(output.clone());
        }

        // Default: successful empty output
        Ok(Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

/// Mock git environment for testing
///
/// Simulates git operations without actual repository manipulation.
#[derive(Debug, Clone)]
pub struct MockGitEnv {
    operations: Arc<Mutex<Vec<String>>>,
    branches: Arc<Mutex<Vec<String>>>,
    current_branch: Arc<Mutex<String>>,
    head_sha: Arc<Mutex<String>>,
    is_clean: Arc<Mutex<bool>>,
}

impl MockGitEnv {
    pub fn new() -> Self {
        Self {
            operations: Arc::new(Mutex::new(Vec::new())),
            branches: Arc::new(Mutex::new(vec!["main".to_string()])),
            current_branch: Arc::new(Mutex::new("main".to_string())),
            head_sha: Arc::new(Mutex::new("abc123".to_string())),
            is_clean: Arc::new(Mutex::new(true)),
        }
    }

    /// Record a git operation
    fn record_operation(&self, operation: String) {
        self.operations.lock().unwrap().push(operation);
    }

    /// Get all recorded operations
    pub fn operations(&self) -> Vec<String> {
        self.operations.lock().unwrap().clone()
    }

    /// Set the current branch
    pub fn set_current_branch(&self, branch: impl Into<String>) {
        *self.current_branch.lock().unwrap() = branch.into();
    }

    /// Set the HEAD SHA
    pub fn set_head_sha(&self, sha: impl Into<String>) {
        *self.head_sha.lock().unwrap() = sha.into();
    }

    /// Set whether working directory is clean
    pub fn set_is_clean(&self, clean: bool) {
        *self.is_clean.lock().unwrap() = clean;
    }
}

impl Default for MockGitEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl GitEnv for MockGitEnv {
    fn worktree_add(&self, path: &Path, branch: &str) -> Result<()> {
        self.record_operation(format!("worktree add {} {}", path.display(), branch));
        self.branches.lock().unwrap().push(branch.to_string());
        Ok(())
    }

    fn worktree_remove(&self, path: &Path) -> Result<()> {
        self.record_operation(format!("worktree remove {}", path.display()));
        Ok(())
    }

    fn worktree_list(&self) -> Result<Vec<String>> {
        self.record_operation("worktree list".to_string());
        Ok(self.branches.lock().unwrap().clone())
    }

    fn merge(&self, branch: &str) -> Result<()> {
        self.record_operation(format!("merge {}", branch));
        Ok(())
    }

    fn commit(&self, message: &str) -> Result<String> {
        self.record_operation(format!("commit: {}", message));
        let sha = self.head_sha.lock().unwrap().clone();
        Ok(sha)
    }

    fn head_sha(&self) -> Result<String> {
        Ok(self.head_sha.lock().unwrap().clone())
    }

    fn create_branch(&self, name: &str) -> Result<()> {
        self.record_operation(format!("create branch {}", name));
        self.branches.lock().unwrap().push(name.to_string());
        Ok(())
    }

    fn checkout(&self, branch: &str) -> Result<()> {
        self.record_operation(format!("checkout {}", branch));
        *self.current_branch.lock().unwrap() = branch.to_string();
        Ok(())
    }

    fn current_branch(&self) -> Result<String> {
        Ok(self.current_branch.lock().unwrap().clone())
    }

    fn is_clean(&self) -> Result<bool> {
        Ok(*self.is_clean.lock().unwrap())
    }
}

/// Mock database environment (placeholder)
#[derive(Debug, Clone, Default)]
pub struct MockDbEnv;

impl MockDbEnv {
    pub fn new() -> Self {
        Self
    }
}

impl DbEnv for MockDbEnv {
    // Placeholder implementation
}
