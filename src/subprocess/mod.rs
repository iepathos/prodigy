//! Unified subprocess abstraction layer for external tool integration
//!
//! This module provides a clean, testable abstraction over subprocess execution,
//! specifically designed for integrating with external tools like git and Claude CLI.
//! It enables consistent process management, error handling, and testing across
//! all subprocess operations in MMM.
//!
//! # Architecture
//!
//! The subprocess system uses a trait-based architecture with dependency injection:
//! - [`ProcessRunner`] - Core trait for process execution
//! - [`SubprocessManager`] - High-level manager that orchestrates different runners
//! - Specialized runners for specific tools ([`GitRunner`], [`ClaudeRunner`])
//!
//! # Examples
//!
//! ## Production Usage
//!
//! ```rust
//! use mmm::subprocess::SubprocessManager;
//!
//! // Create production subprocess manager
//! let subprocess = SubprocessManager::production();
//! let git = subprocess.git();
//! let claude = subprocess.claude();
//! ```
//!
//! ## Testing with Mocks
//!
//! ```rust
//! # use mmm::subprocess::SubprocessManager;
//! let (subprocess, mock) = SubprocessManager::mock();
//!
//! // Configure expected calls
//! mock.expect_success("git", &["status", "--porcelain"], "");
//!
//! // Use in tests
//! let git = subprocess.git();
//! // ... test logic
//! ```

pub mod builder;
pub mod claude;
pub mod error;
pub mod git;
pub mod mock;
pub mod runner;

#[cfg(test)]
mod tests;

pub use builder::ProcessCommandBuilder;
pub use claude::ClaudeRunner;
pub use error::ProcessError;
pub use git::GitRunner;
pub use mock::{MockCommandConfig, MockProcessRunner};
pub use runner::ProcessCommand;
pub use runner::{ExitStatusHelper, ProcessOutput, ProcessRunner, ProcessStream};

use std::sync::Arc;

/// Central manager for subprocess operations across MMM
///
/// `SubprocessManager` provides a unified interface for executing external processes,
/// with specialized methods for common tools like git and Claude CLI. It supports
/// both production execution and testing with mock implementations.
///
/// # Design
///
/// The manager uses dependency injection with the [`ProcessRunner`] trait, allowing
/// different implementations for production and testing. This design enables:
/// - Consistent error handling across all subprocess operations
/// - Easy testing with mock process runners
/// - Centralized configuration and logging
///
/// # Examples
///
/// ```rust
/// use mmm::subprocess::SubprocessManager;
///
/// // Production usage
/// let subprocess = SubprocessManager::production();
/// let git = subprocess.git();
///
/// // Test usage
/// let (subprocess, mock) = SubprocessManager::mock();
/// mock.expect_success("git", &["status"], "clean");
/// ```
#[derive(Clone)]
pub struct SubprocessManager {
    runner: Arc<dyn ProcessRunner>,
}

impl SubprocessManager {
    /// Create a new subprocess manager with the given process runner
    ///
    /// This is primarily used for dependency injection in testing or when
    /// you need a custom process runner implementation.
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }

    /// Create a production subprocess manager
    ///
    /// Uses the real Tokio-based process runner for actual subprocess execution.
    /// This is the standard factory method for production usage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mmm::subprocess::SubprocessManager;
    ///
    /// let subprocess = SubprocessManager::production();
    /// let git = subprocess.git();
    /// ```
    pub fn production() -> Self {
        Self::new(Arc::new(runner::TokioProcessRunner))
    }

    /// Create a mock subprocess manager for testing
    ///
    /// Returns both the manager and the mock runner, allowing tests to configure
    /// expected process calls and their responses.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::subprocess::SubprocessManager;
    /// let (subprocess, mock) = SubprocessManager::mock();
    /// mock.expect_success("git", &["status", "--porcelain"], "");
    ///
    /// let git = subprocess.git();
    /// // Test logic that calls git operations
    /// ```
    #[cfg(test)]
    pub fn mock() -> (Self, MockProcessRunner) {
        let mock = MockProcessRunner::new();
        let runner = Arc::new(mock.clone()) as Arc<dyn ProcessRunner>;
        (Self::new(runner), mock)
    }

    /// Get the underlying process runner
    ///
    /// Returns a cloned Arc to the process runner for direct usage.
    /// Most code should use the specialized runners (`git()`, `claude()`) instead.
    pub fn runner(&self) -> Arc<dyn ProcessRunner> {
        Arc::clone(&self.runner)
    }

    /// Create a git-specific runner
    ///
    /// Returns a [`GitRunnerImpl`] that provides high-level git operations
    /// with proper error handling and logging.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::subprocess::SubprocessManager;
    /// let subprocess = SubprocessManager::production();
    /// let git = subprocess.git();
    /// // Use git operations
    /// ```
    pub fn git(&self) -> git::GitRunnerImpl {
        git::GitRunnerImpl::new(Arc::clone(&self.runner))
    }

    /// Create a Claude CLI-specific runner
    ///
    /// Returns a [`ClaudeRunnerImpl`] that provides high-level Claude CLI operations
    /// with proper error handling and environment setup.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::subprocess::SubprocessManager;
    /// let subprocess = SubprocessManager::production();
    /// let claude = subprocess.claude();
    /// // Use Claude CLI operations
    /// ```
    pub fn claude(&self) -> claude::ClaudeRunnerImpl {
        claude::ClaudeRunnerImpl::new(Arc::clone(&self.runner))
    }
}
