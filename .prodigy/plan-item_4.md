# Implementation Plan: Refactor MapReduceCoordinator God Object

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/coordination/executor.rs:file:0
**Priority Score**: 103.05
**Debt Type**: God Object (God Class)
**Current Metrics**:
- Lines of Code: 2,752 lines
- Functions: 103 functions (57 production, ~46 test functions)
- Cyclomatic Complexity: 246 total, 2.39 average
- Coverage: 0%
- God Object Score: 1.0 (maximum)
- Responsibilities: 5 distinct responsibilities
- MapReduceCoordinator: 16 fields, 22 methods

**Issue**: This file is a classic God Object anti-pattern. The MapReduceCoordinator class has grown to handle 5 different responsibilities (Processing, Utilities, Persistence, Construction, Data Access) with 2,752 lines in a single file. This violates the Single Responsibility Principle and makes the code difficult to test, understand, and maintain. The file contains substantial test code (~1,100 lines) that should be in a separate test file.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 49.2 (20% reduction)
- Maintainability Improvement: 10.3 points
- Test Effort Reduction: 275.2 (by making code more testable)

**Success Criteria**:
- [ ] MapReduceCoordinator reduced to <500 lines with clear, focused responsibility
- [ ] All tests moved to separate test file
- [ ] Phase execution logic extracted to dedicated modules
- [ ] Helper utilities extracted to utility module
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Code coverage maintained or improved

## Implementation Phases

This refactoring will be done incrementally to maintain stability and allow for testing at each step.

### Phase 1: Extract Test Code to Separate File

**Goal**: Move all test modules from executor.rs to executor_tests.rs to reduce file size and improve organization.

**Changes**:
- Move test modules from lines 1673-2752 to `executor_tests.rs`
- Keep production code in executor.rs
- Update imports in test file to access production code
- Verify all tests pass in new location

**Test Modules to Move**:
- `handle_on_failure_tests` (lines 1674-2083)
- `execute_setup_phase_tests` (lines 2085-2522)
- `reduce_interpolation_context_tests` (lines 2524-2752)

**Testing**:
```bash
cargo test --lib mapreduce::coordination::executor
cargo test --lib mapreduce::coordination::executor_tests
```

**Success Criteria**:
- [ ] All tests moved to executor_tests.rs
- [ ] All tests pass in new location
- [ ] executor.rs reduced by ~1,100 lines
- [ ] No compilation errors

### Phase 2: Extract Phase Execution Modules

**Goal**: Extract setup, map, and reduce phase execution logic into separate, focused modules to reduce coordinator complexity.

**Changes**:
Create three new modules in `src/cook/execution/mapreduce/coordination/phases/`:
- `setup_phase.rs` - Setup phase execution logic
- `map_phase.rs` - Map phase execution logic
- `reduce_phase.rs` - Reduce phase execution logic

**Functions to Extract**:

**setup_phase.rs** (~250 lines):
- `execute_setup_phase()` (lines 347-481)
- `execute_setup_step()` (lines 483-565)
- `get_step_display_name()` (lines 329-345)
- Helper: Setup phase event logging
- Helper: Setup checkpoint management

**map_phase.rs** (~400 lines):
- `execute_map_phase_internal()` (lines 615-823)
- `load_work_items()` (lines 567-613)
- `execute_agent_for_item()` (lines 825-1115)
- `execute_step_in_agent_worktree()` (lines 1117-1309)
- `handle_on_failure()` (lines 1311-1417)
- `get_worktree_commits()` (lines 1419-1436)
- `get_worktree_modified_files()` (lines 1438-1459)

**reduce_phase.rs** (~150 lines):
- `execute_reduce_phase()` (lines 1486-1575)
- `build_reduce_interpolation_context()` (lines 1461-1484)
- Helper: Reduce phase event logging

**Module Structure**:
Each module will have:
- A struct holding necessary dependencies (Arc references from coordinator)
- Public methods for phase execution
- Private helper methods for internal logic
- Clear separation of concerns

**Testing**:
```bash
cargo test --lib mapreduce::coordination::phases
cargo build --lib
```

**Success Criteria**:
- [ ] Three new phase modules created
- [ ] Phase execution logic moved to modules
- [ ] MapReduceCoordinator delegates to phase modules
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] executor.rs reduced by ~800 lines

### Phase 3: Extract Display and Summary Utilities

**Goal**: Extract display, summary, and utility methods into a separate utilities module.

**Changes**:
Create `src/cook/execution/mapreduce/coordination/utils.rs`:

**Functions to Extract**:
- `display_map_summary()` (lines 1577-1589)
- `display_reduce_summary()` (lines 1591-1597)
- Orphaned worktree management functions
- Any remaining helper utilities

**Module Structure**:
```rust
pub struct CoordinatorUtils {
    event_logger: Arc<EventLogger>,
    user_interaction: Arc<dyn UserInteraction>,
}

impl CoordinatorUtils {
    pub fn display_map_summary(&self, summary: &AggregationSummary) { ... }
    pub fn display_reduce_summary(&self, summary: &AggregationSummary) { ... }
}
```

**Testing**:
```bash
cargo test --lib mapreduce::coordination::utils
cargo clippy --lib
```

**Success Criteria**:
- [ ] Utils module created with utility functions
- [ ] Display logic extracted from coordinator
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] executor.rs reduced by ~100 lines

### Phase 4: Simplify MapReduceCoordinator Structure

**Goal**: Reduce field count and simplify coordinator by grouping related dependencies.

**Changes**:
- Group related Arc references into context structs
- Create `ExecutionContext` to hold execution-related dependencies
- Create `ResourceContext` to hold resource-related dependencies
- Reduce coordinator to orchestration-only logic

**Before** (16 fields):
```rust
pub struct MapReduceCoordinator {
    agent_manager: Arc<dyn AgentLifecycleManager>,
    _state_manager: Arc<StateManager>,
    user_interaction: Arc<dyn UserInteraction>,
    result_collector: Arc<ResultCollector>,
    subprocess: Arc<SubprocessManager>,
    project_root: PathBuf,
    event_logger: Arc<EventLogger>,
    job_id: String,
    claude_executor: Arc<dyn ClaudeExecutor>,
    _session_manager: Arc<dyn SessionManager>,
    execution_mode: ExecutionMode,
    timeout_enforcer: Arc<Mutex<Option<Arc<TimeoutEnforcer>>>>,
    merge_queue: Arc<MergeQueue>,
    orphaned_worktrees: Arc<Mutex<Vec<OrphanedWorktree>>>,
    dlq: Arc<DeadLetterQueue>,
    retry_counts: Arc<tokio::sync::RwLock<HashMap<String, u32>>>,
}
```

**After** (6-7 fields):
```rust
pub struct MapReduceCoordinator {
    execution_context: Arc<ExecutionContext>,
    resource_context: Arc<ResourceContext>,
    setup_executor: SetupPhaseExecutor,
    map_executor: MapPhaseExecutor,
    reduce_executor: ReducePhaseExecutor,
    utils: CoordinatorUtils,
}

pub struct ExecutionContext {
    pub job_id: String,
    pub project_root: PathBuf,
    pub execution_mode: ExecutionMode,
    pub event_logger: Arc<EventLogger>,
    pub user_interaction: Arc<dyn UserInteraction>,
}

pub struct ResourceContext {
    pub agent_manager: Arc<dyn AgentLifecycleManager>,
    pub result_collector: Arc<ResultCollector>,
    pub subprocess: Arc<SubprocessManager>,
    pub claude_executor: Arc<dyn ClaudeExecutor>,
    pub timeout_enforcer: Arc<Mutex<Option<Arc<TimeoutEnforcer>>>>,
    pub merge_queue: Arc<MergeQueue>,
    pub dlq: Arc<DeadLetterQueue>,
}
```

**Testing**:
```bash
cargo test --lib mapreduce::coordination
cargo build --lib
```

**Success Criteria**:
- [ ] Context structs created
- [ ] Coordinator fields consolidated
- [ ] All phase executors initialized with contexts
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Coordinator down to ~400 lines

### Phase 5: Update Public API and Documentation

**Goal**: Ensure the refactored coordinator maintains its public API and is well-documented.

**Changes**:
- Update `execute_job()` to delegate to phase executors
- Ensure all public methods remain unchanged
- Add module-level documentation for new modules
- Update inline documentation for clarity
- Verify all integration points work correctly

**Public API Methods** (must remain unchanged):
- `new()` - Constructor
- `with_mode()` - Constructor with execution mode
- `get_orphaned_worktrees()` - Get orphaned worktree list
- `register_orphaned_worktree()` - Register orphaned worktree
- `execute_job()` - Main entry point
- `get_results()` - Get execution results
- `clear_results()` - Clear results

**Documentation**:
- Add module docs for phases/ modules
- Add struct docs for context types
- Update executor.rs module doc
- Ensure all public methods have doc comments

**Testing**:
```bash
cargo test --lib
cargo clippy --lib
cargo fmt --check
cargo doc --lib --no-deps
```

**Success Criteria**:
- [ ] All public methods work as before
- [ ] Integration with orchestrator unchanged
- [ ] Documentation complete and accurate
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Final line count: executor.rs <500 lines

## Testing Strategy

**For each phase**:
1. Run unit tests: `cargo test --lib mapreduce::coordination`
2. Run clippy: `cargo clippy --lib`
3. Check formatting: `cargo fmt --check`
4. Verify compilation: `cargo build --lib`

**Phase-specific testing**:
- Phase 1: Verify all tests moved and still pass
- Phase 2: Test each new phase module independently
- Phase 3: Test utility functions in isolation
- Phase 4: Integration tests for context structs
- Phase 5: Full end-to-end workflow test

**Final verification**:
1. Run full test suite: `cargo test`
2. Run linter: `cargo clippy -- -D warnings`
3. Check coverage: `cargo tarpaulin --lib`
4. Run debtmap: `debtmap analyze --file src/cook/execution/mapreduce/coordination/executor.rs`
5. Verify metrics improvement:
   - Lines: 2752 → <500 (82% reduction)
   - Functions: 103 → <30 (71% reduction)
   - God Object Score: 1.0 → <0.3

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the compilation/test failure
3. Identify the root cause (usually missing import or incorrect delegation)
4. Adjust the plan for that specific issue
5. Retry the phase with fix applied

**Common issues and solutions**:
- **Missing imports**: Add use statements for extracted types
- **Lifetime issues**: Ensure Arc cloning for shared ownership
- **Test failures**: Check test fixture setup uses new structure
- **Visibility issues**: Make extracted structs/methods pub where needed

## Notes

**Key Considerations**:
- This is a large refactoring but done incrementally to minimize risk
- Each phase is independently valuable and leaves code in working state
- Tests are moved first to reduce file size and clarify production code
- Phase extraction follows natural boundaries (setup, map, reduce)
- Context structs reduce parameter passing and coupling
- Public API remains unchanged to avoid breaking integration

**Design Decisions**:
- Using context structs instead of builder pattern for clarity
- Each phase executor is a separate struct with clear responsibilities
- Keeping coordination logic in MapReduceCoordinator (its true responsibility)
- DummySessionManager remains in executor.rs as it's test-specific

**Expected Outcome**:
After this refactoring:
- MapReduceCoordinator will be <500 lines and focused on coordination
- Each phase has its own testable module
- Test code is properly separated
- Complexity is distributed across multiple small modules
- Future changes are easier to make and test
- God Object anti-pattern is resolved
