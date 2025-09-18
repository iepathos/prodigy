//! Command execution module for MapReduce operations
//!
//! This module provides a clean abstraction for executing different command types
//! with proper interpolation and context management.

mod claude;
pub mod context;
pub mod executor;
mod handler;
pub mod interpolation;
mod shell;
pub mod step_executor;
pub mod types;

// Test module
#[cfg(test)]
mod tests;

// Re-export public types
pub use context::ExecutionContext;
pub use executor::{CommandError, CommandExecutor, CommandResult, CommandRouter};
pub use interpolation::{InterpolationEngine, StepInterpolator};
pub use step_executor::StepExecutor;
pub use types::{collect_command_types, determine_command_type, validate_command_count};

// Re-export implementations
pub use claude::ClaudeCommandExecutor;
pub use handler::HandlerCommandExecutor;
pub use shell::ShellCommandExecutor;
