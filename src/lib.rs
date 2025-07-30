//! # Memento Mori (mmm)
//!
//! A dead simple Rust CLI tool that makes your code better through Claude CLI integration.
//!
//! ## Usage
//!
//! ```bash
//! mmm cook [--focus "area"] [-n iterations] [--map "pattern"] [--args "value"]
//! ```
//!
//! ## Modules
//!
//! - `abstractions` - Trait-based abstractions for external dependencies (git, Claude CLI)
//! - `analyze` - Project analysis and metrics commands
//! - `config` - Configuration management for the tool
//! - `context` - Context-aware project understanding and analysis
//! - `cook` - Core cooking command implementation with mapping support
//! - `init` - Initialize MMM commands in projects
//! - `simple_state` - Minimal state management with JSON persistence
//! - `worktree` - Git worktree management for parallel sessions
//! - `testing` - Testing utilities and fixtures for comprehensive testing
pub mod abstractions;
pub mod analyze;
pub mod config;
pub mod context;
pub mod cook;
pub mod init;
pub mod simple_state;
pub mod worktree;

#[cfg(test)]
pub mod testing;
