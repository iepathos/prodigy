# Implementation Plan: Refactor Worktree CLI Command Module

## Problem Summary

**Location**: ./src/cli/commands/worktree/cli.rs:file:0
**Priority Score**: 62.43
**Debt Type**: God Object (File-Level)
**Current Metrics**:
- Lines of Code: 449
- Functions: 8
- Cyclomatic Complexity: 95 (avg 11.875 per function, max 27)
- Coverage: 0.0%

**Issue**: Large CLI command handler file mixing input parsing, business logic orchestration, formatting, and I/O. Contains 7 distinct command handlers with varying complexity, making the module difficult to test and maintain. The file exhibits god object characteristics with multiple responsibilities bundled into a single module.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 19.0 points
- Maintainability Improvement: 6.24 points
- Test Effort: 44.9 points

**Success Criteria**:
- [ ] File split into focused modules with single responsibilities
- [ ] Each function under 30 lines with complexity < 10
- [ ] Pure business logic extracted and independently testable
- [ ] I/O and formatting separated from logic
- [ ] Test coverage > 70% for business logic modules
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract MapReduce Cleanup Logic

**Goal**: Separate MapReduce-specific cleanup operations into dedicated module

**Changes**:
- Create new module `src/cli/commands/worktree/mapreduce_cleanup.rs`
- Move `run_mapreduce_cleanup()` function (lines 243-314)
- Extract configuration building logic into pure functions
- Separate path resolution from cleanup coordination
- Update `cli.rs` to use new module

**Rationale**: This function (72 lines) handles a distinct responsibility (MapReduce worktree cleanup) with its own dependencies and logic. Separating it reduces `cli.rs` complexity and enables targeted testing.

**Testing**:
- Unit tests for path resolution logic
- Unit tests for configuration building
- Integration tests for cleanup coordination (existing patterns)
- Run `cargo test --lib` to verify no regressions

**Success Criteria**:
- [ ] `run_mapreduce_cleanup()` moved to new module
- [ ] Path resolution logic is pure and testable
- [ ] Config building logic is pure and testable
- [ ] cli.rs imports and calls new module correctly
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Orphaned Worktree Cleanup Logic

**Goal**: Separate orphaned worktree cleanup into dedicated module

**Changes**:
- Create new module `src/cli/commands/worktree/orphaned_cleanup.rs`
- Move `run_worktree_clean_orphaned()` function (lines 317-449)
- Extract registry file discovery into pure function
- Extract worktree display formatting into pure function
- Separate user confirmation logic from cleanup logic
- Update `cli.rs` to use new module

**Rationale**: This function (133 lines, complexity 27) is the most complex in the file. It handles registry file I/O, user interaction, and worktree cleanup. Extracting it significantly reduces file complexity.

**Testing**:
- Unit tests for registry file discovery logic
- Unit tests for formatting functions
- Mock-based tests for cleanup operations
- Run `cargo test --lib` to verify no regressions

**Success Criteria**:
- [ ] `run_worktree_clean_orphaned()` moved to new module
- [ ] Registry discovery logic is pure and testable
- [ ] Formatting logic is pure and testable
- [ ] cli.rs imports and calls new module correctly
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Age-Based Cleanup Logic

**Goal**: Separate age-based worktree cleanup into dedicated module

**Changes**:
- Create new module `src/cli/commands/worktree/age_cleanup.rs`
- Move `cleanup_old_worktrees()` function (lines 204-240)
- Extract age calculation into pure function
- Extract worktree filtering logic into pure function
- Separate display formatting from cleanup operations
- Update `cli.rs` to use new module

**Rationale**: This function (37 lines) handles time-based filtering, a distinct concern from other cleanup operations. Extracting it improves modularity.

**Testing**:
- Unit tests for age calculation logic
- Unit tests for worktree filtering by age
- Mock-based tests for cleanup operations
- Run `cargo test --lib` to verify no regressions

**Success Criteria**:
- [ ] `cleanup_old_worktrees()` moved to new module
- [ ] Age calculation logic is pure and testable
- [ ] Filtering logic is pure and testable
- [ ] cli.rs imports and calls new module correctly
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Refactor Main Command Handlers

**Goal**: Simplify remaining command handler functions by extracting pure logic

**Changes in `run_worktree_clean()`**:
- Extract parameter validation logic into pure function
- Extract routing logic into pure decision functions
- Reduce function to simple orchestration of operations
- Target: < 30 lines, complexity < 5

**Changes in `run_worktree_merge()`**:
- Extract result formatting into pure function
- Extract error message construction into pure function
- Separate success/failure logic paths
- Target: < 30 lines, complexity < 5

**Changes in `run_worktree_ls()`**:
- Extract table formatting into pure function
- Extract session display logic into pure function
- Target: < 25 lines, complexity < 3

**Rationale**: These handlers currently mix orchestration with formatting and decision logic. Extracting pure functions makes the handlers simple, testable orchestrators.

**Testing**:
- Unit tests for all extracted pure functions
- Unit tests for parameter validation
- Unit tests for formatting functions
- Integration tests verify handler orchestration
- Run `cargo test --lib` to verify no regressions

**Success Criteria**:
- [ ] All handlers under 30 lines
- [ ] All handlers with complexity < 5
- [ ] Pure functions extracted and tested
- [ ] Formatting logic is reusable
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Add Comprehensive Test Coverage

**Goal**: Achieve >70% test coverage for all new modules and refactored logic

**Test Additions**:
- `tests/cli/commands/worktree/mapreduce_cleanup_tests.rs`
  - Path resolution tests
  - Configuration building tests
  - Edge cases (missing $HOME, invalid repo names)

- `tests/cli/commands/worktree/orphaned_cleanup_tests.rs`
  - Registry file discovery tests
  - Multi-job registry handling tests
  - Formatting tests
  - Edge cases (empty registry, missing files)

- `tests/cli/commands/worktree/age_cleanup_tests.rs`
  - Age calculation tests
  - Filtering logic tests
  - Edge cases (zero age, negative durations)

- `tests/cli/commands/worktree/formatting_tests.rs`
  - Table formatting tests
  - Result display tests
  - Error message construction tests

**Rationale**: Testing pure logic is straightforward and provides confidence in refactoring. Test coverage validates that behavior is preserved.

**Testing**:
- Run `cargo test --lib` for all new tests
- Run `cargo tarpaulin` to verify coverage >70%
- Run `just ci` for full validation

**Success Criteria**:
- [ ] All new modules have test files
- [ ] Test coverage >70% for business logic
- [ ] All edge cases covered
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write tests for extracted pure functions first (TDD approach)
2. Run `cargo test --lib` after each extraction
3. Run `cargo clippy` to check for warnings
4. Run `cargo fmt` to ensure formatting
5. Verify no behavior changes via integration tests

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --out Html` - Generate coverage report
3. `cargo clippy -- -D warnings` - Zero warnings
4. Review overall file structure and module organization

**Coverage targets by module**:
- `mapreduce_cleanup.rs`: >80% (mostly pure logic)
- `orphaned_cleanup.rs`: >70% (some I/O)
- `age_cleanup.rs`: >80% (mostly pure logic)
- `cli.rs`: >60% (orchestration, harder to test)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and error messages
3. Check if the issue is:
   - Logic error: Fix the extracted code
   - Integration error: Fix the imports/calls
   - Test error: Fix the test assumptions
4. Adjust the plan if necessary
5. Retry the phase

If multiple phases fail consecutively:
1. Stop and reassess the approach
2. Consider smaller extraction steps
3. Get additional context on the codebase patterns
4. Resume with adjusted plan

## Post-Implementation Validation

After all phases complete:

1. **Complexity Metrics**:
   - Run debtmap again to verify score improvement
   - Expected: Score reduction of ~19 points
   - Target: No function with complexity >10

2. **Coverage Metrics**:
   - Run `cargo tarpaulin --out Html`
   - Expected: Coverage improvement from 0% to >70%
   - Review uncovered lines for gaps

3. **Code Quality**:
   - Run `cargo clippy -- -D warnings`
   - Run `cargo fmt --check`
   - Review for any remaining `unwrap()` calls (per Spec 101)

4. **Module Structure**:
   - Verify clear module boundaries
   - Check for circular dependencies
   - Ensure public APIs are minimal

## Notes

**Key Design Principles**:
- **Separation of Concerns**: I/O at boundaries, pure logic in core
- **Single Responsibility**: Each module handles one aspect of worktree management
- **Testability**: Pure functions are easy to test without mocks
- **Progressive Refactoring**: Each phase is independently valuable

**Architecture After Refactoring**:
```
src/cli/commands/worktree/
├── cli.rs (orchestrator, ~100 lines)
├── mapreduce_cleanup.rs (MapReduce cleanup logic)
├── orphaned_cleanup.rs (orphaned worktree cleanup)
├── age_cleanup.rs (time-based cleanup)
├── operations.rs (existing operations)
└── utils.rs (existing utilities)
```

**Potential Challenges**:
- **Error Handling**: Ensure all error paths preserve context (use `.context()` per Spec 101)
- **Dependencies**: Some functions have dependencies on WorktreeManager; use trait bounds for testability
- **User Interaction**: stdin/stdout interactions are harder to test; keep them thin wrappers

**Future Improvements** (out of scope for this debt item):
- Add integration tests with real worktree operations
- Consider extracting formatting into a shared formatting module
- Add builder pattern for complex configuration objects
