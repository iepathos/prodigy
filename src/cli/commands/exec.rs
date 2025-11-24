//! Exec command implementation
//!
//! This module handles the execution of single commands with retry support.

use anyhow::Result;
use std::path::PathBuf;

/// Execute a single command with retry support
pub async fn run_exec_command(
    command: String,
    retry: u32,
    timeout: Option<u64>,
    path: Option<PathBuf>,
) -> Result<()> {
    use crate::cli::workflow_generator::{generate_exec_workflow, TemporaryWorkflow};

    // Change to specified directory if provided
    if let Some(p) = path.clone() {
        std::env::set_current_dir(&p)?;
    }

    println!("🚀 Executing command: {}", command);
    if retry > 1 {
        println!("   Retry attempts: {}", retry);
    }
    if let Some(t) = timeout {
        println!("   Timeout: {}s", t);
    }

    // Generate temporary workflow
    let (_workflow, temp_path) = generate_exec_workflow(&command, retry, timeout)?;
    let _temp_workflow = TemporaryWorkflow {
        path: temp_path.clone(),
    };

    // Execute using cook command
    let cook_cmd = crate::cook::command::CookCommand {
        playbook: temp_path,
        path,
        max_iterations: 1,
        map: vec![],
        args: vec![],
        fail_fast: false,
        auto_accept: true,
        resume: None,
        quiet: false,
        verbosity: 0,
        dry_run: false,
        params: std::collections::HashMap::new(),
    };

    crate::cook::cook(cook_cmd).await
}

/// Execute a batch of commands on multiple files
pub async fn run_batch_command(
    pattern: String,
    command: String,
    parallel: usize,
    retry: Option<u32>,
    timeout: Option<u64>,
    path: Option<PathBuf>,
) -> Result<()> {
    use crate::cli::workflow_generator::{generate_batch_workflow, TemporaryWorkflow};

    // Change to specified directory if provided
    if let Some(p) = path.clone() {
        std::env::set_current_dir(&p)?;
    }

    println!("📦 Starting batch processing");
    println!("   Pattern: {}", pattern);
    println!("   Command: {}", command);
    println!("   Parallel workers: {}", parallel);
    if let Some(r) = retry {
        println!("   Retry attempts: {}", r);
    }
    if let Some(t) = timeout {
        println!("   Timeout per file: {}s", t);
    }

    // Generate temporary workflow
    let (_workflow, temp_path) =
        generate_batch_workflow(&pattern, &command, parallel, retry, timeout)?;
    let _temp_workflow = TemporaryWorkflow {
        path: temp_path.clone(),
    };

    // Execute using cook command
    let cook_cmd = crate::cook::command::CookCommand {
        playbook: temp_path,
        path,
        max_iterations: 1,
        map: vec![],
        args: vec![],
        fail_fast: false,
        auto_accept: true,
        resume: None,
        quiet: false,
        verbosity: 0,
        dry_run: false,
        params: std::collections::HashMap::new(),
    };

    crate::cook::cook(cook_cmd).await
}
