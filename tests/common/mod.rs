//! Common test utilities and helpers

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Test context builder for setting up test environments
pub struct TestContextBuilder {
    temp_dir: TempDir,
    with_git: bool,
    git_user_email: Option<String>,
    git_user_name: Option<String>,
    with_mmm_dirs: bool,
    initial_files: Vec<(PathBuf, String)>,
}

impl TestContextBuilder {
    /// Create a new test context builder
    pub fn new() -> Result<Self> {
        Ok(Self {
            temp_dir: TempDir::new()?,
            with_git: false,
            git_user_email: None,
            git_user_name: None,
            with_mmm_dirs: false,
            initial_files: Vec::new(),
        })
    }

    /// Enable git repository initialization
    pub fn with_git(mut self) -> Self {
        self.with_git = true;
        self
    }

    /// Set git user email
    pub fn with_git_user(mut self, email: &str, name: &str) -> Self {
        self.git_user_email = Some(email.to_string());
        self.git_user_name = Some(name.to_string());
        self.with_git = true;
        self
    }

    /// Create MMM directories (.mmm, specs/temp)
    pub fn with_mmm_dirs(mut self) -> Self {
        self.with_mmm_dirs = true;
        self
    }

    /// Add an initial file
    pub fn with_file(mut self, path: impl AsRef<Path>, content: &str) -> Self {
        self.initial_files
            .push((path.as_ref().to_path_buf(), content.to_string()));
        self
    }

    /// Build the test context
    pub fn build(self) -> Result<TestContext> {
        let path = self.temp_dir.path();

        // Initialize git if requested
        if self.with_git {
            init_git_repo(path)?;

            // Configure git user
            if let (Some(email), Some(name)) = (self.git_user_email, self.git_user_name) {
                configure_git_user(path, &email, &name)?;
            }
        }

        // Create MMM directories
        if self.with_mmm_dirs {
            create_mmm_directories(path)?;
        }

        // Create initial files
        for (file_path, content) in self.initial_files {
            let full_path = path.join(file_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(full_path, content)?;
        }

        Ok(TestContext {
            temp_dir: self.temp_dir,
        })
    }
}

/// Test context that manages temporary directories and cleanup
pub struct TestContext {
    temp_dir: TempDir,
}

impl TestContext {
    /// Get the path to the test directory
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a file in the test directory
    pub fn create_file(&self, path: impl AsRef<Path>, content: &str) -> Result<PathBuf> {
        let full_path = self.temp_dir.path().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;
        Ok(full_path)
    }

    /// Read a file from the test directory
    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<String> {
        let full_path = self.temp_dir.path().join(path);
        Ok(fs::read_to_string(full_path)?)
    }

    /// Check if a file exists
    pub fn file_exists(&self, path: impl AsRef<Path>) -> bool {
        self.temp_dir.path().join(path).exists()
    }

    /// Run a git command in the test directory
    pub fn git_command(&self, args: &[&str]) -> Result<std::process::Output> {
        Ok(Command::new("git")
            .current_dir(self.path())
            .args(args)
            .output()?)
    }
}

/// Initialize a git repository
pub fn init_git_repo(path: &Path) -> Result<()> {
    Command::new("git")
        .current_dir(path)
        .args(["init"])
        .output()?;
    Ok(())
}

/// Configure git user
pub fn configure_git_user(path: &Path, email: &str, name: &str) -> Result<()> {
    Command::new("git")
        .current_dir(path)
        .args(["config", "user.email", email])
        .output()?;

    Command::new("git")
        .current_dir(path)
        .args(["config", "user.name", name])
        .output()?;

    Ok(())
}

/// Create standard MMM directories
pub fn create_mmm_directories(path: &Path) -> Result<()> {
    fs::create_dir_all(path.join(".mmm"))?;
    fs::create_dir_all(path.join("specs/temp"))?;
    Ok(())
}

/// Create a test playbook
pub fn create_test_playbook(path: &Path, name: &str, commands: &[&str]) -> Result<PathBuf> {
    let playbook_path = path.join(name);
    let mut content = String::from("# Test playbook\ncommands:\n");
    for cmd in commands {
        content.push_str(&format!("  - name: {}\n", cmd));
    }
    fs::write(&playbook_path, content)?;
    Ok(playbook_path)
}

/// Common assertion helpers
pub mod assertions {
    use std::path::Path;

    /// Assert that a file contains specific content
    pub fn assert_file_contains(path: &Path, content: &str) {
        let file_content = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", path.display()));
        assert!(
            file_content.contains(content),
            "File {} does not contain expected content: {}",
            path.display(),
            content
        );
    }

    /// Assert that a file does not contain specific content
    pub fn assert_file_not_contains(path: &Path, content: &str) {
        let file_content = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", path.display()));
        assert!(
            !file_content.contains(content),
            "File {} contains unexpected content: {}",
            path.display(),
            content
        );
    }

    /// Assert command output success
    pub fn assert_command_success(output: &std::process::Output) {
        assert!(
            output.status.success(),
            "Command failed with exit code {:?}\nStderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Parse command string helper (moved from individual tests)
pub fn parse_command_string(command: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), vec![]);
    }

    let cmd = parts[0].to_string();
    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
    (cmd, args)
}