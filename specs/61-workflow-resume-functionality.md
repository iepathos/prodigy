---
number: 61
title: Workflow Resume Functionality
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-09-08
---

# Specification 61: Workflow Resume Functionality

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current workflow resume functionality (`--resume` flag) is completely non-functional despite having the CLI interface defined. When users run long-running workflows that get interrupted (due to errors, user cancellation, or system issues), they have no way to resume from where they left off. This forces them to restart workflows from the beginning, wasting time and computational resources.

### Current Issues

1. **Unimplemented Resume Logic**: The `--resume` flag is parsed but never processed in the execution flow
2. **Session ID Visibility**: Session IDs required for resuming are never displayed to users
3. **ID Mismatch**: Session IDs (`cook-{timestamp}`) don't match worktree IDs (`session-{uuid}`)
4. **No Workflow State Persistence**: Current step and iteration position aren't saved
5. **No Environment Restoration**: Can't rebuild execution context from saved state
6. **Poor User Experience**: Users must manually dig through `.prodigy/session_state.json` to find session IDs

### User Impact

When a workflow is interrupted:
- Users see output like: `Processing input 1/1: 97` and `Executing step 1/3: claude: /prodigy-implement-spec $ARG`
- The worktree path shows: `session-be3527bb-981b-4153-aae7-17a77f5c8273`
- But the actual session ID needed for resume is: `cook-1757231340` (hidden in state file)
- Running `prodigy cook workflows/implement.yml --resume cook-1757231340` starts from the beginning instead of resuming

## Objective

Implement a fully functional workflow resume capability that allows users to continue interrupted workflows from exactly where they left off, with clear session identification and seamless state restoration.

## Requirements

### Functional Requirements

1. **Session Display**
   - Display session ID prominently when workflow starts
   - Show session ID in interruption messages
   - Provide clear resume instructions when workflow is interrupted

2. **State Persistence**
   - Save current workflow step after each successful command
   - Track iteration progress within workflows
   - Persist command-level progress for long-running commands
   - Store environment context (working directory, variables, arguments)

3. **Resume Capability**
   - Load saved session state when `--resume` flag is provided
   - Validate session is in resumable state (InProgress/Interrupted)
   - Skip already completed steps and iterations
   - Restore execution environment (directory, worktree, variables)
   - Continue from exact interruption point

4. **Session Management**
   - List resumable sessions with `prodigy sessions ls`
   - Show session details with `prodigy sessions show <id>`
   - Clean up stale sessions with `prodigy sessions clean`
   - Auto-detect last interrupted session for convenience

5. **Worktree Integration**
   - Reconnect to existing worktrees when resuming
   - Preserve worktree state across resume operations
   - Handle cases where worktree was manually deleted

### Non-Functional Requirements

1. **Reliability**
   - Atomic state updates to prevent corruption
   - Graceful handling of concurrent session access
   - Recovery from partially corrupted state files

2. **Performance**
   - Minimal overhead for state persistence
   - Fast session discovery and loading
   - Efficient state serialization

3. **Usability**
   - Clear, actionable error messages
   - Intuitive session identification
   - Helpful prompts and suggestions

## Acceptance Criteria

- [ ] Session ID is displayed when workflow starts: `🔄 Starting session: cook-1234567890`
- [ ] Interruption shows resume command: `Session interrupted. Resume with: prodigy cook <workflow> --resume cook-1234567890`
- [ ] `--resume` flag actually resumes from last successful step
- [ ] Workflow state includes current step index and iteration number
- [ ] Session state is saved after each successful command execution
- [ ] Resume validates session exists and is in resumable state
- [ ] `prodigy sessions ls` shows all resumable sessions with status
- [ ] `prodigy sessions show <id>` displays detailed session information
- [ ] Resume works correctly with worktree-based execution
- [ ] Documentation clearly explains resume functionality
- [ ] Integration tests cover resume scenarios

## Technical Details

### Implementation Approach

1. **Enhanced Session State Structure**
```rust
pub struct SessionState {
    pub session_id: String,
    pub status: SessionStatus,
    pub workflow_path: PathBuf,
    pub workflow_state: WorkflowState,  // NEW
    pub environment: ExecutionEnvironment,  // NEW
    pub started_at: DateTime<Utc>,
    pub last_checkpoint: DateTime<Utc>,  // NEW
    // ... existing fields
}

pub struct WorkflowState {
    pub current_iteration: usize,
    pub current_step: usize,
    pub completed_steps: Vec<StepResult>,
    pub workflow_config: WorkflowConfig,
    pub input_context: InputContext,  // Arguments, patterns, etc.
}

pub struct ExecutionEnvironment {
    pub working_directory: PathBuf,
    pub worktree_name: Option<String>,
    pub environment_vars: HashMap<String, String>,
    pub command_args: Vec<String>,
}
```

2. **Resume Flow in Orchestrator**
```rust
// In cook() function
if let Some(session_id) = config.command.resume {
    return resume_workflow(session_id, config).await;
}

async fn resume_workflow(session_id: String, config: Config) -> Result<()> {
    // 1. Load session state
    let state = SessionManager::load_session(&session_id)?;
    
    // 2. Validate resumable
    if !state.is_resumable() {
        return Err(anyhow!("Session {} is not resumable (status: {:?})", 
                           session_id, state.status));
    }
    
    // 3. Restore environment
    restore_environment(&state.environment)?;
    
    // 4. Resume workflow execution
    let orchestrator = Orchestrator::from_state(state);
    orchestrator.resume_execution().await
}
```

3. **Checkpoint Management**
```rust
// After each successful step
async fn checkpoint_progress(&mut self) -> Result<()> {
    self.state.workflow_state.current_step += 1;
    self.state.workflow_state.completed_steps.push(step_result);
    self.state.last_checkpoint = Utc::now();
    self.session_manager.save_checkpoint(&self.state).await?;
    Ok(())
}
```

### Architecture Changes

1. **Session Manager Enhancement**
   - Add `load_session()` method that reads from disk
   - Implement `save_checkpoint()` for incremental updates
   - Add session discovery methods for listing/querying

2. **Orchestrator Modifications**
   - Add `from_state()` constructor for resume
   - Implement `resume_execution()` method
   - Integrate checkpoint saving into execution flow

3. **CLI Additions**
   - New `sessions` subcommand with `ls`, `show`, `clean` operations
   - Enhanced cook command to process `--resume` flag
   - Auto-suggestion of last interrupted session

### Data Structures

1. **Session State File** (`.prodigy/session_state.json`)
   - Add `workflow_state` object with step tracking
   - Add `environment` object with execution context
   - Add `last_checkpoint` timestamp

2. **Worktree State** (`.prodigy/worktrees/{name}/state.json`)
   - Link to parent session ID
   - Track worktree-specific state

### APIs and Interfaces

1. **SessionManager Trait Extension**
```rust
pub trait SessionManager {
    async fn load_session(&self, session_id: &str) -> Result<SessionState>;
    async fn save_checkpoint(&self, state: &SessionState) -> Result<()>;
    async fn list_resumable(&self) -> Result<Vec<SessionInfo>>;
    async fn get_last_interrupted(&self) -> Result<Option<String>>;
}
```

2. **CLI Interface**
```bash
# Resume with explicit session ID
prodigy cook workflow.yml --resume cook-1234567890

# Resume last interrupted session
prodigy cook workflow.yml --resume-last

# Session management
prodigy sessions ls              # List all sessions
prodigy sessions show <id>       # Show session details
prodigy sessions clean [--all]   # Clean up old sessions
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/orchestrator.rs` - Main execution logic
  - `src/session/` - Session management
  - `src/main.rs` - CLI command processing
  - `src/cook/command.rs` - Command structure
- **External Dependencies**: None

## Testing Strategy

### Unit Tests
- Session state serialization/deserialization
- Checkpoint saving and loading
- Resume validation logic
- State corruption recovery

### Integration Tests
- Complete workflow interruption and resume
- Multi-iteration workflow resume
- Worktree-based session resume
- Concurrent session handling
- Edge cases (deleted worktrees, corrupted state)

### User Acceptance Tests
1. Start a long-running workflow
2. Interrupt with Ctrl+C
3. Note displayed session ID
4. Resume with `--resume` flag
5. Verify execution continues from interruption point

## Documentation Requirements

### Code Documentation
- Document all new session state fields
- Explain checkpoint strategy in orchestrator
- Add examples to CLI help text

### User Documentation
- Add "Resuming Interrupted Workflows" section to README
- Create troubleshooting guide for resume issues
- Document session management commands
- Provide examples of common resume scenarios

### Architecture Updates
- Update ARCHITECTURE.md with session persistence design
- Document checkpoint strategy and frequency
- Explain session ID generation and lifecycle

## Implementation Notes

1. **Session ID Format**: Consider using more readable format like `cook-2025-09-08-1234` instead of pure timestamp
2. **Checkpoint Frequency**: Save after each successful command, not during command execution
3. **State File Locking**: Use file locking to prevent concurrent modifications
4. **Backward Compatibility**: Handle old session files without workflow_state gracefully
5. **Auto-cleanup**: Consider auto-removing sessions older than 7 days
6. **Progress Display**: Show "Resuming from step X of Y" when resuming

## Migration and Compatibility

### Breaking Changes
- None - this is a new feature

### Migration Path
1. Existing session files without workflow_state will be treated as non-resumable
2. Clear error message will guide users to start fresh workflows
3. Old session files can be manually cleaned up

### Compatibility Considerations
- Maintain backward compatibility with existing workflow files
- Ensure MapReduce jobs (which have their own resume) aren't affected
- Support both timestamped and UUID-based session identification