# Implementation Plan: Refactor SessionManager God Object

## Problem Summary

**Location**: ./src/unified_session/manager.rs:file:0
**Priority Score**: 58.28
**Debt Type**: God Object / High Complexity

**Current Metrics**:
- Lines of Code: 849
- Functions: 36 (19 impl methods + test helpers)
- Cyclomatic Complexity: 200 total (avg 5.6, max 19)
- Coverage: 0.0%
- Responsibilities: 4 (Persistence, Construction, Utilities, Data Access)

**Issue**: SessionManager is a god object with 849 lines and 36 functions mixing persistence, business logic, and utilities. The module has 4 distinct responsibilities that should be separated into focused modules. The high complexity (200 total) and 0% coverage indicate significant technical debt.

## Target State

**Expected Impact**:
- Complexity Reduction: 40.0 points
- Maintainability Improvement: 5.83 points
- Test Effort: 84.9 (high effort due to 0% baseline coverage)

**Success Criteria**:
- [ ] SessionManager split into 3-4 focused modules (<300 lines each)
- [ ] Each module has single responsibility
- [ ] All existing tests continue to pass
- [ ] Cyclomatic complexity reduced by 40+ points
- [ ] No clippy warnings
- [ ] Proper formatting (cargo fmt)
- [ ] Pure functions extracted from I/O operations
- [ ] Each new module has clear, testable public API

## Implementation Phases

### Phase 1: Extract Storage Operations Module

**Goal**: Create a dedicated `storage.rs` module for all filesystem persistence operations.

**Changes**:
- Create `src/unified_session/storage.rs`
- Extract storage operations from SessionManager:
  - `save_session` (lines 341-354)
  - `load_from_storage` (lines 356-382)
  - `delete_from_storage` (lines 384-398)
  - `load_all_sessions` (lines 400-430)
- Create `SessionStorage` struct with methods:
  - `save(&self, session: &UnifiedSession) -> Result<()>`
  - `load(&self, id: &SessionId) -> Result<UnifiedSession>`
  - `delete(&self, id: &SessionId) -> Result<()>`
  - `load_all(&self) -> Result<Vec<UnifiedSession>>`
- Update SessionManager to use SessionStorage
- Update module exports in `mod.rs`

**Testing**:
- Run `cargo test --lib unified_session` to verify all tests pass
- Run `cargo clippy -- -D warnings`
- Verify file exists: `ls -la src/unified_session/storage.rs`

**Success Criteria**:
- [ ] New storage.rs module created with 4 methods
- [ ] SessionManager delegates to SessionStorage
- [ ] All 15 existing tests pass
- [ ] No clippy warnings
- [ ] Code compiles successfully

### Phase 2: Extract Session Lifecycle Operations

**Goal**: Create a `lifecycle.rs` module for session state transitions and status management.

**Changes**:
- Create `src/unified_session/lifecycle.rs`
- Extract lifecycle methods:
  - `start_session` (lines 194-197)
  - `pause_session` (lines 200-203)
  - `resume_session` (lines 206-209)
  - `complete_session` (lines 212-223)
- Create pure helper functions:
  - `fn transition_status(current: SessionStatus, transition: Transition) -> Result<SessionStatus>`
  - `fn validate_transition(from: SessionStatus, to: SessionStatus) -> Result<()>`
  - `fn calculate_duration(started_at: DateTime<Utc>, completed_at: Option<DateTime<Utc>>) -> Option<Duration>`
- Create `SessionLifecycle` struct that wraps SessionManager
- Separate I/O (persistence) from pure logic (status transitions)

**Testing**:
- Run `cargo test --lib unified_session::tests::test_session_lifecycle`
- Run `cargo test --lib unified_session::tests::test_session_failure`
- Verify state transitions work correctly
- Run `cargo clippy -- -D warnings`

**Success Criteria**:
- [ ] lifecycle.rs module created with pure functions
- [ ] Status transitions validated before applying
- [ ] Duration calculations extracted to pure function
- [ ] Tests test_session_lifecycle and test_session_failure pass
- [ ] No clippy warnings

### Phase 3: Extract Checkpoint Management Module

**Goal**: Create a `checkpoints.rs` module for checkpoint creation, restoration, and listing.

**Changes**:
- Create `src/unified_session/checkpoints.rs`
- Extract checkpoint operations:
  - `create_checkpoint` (lines 226-241)
  - `restore_checkpoint` (lines 244-270)
  - `list_checkpoints` (lines 273-276)
- Create pure checkpoint functions:
  - `fn create_checkpoint_from_session(session: &UnifiedSession) -> Result<Checkpoint>`
  - `fn find_checkpoint(checkpoints: &[Checkpoint], id: &CheckpointId) -> Option<&Checkpoint>`
  - `fn restore_session_from_checkpoint(checkpoint: &Checkpoint) -> Result<UnifiedSession>`
- Create `CheckpointManager` struct
- Separate serialization logic from business logic

**Testing**:
- Run `cargo test --lib unified_session::tests::test_checkpoint_creation_and_restore`
- Verify checkpoint create/restore works
- Run `cargo clippy -- -D warnings`

**Success Criteria**:
- [ ] checkpoints.rs module created
- [ ] Pure functions for checkpoint operations
- [ ] test_checkpoint_creation_and_restore passes
- [ ] Checkpoint logic separated from I/O
- [ ] No clippy warnings

### Phase 4: Refactor SessionUpdate Handler to Pure Functions

**Goal**: Extract pure functions from the complex `update_session` method (lines 101-176, complexity 19).

**Changes**:
- Create `src/unified_session/updates.rs`
- Extract pure update application functions:
  - `fn apply_status_update(session: &mut UnifiedSession, status: SessionStatus) -> ()`
  - `fn apply_metadata_update(session: &mut UnifiedSession, metadata: HashMap<String, Value>) -> ()`
  - `fn apply_checkpoint_update(session: &mut UnifiedSession, state: Value) -> Checkpoint`
  - `fn apply_error_update(session: &mut UnifiedSession, error: String) -> ()`
  - `fn apply_progress_update(session: &mut UnifiedSession, current: usize, total: usize) -> ()`
  - `fn apply_timing_update(session: &mut UnifiedSession, operation: String, duration: Duration) -> ()`
- Each function is pure (takes session, returns modified session or update result)
- Simplify `update_session` to dispatch to pure functions then persist
- Reduce cyclomatic complexity from 19 to <5

**Testing**:
- Run all update-related tests:
  - `cargo test --lib unified_session::tests::test_update_metadata`
  - `cargo test --lib unified_session::tests::test_update_files_changed_delta`
  - `cargo test --lib unified_session::tests::test_update_error`
  - `cargo test --lib unified_session::tests::test_update_progress`
- Run `cargo clippy -- -D warnings`

**Success Criteria**:
- [ ] updates.rs module created with 6 pure functions
- [ ] update_session complexity reduced from 19 to <5
- [ ] All 4 update tests pass
- [ ] Pure functions can be unit tested independently
- [ ] No clippy warnings

### Phase 5: Refactor Filtering Logic to Pure Functions

**Goal**: Extract pure filtering logic from `list_sessions` method (lines 279-328).

**Changes**:
- Create `src/unified_session/filters.rs`
- Extract pure filter functions:
  - `fn matches_status_filter(session: &SessionSummary, filter: &Option<SessionStatus>) -> bool`
  - `fn matches_type_filter(session: &SessionSummary, filter: &Option<SessionType>) -> bool`
  - `fn matches_time_filter(session: &SessionSummary, after: &Option<DateTime<Utc>>, before: &Option<DateTime<Utc>>) -> bool`
  - `fn matches_worktree_filter(session: &SessionSummary, worktree_name: &Option<String>) -> bool`
  - `fn apply_session_filter(session: &SessionSummary, filter: &SessionFilter) -> bool`
- Simplify `list_sessions` to load sessions, apply filter, return
- Reduce complexity by extracting conditionals to named predicates

**Testing**:
- Run `cargo test --lib unified_session::tests::test_list_sessions`
- Verify all filter combinations work
- Run `cargo clippy -- -D warnings`

**Success Criteria**:
- [ ] filters.rs module created with 5 pure predicate functions
- [ ] list_sessions complexity reduced
- [ ] test_list_sessions passes with all filter types
- [ ] Filter logic is independently testable
- [ ] No clippy warnings

## Final SessionManager Structure

After all phases, SessionManager will be:

```rust
// src/unified_session/manager.rs (~150 lines)
pub struct SessionManager {
    storage: SessionStorage,
    lifecycle: SessionLifecycle,
    checkpoints: CheckpointManager,
    active_sessions: Arc<RwLock<HashMap<SessionId, UnifiedSession>>>,
}

impl SessionManager {
    pub async fn new(storage: GlobalStorage) -> Result<Self>
    pub async fn create_session(&self, config: SessionConfig) -> Result<SessionId>
    pub async fn load_session(&self, id: &SessionId) -> Result<UnifiedSession>
    pub async fn update_session(&self, id: &SessionId, update: SessionUpdate) -> Result<()>
    pub async fn delete_session(&self, id: &SessionId) -> Result<()>

    // Delegates to lifecycle
    pub async fn start_session(&self, id: &SessionId) -> Result<()>
    pub async fn pause_session(&self, id: &SessionId) -> Result<()>
    pub async fn resume_session(&self, id: &SessionId) -> Result<()>
    pub async fn complete_session(&self, id: &SessionId, success: bool) -> Result<SessionSummary>

    // Delegates to checkpoints
    pub async fn create_checkpoint(&self, id: &SessionId) -> Result<CheckpointId>
    pub async fn restore_checkpoint(&self, id: &SessionId, checkpoint_id: &CheckpointId) -> Result<()>
    pub async fn list_checkpoints(&self, id: &SessionId) -> Result<Vec<Checkpoint>>

    // Delegates to filters
    pub async fn list_sessions(&self, filter: Option<SessionFilter>) -> Result<Vec<SessionSummary>>
    pub async fn get_active_sessions(&self) -> Result<Vec<SessionId>>
}
```

**New Module Structure**:
- `manager.rs` (~150 lines): Orchestration and public API
- `storage.rs` (~100 lines): File I/O operations
- `lifecycle.rs` (~120 lines): State transitions
- `checkpoints.rs` (~100 lines): Checkpoint management
- `updates.rs` (~150 lines): Update application logic
- `filters.rs` (~80 lines): Pure filter predicates

**Total**: ~700 lines across 6 focused modules (vs 849 in one file)

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib unified_session` to verify existing tests pass
2. Run `cargo clippy -- -D warnings` to check for warnings
3. Run `cargo fmt` to ensure consistent formatting
4. Verify specific test cases mentioned in phase success criteria

**After Phase 5 (Final verification)**:
1. `cargo test --lib` - All tests must pass
2. `just ci` - Full CI checks
3. `cargo build --release` - Verify release build
4. Manual verification: Compare before/after complexity with debtmap

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the compilation errors or test failures
3. Identify the issue (missing import, incorrect delegation, etc.)
4. Fix the issue locally
5. Retry the phase with corrected implementation

If multiple attempts fail:
1. Document what's blocking progress
2. Consider alternative approaches (different module boundaries, phased extraction)
3. Consult git history for context on why code is structured this way

## Notes

**Key Considerations**:
- SessionManager has 0% coverage despite comprehensive test suite - tests are in the same file as implementation
- Tests use TestContext helper that creates isolated test environment
- Many tests depend on full SessionManager integration - need to maintain backward compatibility
- Active sessions cache (Arc<RwLock<HashMap>>) is shared state - be careful with refactoring
- Storage operations are async - maintain async boundaries correctly
- Some operations have lock contention concerns - preserve existing lock patterns

**Functional Programming Opportunities**:
- Update application: Pure functions for state transformations
- Filtering: Pure predicates instead of complex conditionals
- Status transitions: Pure FSM instead of imperative updates
- Checkpoint serialization: Separate pure serialization from I/O

**Risks**:
- Breaking test isolation if storage is not properly injected
- Lock ordering issues if refactoring changes acquisition order
- Async runtime issues if blocking operations introduced
- Missing error context if not propagated correctly

**Dependencies to Check**:
- `super::state` module for types (UnifiedSession, SessionConfig, etc.)
- `crate::storage::GlobalStorage` for base storage
- `chrono` for timestamps
- `tokio::fs` for async file operations
