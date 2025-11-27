---
number: 186
title: Non-MapReduce Workflow Resume
category: foundation
priority: critical
status: draft
dependencies: [183, 184, 185]
created: 2025-11-26
---

# Specification 186: Non-MapReduce Workflow Resume

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 183 (Effect-Based Workflow Execution), Spec 184 (Unified Checkpoint System), Spec 185 (Claude Command Retry)

## Context

### Current Problem

Resume for standard (non-MapReduce) workflows is **broken**:

1. **Checkpoints never created**: Standard workflows update `SessionState` but resume looks for checkpoint files that don't exist
2. **Checkpoints only on interrupt**: `should_save_checkpoint()` returns `true` only for `Interrupted`, not `Failed`
3. **Failed step skipped**: Even if checkpoint existed, resume would skip the failed step instead of retrying it
4. **Two disconnected systems**: Session state vs checkpoint files don't synchronize

### Test Evidence

```bash
# Start workflow with 5 steps
$ prodigy run workflow.yml

# Claude command fails at step 3 due to 500 error
# Error: Claude command failed: 500 Internal Server Error

# Try to resume
$ prodigy resume session-abc123
Error: No checkpoints found for session session-abc123

# Check for checkpoint files
$ ls ~/.prodigy/state/session-abc123/checkpoints/
# Directory doesn't exist!
```

### Expected Behavior

```bash
# Claude command fails at step 3
# Checkpoint automatically saved with Failed state

# Resume finds checkpoint
$ prodigy resume session-abc123
Resuming from checkpoint (2/5 steps completed)
Retrying step 3: /claude-command
...
Workflow completed successfully

# Only step 3+ executed, steps 1-2 skipped
```

## Objective

Implement reliable resume for standard workflows that:
1. **Creates checkpoints on failure** - not just on interrupt
2. **Retries the failed step** - doesn't skip it
3. **Preserves all progress** - completed steps not re-executed
4. **Uses unified checkpoint system** - single source of truth (Spec 184)
5. **Integrates with retry** - transient failures retried before checkpointing (Spec 185)

## Requirements

### Functional Requirements

#### FR1: Checkpoint on All Failures
- **MUST** create checkpoint when step fails (not just interrupt)
- **MUST** include failed step index and error message
- **MUST** mark whether failure is retryable
- **MUST** preserve all completed step results

#### FR2: Resume Retries Failed Step
- **MUST** retry the failed step (not skip it)
- **MUST** respect retry policy if configured
- **MUST** skip already-completed steps
- **MUST** restore variables from checkpoint

#### FR3: Worktree Preservation
- **MUST** preserve worktree on failure (don't clean up)
- **MUST** find worktree on resume
- **MUST** execute resumed steps in same worktree
- **MUST** clean up worktree only on success or explicit request

#### FR4: CLI Integration
- **MUST** support `prodigy resume <session-id>` for standard workflows
- **MUST** show clear resume progress ("Resuming from step 3/5")
- **MUST** provide helpful errors if checkpoint missing
- **MUST** support `--force` to restart from beginning

### Non-Functional Requirements

#### NFR1: Consistency with MapReduce
- Resume behavior MUST be consistent with MapReduce resume
- User experience MUST be identical regardless of workflow type

#### NFR2: Reliability
- Zero data loss on failure
- Resume MUST work after process crash
- Resume MUST work after machine restart

## Acceptance Criteria

### Checkpoint Creation

- [ ] **AC1**: Checkpoint created on Claude failure
  - Workflow with 5 steps
  - Claude command at step 3 fails with 500 error (after retries exhausted)
  - Checkpoint exists at `~/.prodigy/state/{repo}/sessions/{session}/checkpoint.json`
  - Checkpoint shows `Failed { step_index: 3, retryable: true }`

- [ ] **AC2**: Checkpoint created on shell failure
  - Shell command at step 2 exits with non-zero
  - Checkpoint created with `Failed { step_index: 2, retryable: false }`

- [ ] **AC3**: Checkpoint preserves completed work
  - Steps 0, 1 completed successfully
  - Step 2 fails
  - Checkpoint includes `completed_steps: [step_0_record, step_1_record]`

- [ ] **AC4**: Variables preserved in checkpoint
  - Step 1 captures variable `${output}`
  - Step 2 fails
  - Checkpoint includes `variables: { "output": "captured value" }`

### Resume Execution

- [ ] **AC5**: Resume retries failed step
  - Checkpoint shows `Failed { step_index: 3 }`
  - `prodigy resume session-xyz` executed
  - Step 3 is **retried** (not skipped)
  - Steps 0, 1, 2 are **skipped** (not re-executed)

- [ ] **AC6**: Resume restores variables
  - Checkpoint has `variables: { "name": "test" }`
  - Resume loads checkpoint
  - Step 3 can use `${name}` variable

- [ ] **AC7**: Resume uses same worktree
  - Workflow created worktree at `/path/to/worktree`
  - Resume finds and uses same worktree
  - No new worktree created

- [ ] **AC8**: Resume completes workflow
  - Steps 3, 4 execute successfully after resume
  - Workflow marked as completed
  - Final merge prompt shown (if configured)

### CLI Behavior

- [ ] **AC9**: Resume shows progress
  - Output: "Resuming session session-xyz"
  - Output: "Loaded checkpoint: 2/5 steps completed"
  - Output: "Retrying step 3: /claude-command"
  - Output: "Executing step 4/5..."

- [ ] **AC10**: Resume handles missing checkpoint
  - `prodigy resume session-missing`
  - Error: "No checkpoint found for session session-missing"
  - Suggests: "The workflow may have completed or checkpoint was not saved"

- [ ] **AC11**: Force restart option
  - `prodigy resume session-xyz --force`
  - Warning: "Force restart will lose 2 completed steps. Continue? [y/N]"
  - Restarts from step 0 if confirmed

### Worktree Handling

- [ ] **AC12**: Worktree preserved on failure
  - Workflow fails at step 3
  - Worktree still exists (not cleaned up)
  - Contains partial work from steps 0-2

- [ ] **AC13**: Worktree cleaned on completion
  - Resume completes successfully
  - User prompted for merge
  - Worktree cleaned up after merge (or kept if user declines)

## Technical Details

### Implementation Approach

#### 1. Resume Command Flow

```rust
/// CLI resume command handler
pub async fn run_resume_command(
    session_id: &SessionId,
    options: ResumeOptions,
) -> Result<()> {
    // 1. Load checkpoint
    let checkpoint = load_checkpoint(session_id).await?
        .ok_or_else(|| anyhow!("No checkpoint found for session {}", session_id))?;

    // 2. Find worktree
    let worktree_path = find_worktree_for_session(session_id).await?;

    // 3. Load workflow from checkpoint path
    let workflow = load_workflow(&checkpoint.workflow_path)?;

    // 4. Plan resume
    let plan = plan_resume(&checkpoint, &workflow);

    // 5. Show resume info
    println!("Resuming session {}", session_id);
    println!("Loaded checkpoint: {}/{} steps completed",
        checkpoint.completed_steps.len(),
        workflow.steps.len()
    );

    if plan.retry_current {
        println!("Retrying step {}: {}",
            plan.start_index,
            workflow.steps[plan.start_index].summary()
        );
    }

    // 6. Create execution environment
    let env = WorkflowEnv {
        session_id: session_id.clone(),
        worktree_path,
        workflow_path: checkpoint.workflow_path.clone(),
        variables: checkpoint.variables.clone(),
        completed_steps: checkpoint.completed_steps.clone(),
        checkpoint_storage: Arc::new(FileCheckpointStorage::new()),
        ..Default::default()
    };

    // 7. Execute remaining steps
    let result = execute_workflow_from(plan.start_index, workflow.steps, plan.skip_steps)
        .run(&env)
        .await?;

    // 8. Handle completion
    if result.success {
        handle_workflow_completion(&env, &workflow).await?;
    }

    Ok(())
}
```

#### 2. Checkpoint Loading

```rust
/// Load checkpoint for resume
async fn load_checkpoint(session_id: &SessionId) -> Result<Option<WorkflowCheckpoint>> {
    let storage = FileCheckpointStorage::new();

    match storage.load(session_id).await {
        Ok(Some(checkpoint)) => {
            // Validate checkpoint
            if let Err(e) = validate_checkpoint(&checkpoint) {
                warn!("Checkpoint validation failed: {}", e);
                // Try loading from history
                return try_load_from_history(&storage, session_id).await;
            }
            Ok(Some(checkpoint))
        }
        Ok(None) => Ok(None),
        Err(CheckpointError::IntegrityError { .. }) => {
            warn!("Checkpoint corrupted, trying history...");
            try_load_from_history(&storage, session_id).await
        }
        Err(e) => Err(e.into()),
    }
}

async fn try_load_from_history(
    storage: &FileCheckpointStorage,
    session_id: &SessionId,
) -> Result<Option<WorkflowCheckpoint>> {
    let history = storage.list_history(session_id).await?;

    for (index, _info) in history.iter().enumerate() {
        if let Ok(Some(checkpoint)) = storage.load_from_history(session_id, index).await {
            if validate_checkpoint(&checkpoint).is_ok() {
                warn!("Recovered checkpoint from history (index {})", index);
                return Ok(Some(checkpoint));
            }
        }
    }

    Ok(None)
}
```

#### 3. Resume Planning (Pure Function)

```rust
/// Pure function: plan resume from checkpoint
pub fn plan_resume(checkpoint: &WorkflowCheckpoint, workflow: &Workflow) -> ResumePlan {
    let completed_indices: HashSet<_> = checkpoint.completed_steps
        .iter()
        .map(|s| s.step_index)
        .collect();

    match &checkpoint.state {
        CheckpointState::BeforeStep { step_index } => {
            // Was about to execute step, execute it
            ResumePlan {
                start_index: *step_index,
                retry_current: true,
                skip_steps: completed_indices,
                variables: checkpoint.variables.clone(),
            }
        }
        CheckpointState::Completed { step_index, .. } => {
            // Step completed, continue with next
            ResumePlan {
                start_index: step_index + 1,
                retry_current: false,
                skip_steps: completed_indices,
                variables: checkpoint.variables.clone(),
            }
        }
        CheckpointState::Failed { step_index, retryable, .. } => {
            // Step failed - retry if retryable
            ResumePlan {
                start_index: *step_index,
                retry_current: *retryable,
                skip_steps: completed_indices,
                variables: checkpoint.variables.clone(),
            }
        }
        CheckpointState::Interrupted { step_index, in_progress } => {
            // Interrupted - retry if was in progress
            ResumePlan {
                start_index: if *in_progress { *step_index } else { *step_index + 1 },
                retry_current: *in_progress,
                skip_steps: completed_indices,
                variables: checkpoint.variables.clone(),
            }
        }
    }
}

/// Resume execution plan
#[derive(Debug, Clone)]
pub struct ResumePlan {
    /// First step to execute
    pub start_index: usize,
    /// Whether to retry the current step (vs skip to next)
    pub retry_current: bool,
    /// Steps to skip (already completed)
    pub skip_steps: HashSet<usize>,
    /// Variables to restore
    pub variables: HashMap<String, Value>,
}
```

#### 4. Execute from Step with Skipping

```rust
/// Execute workflow starting from specific step, skipping completed
pub fn execute_workflow_from(
    start_index: usize,
    steps: Vec<WorkflowStep>,
    skip_steps: HashSet<usize>,
) -> Effect<WorkflowResult, WorkflowError, WorkflowEnv> {
    steps.into_iter()
        .enumerate()
        .filter(|(idx, _)| *idx >= start_index && !skip_steps.contains(idx))
        .fold(
            Effect::pure(WorkflowProgress::new()),
            |acc, (idx, step)| {
                acc.and_then(move |progress| {
                    info!("Executing step {}/{}: {}", idx + 1, total_steps, step.summary());

                    with_checkpointing(idx, &step)
                        .map(|result| progress.with_step_result(idx, result))
                        .context(format!("Step {}", idx))
                })
            },
        )
        .map(|progress| progress.into_result())
}
```

#### 5. Worktree Discovery

```rust
/// Find worktree associated with session
async fn find_worktree_for_session(session_id: &SessionId) -> Result<PathBuf> {
    let prodigy_home = get_prodigy_home()?;
    let worktrees_dir = prodigy_home.join("worktrees");

    // Search through all repos for this session's worktree
    for repo_entry in read_dir(&worktrees_dir).await? {
        let repo_path = repo_entry.path();
        if !repo_path.is_dir() {
            continue;
        }

        let session_worktree = repo_path.join(session_id.as_str());
        if session_worktree.exists() {
            return Ok(session_worktree);
        }
    }

    Err(anyhow!(
        "Worktree not found for session {}. It may have been cleaned up.",
        session_id
    ))
}
```

#### 6. Worktree Preservation on Failure

```rust
/// Modified workflow completion handler - preserve on failure
async fn handle_workflow_outcome(
    env: &WorkflowEnv,
    outcome: &WorkflowOutcome,
) -> Result<()> {
    match outcome {
        WorkflowOutcome::Success => {
            // Prompt for merge, then optionally clean up
            handle_successful_completion(env).await
        }
        WorkflowOutcome::Failed { .. } => {
            // DO NOT clean up worktree
            // Checkpoint already saved by with_checkpointing
            info!(
                "Workflow failed. Worktree preserved at {}. Use 'prodigy resume {}' to continue.",
                env.worktree_path.display(),
                env.session_id
            );
            Ok(())
        }
        WorkflowOutcome::Interrupted => {
            // DO NOT clean up worktree
            info!(
                "Workflow interrupted. Use 'prodigy resume {}' to continue.",
                env.session_id
            );
            Ok(())
        }
    }
}
```

### Architecture Changes

#### Modified Components

1. **resume.rs** - Complete rewrite using unified checkpoint system
2. **workflow_execution.rs** - Use effect-based execution with checkpointing
3. **worktree_manager.rs** - Don't clean up on failure

#### Integration Points

- **Spec 183**: Effect-based step execution
- **Spec 184**: Unified checkpoint save/load
- **Spec 185**: Retry before checkpoint on Claude failures

### Execution Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    Initial Workflow Run                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   Step 0 ──> Checkpoint(BeforeStep) ──> Execute ──> Success     │
│          ──> Checkpoint(Completed) ──> Update Variables          │
│                                                                  │
│   Step 1 ──> Checkpoint(BeforeStep) ──> Execute ──> Success     │
│          ──> Checkpoint(Completed) ──> Update Variables          │
│                                                                  │
│   Step 2 ──> Checkpoint(BeforeStep) ──> Execute ──> FAILURE     │
│          ──> Retry (5x) ──> Still fails                          │
│          ──> Checkpoint(Failed{step:2, retryable:true})          │
│          ──> Preserve Worktree                                   │
│          ──> Exit with error                                     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    prodigy resume session-xyz                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   1. Load Checkpoint                                             │
│      └──> state: Failed{step:2, retryable:true}                 │
│      └──> completed_steps: [0, 1]                               │
│      └──> variables: {...}                                       │
│                                                                  │
│   2. Plan Resume                                                 │
│      └──> start_index: 2 (retry failed step)                    │
│      └──> skip_steps: {0, 1}                                    │
│                                                                  │
│   3. Find Worktree                                               │
│      └──> ~/.prodigy/worktrees/repo/session-xyz                 │
│                                                                  │
│   4. Execute from Step 2                                         │
│      └──> Skip step 0 (completed)                               │
│      └──> Skip step 1 (completed)                               │
│      └──> RETRY Step 2 ──> Success! ──> Checkpoint(Completed)   │
│      └──> Execute Step 3 ──> Success! ──> Checkpoint(Completed) │
│      └──> Execute Step 4 ──> Success! ──> Checkpoint(Completed) │
│                                                                  │
│   5. Workflow Complete                                           │
│      └──> Prompt for merge                                       │
│      └──> Clean up worktree (if merged)                         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Dependencies

### Prerequisites
- **Spec 183**: Effect-Based Workflow Execution
- **Spec 184**: Unified Checkpoint System
- **Spec 185**: Claude Command Retry

### Affected Components
- CLI resume command
- Workflow executor
- Worktree manager
- Session manager

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_plan_resume_retries_failed_step() {
    let checkpoint = WorkflowCheckpoint {
        state: CheckpointState::Failed {
            step_index: 2,
            error: "500 error".into(),
            retryable: true,
        },
        completed_steps: vec![
            CompletedStepRecord { step_index: 0, .. },
            CompletedStepRecord { step_index: 1, .. },
        ],
        ..
    };
    let workflow = create_workflow_with_steps(5);

    let plan = plan_resume(&checkpoint, &workflow);

    assert_eq!(plan.start_index, 2);
    assert!(plan.retry_current);
    assert_eq!(plan.skip_steps, HashSet::from([0, 1]));
}

#[test]
fn test_plan_resume_continues_after_completed() {
    let checkpoint = WorkflowCheckpoint {
        state: CheckpointState::Completed { step_index: 2, output: None },
        completed_steps: vec![
            CompletedStepRecord { step_index: 0, .. },
            CompletedStepRecord { step_index: 1, .. },
            CompletedStepRecord { step_index: 2, .. },
        ],
        ..
    };
    let workflow = create_workflow_with_steps(5);

    let plan = plan_resume(&checkpoint, &workflow);

    assert_eq!(plan.start_index, 3); // Continue after completed
    assert!(!plan.retry_current);
    assert_eq!(plan.skip_steps, HashSet::from([0, 1, 2]));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_resume_flow() {
    // Setup: Create workflow that fails at step 2
    let workflow = create_failing_workflow(5, 2);
    let session_id = SessionId::new();

    // First run - should fail at step 2 and create checkpoint
    let result = execute_workflow(workflow.clone())
        .run(&create_env(session_id.clone()))
        .await;

    assert!(result.is_err());

    // Verify checkpoint created
    let checkpoint = FileCheckpointStorage::new()
        .load(&session_id)
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(checkpoint.state, CheckpointState::Failed { step_index: 2, .. }));
    assert_eq!(checkpoint.completed_steps.len(), 2);

    // Now fix the failure and resume
    let fixed_workflow = create_passing_workflow(5);
    let result = resume_workflow(checkpoint, fixed_workflow)
        .run(&create_env(session_id.clone()))
        .await;

    assert!(result.is_ok());

    // Verify only steps 2-4 were executed
    let execution_log = get_execution_log(&session_id);
    assert!(!execution_log.contains(&"step-0"));
    assert!(!execution_log.contains(&"step-1"));
    assert!(execution_log.contains(&"step-2")); // Retried
    assert!(execution_log.contains(&"step-3"));
    assert!(execution_log.contains(&"step-4"));
}

#[tokio::test]
async fn test_worktree_preserved_on_failure() {
    let session_id = SessionId::new();
    let worktree_manager = WorktreeManager::new();

    // Create worktree
    let worktree_path = worktree_manager
        .create_for_session(&session_id)
        .await
        .unwrap();

    // Execute failing workflow
    let result = execute_workflow(failing_workflow)
        .run(&create_env_with_worktree(session_id.clone(), worktree_path.clone()))
        .await;

    assert!(result.is_err());

    // Worktree should still exist
    assert!(worktree_path.exists());
}

#[tokio::test]
async fn test_resume_uses_same_worktree() {
    let session_id = SessionId::new();

    // First run
    execute_workflow(failing_workflow).run(&env).await;

    // Get worktree path from first run
    let original_worktree = find_worktree_for_session(&session_id).await.unwrap();

    // Resume
    let checkpoint = load_checkpoint(&session_id).await.unwrap().unwrap();
    let resume_env = create_env_for_resume(&session_id, &checkpoint).await;

    // Verify same worktree
    assert_eq!(resume_env.worktree_path, original_worktree);
}
```

### End-to-End Tests

```rust
#[tokio::test]
async fn test_e2e_resume_cli() {
    // Create workflow file
    let workflow_path = create_temp_workflow_file(r#"
        name: test-resume
        commands:
          - shell: "echo step1"
          - shell: "exit 1"  # Will fail
          - shell: "echo step3"
    "#);

    // First run
    let output = Command::new("prodigy")
        .args(["run", workflow_path.to_str().unwrap()])
        .output()
        .await
        .unwrap();

    assert!(!output.status.success());

    // Extract session ID from output
    let session_id = extract_session_id(&output.stderr);

    // Fix workflow and resume
    update_workflow_file(&workflow_path, r#"
        name: test-resume
        commands:
          - shell: "echo step1"
          - shell: "echo step2-fixed"
          - shell: "echo step3"
    "#);

    let output = Command::new("prodigy")
        .args(["resume", &session_id])
        .output()
        .await
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("step2-fixed"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("step3"));
    // step1 should NOT be in output (skipped)
    assert!(!String::from_utf8_lossy(&output.stdout).contains("step1"));
}
```

## Documentation Requirements

### Code Documentation
- Document resume command options
- Document checkpoint state transitions
- Document worktree preservation behavior

### User Documentation

**Add to docs/workflows/resume.md**:

```markdown
# Resuming Workflows

When a workflow fails, Prodigy automatically saves a checkpoint with your progress.
You can resume from where you left off:

## Resuming a Failed Workflow

\`\`\`bash
# Find your session ID
prodigy sessions list

# Resume the workflow
prodigy resume session-abc123
\`\`\`

## What Gets Resumed

- **Completed steps are skipped** - no work is repeated
- **Failed step is retried** - the command that failed runs again
- **Variables are restored** - captured outputs available to remaining steps
- **Same worktree is used** - your partial work is preserved

## Resume Options

\`\`\`bash
# Force restart from beginning (lose progress)
prodigy resume session-abc123 --force

# Resume with verbose output
prodigy resume session-abc123 -v
\`\`\`

## Troubleshooting

**"No checkpoint found"**: The workflow may have completed successfully,
or the checkpoint was not saved (bug). Try running the workflow again.

**"Worktree not found"**: The worktree was cleaned up. Use --force to
restart from the beginning.
\`\`\`
```

## Migration and Compatibility

### Breaking Changes
None - existing workflows continue to work. Resume is fixed, not changed.

### Migration
- Old sessions without checkpoints: Error message explains situation
- New sessions: Automatic checkpointing enabled

## Success Metrics

### Quantitative
- 95%+ resume success rate for failed workflows
- 0 data loss (completed steps preserved)
- Resume finds checkpoint within 100ms

### Qualitative
- Users can confidently interrupt/resume workflows
- Clear progress indication during resume
- Consistent experience with MapReduce resume
