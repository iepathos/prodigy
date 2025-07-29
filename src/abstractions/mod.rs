//! Abstraction layers for external dependencies
//!
//! This module provides trait-based abstractions for external commands
//! (git, Claude CLI) to enable better testing and dependency injection.

pub mod claude;
mod exit_status;
pub mod git;

pub use claude::{ClaudeClient, MockClaudeClient, RealClaudeClient};
pub use git::{GitOperations, MockGitOperations, RealGitOperations};
