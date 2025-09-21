---
number: 104
title: Unify Session Management
category: foundation
priority: critical
status: draft
dependencies: [102]
created: 2025-01-21
---

# Specification 104: Unify Session Management

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [102 - Consolidate Storage Systems]

## Context

The codebase currently has four separate session management implementations:
1. `src/session/` - Original session management
2. `src/cook/session/` - Cooking-specific sessions
3. `src/simple_state/` - Simplified state management
4. `src/storage/traits::SessionStorage` - Abstract session interface

This redundancy creates confusion about which system to use, increases maintenance burden, makes the codebase harder to understand, and likely introduces subtle bugs due to inconsistent behavior between implementations.

## Objective

Consolidate all session management into a single, unified system that handles all session-related functionality consistently across the application.

## Requirements

### Functional Requirements
- Create single source of truth for session state
- Support all existing session operations (create, read, update, delete)
- Handle session lifecycle (initialization, checkpointing, cleanup)
- Maintain backward compatibility with existing session data
- Support both workflow and MapReduce session types
- Provide atomic session updates to prevent corruption

### Non-Functional Requirements
- Reduce session-related code by at least 50%
- Improve session operation performance
- Ensure thread-safe session access
- Simplify session API for consumers

## Acceptance Criteria

- [ ] Single `SessionManager` implementation used everywhere
- [ ] All session operations go through unified API
- [ ] Existing session data migrated successfully
- [ ] No duplicate session logic in codebase
- [ ] Session tests consolidated and passing
- [ ] Session operations are atomic and consistent
- [ ] Documentation describes single session model

## Technical Details

### Implementation Approach

1. **Phase 1: Design Unified Model**
   ```rust
   pub struct UnifiedSession {
       id: SessionId,
       type: SessionType,
       state: SessionState,
       metadata: SessionMetadata,
       checkpoints: Vec<Checkpoint>,
       timings: SessionTimings,
   }

   pub enum SessionType {
       Workflow(WorkflowSession),
       MapReduce(MapReduceSession),
   }
   ```

2. **Phase 2: Create Unified Manager**
   ```rust
   pub struct SessionManager {
       storage: GlobalStorage,  // Uses configurable storage from spec 102
       active_sessions: HashMap<SessionId, UnifiedSession>,
       lock_manager: LockManager,
   }

   impl SessionManager {
       pub async fn create_session(&self, config: SessionConfig) -> Result<SessionId>
       pub async fn load_session(&self, id: &SessionId) -> Result<UnifiedSession>
       pub async fn update_session(&self, id: &SessionId, update: SessionUpdate) -> Result<()>
       pub async fn checkpoint(&self, id: &SessionId) -> Result<()>
       pub async fn complete_session(&self, id: &SessionId) -> Result<()>
   }
   ```

3. **Phase 3: Migrate Components**
   - Update cook module to use unified sessions
   - Migrate MapReduce to unified sessions
   - Remove simple_state module entirely
   - Remove abstract session storage traits

4. **Phase 4: Cleanup**
   - Delete redundant session implementations
   - Consolidate session tests
   - Update documentation

### Architecture Changes

Before:
```
┌─────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐
│   Session   │  │Cook::Session │  │ SimpleState  │  │   Storage   │
│   Manager   │  │   Manager    │  │   Manager    │  │   Traits    │
└─────────────┘  └──────────────┘  └──────────────┘  └─────────────┘
       │                │                 │                 │
       └────────────────┴─────────────────┴─────────────────┘
                                │
                          Inconsistent!
```

After:
```
┌──────────────────────────────────────┐
│         Unified SessionManager        │
├──────────────────────────────────────┤
│  - Single API for all session ops    │
│  - Consistent state management       │
│  - Atomic updates with locking       │
└──────────────────────────────────────┘
                    │
            ┌───────┴────────┐
            │ GlobalStorage  │
            │ (Configurable) │
            └────────────────┘
```

### Data Structures

```rust
pub struct UnifiedSession {
    pub id: SessionId,
    pub session_type: SessionType,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, Value>,
    pub checkpoints: Vec<Checkpoint>,
    pub timings: BTreeMap<String, Duration>,
    pub error: Option<String>,
}

pub enum SessionStatus {
    Initializing,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

pub struct Checkpoint {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub state: Value, // JSON-serializable state
    pub metadata: HashMap<String, Value>,
}
```

### APIs and Interfaces

Unified public API:
```rust
// Primary session operations
pub async fn create_session(config: SessionConfig) -> Result<SessionId>
pub async fn get_session(id: &SessionId) -> Result<UnifiedSession>
pub async fn update_session(id: &SessionId, update: SessionUpdate) -> Result<()>
pub async fn delete_session(id: &SessionId) -> Result<()>

// Session lifecycle
pub async fn start_session(id: &SessionId) -> Result<()>
pub async fn pause_session(id: &SessionId) -> Result<()>
pub async fn resume_session(id: &SessionId) -> Result<()>
pub async fn complete_session(id: &SessionId, result: SessionResult) -> Result<()>

// Checkpointing
pub async fn create_checkpoint(id: &SessionId) -> Result<CheckpointId>
pub async fn restore_checkpoint(id: &SessionId, checkpoint_id: &CheckpointId) -> Result<()>
pub async fn list_checkpoints(id: &SessionId) -> Result<Vec<Checkpoint>>

// Query operations
pub async fn list_sessions(filter: SessionFilter) -> Result<Vec<SessionSummary>>
pub async fn get_active_sessions() -> Result<Vec<SessionId>>
```

## Dependencies

- **Prerequisites**:
  - Spec 102 (Consolidate Storage Systems) - Need unified storage first
- **Affected Components**:
  - Cook module
  - MapReduce execution
  - CLI session commands
  - Worktree management
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test all session operations
- **Integration Tests**: Test session lifecycle end-to-end
- **Concurrency Tests**: Test concurrent session access
- **Migration Tests**: Verify existing sessions work after migration
- **Compatibility Tests**: Ensure backward compatibility

## Documentation Requirements

- **Code Documentation**: Document unified session model
- **API Documentation**: Complete API reference for SessionManager
- **Migration Guide**: Guide for updating code to use unified API
- **Architecture Documentation**: Update ARCHITECTURE.md

## Implementation Notes

- Use file locking to ensure atomic session updates
- Keep session cache in memory for performance
- Implement lazy loading for checkpoint data
- Consider using SQLite for session indexing if needed later
- Ensure graceful handling of corrupted session files
- Leverage configurable storage paths from spec 102 (via `GlobalStorage`)
- Session storage will use the same base directory configuration as other storage
- Future distributed storage support will be behind feature flags per spec 102

## Migration and Compatibility

Automatic migration of existing sessions:
```
Migrating session data to unified format...
  Found 5 workflow sessions
  Found 3 MapReduce sessions
  Converting session formats...
  Migration complete. All sessions accessible through unified API.
```

Code migration for consumers:
```rust
// Before (multiple implementations):
let session = cook::session::SessionManager::load(id)?;
let state = simple_state::StateManager::get(id)?;

// After (unified):
let session = prodigy::session::get_session(id)?;
let state = session.state;
```