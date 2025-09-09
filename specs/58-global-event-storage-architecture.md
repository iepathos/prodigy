---
number: 58
title: Global Event Storage Architecture
category: storage
priority: high
status: draft
dependencies: [51]
created: 2025-01-09
---

# Specification 58: Global Event Storage Architecture

**Category**: storage
**Priority**: high
**Status**: draft
**Dependencies**: [51 - Event Logging]

## Context

Prodigy currently stores event data, Dead Letter Queue (DLQ) entries, and MapReduce state in the local project directory under `.prodigy/`. This approach has several critical limitations:

1. **Worktree Isolation**: MapReduce agents running in separate worktrees cannot access events from other agents, preventing proper event aggregation and monitoring
2. **Repository Pollution**: Local `.prodigy/` directories require gitignore entries and clutter project repositories with runtime artifacts
3. **Cross-Worktree Communication**: The reduce phase cannot easily aggregate events from multiple agents working in different worktrees
4. **Cleanup Complexity**: Event retention and cleanup must be managed separately in each worktree
5. **Resume Capability**: Job state is lost when worktrees are deleted, preventing proper job resumption

Prodigy already uses a global directory (`~/.prodigy/worktrees/{repo_name}/`) for worktree management. Event storage should follow this same pattern to enable centralized management and cross-worktree visibility.

## Objective

Refactor the event storage architecture to use a global directory structure under `~/.prodigy/` that mirrors the existing worktree organization. This will provide centralized event storage accessible by all agents regardless of worktree, eliminate repository pollution, and enable proper MapReduce event aggregation and job resumption capabilities.

## Requirements

### Functional Requirements

1. **Global Event Storage**
   - Store all events in `~/.prodigy/events/{repo_name}/{job_id}/`
   - Support multiple concurrent jobs per repository
   - Maintain event ordering and timestamps
   - Enable cross-worktree event queries

2. **Dead Letter Queue Management**
   - Store DLQ entries in `~/.prodigy/dlq/{repo_name}/{job_id}/`
   - Support retry mechanisms across worktrees
   - Maintain failure metadata and retry counts
   - Enable DLQ inspection and management commands

3. **Job State Persistence**
   - Store job state in `~/.prodigy/state/{repo_name}/{job_id}/`
   - Support checkpoint creation and recovery
   - Enable job resumption after worktree deletion
   - Maintain job metadata and progress tracking

4. **Repository Name Extraction**
   - Derive repository name from project path consistently
   - Handle edge cases (symlinks, nested repos)
   - Match worktree naming conventions
   - Support repository renaming scenarios

5. **Migration Support**
   - Detect existing local `.prodigy/` directories
   - Provide migration utilities for existing events
   - Support backward compatibility during transition
   - Clean up local directories after migration

### Non-Functional Requirements

1. **Performance**
   - Minimal overhead for event writing
   - Efficient event querying across large datasets
   - Support for concurrent read/write operations
   - Optimized file I/O patterns

2. **Reliability**
   - Atomic event writes to prevent corruption
   - Proper file locking for concurrent access
   - Graceful handling of disk space issues
   - Recovery from partial writes

3. **Maintainability**
   - Clear separation of storage concerns
   - Consistent API across storage types
   - Comprehensive error handling
   - Detailed logging for debugging

## Acceptance Criteria

- [ ] Events are stored in `~/.prodigy/events/{repo_name}/{job_id}/` instead of local `.prodigy/`
- [ ] DLQ entries are stored in `~/.prodigy/dlq/{repo_name}/{job_id}/`
- [ ] Job state is persisted in `~/.prodigy/state/{repo_name}/{job_id}/`
- [ ] All MapReduce agents can read/write events regardless of worktree
- [ ] Event aggregation works correctly across multiple worktrees
- [ ] No `.prodigy/` directories are created in local repositories
- [ ] Existing CLI commands work with new storage locations
- [ ] Event retention policies apply globally per repository
- [ ] Job resumption works after worktree deletion
- [ ] All tests pass with new storage architecture

## Technical Details

### Implementation Approach

1. **Create Storage Module**
   ```rust
   pub mod storage {
       pub fn get_global_events_dir(repo_name: &str) -> Result<PathBuf>
       pub fn get_global_dlq_dir(repo_name: &str) -> Result<PathBuf>
       pub fn get_global_state_dir(repo_name: &str) -> Result<PathBuf>
       pub fn extract_repo_name(repo_path: &Path) -> Result<String>
   }
   ```

2. **Update Event Writers**
   - Modify `FileEventWriter` to use global paths
   - Update `EventStore` implementations
   - Adjust retention manager paths

3. **Refactor MapReduceOrchestrator**
   - Use global storage paths
   - Update state management
   - Fix DLQ initialization

4. **Update CLI Commands**
   - Change default paths in event commands
   - Add path resolution logic
   - Support both global and local paths

### Directory Structure

```
~/.prodigy/
├── worktrees/
│   └── {repo_name}/
│       ├── prodigy-session-xxx/
│       └── prodigy-session-yyy/
├── events/
│   └── {repo_name}/
│       └── {job_id}/
│           ├── events-{timestamp}.jsonl
│           ├── checkpoint.json
│           └── archive/
│               └── events_archive_{timestamp}.jsonl.gz
├── dlq/
│   └── {repo_name}/
│       └── {job_id}/
│           └── failed-items.json
└── state/
    └── {repo_name}/
        └── {job_id}/
            ├── job-state.json
            └── agent-states/
                └── agent-{id}.json
```

### API Changes

1. **EventLogger Constructor**
   ```rust
   impl EventLogger {
       pub fn new_global(repo_name: String, job_id: String) -> Result<Self>
   }
   ```

2. **JobStateManager**
   ```rust
   impl JobStateManager {
       pub fn new_global(repo_name: String) -> Result<Self>
   }
   ```

3. **DeadLetterQueue**
   ```rust
   impl DeadLetterQueue {
       pub fn new_global(repo_name: String, job_id: String) -> Result<Self>
   }
   ```

## Dependencies

- **Specification 51**: Event Logging infrastructure that needs to be updated
- **Existing Worktree Manager**: Reuse repository name extraction logic
- **File System Utilities**: For atomic file operations and locking

## Testing Strategy

### Unit Tests
- Test path construction for different repository names
- Verify repository name extraction edge cases
- Test concurrent read/write operations
- Validate atomic write operations

### Integration Tests
- Test multi-worktree event aggregation
- Verify job resumption after worktree deletion
- Test DLQ retry across worktrees
- Validate retention policies on global storage

### Migration Tests
- Test detection of local `.prodigy/` directories
- Verify event migration from local to global
- Test backward compatibility mode
- Validate cleanup of local directories

## Documentation Requirements

### Code Documentation
- Document storage module API
- Add examples for path resolution
- Document migration utilities
- Include troubleshooting guide

### User Documentation
- Update README with new storage architecture
- Document changes to CLI commands
- Add migration guide for existing users
- Include FAQ for common issues

### Architecture Updates
- Update ARCHITECTURE.md with storage design
- Document data flow across worktrees
- Add sequence diagrams for event aggregation
- Include storage hierarchy diagram

## Implementation Notes

1. **Path Canonicalization**: Always canonicalize repository paths to handle symlinks consistently
2. **Directory Creation**: Use `create_dir_all` to ensure parent directories exist
3. **File Locking**: Implement proper file locking for concurrent access
4. **Atomic Writes**: Use temp files and rename for atomic operations
5. **Error Recovery**: Handle disk full and permission errors gracefully

## Migration and Compatibility

### Breaking Changes
- Event storage location changes from local to global
- CLI commands may need explicit path options
- Existing monitoring scripts may need updates

### Migration Path
1. Detect existing local `.prodigy/` on first run
2. Prompt user to migrate or continue with local
3. Provide `prodigy migrate-events` command
4. Support dual-mode operation during transition
5. Remove local storage support in future version

### Compatibility Mode
- Environment variable `PRODIGY_USE_LOCAL_STORAGE=true` for legacy behavior
- Automatic fallback if global directory creation fails
- Warning messages for deprecated local storage
- Sunset plan for local storage support