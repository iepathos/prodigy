---
number: 135
title: Comprehensive Storage Cleanup
category: storage
priority: high
status: draft
dependencies: []
created: 2025-10-16
---

# Specification 135: Comprehensive Storage Cleanup

**Category**: storage
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently tracks extensive workflow execution data in `~/.prodigy/`, including worktrees, sessions, logs, state, events, and DLQ data. Over time, this storage can accumulate significantly:

- **Worktrees**: 59GB - Git worktrees for isolated execution
- **Sessions**: 177MB - Session state and checkpoints
- **Logs**: 31MB - Claude execution logs
- **State**: 24MB - MapReduce job state and checkpoints
- **Events**: Minimal - Event logs for debugging
- **DLQ**: Minimal - Dead letter queue data

Currently, only `prodigy worktree clean` exists to clean up worktrees. There's no comprehensive cleanup command to manage the accumulation of sessions, logs, state, events, and DLQ data. Users need a way to reclaim disk space and manage storage lifecycle across all Prodigy data types.

## Objective

Implement a comprehensive `prodigy clean` command that provides granular control over cleaning all types of Prodigy storage, including worktrees, sessions, logs, state, events, and DLQ data, with safety features and intelligent defaults.

## Requirements

### Functional Requirements

1. **Unified Clean Command**
   - Create new `prodigy clean` command as the primary cleanup interface
   - Support cleaning all storage types from a single command
   - Provide granular flags for selective cleanup
   - Maintain backward compatibility with `prodigy worktree clean`

2. **Storage Type Targeting**
   - Clean worktrees (session and MapReduce)
   - Clean session state and checkpoints
   - Clean Claude execution logs
   - Clean MapReduce job state
   - Clean event logs
   - Clean DLQ data
   - Support cleaning individual or multiple types

3. **Age-Based Filtering**
   - Support `--older-than` flag for all storage types
   - Parse duration strings (e.g., "1h", "24h", "7d", "30d")
   - Apply age-based filtering per storage type
   - Use modification time for age comparison

4. **Safe Cleanup Logic**
   - Never delete active/running sessions
   - Never delete state for resumable workflows
   - Preserve recent data by default (e.g., last 7 days)
   - Warn before destructive operations
   - Support `--dry-run` for all operations

5. **Repository Scoping**
   - Clean storage for current repository by default
   - Support `--all-repos` flag to clean across all repositories
   - Maintain repository isolation during cleanup
   - Display repository name in output

6. **Storage Statistics**
   - Display storage usage before cleanup
   - Show what will be cleaned in dry-run mode
   - Report space reclaimed after cleanup
   - Provide per-type storage breakdown

### Non-Functional Requirements

1. **Safety**
   - Require explicit confirmation for destructive operations
   - Provide `--force` flag to skip confirmations
   - Never delete data needed for recovery
   - Log all cleanup operations for audit

2. **Performance**
   - Handle large directories efficiently
   - Stream file operations to avoid memory issues
   - Parallelize cleanup where safe
   - Provide progress indication for long operations

3. **Usability**
   - Clear, actionable output messages
   - Helpful defaults for common use cases
   - Consistent flag naming across commands
   - Comprehensive help documentation

4. **Reliability**
   - Handle partial failures gracefully
   - Continue cleanup if individual items fail
   - Report all errors with context
   - Support resumable cleanup operations

## Acceptance Criteria

- [ ] `prodigy clean --all --dry-run` displays all storage to be cleaned without modifications
- [ ] `prodigy clean --worktrees --older-than 7d` removes only old worktrees
- [ ] `prodigy clean --sessions --older-than 30d` removes old session state and checkpoints
- [ ] `prodigy clean --logs --older-than 24h` removes old Claude execution logs
- [ ] `prodigy clean --state --older-than 7d` removes old MapReduce job state
- [ ] `prodigy clean --events --older-than 14d` removes old event logs
- [ ] `prodigy clean --dlq --older-than 30d` removes old DLQ data
- [ ] `prodigy clean --all --older-than 30d --all-repos` cleans across all repositories
- [ ] Active sessions are never deleted regardless of age
- [ ] Resumable workflows are protected from cleanup
- [ ] Dry-run mode shows storage usage and reclaim estimate
- [ ] Force flag skips all confirmations
- [ ] Progress indication for long-running operations
- [ ] Storage statistics displayed before and after cleanup
- [ ] Error messages include context and recovery suggestions
- [ ] All cleanup operations are logged
- [ ] Backward compatible: `prodigy worktree clean` continues to work

## Technical Details

### Implementation Approach

1. **Command Structure**
```rust
pub enum CleanCommands {
    /// Clean all storage types
    All {
        #[arg(long)]
        older_than: Option<String>,
        #[arg(long)]
        all_repos: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        force: bool,
    },
    /// Clean worktrees only
    Worktrees {
        #[arg(long)]
        older_than: Option<String>,
        #[arg(long)]
        mapreduce: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        force: bool,
    },
    /// Clean session state
    Sessions {
        #[arg(long)]
        older_than: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        force: bool,
    },
    /// Clean Claude logs
    Logs {
        #[arg(long)]
        older_than: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        force: bool,
    },
    /// Clean MapReduce state
    State {
        #[arg(long)]
        older_than: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        force: bool,
    },
    /// Clean event logs
    Events {
        #[arg(long)]
        older_than: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        force: bool,
    },
    /// Clean DLQ data
    Dlq {
        #[arg(long)]
        older_than: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        force: bool,
    },
}
```

2. **Storage Cleanup Manager**
```rust
pub struct StorageCleanupManager {
    storage: GlobalStorage,
    repo_name: String,
}

impl StorageCleanupManager {
    pub async fn clean_worktrees(&self, config: CleanupConfig) -> Result<CleanupStats>;
    pub async fn clean_sessions(&self, config: CleanupConfig) -> Result<CleanupStats>;
    pub async fn clean_logs(&self, config: CleanupConfig) -> Result<CleanupStats>;
    pub async fn clean_state(&self, config: CleanupConfig) -> Result<CleanupStats>;
    pub async fn clean_events(&self, config: CleanupConfig) -> Result<CleanupStats>;
    pub async fn clean_dlq(&self, config: CleanupConfig) -> Result<CleanupStats>;

    pub async fn get_storage_stats(&self) -> Result<StorageStats>;
    pub async fn is_session_active(&self, session_id: &str) -> Result<bool>;
    pub async fn is_session_resumable(&self, session_id: &str) -> Result<bool>;
}

pub struct CleanupConfig {
    pub older_than: Option<Duration>,
    pub dry_run: bool,
    pub force: bool,
}

pub struct CleanupStats {
    pub items_scanned: usize,
    pub items_removed: usize,
    pub bytes_reclaimed: u64,
    pub errors: Vec<String>,
}

pub struct StorageStats {
    pub worktrees_bytes: u64,
    pub sessions_bytes: u64,
    pub logs_bytes: u64,
    pub state_bytes: u64,
    pub events_bytes: u64,
    pub dlq_bytes: u64,
    pub total_bytes: u64,
}
```

3. **Safety Checks**
   - Query active sessions from session state
   - Check for checkpoint files indicating resumable workflows
   - Verify no active MapReduce jobs for state cleanup
   - Lock storage during cleanup to prevent race conditions

4. **Age Calculation**
   - Use file modification time (`mtime`) for age comparison
   - For sessions: Use `created_at` from session state
   - For worktrees: Use worktree creation time
   - For logs: Use log file modification time
   - For state: Use checkpoint file modification time

### Architecture Changes

1. **New Module**: `src/cli/commands/clean.rs`
   - Implements cleanup command logic
   - Delegates to StorageCleanupManager
   - Handles user interaction and confirmation

2. **New Module**: `src/storage/cleanup.rs`
   - Implements StorageCleanupManager
   - Provides storage-type-specific cleanup logic
   - Handles safety checks and age filtering

3. **Enhanced**: `src/storage/global.rs`
   - Add methods for storage statistics
   - Add methods to list all repositories
   - Add methods to query active sessions

4. **Enhanced**: `src/cli/args.rs`
   - Add `Clean` variant to `Commands` enum
   - Add `CleanCommands` enum for subcommands
   - Deprecate direct use of `WorktreeCommands::Clean`

### Data Structures

**Session State Query**:
```rust
pub struct SessionInfo {
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub status: SessionStatus,
    pub has_checkpoint: bool,
}

pub enum SessionStatus {
    Active,      // Currently running
    Resumable,   // Has checkpoint, can be resumed
    Completed,   // Finished successfully
    Failed,      // Finished with errors
}
```

**Cleanup Result**:
```rust
pub struct CleanupResult {
    pub storage_before: StorageStats,
    pub storage_after: StorageStats,
    pub cleanup_stats: HashMap<String, CleanupStats>,
}
```

### Integration with Existing Code

1. **Worktree Cleanup**: Reuse existing `WorktreeManager::cleanup_session()`
2. **MapReduce Cleanup**: Reuse existing `WorktreeCleanupCoordinator`
3. **Storage Access**: Use existing `GlobalStorage` methods
4. **Duration Parsing**: Reuse existing `parse_duration()` from worktree.rs

## Dependencies

None - builds on existing storage infrastructure.

## Testing Strategy

### Unit Tests
- Test age-based filtering logic
- Test safety check implementations
- Test storage statistics calculation
- Test dry-run mode behavior
- Test cleanup for each storage type
- Test error handling and partial failures

### Integration Tests
- Create test storage with known data
- Verify correct items are cleaned based on age
- Verify active sessions are protected
- Verify resumable workflows are protected
- Test cleanup across multiple repositories
- Test storage statistics accuracy
- Test progress reporting

### Performance Tests
- Test cleanup with 1000+ sessions
- Test cleanup with large log directories
- Test cleanup across 10+ repositories
- Verify memory usage during cleanup
- Benchmark cleanup operation speed

### User Acceptance Tests
- Manual testing of common cleanup scenarios
- Verify output clarity and usefulness
- Test dry-run mode provides accurate preview
- Verify confirmations work as expected
- Test error messages are helpful

## Documentation Requirements

### Code Documentation
- Document all public APIs in cleanup module
- Add examples for common cleanup scenarios
- Document safety guarantees and edge cases
- Add inline comments for complex logic

### User Documentation
- Update CLI help text with cleanup examples
- Add "Storage Management" section to book
- Document cleanup best practices
- Add troubleshooting guide for cleanup issues
- Document storage lifecycle and retention policies

### CLAUDE.md Updates
```markdown
## Storage Management

Prodigy stores execution data in `~/.prodigy/`:
- **worktrees/**: Isolated git worktrees for sessions
- **sessions/**: Session state and checkpoints
- **logs/**: Claude execution logs
- **state/**: MapReduce job state
- **events/**: Event logs for debugging
- **dlq/**: Dead letter queue data

### Cleanup Commands

Clean all storage older than 30 days:
```bash
prodigy clean --all --older-than 30d
```

Clean specific storage types:
```bash
prodigy clean --worktrees --older-than 7d
prodigy clean --logs --older-than 24h
prodigy clean --sessions --older-than 30d
```

Preview cleanup without making changes:
```bash
prodigy clean --all --older-than 30d --dry-run
```

Clean across all repositories:
```bash
prodigy clean --all --older-than 30d --all-repos
```

### Safety Features
- Active sessions are never deleted
- Resumable workflows are protected
- Dry-run mode available for all operations
- Confirmation required for destructive operations
- Use `--force` to skip confirmations
```

## Implementation Notes

### Cleanup Order
1. Events (least critical)
2. Logs (debugging data)
3. DLQ (failed items, retryable)
4. State (MapReduce checkpoints)
5. Sessions (session state)
6. Worktrees (most space, cleaned last)

### Default Retention Periods
- Worktrees: 7 days
- Sessions: 30 days
- Logs: 7 days
- State: 30 days
- Events: 14 days
- DLQ: 30 days

### Error Handling
- Continue cleanup on individual item failures
- Collect all errors and report at end
- Include context in error messages
- Suggest recovery actions where possible
- Log all errors for debugging

### Progress Indication
- Show progress for operations > 1 second
- Display: "Scanning sessions... (123 found)"
- Display: "Cleaning sessions... (45/123) 36%"
- Display: "✓ Cleaned 45 sessions, reclaimed 123MB"

## Migration and Compatibility

### Backward Compatibility
- Keep `prodigy worktree clean` command working
- Delegate to new `prodigy clean --worktrees` internally
- Maintain same flag behavior and output format
- Add deprecation notice to worktree clean help text

### Migration Path
1. Implement new `prodigy clean` command
2. Update `prodigy worktree clean` to delegate
3. Add deprecation notice to documentation
4. Plan removal of `prodigy worktree clean` in future version

### Breaking Changes
None - all changes are additive.

## Success Metrics

- Users can reclaim disk space easily
- Cleanup operations complete without errors
- Active sessions are never accidentally deleted
- Dry-run mode accurately predicts cleanup
- Storage statistics help users understand usage
- Clear documentation reduces support requests

## Future Enhancements

1. **Automatic Cleanup**: Background process to clean old storage
2. **Retention Policies**: Configurable per-type retention periods
3. **Storage Quotas**: Warn when storage exceeds limits
4. **Compression**: Compress old logs and state before deletion
5. **Archival**: Archive old data instead of deleting
6. **Cleanup Scheduling**: Cron-like scheduling for cleanup tasks
