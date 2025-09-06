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
//! - `commands` - Modular command handler architecture for extensible workflow commands
//! - `config` - Configuration management for the tool
//! - `cook` - Core cooking command implementation with mapping support
//! - `git` - Granular, testable git operations layer
//! - `init` - Initialize MMM commands in projects
//! - `scoring` - Unified project health scoring system
//! - `simple_state` - Minimal state management with JSON persistence
//! - `subprocess` - Unified subprocess abstraction layer for testing
//! - `worktree` - Git worktree management for parallel sessions
//! - `session` - Session state management with event-driven architecture
//! - `testing` - Testing utilities and fixtures for comprehensive testing
pub mod abstractions;
pub mod cli;
pub mod commands;
pub mod config;
pub mod cook;
pub mod git;
pub mod init;
pub mod scoring;
pub mod session;
pub mod simple_state;
pub mod subprocess;
pub mod worktree;

pub mod testing;
