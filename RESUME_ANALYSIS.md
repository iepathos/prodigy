# Resume Commands Analysis and Fix Proposal

## Executive Summary

Both `prodigy resume` and `prodigy resume-job` commands have critical implementation issues preventing them from functioning:

1. **`prodigy resume`** - Works for regular workflows but fails for MapReduce workflows because it doesn't properly handle MapReduce job resumption
2. **`prodigy resume-job`** - Completely non-functional; it's a TODO stub that only displays checkpoint information without actually resuming

## Root Cause Analysis

### Issue 1: `prodigy resume-job` is Not Implemented

**Location**: `src/cli/commands/resume.rs:481-578`

**Problem**: The function `run_resume_job_command()` is a stub:

```rust
// Lines 560-575
println!("\n📋 Next steps for resume implementation:");
println!("  1. Load checkpoint using CheckpointManager");
println!("  2. Reconstruct workflow state from checkpoint data");
println!("  3. Determine which phase to resume from (setup/map/reduce)");
println!("  4. Call MapReduceExecutor to continue execution");
```

**Impact**: Users cannot resume MapReduce jobs via CLI, forcing them to restart from scratch.

**Evidence**:
- Function ends with `Ok(())` without performing any actual resume operation
- Prints "Next steps" instead of executing them
- Spec 159 explicitly documents this as a TODO (line 560)

### Issue 2: `prodigy resume` Doesn't Properly Handle MapReduce Jobs

**Location**: `src/cli/commands/resume.rs:195-378`

**Problems**:

1. **Tries regular workflow resume first** (line 90), which fails for MapReduce sessions
2. **Falls back to MapReduce resume** (line 95), but delegates to the broken `run_resume_job_command()`
3. **No direct integration** with `MapReduceResumeManager.resume_job()` which actually works

**Code Flow**:
```rust
try_unified_resume()
  → try_resume_regular_workflow()  // Fails for MapReduce
  → try_resume_mapreduce_from_session()  // Delegates to broken function
     → run_resume_job_command()  // Prints TODO and exits
```

**Why it breaks**:
- Line 384: `run_resume_job_command(job_id.to_string(), false, 0, None).await`
- This calls the stub implementation that does nothing

### Issue 3: Working Implementation is Not Exposed to CLI

**Location**: `src/cook/execution/mapreduce_resume.rs:203-208`

**The Fix**: There's actually a working implementation!

```rust
pub async fn resume_job(
    &self,
    job_id: &str,
    options: EnhancedResumeOptions,
    env: &ExecutionEnvironment,
) -> MRResult<EnhancedResumeResult>
```

**Problem**: This is never called from the CLI layer. The CLI stub reimplements (incompletely) what this already does.

## Proposed Solutions

### Solution 1: Complete the `run_resume_job_command()` Implementation

**Approach**: Connect the CLI command to the existing working `MapReduceResumeManager`.

**Changes Required**:

1. **Import and use `MapReduceResumeManager`**
2. **Load checkpoint and job state**
3. **Create execution environment**
4. **Call `resume_job()` method**
5. **Display results**

**Implementation** (`src/cli/commands/resume.rs:481-578`):

```rust
pub async fn run_resume_job_command(
    job_id: String,
    force: bool,
    max_retries: u32,
    path: Option<PathBuf>,
) -> Result<()> {
    use crate::cook::execution::mapreduce_resume::{
        MapReduceResumeManager, EnhancedResumeOptions
    };
    use crate::cook::orchestrator::ExecutionEnvironment;

    println!("🔄 Resuming MapReduce job: {}", job_id);

    // Acquire resume lock
    let prodigy_home = crate::storage::get_default_storage_dir()
        .context("Failed to determine Prodigy storage directory")?;
    let lock_manager = crate::cook::execution::ResumeLockManager::new(prodigy_home.clone())
        .context("Failed to create resume lock manager")?;
    let _lock = lock_manager
        .acquire_lock(&job_id)
        .await
        .context("Failed to acquire resume lock")?;

    // Find the job directory
    let state_dir = prodigy_home.join("state");
    if !state_dir.exists() {
        return Err(anyhow!(
            "No state directory found at: {}",
            state_dir.display()
        ));
    }

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
            "MapReduce job not found: {}\nSearched in: {}",
            job_id,
            state_dir.display()
        )
    })?;

    println!("📂 Found job at: {}", job_dir.display());

    // Create resume options
    let options = EnhancedResumeOptions {
        force,
        max_additional_retries: max_retries,
        include_dlq_items: true,
        ..Default::default()
    };

    // Load session to get workflow info
    let storage = crate::storage::GlobalStorage::new()
        .context("Failed to create global storage")?;
    let session_manager = crate::unified_session::SessionManager::new(storage).await
        .context("Failed to create session manager")?;

    // Find session for this job
    // Note: This may require loading from session-job mapping
    let session_id = find_session_for_job(&job_id).await?;
    let session_id_obj = crate::unified_session::SessionId::from_string(session_id.clone());
    let session = session_manager.load_session(&session_id_obj).await
        .context("Failed to load session")?;

    // Get workflow data
    let workflow_data = session.workflow_data
        .ok_or_else(|| anyhow!("Session has no workflow data"))?;

    // Load the workflow file
    let workflow_path = PathBuf::from(&workflow_data.workflow_path);
    if !workflow_path.exists() {
        return Err(anyhow!(
            "Workflow file not found: {}\nThe workflow file may have been moved or deleted.",
            workflow_path.display()
        ));
    }

    // Parse the workflow
    let workflow_content = std::fs::read_to_string(&workflow_path)
        .with_context(|| format!("Failed to read workflow file: {}", workflow_path.display()))?;
    let workflow: crate::cook::playbook::Playbook = serde_yaml::from_str(&workflow_content)
        .with_context(|| format!("Failed to parse workflow file: {}", workflow_path.display()))?;

    // Create execution environment
    let project_root = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let env = ExecutionEnvironment {
        repo_path: project_root.clone(),
        playbook: workflow,
        args: vec![],
        verbosity: 0,
    };

    // Create resume manager
    let event_logger = Arc::new(EventLogger::new(&job_id, &job_dir)?);
    let dlq = Arc::new(DeadLetterQueue::new(&job_id, &job_dir)?);
    let state_manager = JobStateManager::new(&job_dir)?;

    let resume_manager = MapReduceResumeManager::new(
        event_logger,
        dlq,
        state_manager,
    );

    // Resume the job
    println!("\n🔍 Loading checkpoint and resuming execution...\n");

    let result = resume_manager.resume_job(&job_id, options, &env).await
        .context("Failed to resume MapReduce job")?;

    // Display summary
    println!("\n✅ Resume complete!");
    println!("📊 Summary:");
    println!("   Phase resumed from: {:?}", result.phase_resumed_from);
    println!("   Items processed: {}", result.items_processed);
    println!("   Items failed: {}", result.items_failed);
    println!("   Items pending: {}", result.items_pending);
    println!("   Duration: {:?}", result.execution_duration);
    println!("   Final status: {:?}", result.final_status);

    if !result.errors.is_empty() {
        println!("\n⚠️  Errors occurred:");
        for error in result.errors.iter().take(5) {
            println!("   - {}", error);
        }
        if result.errors.len() > 5 {
            println!("   ... and {} more", result.errors.len() - 5);
        }
    }

    Ok(())
}

/// Find the session ID for a given job ID
async fn find_session_for_job(job_id: &str) -> Result<String> {
    // Check session-job mappings
    let prodigy_home = crate::storage::get_default_storage_dir()?;
    let mappings_dir = prodigy_home.join("state").join("mappings");

    if !mappings_dir.exists() {
        return Err(anyhow!("No session-job mappings found"));
    }

    // Look for job-to-session mapping
    let job_mapping_file = mappings_dir.join(format!("job-{}.json", job_id));
    if job_mapping_file.exists() {
        let content = fs::read_to_string(&job_mapping_file).await?;
        let mapping: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(session_id) = mapping.get("session_id").and_then(|v| v.as_str()) {
            return Ok(session_id.to_string());
        }
    }

    // Fallback: Extract from job_id if it contains session info
    if job_id.contains("session-") {
        // Format: mapreduce-{timestamp}_session-{uuid}
        if let Some(session_part) = job_id.split("session-").nth(1) {
            return Ok(format!("session-{}", session_part));
        }
    }

    Err(anyhow!("Could not find session ID for job: {}", job_id))
}
```

### Solution 2: Fix Unified Resume Logic

**Problem**: The `try_unified_resume` function doesn't properly handle MapReduce jobs.

**Fix** (`src/cli/commands/resume.rs:82-135`):

```rust
async fn try_unified_resume(id: &str, from_checkpoint: Option<String>) -> Result<()> {
    let id_type = detect_id_type(id);

    match id_type {
        IdType::SessionId => {
            // Check session type FIRST
            match check_session_type(id).await {
                Ok(SessionType::Workflow) => {
                    try_resume_regular_workflow(id, from_checkpoint).await
                }
                Ok(SessionType::MapReduce) => {
                    // It's a MapReduce session, find the job ID
                    try_resume_mapreduce_from_session(id).await
                }
                Err(_) => {
                    // Session not found, try workflow resume as fallback
                    try_resume_regular_workflow(id, from_checkpoint).await
                }
            }
        }
        IdType::MapReduceJobId => {
            // Direct MapReduce job resume
            try_resume_mapreduce_job(id).await
        }
        IdType::Ambiguous => {
            // For ambiguous IDs, check session type first
            match check_session_type(id).await {
                Ok(SessionType::MapReduce) => try_resume_mapreduce_job(id).await,
                Ok(SessionType::Workflow) => try_resume_regular_workflow(id, from_checkpoint).await,
                Err(_) => {
                    // Try both, workflow first
                    try_resume_regular_workflow(id, from_checkpoint.clone())
                        .await
                        .or_else(|_| try_resume_mapreduce_job(id).await)
                }
            }
        }
    }
}
```

## Comprehensive Test Coverage

### Test File: `tests/cli_integration/resume_mapreduce_cli_test.rs`

```rust
//! End-to-end CLI tests for MapReduce resume functionality

use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

mod test_utils;
use test_utils::*;

/// Test that prodigy resume-job actually resumes execution
#[tokio::test]
async fn test_resume_job_executes_successfully() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_mapreduce_workflow(&temp_dir, 10)?;

    // Start workflow and interrupt after 3 items
    let job_id = start_and_interrupt_workflow(&workflow_path, 3).await?;

    // Verify checkpoint exists
    assert!(checkpoint_exists(&job_id));

    // Resume via CLI
    let output = run_cli(&["prodigy", "resume-job", &job_id]).await?;

    // Verify success
    assert!(output.status.success(), "Resume should succeed");
    assert!(output.stdout.contains("Resume complete"), "Should show completion");
    assert!(output.stdout.contains("Items processed"), "Should show progress");

    // Verify all items completed
    let result = verify_all_items_completed(&job_id).await?;
    assert_eq!(result.total, 10);
    assert_eq!(result.completed, 10);

    Ok(())
}

/// Test that prodigy resume works with MapReduce session IDs
#[tokio::test]
async fn test_resume_with_mapreduce_session_id() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_mapreduce_workflow(&temp_dir, 5)?;

    // Start and interrupt
    let (session_id, job_id) = start_and_interrupt_with_session(&workflow_path, 2).await?;

    // Resume using session ID (not job ID)
    let output = run_cli(&["prodigy", "resume", &session_id]).await?;

    // Should auto-detect MapReduce and resume
    assert!(output.status.success());
    assert!(output.stdout.contains("Resume complete") || output.stdout.contains("Resuming MapReduce"));

    // Verify completion
    let result = verify_all_items_completed(&job_id).await?;
    assert_eq!(result.completed, 5);

    Ok(())
}

/// Test resume with failed items in DLQ
#[tokio::test]
async fn test_resume_with_dlq_items() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_failing_mapreduce_workflow(&temp_dir, 10, vec![3, 7])?;

    // Run until items fail
    let job_id = run_until_failures(&workflow_path).await?;

    // Verify DLQ has failed items
    let dlq_count = count_dlq_items(&job_id).await?;
    assert_eq!(dlq_count, 2, "DLQ should have 2 failed items");

    // Resume with force retry
    let output = run_cli(&["prodigy", "resume-job", &job_id, "--force", "--max-retries", "3"]).await?;

    // Should retry DLQ items
    assert!(output.status.success());

    Ok(())
}

/// Test resume from different phases
#[tokio::test]
async fn test_resume_from_each_phase() -> Result<()> {
    // Test resume from setup phase
    let job_id_setup = create_job_interrupted_in_setup().await?;
    let output = run_cli(&["prodigy", "resume-job", &job_id_setup]).await?;
    assert!(output.status.success());
    assert!(output.stdout.contains("Setup") || output.stdout.contains("setup"));

    // Test resume from map phase
    let job_id_map = create_job_interrupted_in_map().await?;
    let output = run_cli(&["prodigy", "resume-job", &job_id_map]).await?;
    assert!(output.status.success());
    assert!(output.stdout.contains("Map") || output.stdout.contains("map"));

    // Test resume from reduce phase
    let job_id_reduce = create_job_interrupted_in_reduce().await?;
    let output = run_cli(&["prodigy", "resume-job", &job_id_reduce]).await?;
    assert!(output.status.success());
    assert!(output.stdout.contains("Reduce") || output.stdout.contains("reduce"));

    Ok(())
}

/// Test error handling for missing job
#[tokio::test]
async fn test_resume_nonexistent_job() -> Result<()> {
    let output = run_cli(&["prodigy", "resume-job", "nonexistent-job-123"]).await?;

    assert!(!output.status.success());
    assert!(output.stderr.contains("not found") || output.stderr.contains("does not exist"));

    Ok(())
}

/// Test error handling for corrupted checkpoint
#[tokio::test]
async fn test_resume_with_corrupted_checkpoint() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_mapreduce_workflow(&temp_dir, 5)?;

    // Create job and checkpoint
    let job_id = start_and_interrupt_workflow(&workflow_path, 2).await?;

    // Corrupt the checkpoint
    corrupt_checkpoint(&job_id).await?;

    // Try to resume
    let output = run_cli(&["prodigy", "resume-job", &job_id]).await?;

    assert!(!output.status.success());
    assert!(
        output.stderr.contains("corrupt")
        || output.stderr.contains("invalid")
        || output.stderr.contains("Failed to load")
    );

    Ok(())
}

/// Test resume with missing workflow file
#[tokio::test]
async fn test_resume_with_missing_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_mapreduce_workflow(&temp_dir, 5)?;

    // Create job
    let job_id = start_and_interrupt_workflow(&workflow_path, 2).await?;

    // Delete workflow file
    std::fs::remove_file(&workflow_path)?;

    // Try to resume
    let output = run_cli(&["prodigy", "resume-job", &job_id]).await?;

    assert!(!output.status.success());
    assert!(output.stderr.contains("Workflow file not found"));

    Ok(())
}

/// Test concurrent resume protection
#[tokio::test]
async fn test_concurrent_resume_blocked() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_mapreduce_workflow(&temp_dir, 10)?;

    let job_id = start_and_interrupt_workflow(&workflow_path, 3).await?;

    // Start first resume
    let resume1 = start_resume_async(&job_id);

    // Try second resume immediately
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let output2 = run_cli(&["prodigy", "resume-job", &job_id]).await?;

    // Second should be blocked
    assert!(!output2.status.success());
    assert!(output2.stderr.contains("already in progress") || output2.stderr.contains("lock"));

    // Wait for first to complete
    let output1 = resume1.await?;
    assert!(output1.status.success());

    Ok(())
}

/// Test verbosity levels affect output
#[tokio::test]
async fn test_resume_with_verbosity() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_mapreduce_workflow(&temp_dir, 3)?;
    let job_id = start_and_interrupt_workflow(&workflow_path, 1).await?;

    // Default verbosity
    let output_default = run_cli(&["prodigy", "resume-job", &job_id]).await?;
    let default_lines = output_default.stdout.lines().count();

    // High verbosity
    let job_id2 = start_and_interrupt_workflow(&workflow_path, 1).await?;
    let output_verbose = run_cli(&["prodigy", "-vvv", "resume-job", &job_id2]).await?;
    let verbose_lines = output_verbose.stdout.lines().count();

    // Verbose should have more output
    assert!(verbose_lines > default_lines, "Verbose mode should produce more output");

    Ok(())
}
```

### Test File: `tests/cli_integration/resume_workflow_cli_test.rs`

```rust
//! Tests for standard workflow resume command

use anyhow::Result;
use tempfile::TempDir;

mod test_utils;
use test_utils::*;

/// Test resume command detects completed sessions
#[tokio::test]
async fn test_resume_completed_session_fails() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_simple_workflow(&temp_dir)?;

    // Run workflow to completion
    let output = run_cli(&["prodigy", "run", workflow_path.to_str().unwrap(), "-y"]).await?;
    assert!(output.status.success());

    // Extract session ID
    let session_id = extract_session_id_from_output(&output.stdout)?;

    // Try to resume completed session
    let resume_output = run_cli(&["prodigy", "resume", &session_id]).await?;

    assert!(!resume_output.status.success());
    assert!(resume_output.stderr.contains("already completed") || resume_output.stderr.contains("cannot be resumed"));

    Ok(())
}

/// Test resume with missing checkpoint
#[tokio::test]
async fn test_resume_missing_checkpoint() -> Result<()> {
    let fake_session_id = "session-nonexistent-12345";

    let output = run_cli(&["prodigy", "resume", fake_session_id]).await?;

    assert!(!output.status.success());
    assert!(output.stderr.contains("not found") || output.stderr.contains("No checkpoints"));

    Ok(())
}

/// Test resume with from-checkpoint flag
#[tokio::test]
async fn test_resume_from_specific_checkpoint() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_multi_step_workflow(&temp_dir)?;

    // Run and interrupt
    let session_id = start_and_interrupt_standard_workflow(&workflow_path).await?;

    // Get checkpoint ID
    let checkpoint_id = get_latest_checkpoint_id(&session_id).await?;

    // Resume from specific checkpoint
    let output = run_cli(&[
        "prodigy",
        "resume",
        &session_id,
        "--from-checkpoint",
        &checkpoint_id
    ]).await?;

    assert!(output.status.success());
    assert!(output.stdout.contains("checkpoint") || output.stdout.contains("Resuming"));

    Ok(())
}
```

### Test Helpers (`tests/cli_integration/test_utils.rs`)

```rust
//! Test utilities for CLI integration tests

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;
use tokio::process::Command as AsyncCommand;

/// Run prodigy CLI command
pub async fn run_cli(args: &[&str]) -> Result<Output> {
    let output = AsyncCommand::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("prodigy")
        .arg("--")
        .args(args)
        .output()
        .await
        .context("Failed to run prodigy CLI")?;

    Ok(output)
}

/// Create a test MapReduce workflow with N items
pub fn create_test_mapreduce_workflow(dir: &TempDir, item_count: usize) -> Result<PathBuf> {
    let workflow_path = dir.path().join("test-mapreduce.yml");
    let items: Vec<serde_json::Value> = (0..item_count)
        .map(|i| serde_json::json!({"id": i, "name": format!("item-{}", i)}))
        .collect();

    let items_file = dir.path().join("items.json");
    std::fs::write(&items_file, serde_json::to_string_pretty(&items)?)?;

    let workflow_content = format!(r#"
name: test-mapreduce-workflow
mode: mapreduce

map:
  input: {}
  json_path: "$[*]"
  max_parallel: 3

  agent_template:
    - shell: "echo Processing ${{item.name}}"
    - shell: "sleep 0.1"

reduce:
  - shell: "echo All done"
"#, items_file.display());

    std::fs::write(&workflow_path, workflow_content)?;
    Ok(workflow_path)
}

/// Start a workflow and interrupt it after N items processed
pub async fn start_and_interrupt_workflow(
    workflow_path: &Path,
    interrupt_after: usize,
) -> Result<String> {
    // Start workflow in background
    let mut child = AsyncCommand::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("prodigy")
        .arg("--")
        .arg("run")
        .arg(workflow_path)
        .arg("-y")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start workflow")?;

    // Wait for N items to process
    wait_for_items_processed(interrupt_after).await?;

    // Send SIGINT to interrupt
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        let pid = Pid::from_raw(child.id().unwrap() as i32);
        kill(pid, Signal::SIGINT).context("Failed to send SIGINT")?;
    }

    // Get job ID from output
    let output = child.wait_with_output().await?;
    let job_id = extract_job_id_from_output(&String::from_utf8_lossy(&output.stderr))?;

    Ok(job_id)
}

/// Extract job ID from CLI output
pub fn extract_job_id_from_output(output: &str) -> Result<String> {
    // Look for patterns like "job_id: mapreduce-..."
    for line in output.lines() {
        if line.contains("mapreduce-") {
            if let Some(start) = line.find("mapreduce-") {
                let job_id_part = &line[start..];
                if let Some(end) = job_id_part.find(|c: char| c.is_whitespace()) {
                    return Ok(job_id_part[..end].to_string());
                } else {
                    return Ok(job_id_part.to_string());
                }
            }
        }
    }
    anyhow::bail!("Could not extract job ID from output")
}

/// Check if checkpoint exists for job
pub fn checkpoint_exists(job_id: &str) -> bool {
    let prodigy_home = dirs::home_dir().unwrap().join(".prodigy");
    let checkpoint_dir = prodigy_home.join("state").join("prodigy").join("mapreduce").join("jobs").join(job_id);
    checkpoint_dir.exists()
}

/// Verify all items completed
pub async fn verify_all_items_completed(job_id: &str) -> Result<CompletionStatus> {
    // Load job state and verify
    let state_path = get_job_state_path(job_id)?;
    let state_content = tokio::fs::read_to_string(&state_path).await?;
    let state: serde_json::Value = serde_json::from_str(&state_content)?;

    let total = state["total_items"].as_u64().unwrap_or(0) as usize;
    let completed = state["completed_items"].as_u64().unwrap_or(0) as usize;

    Ok(CompletionStatus { total, completed })
}

pub struct CompletionStatus {
    pub total: usize,
    pub completed: usize,
}

fn get_job_state_path(job_id: &str) -> Result<PathBuf> {
    let prodigy_home = dirs::home_dir().unwrap().join(".prodigy");
    let state_file = prodigy_home
        .join("state")
        .join("prodigy")
        .join("mapreduce")
        .join("jobs")
        .join(job_id)
        .join("job-state.json");
    Ok(state_file)
}

// Additional helper functions...
```

## Implementation Priority

### Phase 1: Critical Fix (1-2 days)
1. ✅ Complete `run_resume_job_command()` implementation
2. ✅ Add `find_session_for_job()` helper
3. ✅ Test with real MapReduce workflow

### Phase 2: Integration (1 day)
1. ✅ Fix `try_unified_resume()` logic
2. ✅ Add end-to-end CLI tests
3. ✅ Verify error messaging

### Phase 3: Comprehensive Testing (2-3 days)
1. ✅ Implement all test cases from spec 160
2. ✅ Add edge case tests
3. ✅ Verify coverage >90%

## Success Criteria

- [ ] `prodigy resume-job <job_id>` successfully resumes MapReduce workflows
- [ ] `prodigy resume <session_id>` works for both standard and MapReduce workflows
- [ ] All error scenarios provide helpful messages
- [ ] Resume can be interrupted and resumed again
- [ ] Test coverage exceeds 90% for resume modules
- [ ] All existing tests continue to pass

## Files to Modify

1. **`src/cli/commands/resume.rs`**
   - Lines 481-578: Replace `run_resume_job_command()` stub with working implementation
   - Lines 82-135: Fix `try_unified_resume()` to check session type first
   - Add `find_session_for_job()` helper function

2. **`tests/cli_integration/resume_mapreduce_cli_test.rs`** (NEW)
   - Add comprehensive end-to-end CLI tests

3. **`tests/cli_integration/resume_workflow_cli_test.rs`** (NEW)
   - Add standard workflow resume tests

4. **`tests/cli_integration/test_utils.rs`** (UPDATE)
   - Add helper functions for CLI testing

## Risk Assessment

**Low Risk Changes**:
- Completing the stub implementation
- Adding new tests

**Medium Risk Changes**:
- Changing `try_unified_resume()` logic (may affect existing behavior)
- Loading session-job mappings (may not exist for old jobs)

**Mitigation**:
- Test thoroughly with existing workflows
- Add fallback logic for missing mappings
- Keep detailed error messages for debugging

## References

- **Spec 134**: MapReduce Checkpoint and Resume
- **Spec 159**: MapReduce Resume CLI Implementation
- **Spec 160**: Comprehensive Resume Test Coverage
- **CLAUDE.md**: Lines documenting resume functionality
