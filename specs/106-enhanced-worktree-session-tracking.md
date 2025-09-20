---
number: 106
title: Enhanced Worktree Session Tracking Display
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-09-20
---

# Specification 106: Enhanced Worktree Session Tracking Display

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current `prodigy worktree ls` command displays minimal information about active sessions, showing only the session ID, status, and a non-functional progress indicator (0/10). This output lacks critical information that users need to manage their sessions effectively:

- When the session was last active
- What workflow is being executed
- What arguments were passed to the workflow
- Actual progress through the workflow

The current output format shows:
```
🔄 session-08af4aa9-0e37-4d06-9022-c687cd2ba8db - InProgress (0/10)
```

This provides insufficient context for users to identify and manage their sessions, especially when multiple sessions are running or interrupted.

## Objective

Enhance the `prodigy worktree ls` command to display comprehensive session information that enables users to:
1. Quickly identify what each session is doing
2. Understand when sessions were last active
3. See actual workflow progress
4. Identify session arguments for reproducibility
5. Make informed decisions about session management

## Requirements

### Functional Requirements

1. **Display Workflow Information**
   - Show the workflow file path or name being executed
   - Display any arguments passed to the workflow
   - Show workflow mode (standard, mapreduce, etc.)

2. **Temporal Information**
   - Display last activity timestamp for each session
   - Show session start time
   - Calculate and display session duration or idle time

3. **Progress Tracking**
   - Show actual progress through workflow steps
   - Display current/total steps or iterations
   - For MapReduce jobs, show items processed/total items

4. **Enhanced Status Display**
   - Maintain existing status indicators (InProgress, Interrupted, Failed, Completed)
   - Add sub-status for more detail (e.g., "Waiting for user input", "Running tests")
   - Show error summary for failed sessions

5. **Session Metadata**
   - Display session branch name
   - Show parent branch for merge context
   - Include worktree path if relevant

### Non-Functional Requirements

1. **Performance**
   - Command should execute in under 500ms for up to 100 sessions
   - Efficiently read session state without blocking
   - Cache session data where appropriate

2. **Usability**
   - Output should be scannable and readable
   - Important information should be prominent
   - Support for both verbose and compact display modes

3. **Compatibility**
   - Maintain backward compatibility with existing scripts
   - Support JSON output format for programmatic access
   - Work with both local and global storage backends

## Acceptance Criteria

- [ ] `prodigy worktree ls` displays workflow name/path for each session
- [ ] Command arguments are shown when present
- [ ] Last activity timestamp is displayed in human-readable format
- [ ] Actual progress (current/total steps) replaces hardcoded (0/10)
- [ ] Sessions are sorted by last activity time (most recent first)
- [ ] Failed sessions show error summary
- [ ] Interrupted sessions show when they were interrupted
- [ ] Long-running sessions show duration
- [ ] Output supports `--json` flag for machine-readable format
- [ ] Performance meets sub-500ms requirement for typical usage
- [ ] Documentation is updated with new output format
- [ ] Tests cover all new display functionality

## Technical Details

### Implementation Approach

1. **Session State Reading**
   - Read session state from `.prodigy/session_state.json` in each worktree
   - Parse workflow state to extract progress information
   - Read workflow file to get workflow metadata

2. **Data Structure Enhancement**
   ```rust
   pub struct EnhancedSessionInfo {
       pub session_id: String,
       pub status: WorktreeStatus,
       pub workflow_path: PathBuf,
       pub workflow_args: Vec<String>,
       pub started_at: DateTime<Utc>,
       pub last_activity: DateTime<Utc>,
       pub current_step: usize,
       pub total_steps: Option<usize>,
       pub error_summary: Option<String>,
       pub branch_name: String,
       pub parent_branch: Option<String>,
   }
   ```

3. **Display Format**
   ```
   📂 workflow.yaml (--arg1 value1 --arg2)
     └─ session-abc123 [feature-branch → main]
        Status: InProgress (step 3/10) • Started: 2h ago • Last active: 5m ago

   📂 mapreduce-job.yaml
     └─ session-def456 [hotfix-branch]
        Status: Interrupted (processed 45/100 items) • Started: 1d ago • Interrupted: 3h ago

   📂 test-workflow.yaml (--dry-run)
     └─ session-ghi789 [test-branch → develop]
        Status: Failed (step 2/5) • Started: 30m ago • Failed: 10m ago
        Error: "Command 'cargo test' failed with exit code 1"
   ```

### Architecture Changes

1. **Enhanced WorktreeManager**
   - Add method to read detailed session state
   - Implement session data aggregation
   - Add caching layer for performance

2. **New Display Module**
   - Create `worktree::display` module for formatting
   - Support multiple output formats (default, verbose, json)
   - Implement smart truncation for long values

### Data Structures

```rust
// In worktree module
pub struct DetailedWorktreeList {
    pub sessions: Vec<EnhancedSessionInfo>,
    pub summary: WorktreeSummary,
}

pub struct WorktreeSummary {
    pub total: usize,
    pub in_progress: usize,
    pub interrupted: usize,
    pub failed: usize,
    pub completed: usize,
}
```

### APIs and Interfaces

```rust
// Enhanced WorktreeManager methods
impl WorktreeManager {
    pub async fn list_detailed(&self) -> Result<DetailedWorktreeList>;
    pub async fn get_session_details(&self, session_id: &str) -> Result<EnhancedSessionInfo>;
}

// Display formatting
pub trait SessionDisplay {
    fn format_default(&self) -> String;
    fn format_verbose(&self) -> String;
    fn format_json(&self) -> Value;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `worktree` module
  - `cli::worktree` command handler
  - Session state serialization
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test session info parsing
  - Test display formatting
  - Test time calculations
  - Test progress extraction

- **Integration Tests**:
  - Test with multiple active sessions
  - Test with various workflow types
  - Test performance with many sessions
  - Test JSON output format

- **Performance Tests**:
  - Benchmark with 100+ sessions
  - Test caching effectiveness
  - Measure display rendering time

- **User Acceptance**:
  - Verify readability of output
  - Test with real workflow scenarios
  - Validate information completeness

## Documentation Requirements

- **Code Documentation**:
  - Document new data structures
  - Add examples for display formats
  - Document performance considerations

- **User Documentation**:
  - Update `prodigy worktree ls` help text
  - Add examples to README
  - Document JSON output schema

- **Architecture Updates**:
  - Update ARCHITECTURE.md with session tracking details
  - Document caching strategy

## Implementation Notes

1. **Performance Optimization**
   - Consider lazy loading of session details
   - Implement parallel session state reading
   - Cache workflow metadata

2. **Error Handling**
   - Gracefully handle corrupted session state
   - Show partial information when available
   - Provide clear error messages

3. **Backward Compatibility**
   - Support reading old session state format
   - Provide migration path for existing sessions
   - Maintain existing CLI flags

## Migration and Compatibility

- No breaking changes to existing command interface
- Old session states will show reduced information
- New fields will be populated for new sessions
- JSON output is additive (new fields won't break parsers)