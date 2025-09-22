//! Checkpoint command implementation
//!
//! This module handles all checkpoint-related CLI commands including
//! listing, cleaning, and showing detailed checkpoint information.

use crate::cli::args::CheckpointCommands;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Find the most recent checkpoint in the checkpoint directory
pub async fn find_latest_checkpoint(checkpoint_dir: &PathBuf) -> Option<String> {
    use tokio::fs;

    if !checkpoint_dir.exists() {
        return None;
    }

    let mut entries = match fs::read_dir(checkpoint_dir).await {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    let mut latest_checkpoint = None;
    let mut latest_time = None;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    if latest_time.is_none_or(|time| modified > time) {
                        latest_time = Some(modified);
                        if let Some(name) = path.file_stem() {
                            latest_checkpoint = Some(name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    latest_checkpoint
}

/// Execute checkpoint-related commands
pub async fn run_checkpoints_command(command: CheckpointCommands) -> Result<()> {
    use crate::cook::workflow::CheckpointManager;

    match command {
        CheckpointCommands::List {
            workflow_id,
            path,
            verbose,
        } => {
            let working_dir = match path {
                Some(p) => p,
                None => std::env::current_dir().context("Failed to get current directory")?,
            };
            let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");

            if !checkpoint_dir.exists() {
                println!("No checkpoints found.");
                return Ok(());
            }

            let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());

            if let Some(id) = workflow_id {
                list_specific_checkpoint(&checkpoint_manager, &id, verbose).await
            } else {
                list_all_checkpoints(&checkpoint_manager, &checkpoint_dir).await
            }
        }
        CheckpointCommands::Clean {
            workflow_id,
            all,
            force,
            path,
        } => {
            let working_dir = match path {
                Some(p) => p,
                None => std::env::current_dir().context("Failed to get current directory")?,
            };
            let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");

            if !checkpoint_dir.exists() {
                println!("No checkpoints to clean.");
                return Ok(());
            }

            if let Some(id) = workflow_id {
                clean_specific_checkpoint(&checkpoint_dir, &id, force).await
            } else if all {
                clean_all_checkpoints(&checkpoint_dir, force).await
            } else {
                println!("Please specify --workflow-id or --all");
                Ok(())
            }
        }
        CheckpointCommands::Show {
            workflow_id,
            version: _,
            path,
        } => {
            let working_dir = match path {
                Some(p) => p,
                None => std::env::current_dir().context("Failed to get current directory")?,
            };
            let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");
            let checkpoint_manager = CheckpointManager::new(checkpoint_dir);

            show_checkpoint_details(&checkpoint_manager, &workflow_id).await
        }
    }
}

/// List checkpoints for a specific workflow ID
async fn list_specific_checkpoint(
    checkpoint_manager: &crate::cook::workflow::CheckpointManager,
    workflow_id: &str,
    verbose: bool,
) -> Result<()> {
    match checkpoint_manager.load_checkpoint(workflow_id).await {
        Ok(checkpoint) => {
            println!("📋 Checkpoint for workflow: {}", workflow_id);
            println!("   Status: {:?}", checkpoint.execution_state.status);
            println!(
                "   Step: {}/{}",
                checkpoint.execution_state.current_step_index,
                checkpoint.execution_state.total_steps
            );
            println!("   Created: {}", checkpoint.timestamp);

            if verbose {
                println!("\n   Completed Steps:");
                for step in &checkpoint.completed_steps {
                    println!(
                        "     {} - {} ({})",
                        step.step_index,
                        step.command,
                        if step.success { "✓" } else { "✗" }
                    );
                    if let Some(ref retry) = step.retry_state {
                        println!(
                            "       Retry: {}/{}",
                            retry.current_attempt, retry.max_attempts
                        );
                    }
                }
            }
        }
        Err(e) => {
            println!("Error loading checkpoint for {}: {}", workflow_id, e);
        }
    }
    Ok(())
}

/// List all available checkpoints
async fn list_all_checkpoints(
    checkpoint_manager: &crate::cook::workflow::CheckpointManager,
    checkpoint_dir: &PathBuf,
) -> Result<()> {
    println!("📋 Available checkpoints:");

    let mut entries = tokio::fs::read_dir(checkpoint_dir).await?;
    let mut checkpoints = Vec::new();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            if let Some(name) = path.file_stem() {
                let workflow_id = name.to_string_lossy().to_string();
                if let Ok(checkpoint) = checkpoint_manager.load_checkpoint(&workflow_id).await {
                    checkpoints.push((workflow_id, checkpoint));
                }
            }
        }
    }

    if checkpoints.is_empty() {
        println!("  No checkpoints found.");
    } else {
        for (id, checkpoint) in checkpoints {
            println!(
                "\n  {} - Status: {:?}",
                id, checkpoint.execution_state.status
            );
            println!(
                "    Step: {}/{}",
                checkpoint.execution_state.current_step_index,
                checkpoint.execution_state.total_steps
            );
            println!("    Created: {}", checkpoint.timestamp);
        }
    }
    Ok(())
}

/// Clean a specific checkpoint
async fn clean_specific_checkpoint(
    checkpoint_dir: &PathBuf,
    workflow_id: &str,
    force: bool,
) -> Result<()> {
    let checkpoint_path = checkpoint_dir.join(format!("{}.json", workflow_id));
    if checkpoint_path.exists() {
        if !force {
            print!("Delete checkpoint for {}? [y/N] ", workflow_id);
            use std::io::{self, Write};
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }
        tokio::fs::remove_file(&checkpoint_path).await?;
        println!("✅ Deleted checkpoint for {}", workflow_id);
    } else {
        println!("No checkpoint found for {}", workflow_id);
    }
    Ok(())
}

/// Clean all completed checkpoints
async fn clean_all_checkpoints(checkpoint_dir: &PathBuf, force: bool) -> Result<()> {
    use crate::cook::workflow::CheckpointManager;

    let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
    let mut entries = tokio::fs::read_dir(checkpoint_dir).await?;
    let mut deleted = 0;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            if let Some(name) = path.file_stem() {
                let workflow_id = name.to_string_lossy().to_string();
                if let Ok(checkpoint) = checkpoint_manager.load_checkpoint(&workflow_id).await {
                    use crate::cook::workflow::checkpoint::WorkflowStatus;
                    if checkpoint.execution_state.status == WorkflowStatus::Completed {
                        if !force {
                            println!("Delete completed checkpoint for {}?", workflow_id);
                        }
                        tokio::fs::remove_file(&path).await?;
                        deleted += 1;
                    }
                }
            }
        }
    }

    println!("✅ Deleted {} completed checkpoints", deleted);
    Ok(())
}

/// Show detailed information about a specific checkpoint
async fn show_checkpoint_details(
    checkpoint_manager: &crate::cook::workflow::CheckpointManager,
    workflow_id: &str,
) -> Result<()> {
    match checkpoint_manager.load_checkpoint(workflow_id).await {
        Ok(checkpoint) => {
            println!("📋 Checkpoint Details for: {}", workflow_id);
            println!("\nExecution State:");
            println!("  Status: {:?}", checkpoint.execution_state.status);
            println!(
                "  Current Step: {}/{}",
                checkpoint.execution_state.current_step_index,
                checkpoint.execution_state.total_steps
            );
            println!("  Start Time: {}", checkpoint.execution_state.start_time);
            println!(
                "  Last Checkpoint: {}",
                checkpoint.execution_state.last_checkpoint
            );

            println!("\nWorkflow Info:");
            if let Some(ref name) = checkpoint.workflow_name {
                println!("  Name: {}", name);
            }
            if let Some(ref path) = checkpoint.workflow_path {
                println!("  Path: {}", path.display());
            }
            println!("  Version: {}", checkpoint.version);
            println!("  Hash: {}", checkpoint.workflow_hash);

            println!("\nCompleted Steps: {}", checkpoint.completed_steps.len());
            for step in &checkpoint.completed_steps {
                println!(
                    "  [{}] {} - {} (Duration: {:?})",
                    step.step_index,
                    step.command,
                    if step.success {
                        "✓ Success"
                    } else {
                        "✗ Failed"
                    },
                    step.duration
                );

                if let Some(ref retry) = step.retry_state {
                    println!(
                        "      Retry: {}/{} attempts",
                        retry.current_attempt, retry.max_attempts
                    );
                    if !retry.failure_history.is_empty() {
                        println!("      Failures: {:?}", retry.failure_history);
                    }
                }

                if !step.captured_variables.is_empty() {
                    println!(
                        "      Variables: {:?}",
                        step.captured_variables.keys().collect::<Vec<_>>()
                    );
                }
            }

            if !checkpoint.variable_state.is_empty() {
                println!("\nVariable State:");
                for key in checkpoint.variable_state.keys() {
                    println!("  {}", key);
                }
            }

            if let Some(ref mapreduce) = checkpoint.mapreduce_state {
                println!("\nMapReduce State:");
                println!("  Completed Items: {}", mapreduce.completed_items.len());
                println!("  Failed Items: {}", mapreduce.failed_items.len());
                println!("  In Progress: {}", mapreduce.in_progress_items.len());
                println!("  Reduce Completed: {}", mapreduce.reduce_completed);
            }
        }
        Err(e) => {
            println!("Error loading checkpoint for {}: {}", workflow_id, e);
        }
    }
    Ok(())
}
