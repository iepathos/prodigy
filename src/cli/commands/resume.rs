//! Resume command implementations
//!
//! This module handles resuming interrupted workflows and MapReduce jobs.

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use tokio::fs;

/// Resume an interrupted workflow or MapReduce job
///
/// This function provides a unified resume interface that works for both:
/// - Regular workflow sessions (session-xxx IDs)
/// - MapReduce jobs (mapreduce-xxx or session-mapreduce-xxx IDs)
///
/// It auto-detects the ID type and attempts the appropriate resume strategy.
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
            "No session ID provided. Please specify a session ID or job ID to resume.\n\
             Use 'prodigy sessions list' to see available sessions.\n\
             Use 'prodigy resume-job list' to see MapReduce jobs."
        ));
    };

    // Try to detect the type of ID and resume appropriately
    let resume_result = try_unified_resume(&session_id, from_checkpoint).await;

    match resume_result {
        Ok(()) => Ok(()),
        Err(e) => {
            // Provide helpful error message with suggestions
            Err(anyhow!(
                "Failed to resume {}: {}\n\n\
                 Troubleshooting:\n\
                 - Check if the session/job exists: 'prodigy sessions list' or 'prodigy resume-job list'\n\
                 - Ensure the worktree hasn't been cleaned up\n\
                 - For MapReduce jobs, try: 'prodigy resume-job {}'\n\
                 - For regular workflows, ensure checkpoint files exist",
                session_id,
                e,
                session_id
            ))
        }
    }
}

/// Enum representing the session type
enum SessionType {
    Workflow,
    MapReduce,
}

/// Check the session type by loading it from UnifiedSessionManager
async fn check_session_type(id: &str) -> Result<SessionType> {
    let storage =
        crate::storage::GlobalStorage::new().context("Failed to create global storage")?;
    let session_manager = crate::unified_session::SessionManager::new(storage)
        .await
        .context("Failed to create session manager")?;
    let session_id = crate::unified_session::SessionId::from_string(id.to_string());

    let session = session_manager
        .load_session(&session_id)
        .await
        .context("Session not found in UnifiedSessionManager")?;

    // Determine session type from session_type field
    match session.session_type {
        crate::unified_session::SessionType::MapReduce => Ok(SessionType::MapReduce),
        crate::unified_session::SessionType::Workflow => Ok(SessionType::Workflow),
    }
}

/// Try to resume using a unified approach that handles both session and job IDs
async fn try_unified_resume(id: &str, from_checkpoint: Option<String>) -> Result<()> {
    // Determine the ID type and try appropriate resume strategies
    let id_type = detect_id_type(id);

    match id_type {
        IdType::SessionId => {
            // First try regular workflow resume
            match try_resume_regular_workflow(id, from_checkpoint.clone()).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    // If that fails, maybe it's a MapReduce job with session ID
                    // Try to find a MapReduce job for this session
                    try_resume_mapreduce_from_session(id).await.or(Err(e))
                }
            }
        }
        IdType::MapReduceJobId => {
            // Try MapReduce job resume first
            try_resume_mapreduce_job(id).await
        }
        IdType::Ambiguous => {
            // For ambiguous IDs, check the session type first
            match check_session_type(id).await {
                Ok(SessionType::Workflow) => {
                    // It's a workflow session, use workflow resume
                    try_resume_regular_workflow(id, from_checkpoint.clone()).await
                }
                Ok(SessionType::MapReduce) => {
                    // It's a MapReduce session, use MapReduce resume
                    try_resume_mapreduce_job(id).await
                }
                Err(_) => {
                    // Session not found in UnifiedSessionManager, try workflow first
                    match try_resume_regular_workflow(id, from_checkpoint.clone()).await {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            // Check if the error is about a completed/cancelled session
                            // These are definitive errors that should not be overridden
                            let error_msg = e.to_string();
                            if error_msg.contains("already completed")
                                || error_msg.contains("was cancelled")
                            {
                                return Err(e);
                            }
                            // Otherwise, try MapReduce as fallback
                            try_resume_mapreduce_job(id).await
                        }
                    }
                }
            }
        }
    }
}

/// Enum representing the detected ID type
enum IdType {
    SessionId,      // Matches pattern "session-xxx"
    MapReduceJobId, // Matches pattern "mapreduce-xxx"
    Ambiguous,      // Unknown pattern, try both
}

/// Find the worktree directory for a given session ID
///
/// Searches through all repo subdirectories in the worktrees directory
/// to find the worktree matching the session ID.
async fn find_worktree_for_session(worktrees_dir: &PathBuf, session_id: &str) -> Result<PathBuf> {
    if !worktrees_dir.exists() {
        return Err(anyhow!(
            "Worktrees directory does not exist: {}",
            worktrees_dir.display()
        ));
    }

    // Iterate through all repo subdirectories
    let mut repo_dirs = fs::read_dir(worktrees_dir)
        .await
        .context("Failed to read worktrees directory")?;

    while let Some(repo_entry) = repo_dirs.next_entry().await? {
        if !repo_entry.path().is_dir() {
            continue;
        }

        // Check if this repo has the worktree
        let potential_worktree = repo_entry.path().join(session_id);
        if potential_worktree.exists() {
            return Ok(potential_worktree);
        }
    }

    // Not found - provide helpful error
    Err(anyhow!(
        "Worktree not found for session: {}\n\
         Searched in: {}\n\
         The worktree may have been cleaned up. You cannot resume this session.",
        session_id,
        worktrees_dir.display()
    ))
}

/// Detect the type of ID based on its format
fn detect_id_type(id: &str) -> IdType {
    if id.starts_with("session-mapreduce-") || id.starts_with("mapreduce-") {
        IdType::MapReduceJobId
    } else if id.starts_with("session-") {
        IdType::SessionId
    } else {
        IdType::Ambiguous
    }
}

/// Try to resume a regular workflow session
async fn try_resume_regular_workflow(
    session_id: &str,
    from_checkpoint: Option<String>,
) -> Result<()> {
    // Find checkpoint directory for this session using storage abstraction
    let prodigy_home = crate::storage::get_default_storage_dir()
        .context("Failed to determine Prodigy storage directory")?;

    // Check if session exists and is resumable by loading session metadata
    let storage =
        crate::storage::GlobalStorage::new().context("Failed to create global storage")?;
    let session_manager = crate::unified_session::SessionManager::new(storage)
        .await
        .context("Failed to create session manager")?;
    let session_id_obj = crate::unified_session::SessionId::from_string(session_id.to_string());

    // Try to load the session to check its status
    if let Ok(session) = session_manager.load_session(&session_id_obj).await {
        use crate::unified_session::SessionStatus;

        // Check if session is in a non-resumable state
        match session.status {
            SessionStatus::Completed => {
                return Err(anyhow!(
                    "Session {} has already completed and cannot be resumed.\n\
                     There is nothing to resume for this session.",
                    session_id
                ));
            }
            SessionStatus::Cancelled => {
                return Err(anyhow!(
                    "Session {} was cancelled and cannot be resumed.",
                    session_id
                ));
            }
            _ => {
                // Session is resumable (Paused, Running, Failed, etc.)
            }
        }
    }

    let checkpoint_dir = prodigy_home
        .join("state")
        .join(session_id)
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

    // Find the worktree for this session by searching in the worktrees directory
    let worktrees_dir = prodigy_home.join("worktrees");

    let worktree_path = find_worktree_for_session(&worktrees_dir, session_id).await?;

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

    // Use the worktree path as the project root for resuming
    // This ensures the orchestrator can find the correct session files
    println!("      Project root: {}", worktree_path.display());
    println!();

    // Execute prodigy run with --resume flag
    // This tells the orchestrator to use the existing worktree instead of creating a new one
    let workflow_pathbuf = PathBuf::from(workflow_path);
    let cook_cmd = crate::cook::command::CookCommand {
        playbook: workflow_pathbuf,
        path: Some(worktree_path.clone()), // Use worktree path, not current dir
        max_iterations: 1,
        map: vec![],
        args: vec![],
        fail_fast: false,
        auto_accept: false,
        metrics: false,
        resume: Some(session_id.to_string()), // This is the key - tells orchestrator to resume
        verbosity: 0,
        quiet: false,
        dry_run: false,
    };

    crate::cook::cook(cook_cmd).await
}

/// Try to resume a MapReduce job by job ID
async fn try_resume_mapreduce_job(job_id: &str) -> Result<()> {
    // Delegate to the existing MapReduce job resume command
    run_resume_job_command(job_id.to_string(), false, 0, None).await
}

/// Try to find and resume a MapReduce job associated with a session ID
async fn try_resume_mapreduce_from_session(session_id: &str) -> Result<()> {
    // Check if session exists and is resumable by loading session metadata
    let storage =
        crate::storage::GlobalStorage::new().context("Failed to create global storage")?;
    let session_manager = crate::unified_session::SessionManager::new(storage)
        .await
        .context("Failed to create session manager")?;
    let session_id_obj = crate::unified_session::SessionId::from_string(session_id.to_string());

    // Try to load the session to check its status
    if let Ok(session) = session_manager.load_session(&session_id_obj).await {
        use crate::unified_session::SessionStatus;

        // Check if session is in a non-resumable state
        match session.status {
            SessionStatus::Completed => {
                return Err(anyhow!(
                    "Session {} has already completed and cannot be resumed.\n\
                     There is nothing to resume for this session.",
                    session_id
                ));
            }
            SessionStatus::Cancelled => {
                return Err(anyhow!(
                    "Session {} was cancelled and cannot be resumed.",
                    session_id
                ));
            }
            _ => {
                // Session is resumable (Paused, Running, Failed, etc.)
            }
        }
    }

    // Look for MapReduce jobs in the global storage
    let prodigy_home = crate::storage::get_default_storage_dir()
        .context("Failed to determine Prodigy storage directory")?;

    // Try to find a MapReduce job for this session
    // MapReduce jobs are stored at: ~/.prodigy/state/{repo}/mapreduce/jobs/{job-id}/
    let state_dir = prodigy_home.join("state");

    if !state_dir.exists() {
        return Err(anyhow!("No state directory found"));
    }

    // Search for MapReduce jobs containing the session ID
    let mut found_job_id: Option<String> = None;

    if let Ok(entries) = fs::read_dir(&state_dir).await {
        let mut entries = entries;
        while let Ok(Some(repo_entry)) = entries.next_entry().await {
            if !repo_entry.path().is_dir() {
                continue;
            }

            let mapreduce_dir = repo_entry.path().join("mapreduce").join("jobs");
            if !mapreduce_dir.exists() {
                continue;
            }

            if let Ok(job_entries) = fs::read_dir(&mapreduce_dir).await {
                let mut job_entries = job_entries;
                while let Ok(Some(job_entry)) = job_entries.next_entry().await {
                    let job_name = job_entry.file_name();
                    let job_id = job_name.to_string_lossy();

                    // Check if this job is associated with the session
                    if job_id.contains(session_id) {
                        found_job_id = Some(job_id.to_string());
                        break;
                    }
                }
            }

            if found_job_id.is_some() {
                break;
            }
        }
    }

    if let Some(job_id) = found_job_id {
        println!("Found MapReduce job: {}", job_id);
        try_resume_mapreduce_job(&job_id).await
    } else {
        Err(anyhow!(
            "No MapReduce job found for session: {}",
            session_id
        ))
    }
}

/// Resume a MapReduce job from its checkpoint
pub async fn run_resume_job_command(
    job_id: String,
    _force: bool,
    _max_retries: u32,
    _path: Option<PathBuf>,
) -> Result<()> {
    println!("🔄 Resuming MapReduce job: {}", job_id);

    // Find the MapReduce job checkpoint
    let prodigy_home = crate::storage::get_default_storage_dir()
        .context("Failed to determine Prodigy storage directory")?;

    // Search for the job in the global storage
    let state_dir = prodigy_home.join("state");
    if !state_dir.exists() {
        return Err(anyhow!(
            "No state directory found at: {}",
            state_dir.display()
        ));
    }

    // Find the job checkpoint
    let mut job_path: Option<PathBuf> = None;

    if let Ok(entries) = fs::read_dir(&state_dir).await {
        let mut entries = entries;
        while let Ok(Some(repo_entry)) = entries.next_entry().await {
            if !repo_entry.path().is_dir() {
                continue;
            }

            let potential_job_path = repo_entry
                .path()
                .join("mapreduce")
                .join("jobs")
                .join(&job_id);

            if potential_job_path.exists() {
                job_path = Some(potential_job_path);
                break;
            }
        }
    }

    let job_dir = job_path.ok_or_else(|| {
        anyhow!(
            "MapReduce job not found: {}\n\
             Searched in: {}",
            job_id,
            state_dir.display()
        )
    })?;

    println!("📂 Found job at: {}", job_dir.display());

    // Check for checkpoint files
    if let Ok(mut entries) = fs::read_dir(&job_dir).await {
        println!("\n📋 Available checkpoints:");
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            if let Some(name_str) = name.to_str() {
                if name_str.contains("checkpoint") {
                    println!("  - {}", name_str);
                }
            }
        }
    }

    println!("\n🔍 Analyzing checkpoints for job: {}", job_id);

    // TODO: Implement full resume logic with CheckpointManager
    // This is a basic implementation that will be enhanced with:
    // - Loading checkpoint state from the job directory
    // - Reconstructing workflow state
    // - Resuming execution from the appropriate phase (setup/map/reduce)

    println!("\n📋 Next steps for resume implementation:");
    println!("  1. Load checkpoint using CheckpointManager");
    println!("  2. Reconstruct workflow state from checkpoint data");
    println!("  3. Determine which phase to resume from (setup/map/reduce)");
    println!("  4. Call MapReduceExecutor to continue execution");
    println!("\n📂 Checkpoint location: {}", job_dir.display());
    println!(
        "\n💡 Use 'prodigy resume {}' to retry once implementation is complete",
        job_id
    );

    Ok(())
}
