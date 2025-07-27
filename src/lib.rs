//! # Memento Mori (mmm)
//!
//! A dead simple Rust CLI tool that makes your code better through Claude CLI integration.
//!
//! ## Usage
//!
//! ```bash
//! mmm improve [--show-progress] [--focus "area"] [-n iterations]
//! ```
//!
//! ## Modules
//!
//! - `config` - Configuration management for the tool
//! - `improve` - Core improvement command implementation
//! - `simple_state` - Minimal state management with JSON persistence
//! - `worktree` - Git worktree management for parallel sessions
pub mod config;
pub mod improve;
pub mod simple_state;
pub mod worktree;
