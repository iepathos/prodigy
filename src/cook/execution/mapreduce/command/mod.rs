//! Command execution module for MapReduce operations
//!
//! This module provides a clean abstraction for executing different command types
//! with proper interpolation and context management.

mod claude;
pub mod executor;
mod handler;
pub mod interpolation;
mod shell;

// Re-export public types
pub use executor::{CommandError, CommandExecutor, CommandResult, CommandRouter};
pub use interpolation::InterpolationEngine;

// Re-export implementations
pub use claude::ClaudeCommandExecutor;
pub use handler::HandlerCommandExecutor;
pub use shell::ShellCommandExecutor;
