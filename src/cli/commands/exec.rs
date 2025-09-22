//! Exec command implementation
//!
//! This module handles the execution of single commands with retry support.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

/// Execute a single command with retry support
pub async fn run_exec_command(
    command: String,
    _retry: u32,
    _timeout: Option<u64>,
    path: Option<PathBuf>,
) -> Result<()> {
    // Parse the command string to extract command type and content
    let (command_type, command_content) = parse_command_string(&command)?;

    // Get the working directory
    let working_dir = path.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    match command_type.as_str() {
        "shell" => execute_shell_command(&command_content, &working_dir),
        "claude" => {
            // For Claude commands, we need more complex setup
            Err(anyhow::anyhow!(
                "Claude commands require a full workflow setup. Use 'prodigy run' instead."
            ))
        }
        _ => Err(anyhow::anyhow!(
            "Unknown command type '{}'. Supported types: shell, claude",
            command_type
        )),
    }
}

/// Parse the command string to extract type and content
fn parse_command_string(command: &str) -> Result<(String, String)> {
    // Split by colon to get command type and content
    let parts: Vec<&str> = command.splitn(2, ':').collect();

    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid command format. Expected 'type: command' (e.g., 'shell: echo hello')"
        ));
    }

    let command_type = parts[0].trim().to_string();
    let command_content = parts[1].trim().to_string();

    Ok((command_type, command_content))
}

/// Execute a shell command
fn execute_shell_command(command: &str, working_dir: &PathBuf) -> Result<()> {
    // Determine the shell to use
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };

    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    // Execute the command
    let output = ProcessCommand::new(shell)
        .arg(shell_arg)
        .arg(command)
        .current_dir(working_dir)
        .output()
        .context("Failed to execute shell command")?;

    // Write stdout
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    // Write stderr
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    // Check exit status
    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        return Err(anyhow::anyhow!(
            "Command failed with exit code {}",
            exit_code
        ));
    }

    Ok(())
}

/// Execute a batch of commands on multiple files
pub async fn run_batch_command(
    _pattern: String,
    _command: String,
    _parallel: usize,
    _retry: Option<u32>,
    _timeout: Option<u64>,
    _path: Option<PathBuf>,
) -> Result<()> {
    // TODO: Implement batch command functionality
    Err(anyhow::anyhow!(
        "Batch command implementation not yet available"
    ))
}