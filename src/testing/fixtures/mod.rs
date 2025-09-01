//! Test fixtures and data builders
//!
//! This module provides test data builders and common fixtures for testing.

pub mod builders;

use crate::testing::mocks::{
    MockClaudeClientBuilder, MockFileSystemBuilder, MockGitOperationsBuilder,
    MockSubprocessManagerBuilder,
};

/// Common test fixtures for various scenarios
pub struct Fixtures;

impl Fixtures {
    /// Create a clean git repository fixture
    pub fn clean_git_repo() -> MockGitOperationsBuilder {
        MockGitOperationsBuilder::new()
            .is_repo(true)
            .with_clean_status()
            .with_commit_message("Initial commit")
    }

    /// Create a dirty git repository fixture with uncommitted changes
    pub fn dirty_git_repo() -> MockGitOperationsBuilder {
        MockGitOperationsBuilder::new()
            .is_repo(true)
            .with_dirty_status(vec![
                "M  src/main.rs",
                "A  src/new_feature.rs",
                "?? temp.txt",
            ])
            .with_commit_message("Previous commit")
    }

    /// Create a successful Claude client fixture
    pub fn successful_claude() -> MockClaudeClientBuilder {
        MockClaudeClientBuilder::new()
            .with_success("/prodigy-code-review", "No issues found. Code looks good!")
            .with_success(
                "/prodigy-implement-spec",
                "Specification implemented successfully",
            )
            .with_success("/prodigy-lint", "Linting completed. No issues found.")
    }

    /// Create a Claude client that simulates rate limiting
    pub fn rate_limited_claude() -> MockClaudeClientBuilder {
        MockClaudeClientBuilder::new().with_error(
            "/prodigy-code-review",
            "Error: Rate limit exceeded. Please try again later.",
        )
    }

    /// Create an unavailable Claude client
    pub fn unavailable_claude() -> MockClaudeClientBuilder {
        MockClaudeClientBuilder::new().unavailable()
    }

    /// Create a subprocess manager for git operations
    pub fn git_subprocess() -> MockSubprocessManagerBuilder {
        MockSubprocessManagerBuilder::new()
            .with_success("git", "")
            .with_success("cargo", "")
    }

    /// Create a standard Rust project file system
    pub fn rust_project_fs() -> MockFileSystemBuilder {
        MockFileSystemBuilder::new().with_project_structure()
    }

    /// Create a complex project file system with multiple modules
    pub fn complex_project_fs() -> MockFileSystemBuilder {
        MockFileSystemBuilder::new()
            .with_project_structure()
            .with_directory("src/modules")
            .with_file(
                "src/modules/auth.rs",
                "pub mod auth {\n    // Authentication logic\n}",
            )
            .with_file(
                "src/modules/database.rs",
                "pub mod database {\n    // Database logic\n}",
            )
            .with_directory("src/handlers")
            .with_file(
                "src/handlers/api.rs",
                "pub mod api {\n    // API handlers\n}",
            )
            .with_file(
                "src/config.rs",
                "pub struct Config {\n    // Configuration\n}",
            )
            .with_file(
                "tests/integration_test.rs",
                "#[test]\nfn test_integration() {\n    assert!(true);\n}",
            )
    }
}
