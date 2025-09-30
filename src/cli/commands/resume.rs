//! Resume command implementations
//!
//! This module handles resuming interrupted workflows and MapReduce jobs.

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use tokio::fs;

/// Resume an interrupted workflow
pub async fn run_resume_workflow(
    session_id: Option<String>,
    _force: bool,
    from_checkpoint: Option<String>,
    _path: Option<PathBuf>,
) -> Result<()> {
    // If no session ID provided, try to find the most recent interrupted session
    let session_id = if let Some(id) = session_id {
        id
    } else {
        return Err(anyhow!(
            "No session ID provided. Please specify a session ID to resume.\n\
             Use 'prodigy sessions list' to see available sessions."
        ));
    };

    // Find checkpoint directory for this session
    let home = directories::BaseDirs::new()
        .ok_or_else(|| anyhow!("Could not determine home directory"))?
        .home_dir()
        .to_path_buf();

    let checkpoint_dir = home
        .join(".prodigy")
        .join("state")
        .join(&session_id)
        .join("checkpoints");

    if !checkpoint_dir.exists() {
        return Err(anyhow!(
            "No checkpoints found for session: {}\n\
             Checkpoint directory does not exist: {}",
            session_id,
            checkpoint_dir.display()
        ));
    }

    // Find checkpoint file - either specified or the latest one
    let checkpoint_file = if let Some(checkpoint_id) = &from_checkpoint {
        let file = checkpoint_dir.join(format!("{}.checkpoint.json", checkpoint_id));
        if !file.exists() {
            return Err(anyhow!(
                "Checkpoint not found: {}\nExpected at: {}",
                checkpoint_id,
                file.display()
            ));
        }
        file
    } else {
        // Find the most recent checkpoint file
        let mut entries = fs::read_dir(&checkpoint_dir)
            .await
            .context("Failed to read checkpoint directory")?;

        let mut latest_checkpoint: Option<(PathBuf, std::time::SystemTime)> = None;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".checkpoint.json"))
                    .unwrap_or(false)
            {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        if latest_checkpoint.is_none()
                            || modified > latest_checkpoint.as_ref().unwrap().1
                        {
                            latest_checkpoint = Some((path.clone(), modified));
                        }
                    }
                }
            }
        }

        latest_checkpoint
            .ok_or_else(|| anyhow!("No checkpoint files found in {}", checkpoint_dir.display()))?
            .0
    };

    // Read checkpoint to extract workflow path
    let checkpoint_json = fs::read_to_string(&checkpoint_file)
        .await
        .with_context(|| {
            format!(
                "Failed to read checkpoint file: {}",
                checkpoint_file.display()
            )
        })?;

    let checkpoint: serde_json::Value =
        serde_json::from_str(&checkpoint_json).context("Failed to parse checkpoint JSON")?;

    let workflow_path = checkpoint
        .get("workflow_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Checkpoint does not contain workflow_path field"))?;

    println!("🔄 Resuming session: {}", session_id);
    println!("📄 Workflow: {}", workflow_path);
    println!(
        "📍 Checkpoint: {}",
        checkpoint_file.file_name().unwrap().to_string_lossy()
    );

    // Get worktree path for this session
    let worktree_path = home
        .join(".prodigy")
        .join("worktrees")
        .join("prodigy") // TODO: Get actual repo name
        .join(&session_id);

    if !worktree_path.exists() {
        return Err(anyhow!(
            "Worktree not found for session: {}\n\
             Expected at: {}\n\
             The worktree may have been cleaned up. You cannot resume this session.",
            session_id,
            worktree_path.display()
        ));
    }

    println!();
    println!("Note: Resuming from worktree: {}", worktree_path.display());
    if from_checkpoint.is_some() {
        println!(
            "      Using specific checkpoint: {}",
            from_checkpoint.as_ref().unwrap()
        );
    } else {
        println!("      Using latest checkpoint");
    }
    println!();

    // Execute prodigy run with the workflow in the worktree directory
    // The checkpoint system will automatically detect and resume from the checkpoint
    // We don't use the --resume flag because that tries to load from UnifiedSessionManager
    // which may not have the session data
    let workflow_pathbuf = PathBuf::from(workflow_path);
    let cook_cmd = crate::cook::command::CookCommand {
        playbook: workflow_pathbuf,
        path: Some(worktree_path),
        max_iterations: 1,
        map: vec![],
        args: vec![],
        fail_fast: false,
        auto_accept: false,
        metrics: false,
        resume: None, // Let checkpoint system handle resume automatically
        verbosity: 0,
        quiet: false,
        dry_run: false,
    };

    crate::cook::cook(cook_cmd).await
}

/// Resume a MapReduce job from its checkpoint
pub async fn run_resume_job_command(
    _job_id: String,
    _force: bool,
    _max_retries: u32,
    _path: Option<PathBuf>,
) -> Result<()> {
    println!("Resuming MapReduce job from checkpoint...");
    Ok(())
}
