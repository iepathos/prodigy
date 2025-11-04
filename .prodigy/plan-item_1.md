# Implementation Plan: Refactor WorkflowExecutor God Object

## Problem Summary

**Location**: ./src/cook/workflow/executor.rs:file:0
**Priority Score**: 90.12
**Debt Type**: God Object (GodClass)
**Current Metrics**:
- Lines of Code: 2354
- Functions: 93
- Cyclomatic Complexity: 237 (max: 28, avg: 2.55)
- Coverage: 0%
- Field Count: 25
- Method Count: 26
- Responsibilities: 8 (Computation, Formatting & Output, Data Access, Processing, Validation, Construction, Persistence, Utilities)

**Issue**: The WorkflowExecutor module exhibits a god object anti-pattern. While previous refactoring created 10 submodules (builder, commands, context, data_structures, failure_handler, orchestration, pure, step_executor, types, validation), the main executor.rs file still contains ~2354 lines with the core `WorkflowExecutor` struct having 25 fields and mixed responsibilities including git operations, commit handling, and MapReduce orchestration.

**Current Submodules**:
- builder.rs (500 lines)
- commands.rs (2213 lines) - largest submodule
- context.rs (606 lines)
- data_structures.rs (257 lines)
- failure_handler.rs (439 lines)
- orchestration.rs (341 lines)
- pure.rs (650 lines)
- step_executor.rs (873 lines)
- types.rs (156 lines) - already exists!
- validation.rs (958 lines)
- validation_tests.rs (785 lines)

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 47.4
- Maintainability Improvement: 9.01
- Test Effort Reduction: 235.4

**Success Criteria**:
- [ ] Extract git operations to dedicated module (reduce executor.rs by ~100 lines)
- [ ] Extract commit handling logic to dedicated module (reduce executor.rs by ~200 lines)
- [ ] Extract MapReduce orchestration to dedicated module (reduce executor.rs by ~270 lines)
- [ ] Reduce WorkflowExecutor field count from 25 to <15 by grouping related fields
- [ ] Executor.rs reduced to <1500 lines (from 2354)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Git Operations Module

**Goal**: Move all git-related operations from WorkflowExecutor to a new `git_support.rs` module. This includes methods that interact with git directly.

**Changes**:
- Create `src/cook/workflow/executor/git_support.rs`
- Move `get_current_head()` to git_support module (lines 1209-1218)
- Move `check_for_changes()` to git_support module (lines 1220-1229)
- Move `get_commits_between()` to git_support module (lines 1231-1296)
- Create `GitOperationsHelper` struct that takes `Arc<dyn GitOperations>` as dependency
- Add pure helper functions for git result processing
- Update WorkflowExecutor to use GitOperationsHelper for git operations
- Update module declaration in executor.rs to include git_support

**Testing**:
```bash
# Verify compilation
cargo build --lib

# Run git operation tests
cargo test --lib --test '*executor*' get_current_head
cargo test --lib --test '*executor*' get_commits_between

# Check for clippy warnings
cargo clippy --tests
```

**Success Criteria**:
- [ ] git_support.rs module created with ~120 lines
- [ ] GitOperationsHelper properly encapsulates git operations
- [ ] All git-related tests pass (get_current_head, get_commits_between tests)
- [ ] WorkflowExecutor delegates to GitOperationsHelper
- [ ] Executor.rs reduced by ~100 lines
- [ ] Ready to commit

### Phase 2: Extract Commit Handling Module

**Goal**: Move commit verification and squashing logic to a dedicated `commit_handler.rs` module to separate commit management from workflow orchestration.

**Changes**:
- Create `src/cook/workflow/executor/commit_handler.rs`
- Move `handle_commit_verification()` to commit_handler module (lines 445-514)
- Move `handle_commit_squashing()` to commit_handler module (lines 516-573)
- Move `handle_no_commits_error()` to commit_handler module (lines 1298-1352)
- Move `generate_commit_message()` helper if it exists
- Move `create_auto_commit()` helper if it exists
- Create `CommitHandler` struct that encapsulates commit-related operations
- Update WorkflowExecutor to use CommitHandler
- Update module declaration in executor.rs to include commit_handler

**Testing**:
```bash
# Verify compilation
cargo build --lib

# Run commit-related tests
cargo test --lib commit
cargo test --lib handle_commit_verification

# Run integration tests
cargo test --test '*workflow*'
```

**Success Criteria**:
- [ ] commit_handler.rs module created with ~220 lines
- [ ] CommitHandler properly encapsulates commit operations
- [ ] All commit-related tests pass
- [ ] WorkflowExecutor delegates to CommitHandler
- [ ] No regressions in commit tracking behavior
- [ ] Executor.rs reduced by ~200 lines (total: ~400 lines reduced)
- [ ] Ready to commit

### Phase 3: Extract MapReduce Orchestration Module

**Goal**: Move the large `execute_mapreduce()` method to a dedicated orchestrator in the mapreduce module structure, reducing the executor's responsibilities.

**Changes**:
- Analyze `execute_mapreduce()` method (lines 1354-1624, ~270 lines)
- Create `src/cook/execution/mapreduce/workflow_adapter.rs` (or similar location in mapreduce module)
- Move MapReduce setup, execution, and result aggregation logic
- Create `MapReduceWorkflowAdapter` struct that handles MapReduce-specific workflow execution
- Update WorkflowExecutor::execute_mapreduce() to delegate to the adapter
- Ensure proper environment context passing (immutable pattern per SPEC 128)
- Preserve worktree isolation guarantees (SPEC 127, 134)

**Testing**:
```bash
# Verify compilation
cargo build --lib

# Run MapReduce tests
cargo test --lib mapreduce

# Run full MapReduce integration tests
cargo test --test '*mapreduce*'

# Verify worktree behavior
cargo test --lib worktree
```

**Success Criteria**:
- [ ] MapReduceWorkflowAdapter created in appropriate mapreduce module
- [ ] execute_mapreduce() simplified to delegation call (~10-20 lines)
- [ ] All MapReduce tests pass
- [ ] Worktree isolation preserved (SPEC 127, 134)
- [ ] Environment context properly passed (SPEC 128)
- [ ] Executor.rs reduced by ~260 lines (total: ~660 lines reduced)
- [ ] Ready to commit

### Phase 4: Reduce WorkflowExecutor Field Count

**Goal**: Group related fields in the WorkflowExecutor struct to reduce field count from 25 to ~12, improving cohesion.

**Changes**:
- Analyze the 25 fields in WorkflowExecutor struct (lines 96-139)
- Identify groupings:
  - **Execution dependencies**: claude_executor, session_manager, user_interaction, command_registry, subprocess, git_operations, retry_state_manager
  - **Checkpoint state**: checkpoint_manager, workflow_id, checkpoint_completed_steps, workflow_path, current_workflow, current_step_index, resume_context
  - **Environment**: environment_manager, global_environment_config
  - **Runtime state**: completed_steps, timing_tracker, test_config, sensitive_config
  - **Dry-run state**: dry_run, assumed_commits, dry_run_commands, dry_run_validations, dry_run_potential_handlers
- Create `ExecutionDependencies` struct for core dependencies
- Create `CheckpointState` struct for checkpoint-related fields
- Create `DryRunState` struct for dry-run tracking
- Update WorkflowExecutor to use these grouped structs
- Update builder.rs to construct grouped structs

**Testing**:
```bash
# Verify compilation
cargo build --lib

# Run all executor tests
cargo test --lib executor

# Verify builder pattern
cargo test --lib builder

# Check for unused fields
cargo clippy --lib
```

**Success Criteria**:
- [ ] ExecutionDependencies, CheckpointState, DryRunState structs created
- [ ] WorkflowExecutor reduced to ~12 fields (from 25)
- [ ] All tests pass without modification
- [ ] Builder pattern updated and working correctly
- [ ] Code is more maintainable and fields are better organized
- [ ] No clippy warnings about unused fields
- [ ] Ready to commit

### Phase 5: Final Cleanup and Documentation

**Goal**: Clean up remaining code smells, update documentation, and verify all metrics have improved.

**Changes**:
- Review all delegated methods in executor.rs for opportunities to simplify
- Update module-level documentation in executor.rs to reflect new structure
- Add inline documentation for complex delegation patterns
- Ensure each submodule has clear documentation about its purpose
- Run full test suite and verify coverage
- Run `cargo clippy` and fix any warnings
- Run `cargo fmt` to ensure consistent formatting
- Generate new debtmap analysis to verify improvements

**Testing**:
```bash
# Full CI checks
just ci

# Coverage check (if available)
cargo tarpaulin --lib

# Complexity analysis
debtmap analyze src/cook/workflow/executor.rs

# Manual verification
wc -l src/cook/workflow/executor.rs
ls -lh src/cook/workflow/executor/*.rs
```

**Success Criteria**:
- [ ] All module documentation updated with clear purpose statements
- [ ] No clippy warnings
- [ ] Proper formatting applied
- [ ] Debtmap shows improvement:
  - God object score reduced
  - Complexity metrics improved
  - Maintainability score increased
- [ ] Executor.rs <1500 lines (target ~1400 after all extractions)
- [ ] Test coverage maintained or improved
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo build --lib` to verify compilation
2. Run phase-specific tests (see each phase's testing section)
3. Run `cargo clippy --tests` to check for warnings
4. Run `cargo fmt` to ensure formatting
5. Create commit with clear message explaining the refactoring

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Coverage verification
3. `debtmap analyze` - Complexity analysis
4. Compare metrics:
   - Before: 2354 lines, 93 functions, complexity 237, 25 fields
   - After: <1500 lines, ~80 functions, complexity <180, ~12 fields

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and compiler errors carefully
3. Adjust the approach:
   - If trait bounds are problematic, keep methods on executor with delegation
   - If tests fail, check for subtle state dependencies or missing imports
   - If integration breaks, verify environment context passing
   - If builder fails, ensure all grouped fields are properly initialized
4. Retry with adjusted approach
5. Consider splitting the phase into smaller sub-phases if needed

## Notes

### Key Insights from Analysis

1. **types.rs already exists**: A types.rs module already exists (156 lines) with some basic types. We should use it, not recreate it.

2. **commands.rs is the largest submodule**: At 2213 lines, commands.rs is larger than the main executor.rs and may benefit from future splitting, but that's outside this debt item's scope.

3. **Good progress already made**: The module structure is quite good with 10+ submodules. This plan focuses on extracting the remaining large methods from the impl blocks.

4. **Field grouping is key**: The 25-field WorkflowExecutor struct is the core issue. Grouping related fields will dramatically improve maintainability.

5. **Three large methods to extract**:
   - Git operations (~100 lines)
   - Commit handling (~200 lines)
   - MapReduce execution (~270 lines)

### Preservation Concerns

1. **Existing Tests**: All existing tests must pass without modification. This is pure refactoring.

2. **Git Operations**: Already abstracted behind GitOperations trait, so extraction should be straightforward.

3. **Async Complexity**: Several methods are async and require careful handling of trait bounds.

4. **Builder Pattern**: The builder.rs file constructs WorkflowExecutor. Field grouping changes must be reflected there.

5. **MapReduce Worktree Isolation**: SPEC 127 and 134 guarantee worktree isolation. Must preserve this behavior exactly.

6. **Resume Context**: Error recovery and resume functionality must continue to work correctly.

### Alternative Approaches Considered

1. **Full Rewrite**: Rejected - too risky, prefer incremental refactoring
2. **Split by Workflow Mode**: Rejected - would duplicate code between standard and MapReduce
3. **Extract to Traits**: Considered but rejected - adds complexity without clear benefit
4. **Combine with commands.rs refactoring**: Rejected - commands.rs is a separate debt item

### Success Indicators

- Executor.rs: 2354 lines → <1500 lines (~36% reduction)
- WorkflowExecutor fields: 25 → ~12 (~52% reduction)
- God object score: 1.0 → <0.7 (improved)
- Debtmap priority score: 90.12 → <60 (target)
- Maintainability: Easier to find and modify specific functionality
- Developer experience: Clear module boundaries and responsibilities

### Timeline Estimate

- Phase 1 (Git Ops): 2-3 hours
- Phase 2 (Commit Handling): 3-4 hours
- Phase 3 (MapReduce): 4-5 hours (most complex)
- Phase 4 (Field Grouping): 3-4 hours
- Phase 5 (Cleanup/Docs): 2-3 hours
- **Total**: 14-19 hours of focused work
