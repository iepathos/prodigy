//! # Memento Mori (mmm)
//!
//! A dead simple Rust CLI tool that makes your code better through Claude CLI integration.
//!
//! ## Usage
//!
//! ```bash
//! mmm cook [--show-progress] [--focus "area"] [-n iterations] [--map "pattern"] [--args "value"]
//! ```
//!
//! ## Modules
//!
//! - `abstractions` - Trait-based abstractions for external dependencies (git, Claude CLI)
//! - `config` - Configuration management for the tool
//! - `cook` - Core cooking command implementation with mapping support
//! - `simple_state` - Minimal state management with JSON persistence
//! - `worktree` - Git worktree management for parallel sessions
//! - `testing` - Testing utilities and fixtures for comprehensive testing
pub mod abstractions;
pub mod config;
pub mod cook;
pub mod simple_state;
pub mod worktree;

#[cfg(test)]
pub mod testing;
