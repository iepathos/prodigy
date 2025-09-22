//! CLI command handlers
//!
//! This module contains all CLI-related functionality including:
//! - Argument parsing structures
//! - Command implementations
//! - Help text generation
//! - Input validation

pub mod args;
pub mod commands;
pub mod events;
pub mod help;
pub mod router;
pub mod validation;
pub mod workflow_generator;
pub mod yaml_migrator;
pub mod yaml_validator;

// Re-export the main CLI structures for convenience
pub use args::{Cli, Commands};
pub use help::{generate_command_help, generate_help, get_log_level};
pub use router::execute_command;
pub use validation::{validate_directory, validate_timeout, validate_workflow_file};
