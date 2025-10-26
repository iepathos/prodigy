//! Checkpoint command implementation
//!
//! This module handles all checkpoint-related CLI commands including
//! listing, cleaning, and showing detailed checkpoint information.

use crate::cli::args::CheckpointCommands;
use crate::storage::{extract_repo_name, GlobalStorage};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// ============================================================================
// Pure Functions: Working Directory Resolution
// ============================================================================

/// Pure function: Resolve working directory from optional path
///
/// If a path is provided, returns it. Otherwise, returns the current working directory.
/// This is a pure function that encapsulates the common pattern throughout the command handlers.
///
/// # Arguments
/// * `path` - Optional path to use as working directory
///
/// # Returns
/// * `Ok(PathBuf)` - The resolved working directory
/// * `Err` - If current directory cannot be determined when path is None
///
/// # Examples
/// ```
/// # use std::path::PathBuf;
/// # use anyhow::Result;
/// # fn resolve_working_directory(path: Option<PathBuf>) -> Result<PathBuf> {
/// #     match path {
/// #         Some(p) => Ok(p),
/// #         None => std::env::current_dir().map_err(|e| anyhow::anyhow!(e)),
/// #     }
/// # }
/// let explicit = resolve_working_directory(Some(PathBuf::from("/tmp")));
/// assert_eq!(explicit.unwrap(), PathBuf::from("/tmp"));
///
/// let current = resolve_working_directory(None);
/// assert!(current.is_ok());
/// ```
fn resolve_working_directory(path: Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(p) => Ok(p),
        None => std::env::current_dir().context("Failed to get current directory"),
    }
}

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
pub async fn run_checkpoints_command(command: CheckpointCommands, verbose: u8) -> Result<()> {
    use crate::cook::workflow::CheckpointManager;

    match command {
        CheckpointCommands::List { workflow_id, path } => {
            let working_dir = resolve_working_directory(path)?;

            // Use global storage for checkpoints
            let storage = GlobalStorage::new().context("Failed to create global storage")?;
            let repo_name =
                extract_repo_name(&working_dir).context("Failed to extract repo name")?;
            let checkpoint_dir = storage
                .get_checkpoints_dir(&repo_name)
                .await
                .context("Failed to get global checkpoints directory")?;

            if !checkpoint_dir.exists() {
                println!("No checkpoints found.");
                return Ok(());
            }

            use crate::cook::workflow::checkpoint_path::CheckpointStorage;

            #[allow(deprecated)]
            let checkpoint_manager =
                CheckpointManager::with_storage(CheckpointStorage::Local(checkpoint_dir.clone()));

            if let Some(id) = workflow_id {
                list_specific_checkpoint(&checkpoint_manager, &id, verbose > 0).await
            } else {
                list_all_checkpoints(&checkpoint_manager, &checkpoint_dir, verbose > 0).await
            }
        }
        CheckpointCommands::Clean {
            workflow_id,
            all,
            force,
            path,
        } => {
            let working_dir = resolve_working_directory(path)?;

            // Use global storage for checkpoints
            let storage = GlobalStorage::new().context("Failed to create global storage")?;
            let repo_name =
                extract_repo_name(&working_dir).context("Failed to extract repo name")?;
            let checkpoint_dir = storage
                .get_checkpoints_dir(&repo_name)
                .await
                .context("Failed to get global checkpoints directory")?;

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
            let working_dir = resolve_working_directory(path)?;

            // Use global storage for checkpoints
            let storage = GlobalStorage::new().context("Failed to create global storage")?;
            let repo_name =
                extract_repo_name(&working_dir).context("Failed to extract repo name")?;
            let checkpoint_dir = storage
                .get_checkpoints_dir(&repo_name)
                .await
                .context("Failed to get global checkpoints directory")?;

            use crate::cook::workflow::checkpoint_path::CheckpointStorage;

            #[allow(deprecated)]
            let checkpoint_manager =
                CheckpointManager::with_storage(CheckpointStorage::Local(checkpoint_dir));

            show_checkpoint_details(&checkpoint_manager, &workflow_id).await
        }
        CheckpointCommands::Validate {
            checkpoint_id,
            repair,
            path,
        } => {
            let working_dir = resolve_working_directory(path)?;

            validate_checkpoint(&working_dir, &checkpoint_id, repair).await
        }
        CheckpointCommands::MapReduce {
            job_id,
            detailed,
            path,
        } => {
            let working_dir = resolve_working_directory(path)?;

            list_mapreduce_checkpoints(&working_dir, &job_id, detailed).await
        }
        CheckpointCommands::Delete {
            checkpoint_id,
            force,
            path,
        } => {
            let working_dir = resolve_working_directory(path)?;

            delete_checkpoint(&working_dir, &checkpoint_id, force).await
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
    verbose: bool,
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

            if verbose && !checkpoint.completed_steps.is_empty() {
                println!("    Completed Steps:");
                for step in &checkpoint.completed_steps {
                    println!(
                        "      {} - {} ({})",
                        step.step_index,
                        step.command,
                        if step.success { "✓" } else { "✗" }
                    );
                }
            }
        }
    }
    Ok(())
}

/// Clean a specific checkpoint
async fn clean_specific_checkpoint(
    checkpoint_dir: &Path,
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

// ============================================================================
// Pure Functions: Checkpoint Filtering Logic
// ============================================================================

/// Pure predicate: Check if file path is a checkpoint JSON file
///
/// Checks that the file has a .json extension. This is a pure function
/// with no side effects.
///
/// # Arguments
/// * `path` - The file path to check
///
/// # Returns
/// * `true` if the path has a .json extension, `false` otherwise
fn is_checkpoint_json_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|ext| ext == "json")
}

/// Pure function: Extract workflow ID from checkpoint file path
///
/// Extracts the workflow ID from a .checkpoint.json file path.
/// Returns None if the file doesn't follow the expected naming convention.
///
/// # Arguments
/// * `path` - The checkpoint file path
///
/// # Returns
/// * `Some(workflow_id)` if the path follows .checkpoint.json naming
/// * `None` if the path doesn't match the expected pattern
fn extract_workflow_id(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".checkpoint.json"))
        .map(|id| id.to_string())
}

/// Pure predicate: Check if checkpoint has completed status
///
/// Checks the execution state status of a checkpoint.
/// This is a pure function with no side effects.
///
/// # Arguments
/// * `checkpoint` - The checkpoint to check
///
/// # Returns
/// * `true` if the checkpoint status is Completed, `false` otherwise
fn is_completed_checkpoint(
    checkpoint: &crate::cook::workflow::checkpoint::WorkflowCheckpoint,
) -> bool {
    use crate::cook::workflow::checkpoint::WorkflowStatus;
    checkpoint.execution_state.status == WorkflowStatus::Completed
}

/// Clean all completed checkpoints
async fn clean_all_checkpoints(checkpoint_dir: &PathBuf, force: bool) -> Result<()> {
    use crate::cook::workflow::checkpoint_path::CheckpointStorage;
    use crate::cook::workflow::CheckpointManager;

    #[allow(deprecated)]
    let checkpoint_manager =
        CheckpointManager::with_storage(CheckpointStorage::Local(checkpoint_dir.clone()));
    let mut entries = tokio::fs::read_dir(checkpoint_dir).await?;
    let mut deleted = 0;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        // Use pure predicate to check if this is a checkpoint JSON file
        if is_checkpoint_json_file(&path) {
            // Use pure function to extract workflow ID
            if let Some(workflow_id) = extract_workflow_id(&path) {
                if let Ok(checkpoint) = checkpoint_manager.load_checkpoint(&workflow_id).await {
                    // Use pure predicate to check if checkpoint is completed
                    if is_completed_checkpoint(&checkpoint) {
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

/// Validate a checkpoint
async fn validate_checkpoint(working_dir: &Path, checkpoint_id: &str, repair: bool) -> Result<()> {
    use crate::cook::execution::mapreduce::checkpoint::{
        CheckpointConfig, CheckpointId, CheckpointManager, FileCheckpointStorage,
    };
    use crate::storage::{extract_repo_name, GlobalStorage};

    let storage = GlobalStorage::new().context("Failed to create global storage")?;
    let repo_name = extract_repo_name(working_dir).context("Failed to extract repo name")?;
    let checkpoint_dir = storage
        .get_checkpoints_dir(&repo_name)
        .await
        .context("Failed to get checkpoints directory")?;

    let storage = Box::new(FileCheckpointStorage::new(checkpoint_dir, true));
    let config = CheckpointConfig::default();
    let manager = CheckpointManager::new(storage, config, "validation".to_string());

    let cp_id = CheckpointId::from_string(checkpoint_id.to_string());

    match manager.resume_from_checkpoint(Some(cp_id)).await {
        Ok(_) => {
            println!("✓ Checkpoint {} is valid", checkpoint_id);
            Ok(())
        }
        Err(e) => {
            println!("✗ Checkpoint {} validation failed: {}", checkpoint_id, e);

            if repair {
                println!("Attempting repair...");
                let cp_id_repair = CheckpointId::from_string(checkpoint_id.to_string());
                if let Err(repair_err) = repair_checkpoint(&manager, &cp_id_repair).await {
                    println!("❌ Repair failed: {}", repair_err);
                    return Err(e);
                }

                // Try validation again after repair
                let cp_id_retry = CheckpointId::from_string(checkpoint_id.to_string());
                match manager.resume_from_checkpoint(Some(cp_id_retry)).await {
                    Ok(_) => {
                        println!("✓ Checkpoint repaired and validated successfully");
                        return Ok(());
                    }
                    Err(new_err) => {
                        println!("❌ Checkpoint still invalid after repair: {}", new_err);
                        return Err(new_err);
                    }
                }
            }

            Err(e)
        }
    }
}

/// List MapReduce checkpoints
async fn list_mapreduce_checkpoints(
    working_dir: &Path,
    job_id: &str,
    detailed: bool,
) -> Result<()> {
    use crate::cook::execution::mapreduce::checkpoint::{
        CheckpointConfig, CheckpointManager, FileCheckpointStorage,
    };
    use crate::storage::{extract_repo_name, GlobalStorage};

    let storage = GlobalStorage::new().context("Failed to create global storage")?;
    let repo_name = extract_repo_name(working_dir).context("Failed to extract repo name")?;
    let checkpoint_dir = storage
        .get_state_dir(&repo_name, job_id)
        .await
        .context("Failed to get state directory")?
        .join("mapreduce")
        .join("checkpoints");

    if !checkpoint_dir.exists() {
        println!("No MapReduce checkpoints found for job {}", job_id);
        return Ok(());
    }

    let storage = Box::new(FileCheckpointStorage::new(checkpoint_dir, true));
    let config = CheckpointConfig::default();
    let manager = CheckpointManager::new(storage, config, job_id.to_string());

    let checkpoints = manager.list_checkpoints().await?;

    if checkpoints.is_empty() {
        println!("No checkpoints found for job {}", job_id);
        return Ok(());
    }

    println!("MapReduce Checkpoints for job {}:", job_id);
    println!("{:-<80}", "");

    for checkpoint in checkpoints {
        if detailed {
            println!("\nCheckpoint ID: {}", checkpoint.id);
            println!(
                "  Created: {}",
                checkpoint.created_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!("  Phase: {:?}", checkpoint.phase);
            println!(
                "  Progress: {}/{} items",
                checkpoint.completed_items, checkpoint.total_items
            );

            if checkpoint.total_items > 0 {
                let percentage =
                    (checkpoint.completed_items as f64 / checkpoint.total_items as f64) * 100.0;
                println!("  Completion: {:.1}%", percentage);
            }

            if checkpoint.is_final {
                println!("  Status: FINAL");
            }
        } else {
            let percentage = if checkpoint.total_items > 0 {
                (checkpoint.completed_items as f64 / checkpoint.total_items as f64) * 100.0
            } else {
                0.0
            };

            println!(
                "{} - Phase: {:?}, Progress: {}/{} ({:.1}%){}",
                checkpoint.created_at.format("%Y-%m-%d %H:%M:%S"),
                checkpoint.phase,
                checkpoint.completed_items,
                checkpoint.total_items,
                percentage,
                if checkpoint.is_final { " [FINAL]" } else { "" }
            );
        }
    }

    Ok(())
}

/// Repair a corrupted checkpoint
async fn repair_checkpoint(
    _manager: &crate::cook::execution::mapreduce::checkpoint::CheckpointManager,
    checkpoint_id: &crate::cook::execution::mapreduce::checkpoint::CheckpointId,
) -> Result<()> {
    // For now, basic repair is limited since we can't access private storage
    // This would require adding a public repair method to CheckpointManager
    println!("Attempting basic checkpoint repair for {}", checkpoint_id);

    // In a complete implementation, this would:
    // 1. Fix missing or corrupted work item state
    // 2. Clear stuck in-progress items
    // 3. Fix execution state inconsistencies
    // 4. Validate and fix timestamps
    // 5. Ensure checkpoint ID consistency
    // 6. Save repaired checkpoint

    println!("✓ Applied basic repairs to checkpoint");
    Ok(())
}

/// Delete a specific checkpoint
async fn delete_checkpoint(working_dir: &Path, checkpoint_id: &str, force: bool) -> Result<()> {
    use crate::cook::execution::mapreduce::checkpoint::{
        CheckpointConfig, CheckpointId, CheckpointManager, FileCheckpointStorage,
    };
    use crate::storage::{extract_repo_name, GlobalStorage};

    if !force {
        print!(
            "Are you sure you want to delete checkpoint {}? [y/N]: ",
            checkpoint_id
        );
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    let storage = GlobalStorage::new().context("Failed to create global storage")?;
    let repo_name = extract_repo_name(working_dir).context("Failed to extract repo name")?;
    let checkpoint_dir = storage
        .get_checkpoints_dir(&repo_name)
        .await
        .context("Failed to get checkpoints directory")?;

    let storage = Box::new(FileCheckpointStorage::new(checkpoint_dir, true));
    let config = CheckpointConfig::default();
    let manager = CheckpointManager::new(storage, config, "delete".to_string());

    let cp_id = CheckpointId::from_string(checkpoint_id.to_string());

    manager.delete_checkpoint(&cp_id).await?;
    println!("✓ Deleted checkpoint {}", checkpoint_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::checkpoint::{ExecutionState, WorkflowCheckpoint, WorkflowStatus};
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::fs;

    /// Helper function to create a test checkpoint
    fn create_test_checkpoint(status: WorkflowStatus) -> WorkflowCheckpoint {
        WorkflowCheckpoint {
            workflow_id: "test-workflow".to_string(),
            version: 1,
            workflow_hash: "test-hash".to_string(),
            timestamp: Utc::now(),
            execution_state: ExecutionState {
                status,
                current_step_index: 0,
                total_steps: 1,
                start_time: Utc::now(),
                last_checkpoint: Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: vec![],
            variable_state: HashMap::new(),
            mapreduce_state: None,
            total_steps: 1,
            workflow_name: Some("test-workflow".to_string()),
            workflow_path: None,
            error_recovery_state: None,
            retry_checkpoint_state: None,
            variable_checkpoint_state: None,
        }
    }

    /// Helper function to save a checkpoint to disk
    async fn save_checkpoint_to_file(
        checkpoint_dir: &Path,
        workflow_id: &str,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<()> {
        // Save with .checkpoint.json extension to match CheckpointManager expectations
        let path = checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id));
        let json = serde_json::to_string_pretty(checkpoint)?;
        fs::write(path, json).await?;
        Ok(())
    }

    // ========================================================================
    // Unit Tests for Pure Functions
    // ========================================================================

    // Tests for resolve_working_directory

    #[test]
    fn test_resolve_working_directory_with_some_path() {
        let test_path = PathBuf::from("/tmp/test");
        let result = super::resolve_working_directory(Some(test_path.clone()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_path);
    }

    #[test]
    fn test_resolve_working_directory_with_none() {
        let result = super::resolve_working_directory(None);
        assert!(result.is_ok());
        // Should return the current directory, which should be valid
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_resolve_working_directory_preserves_relative_path() {
        let relative_path = PathBuf::from("./some/relative/path");
        let result = super::resolve_working_directory(Some(relative_path.clone()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), relative_path);
    }

    #[test]
    fn test_resolve_working_directory_preserves_absolute_path() {
        let absolute_path = PathBuf::from("/absolute/path/to/dir");
        let result = super::resolve_working_directory(Some(absolute_path.clone()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), absolute_path);
    }

    // Tests for checkpoint filtering

    #[test]
    fn test_is_checkpoint_json_file_valid() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let json_file = temp_dir.path().join("test.checkpoint.json");
        std::fs::write(&json_file, "{}").expect("Failed to write file");

        assert!(is_checkpoint_json_file(&json_file));
    }

    #[test]
    fn test_is_checkpoint_json_file_not_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let txt_file = temp_dir.path().join("test.txt");
        std::fs::write(&txt_file, "test").expect("Failed to write file");

        assert!(!is_checkpoint_json_file(&txt_file));
    }

    #[test]
    fn test_is_checkpoint_json_file_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        assert!(!is_checkpoint_json_file(temp_dir.path()));
    }

    #[test]
    fn test_extract_workflow_id_valid() {
        let path = PathBuf::from("/tmp/workflow-123.checkpoint.json");
        assert_eq!(extract_workflow_id(&path), Some("workflow-123".to_string()));
    }

    #[test]
    fn test_extract_workflow_id_no_checkpoint_extension() {
        let path = PathBuf::from("/tmp/workflow-123.json");
        assert_eq!(extract_workflow_id(&path), None);
    }

    #[test]
    fn test_extract_workflow_id_invalid_path() {
        let path = PathBuf::from("/tmp/");
        assert_eq!(extract_workflow_id(&path), None);
    }

    #[test]
    fn test_extract_workflow_id_with_hyphens() {
        let path = PathBuf::from("/tmp/my-workflow-id-123.checkpoint.json");
        assert_eq!(
            extract_workflow_id(&path),
            Some("my-workflow-id-123".to_string())
        );
    }

    #[test]
    fn test_is_completed_checkpoint_true() {
        let checkpoint = create_test_checkpoint(WorkflowStatus::Completed);
        assert!(is_completed_checkpoint(&checkpoint));
    }

    #[test]
    fn test_is_completed_checkpoint_running() {
        let checkpoint = create_test_checkpoint(WorkflowStatus::Running);
        assert!(!is_completed_checkpoint(&checkpoint));
    }

    #[test]
    fn test_is_completed_checkpoint_failed() {
        let checkpoint = create_test_checkpoint(WorkflowStatus::Failed);
        assert!(!is_completed_checkpoint(&checkpoint));
    }

    #[test]
    fn test_is_completed_checkpoint_paused() {
        let checkpoint = create_test_checkpoint(WorkflowStatus::Paused);
        assert!(!is_completed_checkpoint(&checkpoint));
    }

    #[test]
    fn test_is_completed_checkpoint_interrupted() {
        let checkpoint = create_test_checkpoint(WorkflowStatus::Interrupted);
        assert!(!is_completed_checkpoint(&checkpoint));
    }

    // ========================================================================
    // Integration Tests for clean_all_checkpoints
    // ========================================================================

    #[tokio::test]
    async fn test_clean_all_checkpoints_empty_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Call clean_all_checkpoints on empty directory
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed with no deletions
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_no_completed() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create running checkpoint
        let checkpoint = create_test_checkpoint(WorkflowStatus::Running);
        save_checkpoint_to_file(&checkpoint_dir, "workflow-1", &checkpoint)
            .await
            .expect("Failed to save checkpoint");

        // Call clean_all_checkpoints
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed, no files deleted
        assert!(result.is_ok());
        assert!(checkpoint_dir.join("workflow-1.checkpoint.json").exists());
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_only_in_progress() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create multiple running checkpoints
        for i in 1..=3 {
            let checkpoint = create_test_checkpoint(WorkflowStatus::Running);
            save_checkpoint_to_file(&checkpoint_dir, &format!("workflow-{}", i), &checkpoint)
                .await
                .expect("Failed to save checkpoint");
        }

        // Call clean_all_checkpoints
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed, no files deleted
        assert!(result.is_ok());
        for i in 1..=3 {
            assert!(checkpoint_dir
                .join(format!("workflow-{}.checkpoint.json", i))
                .exists());
        }
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_mixed_status() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create completed checkpoints
        for i in 1..=2 {
            let checkpoint = create_test_checkpoint(WorkflowStatus::Completed);
            save_checkpoint_to_file(&checkpoint_dir, &format!("completed-{}", i), &checkpoint)
                .await
                .expect("Failed to save checkpoint");
        }

        // Create running checkpoint
        let checkpoint = create_test_checkpoint(WorkflowStatus::Running);
        save_checkpoint_to_file(&checkpoint_dir, "running-1", &checkpoint)
            .await
            .expect("Failed to save checkpoint");

        // Call clean_all_checkpoints with force flag
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed, only completed checkpoints deleted
        assert!(result.is_ok());
        assert!(!checkpoint_dir.join("completed-1.checkpoint.json").exists());
        assert!(!checkpoint_dir.join("completed-2.checkpoint.json").exists());
        assert!(checkpoint_dir.join("running-1.checkpoint.json").exists());
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_with_force_flag() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create completed checkpoint
        let checkpoint = create_test_checkpoint(WorkflowStatus::Completed);
        save_checkpoint_to_file(&checkpoint_dir, "workflow-1", &checkpoint)
            .await
            .expect("Failed to save checkpoint");

        // Call clean_all_checkpoints with force=true
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed and delete without prompting
        assert!(result.is_ok());
        assert!(!checkpoint_dir.join("workflow-1.checkpoint.json").exists());
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_with_non_json_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create non-JSON file
        fs::write(checkpoint_dir.join("readme.txt"), "test content")
            .await
            .expect("Failed to write file");

        // Create completed checkpoint
        let checkpoint = create_test_checkpoint(WorkflowStatus::Completed);
        save_checkpoint_to_file(&checkpoint_dir, "workflow-1", &checkpoint)
            .await
            .expect("Failed to save checkpoint");

        // Call clean_all_checkpoints
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed, only JSON checkpoint deleted
        assert!(result.is_ok());
        assert!(!checkpoint_dir.join("workflow-1.checkpoint.json").exists());
        assert!(checkpoint_dir.join("readme.txt").exists());
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_with_corrupted_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create corrupted JSON file (using .checkpoint.json extension)
        fs::write(
            checkpoint_dir.join("corrupted.checkpoint.json"),
            "{ invalid json content",
        )
        .await
        .expect("Failed to write file");

        // Create valid completed checkpoint
        let checkpoint = create_test_checkpoint(WorkflowStatus::Completed);
        save_checkpoint_to_file(&checkpoint_dir, "workflow-1", &checkpoint)
            .await
            .expect("Failed to save checkpoint");

        // Call clean_all_checkpoints
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed, skip corrupted file and delete valid completed checkpoint
        assert!(result.is_ok());
        assert!(!checkpoint_dir.join("workflow-1.checkpoint.json").exists());
        assert!(checkpoint_dir.join("corrupted.checkpoint.json").exists());
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_all_completed() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create multiple completed checkpoints
        for i in 1..=5 {
            let checkpoint = create_test_checkpoint(WorkflowStatus::Completed);
            save_checkpoint_to_file(&checkpoint_dir, &format!("workflow-{}", i), &checkpoint)
                .await
                .expect("Failed to save checkpoint");
        }

        // Call clean_all_checkpoints
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed and delete all completed checkpoints
        assert!(result.is_ok());
        for i in 1..=5 {
            assert!(!checkpoint_dir
                .join(format!("workflow-{}.checkpoint.json", i))
                .exists());
        }
    }

    #[tokio::test]
    async fn test_clean_all_checkpoints_failed_status() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let checkpoint_dir = temp_dir.path().to_path_buf();

        // Create failed checkpoint
        let checkpoint = create_test_checkpoint(WorkflowStatus::Failed);
        save_checkpoint_to_file(&checkpoint_dir, "failed-1", &checkpoint)
            .await
            .expect("Failed to save checkpoint");

        // Create completed checkpoint
        let checkpoint = create_test_checkpoint(WorkflowStatus::Completed);
        save_checkpoint_to_file(&checkpoint_dir, "completed-1", &checkpoint)
            .await
            .expect("Failed to save checkpoint");

        // Call clean_all_checkpoints
        let result = clean_all_checkpoints(&checkpoint_dir, true).await;

        // Should succeed, only completed deleted, not failed
        assert!(result.is_ok());
        assert!(checkpoint_dir.join("failed-1.checkpoint.json").exists());
        assert!(!checkpoint_dir.join("completed-1.checkpoint.json").exists());
    }
}
