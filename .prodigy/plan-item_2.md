# Implementation Plan: Retire mod_old.rs God Object

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/mod_old.rs:MapReduceExecutor:882
**Priority Score**: 163.69723256426784
**Debt Type**: God Object / Excessive Complexity
**Current Metrics**:
- Lines of Code: 4027
- Functions: 95 (74 impl methods, 21+ tests)
- Cyclomatic Complexity: 295 total, 28 max, 3.1 avg
- Coverage: 0%
- God Object Score: 1.0 (maximum)
- Responsibilities: 8 distinct areas
- Fields: 25
- Methods: 77

**Issue**: The `mod_old.rs` file is a massive 4027-line God Object containing the old MapReduce implementation. A refactored implementation already exists in `mod.rs` with properly decomposed modules. The old file should be retired by:
1. Identifying any missing functionality not yet migrated
2. Ensuring all tests pass with the new implementation
3. Updating all imports to use the new modules
4. Removing the old file entirely

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 59.0 points
- Maintainability Improvement: 16.37 points
- Test Effort: 402.7 (currently 0% coverage in old file)

**Success Criteria**:
- [ ] All functionality from mod_old.rs is available in new modules
- [ ] All tests pass with new implementation
- [ ] No code references mod_old.rs
- [ ] File mod_old.rs is deleted
- [ ] Code coverage maintained or improved
- [ ] All existing integration tests pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

This is a **retirement** project, not a refactoring project. The new modular implementation already exists. Our job is to safely transition away from the old code.

### Phase 1: Analyze Migration Gap

**Goal**: Identify any functionality in mod_old.rs that hasn't been migrated to the new modular structure

**Changes**:
- Compare public API of mod_old.rs vs mod.rs
- Check for any unique functions/types only in mod_old.rs
- Verify all imports in the codebase
- Document any missing pieces

**Testing**:
- Run `cargo build` to identify compile errors
- Run `grep -r "mod_old" src/` to find references
- Check for any integration tests specifically using mod_old

**Success Criteria**:
- [ ] Complete list of functions only in mod_old.rs
- [ ] List of files importing from mod_old
- [ ] Documentation of migration status
- [ ] Ready to proceed with migration

### Phase 2: Migrate Missing Functionality (if any)

**Goal**: Move any remaining unique functionality from mod_old.rs to appropriate new modules

**Changes**:
- For each unique function/type identified:
  - Determine correct target module based on responsibility
  - Copy implementation to new module
  - Add tests for the migrated code
  - Update visibility as needed
- Update re-exports in mod.rs if needed

**Testing**:
- Run `cargo test --lib` for each migrated piece
- Verify function signatures match
- Check that all dependencies are satisfied

**Success Criteria**:
- [ ] All unique functionality migrated
- [ ] Tests pass for migrated code
- [ ] No functionality lost
- [ ] Ready to update imports

### Phase 3: Update Imports

**Goal**: Change all imports from mod_old to the new modular structure

**Changes**:
- Find all `use crate::cook::execution::mapreduce::mod_old` imports
- Replace with appropriate new module imports
- Update any type paths (e.g., `mod_old::MapReduceExecutor` → `MapReduceExecutor`)
- Fix any visibility issues that arise

**Testing**:
- Run `cargo build` after each file's imports are updated
- Run `cargo test` to verify no behavioral changes
- Check that all integration tests still pass

**Success Criteria**:
- [ ] No references to mod_old remain
- [ ] All code compiles
- [ ] All tests pass
- [ ] Ready to remove old file

### Phase 4: Remove mod_old.rs

**Goal**: Delete the old God Object file and clean up module declarations

**Changes**:
- Remove `pub mod mod_old;` declaration from parent module
- Delete `src/cook/execution/mapreduce/mod_old.rs`
- Update any documentation references
- Clean up any dev comments mentioning the old file

**Testing**:
- Run `cargo build` - should succeed
- Run `cargo test --all` - all tests should pass
- Run `cargo clippy` - no warnings about missing modules
- Run `just ci` - full CI validation

**Success Criteria**:
- [ ] File deleted
- [ ] Build succeeds
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Git shows clean removal

### Phase 5: Verification & Cleanup

**Goal**: Final verification that the retirement is complete and code quality is maintained

**Changes**:
- Run full test suite
- Check code coverage with tarpaulin
- Run clippy with all warnings
- Verify formatting
- Update CHANGELOG if applicable

**Testing**:
- `cargo test --all --verbose`
- `cargo tarpaulin --lib --out Stdout`
- `cargo clippy -- -W clippy::all`
- `cargo fmt -- --check`
- `just ci`

**Success Criteria**:
- [ ] All tests pass (100% of existing tests)
- [ ] Coverage maintained or improved from baseline
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Documentation updated
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify library tests pass
2. Run `cargo build` to check compilation
3. Run `cargo clippy` to check for warnings
4. Commit working code at end of each phase

**Integration testing**:
- Test MapReduce workflows end-to-end
- Verify resume functionality works
- Test DLQ operations
- Verify checkpoint/restore
- Test parallel agent execution

**Final verification**:
1. `just ci` - Full CI checks including:
   - Build
   - All tests
   - Clippy
   - Format check
2. `cargo tarpaulin --lib` - Verify coverage
3. Manual smoke test of key workflows

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure reason
3. If it's a missing migration:
   - Go back to Phase 2
   - Migrate the missing piece
   - Retry the failed phase
4. If it's an incompatibility:
   - Document the issue
   - Create a bridge/adapter if needed
   - Retry with the fix

## Notes

### Key Success Factors

1. **Don't rush**: The old code is 4000+ lines. Take time to verify completeness
2. **Test continuously**: Run tests after every significant change
3. **One file at a time**: When updating imports, do one file at a time and test
4. **Verify equivalence**: Make sure new code does exactly what old code did

### Common Pitfalls to Avoid

1. **Assuming complete migration**: Some edge case might only be in old code
2. **Breaking APIs**: Make sure public interfaces remain compatible
3. **Losing test coverage**: Verify tests exercise new code paths
4. **Visibility issues**: Private functions in old code might need to be pub in new modules

### Module Mapping (from analysis)

The new modular structure already has:
- `agent/` - Agent lifecycle and management
- `aggregation/` - Result aggregation
- `checkpoint.rs` - State persistence
- `command/` - Command execution
- `coordination/` - Work scheduling and orchestration
- `phases/` - Phase execution
- `progress/` - Progress tracking
- `resources/` - Resource management
- `state/` - State management
- `utils.rs` - Pure utility functions
- `types.rs` - Type definitions

This is a well-designed decomposition following functional programming principles.

### Expected Outcome

After completion:
- 4027 lines of legacy code removed
- God Object eliminated
- 8 focused modules with single responsibilities
- Improved testability
- Better maintainability
- Foundation for future development

### Estimated Impact

- **Complexity Reduction**: 59.0 points (massive)
- **Maintainability**: +16.37 points
- **Code Organization**: From 1 file (4027 lines) to ~15 focused modules (<500 lines each)
- **Testability**: From god object to isolated, testable units
