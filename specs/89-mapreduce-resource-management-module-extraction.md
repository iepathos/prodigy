---
number: 89
title: MapReduce Resource Management Module Extraction
category: optimization
priority: high
status: draft
dependencies: [87, 88]
created: 2025-09-17
---

# Specification 89: MapReduce Resource Management Module Extraction

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [87 - Agent Module, 88 - Command Execution Module]

## Context

Resource management in the MapReduce executor is currently intertwined with execution logic. This includes worktree session management, git branch operations, resource cleanup, and lifecycle management. These responsibilities are scattered throughout the main module, making it difficult to track resource usage, ensure proper cleanup, and prevent resource leaks in error scenarios.

## Objective

Extract all resource management functionality into a dedicated module that provides centralized control over worktree sessions, git operations, and resource lifecycle. This will ensure proper resource cleanup, prevent leaks, and provide better visibility into resource usage patterns.

## Requirements

### Functional Requirements
- Centralize worktree session acquisition and release
- Manage git branch creation and merging operations
- Implement automatic resource cleanup on errors
- Track resource usage and provide metrics
- Support resource pooling and reuse
- Ensure thread-safe resource access

### Non-Functional Requirements
- Guarantee resource cleanup even on panics
- Minimize resource allocation overhead
- Support configurable resource limits
- Enable resource usage monitoring
- Maintain RAII principles throughout

## Acceptance Criteria

- [ ] Resource management module created at `src/cook/execution/mapreduce/resources/`
- [ ] Worktree manager extracted to `resources/worktree.rs`
- [ ] Git operations extracted to `resources/git.rs`
- [ ] Resource cleanup logic in `resources/cleanup.rs`
- [ ] Resource pool implementation in `resources/pool.rs`
- [ ] All resource operations removed from main module
- [ ] Main module reduced by approximately 300 lines
- [ ] No resource leaks in error scenarios
- [ ] Resource usage metrics available
- [ ] All existing tests pass without resource issues

## Technical Details

### Implementation Approach

1. **Module Structure**:
   ```
   src/cook/execution/mapreduce/resources/
   ├── mod.rs          # Module exports and ResourceManager
   ├── worktree.rs     # Worktree session management
   ├── git.rs          # Git branch operations
   ├── cleanup.rs      # Resource cleanup coordination
   └── pool.rs         # Resource pooling implementation
   ```

2. **Key Extractions**:
   - `acquire_worktree_session` → `worktree.rs`
   - `create_agent_branch` → `git.rs`
   - `merge_agent_to_parent` → `git.rs`
   - `cleanup_orphaned_resources` → `cleanup.rs`
   - `get_worktree_commits` → `git.rs`
   - `get_modified_files` → `git.rs`

### Architecture Changes

- Implement RAII guards for all resources
- Use Arc<RwLock> for shared resource state
- Create resource acquisition context
- Implement automatic cleanup on drop

### Data Structures

```rust
pub struct ResourceManager {
    worktree_pool: Arc<WorktreePool>,
    active_sessions: Arc<RwLock<HashMap<String, WorktreeSession>>>,
    cleanup_registry: Arc<RwLock<Vec<Box<dyn CleanupTask>>>>,
}

pub struct ResourceGuard<T> {
    resource: Option<T>,
    cleanup: Box<dyn FnOnce(T)>,
}

impl<T> Drop for ResourceGuard<T> {
    fn drop(&mut self) {
        if let Some(resource) = self.resource.take() {
            (self.cleanup)(resource);
        }
    }
}

pub trait ResourcePool<T> {
    async fn acquire(&self) -> Result<ResourceGuard<T>, ResourceError>;
    fn release(&self, resource: T);
    fn metrics(&self) -> PoolMetrics;
}
```

### APIs and Interfaces

```rust
impl ResourceManager {
    pub async fn acquire_worktree(&self, request: WorktreeRequest)
        -> Result<ResourceGuard<WorktreeSession>, ResourceError>;

    pub async fn create_branch(&self, session: &WorktreeSession, name: &str)
        -> Result<(), GitError>;

    pub async fn cleanup_all(&self) -> Result<(), CleanupError>;

    pub fn register_cleanup(&self, task: Box<dyn CleanupTask>);
}

pub trait CleanupTask: Send + Sync {
    async fn cleanup(&self) -> Result<(), CleanupError>;
    fn priority(&self) -> CleanupPriority;
}
```

## Dependencies

- **Prerequisites**:
  - Phase 1: Utils module extraction (completed)
  - Phase 2: Agent module extraction (spec 87)
  - Phase 3: Command execution module (spec 88)
- **Affected Components**: Agent execution, worktree pool, git operations
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test resource acquisition and release
- **Stress Tests**: Verify behavior under resource exhaustion
- **Cleanup Tests**: Ensure cleanup in all error scenarios
- **Concurrency Tests**: Validate thread-safety
- **Leak Tests**: Use memory profiling to detect leaks

## Documentation Requirements

- **Code Documentation**: Document resource lifecycle
- **Operations Guide**: Resource tuning and monitoring
- **Architecture Updates**: Resource management patterns
- **Troubleshooting**: Common resource issues and solutions

## Implementation Notes

- Use RAII pattern consistently
- Implement drop guards for critical resources
- Add metrics collection from the start
- Consider using tokio's resource management primitives
- Ensure cleanup order is correct for dependent resources
- Add resource usage logging for debugging

## Migration and Compatibility

- Transparent to existing MapReduce workflows
- No changes to public API
- Internal refactoring only
- Consider gradual migration with feature flags
- Maintain backward compatibility for resource limits