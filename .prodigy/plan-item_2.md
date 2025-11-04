# Implementation Plan: Extract Merge Workflow Orchestration from WorktreeManager

## Problem Summary

**Location**: ./src/worktree/manager.rs:file:0
**Priority Score**: 92.77
**Debt Type**: God Object / High Complexity
**Current Metrics**:
- Lines of Code: 2258
- Functions: 67 (35 impl methods + 32 test functions)
- Cyclomatic Complexity: 245 total, 18 max, 3.65 average
- Coverage: 0%

**Issue**: The WorktreeManager is a god object with 35 methods spanning 1297 lines of implementation. While previous refactoring extracted validation, utilities, and queries modules, the core manager still handles too many responsibilities:
1. Session lifecycle management
2. Git worktree operations
3. Merge workflow execution (14 methods, ~400 lines)
4. Checkpoint management
5. Cleanup operations
6. State persistence

The debtmap analysis recommends extracting 28 methods related to "Utilities" responsibility, but upon code inspection, the **merge workflow execution** is the largest opportunity for extraction with clear boundaries.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 49.0 points
- Coverage Improvement: Not applicable (test code)
- Maintainability Improvement: 9.28 points

**Success Criteria**:
- [ ] Extract merge workflow orchestration to dedicated module (`merge_orchestrator.rs`)
- [ ] Reduce WorktreeManager impl block from 1297 lines to ~850 lines
- [ ] Achieve single responsibility for each module
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Merge Workflow Orchestrator Module

**Goal**: Create new `merge_orchestrator.rs` module to handle all merge workflow execution logic, removing ~450 lines from manager.rs.

**Changes**:
- Create `src/worktree/merge_orchestrator.rs`
- Extract these methods from WorktreeManager into `MergeOrchestrator` struct:
  - `execute_merge_workflow` (orchestrates merge execution)
  - `execute_claude_merge` (Claude-assisted merge)
  - `execute_custom_merge_workflow` (custom workflow execution)
  - `init_merge_variables` (variable initialization)
  - `execute_merge_shell_command` (shell command execution)
  - `execute_merge_claude_command` (Claude command execution)
  - `interpolate_merge_variables` (variable interpolation - delegates to utilities)
  - `log_execution_context` (logging)
  - `log_claude_execution_details` (logging)
  - `save_merge_checkpoint` (checkpoint creation)
  - `create_merge_checkpoint_manager` (checkpoint manager setup)
- Move these to `MergeOrchestrator` with dependency injection for:
  - `SubprocessManager` (for git/shell commands)
  - `ClaudeExecutor` (for Claude commands)
  - Base directory path
  - Repo path
  - Verbosity level
  - Custom merge workflow config
  - Workflow environment variables
- Update WorktreeManager to:
  - Create and use MergeOrchestrator instance
  - Delegate merge operations to orchestrator

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] `merge_orchestrator.rs` module created with ~450 lines
- [ ] WorktreeManager reduced by ~450 lines
- [ ] All merge-related tests pass
- [ ] No new clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Session Query Operations

**Goal**: Move session query operations to `manager_queries.rs`, further reducing WorktreeManager complexity.

**Changes**:
- Move these methods from WorktreeManager to `manager_queries.rs`:
  - `list_sessions` (already uses helper methods)
  - `list_git_worktree_sessions` (git query)
  - `list_detailed` (enhanced query with workflow info)
  - `list_metadata_sessions` (metadata query)
  - `create_worktree_session` (session construction)
  - `find_session_by_name` (lookup)
- Update `manager_queries.rs` to accept WorktreeManager context via parameters
- Change these to free functions or add QueryService struct
- Update WorktreeManager to delegate to query functions/service

**Testing**:
- Run `cargo test --lib --test-threads=1` (for git operations)
- Verify list operations work correctly
- Check detailed session info extraction

**Success Criteria**:
- [ ] Session query methods extracted to `manager_queries.rs`
- [ ] WorktreeManager reduced by ~200 lines
- [ ] All list/query tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Extract Cleanup Operations Module

**Goal**: Create `cleanup_operations.rs` module for all cleanup-related logic, separating it from core manager.

**Changes**:
- Create `src/worktree/cleanup_operations.rs`
- Extract these methods into `CleanupService` struct:
  - `cleanup_session` (worktree cleanup)
  - `cleanup_all_sessions` (batch cleanup)
  - `cleanup_session_after_merge` (post-merge cleanup)
  - `cleanup_merged_sessions` (merged session cleanup)
  - `detect_mergeable_sessions` (mergeable detection)
  - `perform_auto_cleanup` (auto cleanup)
  - `show_cleanup_diagnostics` (diagnostics)
  - `show_manual_cleanup_message` (user messaging)
- Move `CleanupConfig` and `CleanupPolicy` to cleanup module
- Update WorktreeManager to use CleanupService
- Inject dependencies: subprocess manager, base_dir, repo_path

**Testing**:
- Run cleanup-related tests
- Verify cleanup config tests pass
- Check merged session detection

**Success Criteria**:
- [ ] `cleanup_operations.rs` module created with ~350 lines
- [ ] WorktreeManager reduced by ~350 lines
- [ ] All cleanup tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Extract State Management Operations

**Goal**: Consolidate state persistence operations into a dedicated module, separating pure state management from orchestration.

**Changes**:
- Create `src/worktree/state_manager.rs`
- Extract these methods into `StateManager` struct:
  - `update_session_state` (state updates)
  - `update_session_state_after_merge` (post-merge update)
  - `update_checkpoint` (checkpoint updates)
  - `restore_session` (session restoration)
  - `mark_session_abandoned` (abandonment marking)
  - `get_last_successful_command` (command retrieval)
  - `get_session_state` (state loading)
  - `load_session_state` (state deserialization)
- Implement StateManager with:
  - Base directory path
  - Atomic file operations (temp file + rename)
  - Error context wrapping
- Update WorktreeManager to use StateManager

**Testing**:
- Run checkpoint update tests
- Verify session restoration
- Check atomic file operations

**Success Criteria**:
- [ ] `state_manager.rs` module created with ~150 lines
- [ ] WorktreeManager reduced by ~150 lines
- [ ] All state management tests pass
- [ ] Atomic updates verified
- [ ] Ready to commit

### Phase 5: Finalize WorktreeManager Core Responsibilities

**Goal**: Ensure WorktreeManager has clear, focused responsibilities as the main orchestrator.

**Changes**:
- Review remaining WorktreeManager methods:
  - Keep: `new`, `with_config`, `create_session_worktree`, `merge_session`
  - Keep: High-level orchestration methods that coordinate between modules
  - Keep: Methods that require multiple module interactions
- Update documentation:
  - Document clear responsibility boundaries
  - Update module-level docs to reflect new architecture
  - Add examples of how modules interact
- Verify separation of concerns:
  - I/O operations stay at edges (manager coordinates)
  - Pure functions in utilities/validation modules
  - State operations in state_manager
  - Cleanup in cleanup_operations
  - Merge workflows in merge_orchestrator
  - Queries in manager_queries

**Testing**:
- Run full test suite: `cargo test`
- Run clippy: `cargo clippy --all-targets`
- Run formatter: `cargo fmt --check`
- Verify integration tests pass

**Success Criteria**:
- [ ] WorktreeManager has ~300-400 lines of core orchestration
- [ ] Clear module boundaries documented
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code formatted correctly
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure formatting
4. Commit with descriptive message

**Final verification**:
1. `cargo test --all` - All tests pass
2. `cargo clippy --all-targets` - No warnings
3. `cargo fmt --check` - Properly formatted
4. Manual review of public API stability
5. Verify no breaking changes to external callers

## Rollback Plan

If a phase fails:
1. Review the error carefully
2. Run `git diff` to see what changed
3. Run `git reset --hard HEAD~1` to revert the commit
4. Analyze the failure cause
5. Adjust the plan if needed
6. Retry the phase with corrections

## Notes

### Architecture Principles

This refactoring follows these key principles:

1. **Separation of Concerns**: Each module has one clear responsibility
   - `merge_orchestrator`: Merge workflow execution
   - `manager_queries`: Session queries and listing
   - `cleanup_operations`: Cleanup logic and policies
   - `state_manager`: State persistence and updates
   - `manager_validation`: Pure validation functions (already exists)
   - `manager_utilities`: String utilities and formatting (already exists)

2. **Dependency Injection**: Modules receive their dependencies explicitly
   - Easier to test in isolation
   - Clear dependency graph
   - No hidden global state

3. **I/O at Edges**: Keep side effects in orchestrator, logic in pure functions
   - Pure functions in validation/utilities
   - I/O operations in services (merge, cleanup, state)
   - WorktreeManager coordinates between services

4. **Incremental Progress**: Each phase is independently valuable
   - Can commit after each phase
   - Tests pass after each phase
   - Clear rollback points

### Expected Line Count Reduction

- **Before**: 2258 lines total (1297 impl + 961 tests)
- **After Phase 1**: ~1808 lines (merge orchestrator extracted)
- **After Phase 2**: ~1608 lines (queries extracted)
- **After Phase 3**: ~1258 lines (cleanup extracted)
- **After Phase 4**: ~1108 lines (state management extracted)
- **After Phase 5**: ~1000 lines (final cleanup)
- **Final Target**: ~1000 lines (300-400 impl + 600-700 tests)

### Complexity Reduction

- **Current**: 35 methods, 245 cyclomatic complexity
- **Target**: ~8-10 orchestration methods, <100 complexity
- **Extracted modules**: Each with <10 methods, focused responsibilities

### Why This Approach vs. Debtmap Recommendation

The debtmap recommends extracting 28 methods to `manager_utilities`, but code inspection reveals:
- Many of those methods are I/O-heavy (not pure utilities)
- Merge workflow is a cohesive 14-method cluster (~400 lines)
- Better separation is by responsibility domain, not just method count
- This approach achieves the same complexity reduction with better boundaries
