# Specification 30: Interrupted Worktree Recovery and State Tracking

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: 24, 29

## Context

Currently, when MMM has Claude working on a worktree and the process gets interrupted (via Ctrl-C, kill signal, or unexpected termination), the state tracking doesn't capture this interruption. This leads to several issues:

1. We don't know which worktrees were abandoned due to interruption
2. We can't distinguish between completed, failed, and interrupted sessions
3. We have no ability to resume work on an interrupted worktree
4. Users may accumulate many abandoned worktrees without knowing their status

The existing worktree state management (Spec 29) provides a foundation for tracking worktree metadata, but doesn't handle process interruptions or provide recovery mechanisms.

## Objective

Enhance MMM's worktree state tracking to detect and record when Claude's work on a worktree is interrupted, and provide mechanisms to resume or clean up interrupted sessions.

## Requirements

### Functional Requirements
- Detect when an MMM improve session is interrupted (SIGINT, SIGTERM, process kill)
- Update worktree state to reflect interruption status
- Track partial progress (last successful iteration, files modified)
- Provide a resume mechanism to continue from interruption point
- Show interruption details in worktree list command
- Add cleanup options specific to interrupted sessions

### Non-Functional Requirements
- Minimal performance overhead for interrupt detection
- Reliable state persistence even during abrupt termination
- Clear distinction between failed and interrupted states
- Graceful handling of corrupted state files

## Acceptance Criteria

- [ ] Process interruptions are detected and recorded in worktree state
- [ ] Worktree state shows "interrupted" status with timestamp
- [ ] `mmm worktree list` displays interrupted sessions with special marker
- [ ] `mmm improve --resume <session-id>` continues from last successful iteration
- [ ] Interrupted sessions preserve iteration count and partial progress
- [ ] Signal handlers properly update state before process termination
- [ ] State files are written atomically to prevent corruption
- [ ] Documentation includes recovery workflow examples

## Technical Details

### Implementation Approach

1. **Signal Handler Registration**
   - Install signal handlers for SIGINT (Ctrl-C) and SIGTERM
   - Update worktree state to "interrupted" before exit
   - Use atomic file operations for state persistence

2. **State Enhancement**
   - Add "interrupted" to WorktreeStatus enum
   - Track interruption metadata (timestamp, signal type, last checkpoint)
   - Record partial iteration progress

3. **Resume Functionality**
   - Add `--resume <session-id>` flag to improve command
   - Load interrupted session state and worktree
   - Continue from last successful iteration
   - Preserve original focus and configuration

4. **Periodic Checkpointing**
   - Update state after each successful Claude command
   - Record checkpoint metadata for recovery
   - Minimize data loss on abrupt termination

### Architecture Changes

1. **Update WorktreeStatus Enum**
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    InProgress,
    Completed,
    Failed,
    Abandoned,
    Interrupted,  // New status
}
```

2. **Enhanced WorktreeState**
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorktreeState {
    // ... existing fields ...
    pub interrupted_at: Option<DateTime<Utc>>,
    pub interruption_type: Option<InterruptionType>,
    pub last_checkpoint: Option<Checkpoint>,
    pub resumable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum InterruptionType {
    UserInterrupt,    // SIGINT (Ctrl-C)
    Termination,      // SIGTERM
    ProcessKill,      // SIGKILL or unexpected exit
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Checkpoint {
    pub iteration: u32,
    pub timestamp: DateTime<Utc>,
    pub last_command: String,
    pub last_command_type: CommandType,
    pub last_spec_id: Option<String>,
    pub files_modified: Vec<String>,
    pub command_output: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    CodeReview,
    ImplementSpec,
    Lint,
    Custom(String),
}
```

3. **Signal Handler Implementation**
```rust
// src/improve/mod.rs
use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};

fn setup_interrupt_handlers(worktree_manager: Arc<WorktreeManager>, session_name: String) -> Result<()> {
    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
    
    thread::spawn(move || {
        for sig in signals.forever() {
            match sig {
                SIGINT => {
                    update_interrupted_state(&worktree_manager, &session_name, InterruptionType::UserInterrupt);
                    std::process::exit(130); // Standard exit code for SIGINT
                }
                SIGTERM => {
                    update_interrupted_state(&worktree_manager, &session_name, InterruptionType::Termination);
                    std::process::exit(143); // Standard exit code for SIGTERM
                }
                _ => unreachable!(),
            }
        }
    });
    
    Ok(())
}

fn update_interrupted_state(
    worktree_manager: &WorktreeManager,
    session_name: &str,
    interruption_type: InterruptionType,
) {
    let _ = worktree_manager.update_session_state(session_name, |state| {
        state.status = WorktreeStatus::Interrupted;
        state.interrupted_at = Some(Utc::now());
        state.interruption_type = Some(interruption_type);
        state.resumable = true;
    });
}
```

4. **Checkpoint During Execution**
```rust
async fn run_improvement_loop(
    cmd: command::ImproveCommand,
    session: &WorktreeSession,
    worktree_manager: &WorktreeManager,
) -> Result<()> {
    let mut iteration = 1;
    
    while iteration <= cmd.max_iterations {
        // Track which command we're about to execute
        let command_type = determine_command_type(&workflow_step);
        
        // Before each Claude command
        worktree_manager.create_checkpoint(&session.name, Checkpoint {
            iteration,
            timestamp: Utc::now(),
            last_command: workflow_step.to_string(),
            last_command_type: command_type.clone(),
            last_spec_id: None, // Will be updated if applicable
            files_modified: vec![],
            command_output: None,
        })?;
        
        // Execute Claude command
        let result = execute_claude_command(...).await?;
        
        // Update checkpoint after success with results
        worktree_manager.update_checkpoint(&session.name, |checkpoint| {
            checkpoint.command_output = Some(result.output);
            if command_type == CommandType::CodeReview {
                checkpoint.last_spec_id = extract_spec_id(&result);
            }
            checkpoint.files_modified = detect_modified_files();
        })?;
        
        iteration += 1;
    }
    
    Ok(())
}

fn determine_command_type(command: &str) -> CommandType {
    if command.contains("mmm-code-review") {
        CommandType::CodeReview
    } else if command.contains("mmm-implement-spec") {
        CommandType::ImplementSpec
    } else if command.contains("mmm-lint") {
        CommandType::Lint
    } else {
        CommandType::Custom(command.to_string())
    }
}
```

5. **Resume Command Implementation**
```rust
// src/main.rs
#[derive(Parser)]
struct ImproveArgs {
    // ... existing fields ...
    
    /// Resume an interrupted session
    #[arg(long, value_name = "SESSION_ID")]
    resume: Option<String>,
}

// src/improve/mod.rs
pub async fn resume_session(session_id: &str, cmd: ImproveCommand) -> Result<()> {
    let worktree_manager = WorktreeManager::new(std::env::current_dir()?)?;
    
    // Load interrupted session state
    let state = worktree_manager.load_session_state(session_id)?;
    
    if state.status != WorktreeStatus::Interrupted || !state.resumable {
        return Err(anyhow!("Session {} is not resumable", session_id));
    }
    
    // Restore worktree session
    let session = worktree_manager.restore_session(session_id)?;
    
    // Determine where to resume from
    let (start_iteration, resume_point) = match &state.last_checkpoint {
        Some(checkpoint) => {
            println!("Last checkpoint: {} command at iteration {}", 
                     checkpoint.last_command_type, checkpoint.iteration);
            
            // Determine if we need to retry the last command or move to next
            match checkpoint.last_command_type {
                CommandType::CodeReview => {
                    if checkpoint.last_spec_id.is_some() {
                        // Review completed successfully, continue with implement
                        (checkpoint.iteration, ResumePoint::NextCommand)
                    } else {
                        // Review didn't complete, retry it
                        (checkpoint.iteration, ResumePoint::RetryCommand)
                    }
                }
                CommandType::ImplementSpec => {
                    // Check if implementation was completed
                    if checkpoint.command_output.is_some() {
                        (checkpoint.iteration, ResumePoint::NextCommand)
                    } else {
                        (checkpoint.iteration, ResumePoint::RetryCommand)
                    }
                }
                CommandType::Lint => {
                    // Lint is usually quick, just retry
                    (checkpoint.iteration, ResumePoint::RetryCommand)
                }
                CommandType::Custom(_) => {
                    // For custom commands, be conservative and retry
                    (checkpoint.iteration, ResumePoint::RetryCommand)
                }
            }
        }
        None => (1, ResumePoint::FromStart),
    };
    
    println!("Resuming session {} from iteration {} ({})", 
             session_id, start_iteration, resume_point);
    
    // Continue improvement loop with resume context
    run_improvement_loop_from(cmd, session, worktree_manager, start_iteration, resume_point).await
}

#[derive(Debug)]
enum ResumePoint {
    FromStart,
    RetryCommand,
    NextCommand,
}
```

### APIs and Interfaces

```rust
impl WorktreeManager {
    pub fn create_checkpoint(&self, session_name: &str, checkpoint: Checkpoint) -> Result<()>;
    pub fn update_checkpoint<F>(&self, session_name: &str, updater: F) -> Result<()>
        where F: FnOnce(&mut Checkpoint);
    pub fn load_session_state(&self, session_id: &str) -> Result<WorktreeState>;
    pub fn restore_session(&self, session_id: &str) -> Result<WorktreeSession>;
    pub fn list_interrupted_sessions(&self) -> Result<Vec<WorktreeState>>;
    pub fn mark_session_abandoned(&self, session_id: &str) -> Result<()>;
    pub fn get_last_successful_command(&self, session_id: &str) -> Result<Option<(String, CommandType)>>;
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 24: Git worktree isolation
  - Spec 29: Centralized worktree state management
- **Affected Components**:
  - `worktree/state.rs`: Enhanced state structures
  - `worktree/manager.rs`: Checkpoint and recovery logic
  - `improve/mod.rs`: Signal handling and checkpointing
  - `main.rs`: Resume command support
- **External Dependencies**:
  - `signal-hook` crate for signal handling
  - `atomicwrites` or similar for atomic file operations

## Testing Strategy

- **Unit Tests**:
  - Signal handler registration and cleanup
  - State update on interruption
  - Checkpoint creation and restoration
  - Atomic file write operations
- **Integration Tests**:
  - Full interruption and resume cycle
  - Multiple checkpoint recovery
  - Concurrent session interruption handling
- **Performance Tests**:
  - Checkpoint overhead measurement
  - State persistence performance
- **User Acceptance**:
  - Clear interruption status in list output
  - Smooth resume experience
  - No data loss on interruption

## Documentation Requirements

- **Code Documentation**:
  - Signal handling implementation details
  - Checkpoint format and recovery process
  - State transition diagrams
- **User Documentation**:
  - How to resume interrupted sessions
  - Understanding session states
  - Best practices for long-running improvements
- **Architecture Updates**:
  - Add interrupt handling to data flow
  - Document state persistence guarantees

## Implementation Notes

1. **Atomic Writes**: Use temporary files and atomic rename for state updates
2. **Signal Safety**: Minimize work in signal handlers, use atomic flags
3. **Recovery Validation**: Verify worktree and branch still exist before resume
4. **Partial State**: Handle cases where checkpoint is incomplete
5. **User Communication**: Clear messages about what will be resumed
6. **Command Tracking**: Store full command string and type for accurate resume
7. **Workflow State**: For configurable workflows, track position in workflow sequence
8. **Spec ID Preservation**: Maintain spec IDs between review and implement phases

## Migration and Compatibility

- Existing worktrees without checkpoint data are marked non-resumable
- Interrupted status is inferred from incomplete sessions on first run
- State schema version bump to handle new fields
- Graceful handling of older state files without new fields