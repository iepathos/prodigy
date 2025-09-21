//! # Prodigy
//!
//! A dead simple Rust CLI tool that makes your code better through Claude CLI integration.
//!
//! ## Usage
//!
//! ```bash
//! prodigy cook [--focus "area"] [-n iterations] [--map "pattern"] [--args "value"]
//! ```
//!
//! ## Modules
//!
//! - `abstractions` - Trait-based abstractions for external dependencies (git, Claude CLI)
//! - `analytics` - Claude session correlation and analytics
//! - `commands` - Modular command handler architecture for extensible workflow commands
//! - `config` - Configuration management for the tool
//! - `cook` - Core cooking command implementation with mapping support
//! - `git` - Granular, testable git operations layer
//! - `init` - Initialize Prodigy commands in projects
//! - `scoring` - Unified project health scoring system
//! - `simple_state` - Minimal state management with JSON persistence
//! - `storage` - Global storage management for events, DLQ, and job state
//! - `subprocess` - Unified subprocess abstraction layer for testing
//! - `worktree` - Git worktree management for parallel sessions
//! - `session` - Session state management with event-driven architecture
//! - `testing` - Testing utilities and fixtures for comprehensive testing
pub mod abstractions;
pub mod analytics;
pub mod cli;
pub mod commands;
pub mod config;
pub mod cook;
pub mod error;
pub mod git;
pub mod init;
pub mod resume_logic;
pub mod scoring;
pub mod session;
pub mod simple_state;
pub mod storage;
pub mod subprocess;
pub mod worktree;

pub mod testing;

// Re-export core error types for library consumers
pub use error::{ErrorCode, ProdigyError};

/// Standard result type for library operations
pub type LibResult<T> = Result<T, ProdigyError>;
