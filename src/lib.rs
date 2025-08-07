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
//! - `analysis` - Unified analysis function for consistent behavior
//! - `analyze` - Project analysis and metrics commands
//! - `config` - Configuration management for the tool
//! - `context` - Context-aware project understanding and analysis
//! - `cook` - Core cooking command implementation with mapping support
//! - `git` - Granular, testable git operations layer
//! - `init` - Initialize MMM commands in projects
//! - `metrics` - Real metrics tracking for code improvements
//! - `scoring` - Unified project health scoring system
//! - `simple_state` - Minimal state management with JSON persistence
//! - `subprocess` - Unified subprocess abstraction layer for testing
//! - `worktree` - Git worktree management for parallel sessions
//! - `session` - Session state management with event-driven architecture
//! - `testing` - Testing utilities and fixtures for comprehensive testing
pub mod abstractions;
pub mod analysis;
pub mod analyze;
pub mod config;
pub mod context;
pub mod cook;
pub mod git;
pub mod init;
pub mod metrics;
pub mod scoring;
pub mod session;
pub mod simple_state;
pub mod subprocess;
pub mod worktree;

pub mod testing;
