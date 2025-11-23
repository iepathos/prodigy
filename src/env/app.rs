//! Combined application environment
//!
//! Provides a unified environment type that combines all I/O capabilities.

use super::mock::{MockDbEnv, MockFileEnv, MockGitEnv, MockProcessEnv};
use super::real::{RealDbEnv, RealFileEnv, RealGitEnv, RealProcessEnv};
use super::traits::{DbEnv, FileEnv, GitEnv, ProcessEnv};
use std::path::PathBuf;
use std::sync::Arc;

/// Combined application environment
///
/// Provides access to all I/O capabilities (file system, process execution, git, database).
/// Can be constructed with real or mock implementations for testing.
///
/// # Examples
///
/// ```
/// use prodigy::env::AppEnv;
///
/// // Create real environment for production use
/// let env = AppEnv::real();
///
/// // Create mock environment for testing
/// let test_env = AppEnv::mock();
/// ```
#[derive(Clone)]
pub struct AppEnv {
    pub fs: Arc<dyn FileEnv>,
    pub process: Arc<dyn ProcessEnv>,
    pub git: Arc<dyn GitEnv>,
    pub db: Arc<dyn DbEnv>,
}

impl AppEnv {
    /// Create an environment with real implementations
    ///
    /// Use this in production code to interact with the actual file system,
    /// processes, git repositories, and databases.
    pub fn real() -> Self {
        Self::real_with_git_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Create a real environment with a specific git working directory
    pub fn real_with_git_dir(git_dir: PathBuf) -> Self {
        Self {
            fs: Arc::new(RealFileEnv::new()),
            process: Arc::new(RealProcessEnv::new()),
            git: Arc::new(RealGitEnv::new(git_dir)),
            db: Arc::new(RealDbEnv::new()),
        }
    }

    /// Create an environment with mock implementations
    ///
    /// Use this in tests to avoid actual I/O operations.
    pub fn mock() -> Self {
        Self {
            fs: Arc::new(MockFileEnv::new()),
            process: Arc::new(MockProcessEnv::new()),
            git: Arc::new(MockGitEnv::new()),
            db: Arc::new(MockDbEnv::new()),
        }
    }

    /// Create a custom environment with specific implementations
    pub fn custom(
        fs: Arc<dyn FileEnv>,
        process: Arc<dyn ProcessEnv>,
        git: Arc<dyn GitEnv>,
        db: Arc<dyn DbEnv>,
    ) -> Self {
        Self {
            fs,
            process,
            git,
            db,
        }
    }
}

impl std::fmt::Debug for AppEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppEnv")
            .field("fs", &"dyn FileEnv")
            .field("process", &"dyn ProcessEnv")
            .field("git", &"dyn GitEnv")
            .field("db", &"dyn DbEnv")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_mock_env_creation() {
        let env = AppEnv::mock();
        let mock_fs = env.fs.clone();

        // Verify mock file system works
        let path = Path::new("test.txt");
        assert!(!mock_fs.exists(path));
    }

    #[test]
    fn test_real_env_creation() {
        let env = AppEnv::real();
        // Just verify it constructs without panic
        assert!(env.fs.exists(Path::new(".")));
    }
}
