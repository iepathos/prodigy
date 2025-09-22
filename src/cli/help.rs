//! Help text generation and utilities
//!
//! This module provides utilities for generating help text and command descriptions.

use crate::cli::args::Cli;
use clap::CommandFactory;

/// Generate comprehensive help text for the CLI
pub fn generate_help() -> String {
    Cli::command().render_help().to_string()
}

/// Generate usage information for a specific command
pub fn generate_command_help(command: &str) -> anyhow::Result<String> {
    let mut cmd = Cli::command();

    // Find the subcommand
    if let Some(subcommand) = cmd.find_subcommand_mut(command) {
        Ok(subcommand.render_help().to_string())
    } else {
        Err(anyhow::anyhow!("Command '{}' not found", command))
    }
}

/// Get the log level description based on verbosity
pub fn get_log_level(verbose: u8) -> &'static str {
    match verbose {
        0 => "info",
        1 => "debug",
        2 => "trace",
        _ => "trace,hyper=debug,tower=debug", // -vvv shows everything including dependencies
    }
}
