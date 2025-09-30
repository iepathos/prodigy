//! Test isolation fixtures for environment, working directory, and git repository isolation
//!
//! These fixtures use RAII pattern to ensure proper cleanup even when tests panic.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

/// Environment variable isolation fixture
///
/// Automatically saves and restores environment variables when dropped.
/// This ensures that environment variable changes don't leak between tests.
///
/// # Example
///
/// ```
/// use prodigy::testing::fixtures::isolation::TestEnv;
///
/// # fn example() -> anyhow::Result<()> {
/// let mut env = TestEnv::new();
/// env.set("PRODIGY_AUTO_MERGE", "true");
///
/// // Environment variable is set for this scope
/// assert_eq!(std::env::var("PRODIGY_AUTO_MERGE")?, "true");
///
/// // When env drops, original value is restored
/// # Ok(())
/// # }
/// ```
pub struct TestEnv {
    saved_vars: HashMap<String, Option<String>>,
}

impl TestEnv {
    /// Create a new environment isolation fixture
    pub fn new() -> Self {
        TestEnv {
            saved_vars: HashMap::new(),
        }
    }

    /// Set an environment variable, saving the original value
    pub fn set(&mut self, key: &str, value: &str) {
        // Save original value if not already saved
        if !self.saved_vars.contains_key(key) {
            self.saved_vars
                .insert(key.to_string(), std::env::var(key).ok());
        }
        std::env::set_var(key, value);
    }

    /// Remove an environment variable, saving the original value
    pub fn remove(&mut self, key: &str) {
        if !self.saved_vars.contains_key(key) {
            self.saved_vars
                .insert(key.to_string(), std::env::var(key).ok());
        }
        std::env::remove_var(key);
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // Restore all environment variables
        for (key, value) in &self.saved_vars {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }
}

/// Working directory isolation fixture
///
/// Automatically saves and restores the current working directory when dropped.
/// This ensures that working directory changes don't affect parallel tests.
///
/// # Example
///
/// ```
/// use prodigy::testing::fixtures::isolation::TestWorkingDir;
/// use std::env;
///
/// # fn example() -> anyhow::Result<()> {
/// let wd = TestWorkingDir::new()?;
/// let temp_dir = tempfile::TempDir::new()?;
///
/// wd.change_to(temp_dir.path())?;
///
/// // Working directory is changed
/// assert_eq!(env::current_dir()?, temp_dir.path());
///
/// // When wd drops, original directory is restored
/// # Ok(())
/// # }
/// ```
pub struct TestWorkingDir {
    original_dir: PathBuf,
}

impl TestWorkingDir {
    /// Create a new working directory isolation fixture
    ///
    /// If the current directory no longer exists (e.g., another test changed to a temp directory
    /// that was deleted), this will temporarily change to the system temp directory to establish
    /// a valid working directory baseline.
    pub fn new() -> Result<Self> {
        let original_dir = match std::env::current_dir() {
            Ok(dir) if dir.exists() => dir,
            Ok(_dir) => {
                // Current directory path is valid but doesn't exist anymore
                // Change to a safe directory (system temp)
                let temp_dir = std::env::temp_dir();
                std::env::set_current_dir(&temp_dir)
                    .context("Failed to change to temp directory")?;
                temp_dir
            }
            Err(_) => {
                // Can't determine current directory, use system temp
                let temp_dir = std::env::temp_dir();
                std::env::set_current_dir(&temp_dir)
                    .context("Failed to change to temp directory")?;
                temp_dir
            }
        };

        Ok(TestWorkingDir { original_dir })
    }

    /// Change to the specified directory
    pub fn change_to(&self, path: &Path) -> Result<()> {
        std::env::set_current_dir(path)
            .with_context(|| format!("Failed to change directory to {}", path.display()))
    }
}

impl Drop for TestWorkingDir {
    fn drop(&mut self) {
        // Always restore original directory, ignore errors during cleanup
        let _ = std::env::set_current_dir(&self.original_dir);
    }
}

/// Git repository test fixture
///
/// Creates an isolated git repository in a unique temporary directory.
/// Automatically cleans up the repository when dropped.
///
/// # Example
///
/// ```
/// use prodigy::testing::fixtures::isolation::TestGitRepo;
///
/// # fn example() -> anyhow::Result<()> {
/// let repo = TestGitRepo::new()?;
///
/// // Create a commit
/// repo.commit("Initial commit")?;
///
/// // Repository is automatically cleaned up when repo drops
/// # Ok(())
/// # }
/// ```
pub struct TestGitRepo {
    #[allow(dead_code)]
    temp_dir: TempDir,
    path: PathBuf,
}

impl TestGitRepo {
    /// Create a new isolated git repository
    pub fn new() -> Result<Self> {
        // Use unique suffix to avoid collisions in parallel tests
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("System time before UNIX epoch")?
                .as_nanos()
        );

        let temp_dir = TempDir::with_prefix(format!("prodigy-test-{}", suffix))
            .context("Failed to create temporary directory")?;
        let path = temp_dir.path().to_path_buf();

        // Initialize git repository
        let output = Command::new("git")
            .current_dir(&path)
            .args(["init"])
            .output()
            .context("Failed to initialize git repository")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git init failed: {}", stderr);
        }

        // Configure git user
        Command::new("git")
            .current_dir(&path)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .context("Failed to configure git user email")?;

        Command::new("git")
            .current_dir(&path)
            .args(["config", "user.name", "Test User"])
            .output()
            .context("Failed to configure git user name")?;

        Ok(TestGitRepo { temp_dir, path })
    }

    /// Get the path to the git repository
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create a commit in the repository
    pub fn commit(&self, message: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["commit", "--allow-empty", "-m", message])
            .output()
            .context("Failed to create git commit")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git commit failed: {}", stderr);
        }

        Ok(())
    }

    /// Create a branch in the repository
    pub fn create_branch(&self, branch_name: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["checkout", "-b", branch_name])
            .output()
            .context("Failed to create git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git branch creation failed: {}", stderr);
        }

        Ok(())
    }

    /// Checkout a branch
    pub fn checkout(&self, branch_name: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["checkout", branch_name])
            .output()
            .context("Failed to checkout git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git checkout failed: {}", stderr);
        }

        Ok(())
    }
}

/// Composite test context with all isolation fixtures
///
/// Combines environment, working directory, and optional git repository isolation.
///
/// # Example
///
/// ```
/// use prodigy::testing::fixtures::isolation::IsolatedTestContext;
///
/// # fn example() -> anyhow::Result<()> {
/// let mut ctx = IsolatedTestContext::new()?.with_git_repo()?;
///
/// ctx.env.set("PRODIGY_AUTO_MERGE", "true");
/// ctx.working_dir.change_to(ctx.git_repo.as_ref().unwrap().path())?;
///
/// // All isolation is automatically cleaned up when ctx drops
/// # Ok(())
/// # }
/// ```
pub struct IsolatedTestContext {
    pub env: TestEnv,
    pub working_dir: TestWorkingDir,
    pub git_repo: Option<TestGitRepo>,
}

impl IsolatedTestContext {
    /// Create a new isolated test context
    pub fn new() -> Result<Self> {
        Ok(IsolatedTestContext {
            env: TestEnv::new(),
            working_dir: TestWorkingDir::new()?,
            git_repo: None,
        })
    }

    /// Add an isolated git repository to the context
    pub fn with_git_repo(mut self) -> Result<Self> {
        self.git_repo = Some(TestGitRepo::new()?);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_set_and_restore() {
        let original_value = std::env::var("TEST_VAR_123").ok();

        {
            let mut env = TestEnv::new();
            env.set("TEST_VAR_123", "test_value");
            assert_eq!(std::env::var("TEST_VAR_123").unwrap(), "test_value");
        }

        // After drop, original value should be restored
        assert_eq!(std::env::var("TEST_VAR_123").ok(), original_value);
    }

    #[test]
    fn test_env_remove_and_restore() {
        std::env::set_var("TEST_VAR_456", "original");

        {
            let mut env = TestEnv::new();
            env.remove("TEST_VAR_456");
            assert!(std::env::var("TEST_VAR_456").is_err());
        }

        // After drop, original value should be restored
        assert_eq!(std::env::var("TEST_VAR_456").unwrap(), "original");

        // Cleanup
        std::env::remove_var("TEST_VAR_456");
    }

    #[test]
    fn test_working_dir_restore() -> Result<()> {
        // Ensure we're in a valid directory first
        let valid_dir = std::env::temp_dir();
        std::env::set_current_dir(&valid_dir)?;

        // Create TestWorkingDir to capture valid CWD
        let wd = TestWorkingDir::new()?;
        let original_dir = std::env::current_dir()?.canonicalize()?;

        // Create temp directory and change to it
        let temp_dir = TempDir::new()?;
        wd.change_to(temp_dir.path())?;
        assert_eq!(
            std::env::current_dir()?.canonicalize()?,
            temp_dir.path().canonicalize()?
        );

        // Drop wd to restore original directory
        drop(wd);

        // After drop, original directory should be restored
        assert_eq!(std::env::current_dir()?.canonicalize()?, original_dir);

        // Keep temp_dir alive until the end to avoid directory deletion issues
        Ok(())
    }

    #[test]
    fn test_git_repo_creation() -> Result<()> {
        let repo = TestGitRepo::new()?;
        assert!(repo.path().exists());
        assert!(repo.path().join(".git").exists());
        Ok(())
    }

    #[test]
    fn test_git_repo_commit() -> Result<()> {
        let repo = TestGitRepo::new()?;
        repo.commit("Test commit")?;

        // Verify commit was created
        let output = Command::new("git")
            .current_dir(repo.path())
            .args(["log", "--oneline"])
            .output()?;

        assert!(output.status.success());
        let log = String::from_utf8_lossy(&output.stdout);
        assert!(log.contains("Test commit"));
        Ok(())
    }

    #[test]
    fn test_isolated_context() -> Result<()> {
        // Create context which includes TestWorkingDir before git repo
        let mut ctx = IsolatedTestContext::new()?;
        ctx = ctx.with_git_repo()?;

        ctx.env.set("TEST_VAR_789", "value");
        assert_eq!(std::env::var("TEST_VAR_789")?, "value");

        let git_repo = ctx.git_repo.as_ref().unwrap();
        ctx.working_dir.change_to(git_repo.path())?;
        // Use canonicalize to handle symlinks (e.g., /var -> /private/var on macOS)
        assert_eq!(
            std::env::current_dir()?.canonicalize()?,
            git_repo.path().canonicalize()?
        );

        Ok(())
    }

    #[test]
    fn test_panic_safety() {
        let original_value = std::env::var("PANIC_TEST_VAR").ok();

        let result = std::panic::catch_unwind(|| {
            let mut env = TestEnv::new();
            env.set("PANIC_TEST_VAR", "should_be_restored");
            panic!("Test panic");
        });

        assert!(result.is_err());
        // Even after panic, original value should be restored
        assert_eq!(std::env::var("PANIC_TEST_VAR").ok(), original_value);
    }
}
