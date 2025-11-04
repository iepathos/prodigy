# Implementation Plan: Refactor Checkpoint Manager Module

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/checkpoint/manager.rs:file:0
**Priority Score**: 57.28
**Debt Type**: God Object (File-level)
**Current Metrics**:
- Lines of Code: 2283
- Functions: 73
- Cyclomatic Complexity: 177 (avg 2.42 per function, max 13)
- Coverage: 0%
- God Object Score: 1.0 (Critical severity)
- Responsibilities: 7 (Construction, Computation, Persistence, Filtering & Selection, Data Access, Validation, Utilities)
- Struct Ratio: 0.96 (28 structs/enums, 3 implementations)

**Issue**: Critical god class with CheckpointManager having 22 methods across 7 responsibilities in a single 2283-line file. The module mixes data structures (28 structs/enums), core checkpoint management logic, storage implementation, compression algorithms, and extensive test code. This violates single responsibility principle and makes the code difficult to maintain, test, and understand.

## Target State

**Expected Impact**:
- Complexity Reduction: 35.4 points
- Maintainability Improvement: 5.73 points
- Test Effort Reduction: 228.3 lines

**Success Criteria**:
- [ ] Split module into focused sub-modules (<500 lines each)
- [ ] Separate data structures from implementation logic
- [ ] Extract compression logic to dedicated module
- [ ] Move CheckpointManager to focused manager module
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`
- [ ] Each module has a clear, single responsibility

## Implementation Phases

### Phase 1: Extract Data Structures (Types Module)

**Goal**: Extract all data structure definitions into a dedicated `types.rs` module

**Changes**:
- Create `src/cook/execution/mapreduce/checkpoint/types.rs`
- Move 28 structs/enums with their basic trait implementations:
  - `CheckpointId` (with Display, Default implementations)
  - `MapReduceCheckpoint`, `CheckpointMetadata`
  - `CheckpointReason`, `PhaseType`
  - `ExecutionState`, `PhaseResult`, `MapPhaseResults`
  - `WorkItemState`, `WorkItem`, `WorkItemProgress`, `CompletedWorkItem`, `FailedWorkItem`, `WorkItemBatch`
  - `AgentState`, `AgentInfo`, `ResourceAllocation`
  - `VariableState`, `ResourceState`, `ErrorState`, `DlqItem`
  - `CheckpointConfig`, `RetentionPolicy`
  - `ResumeState`, `ResumeStrategy`, `CheckpointInfo`
- Keep only basic implementations (Display, Default, From)
- Update `manager.rs` to import from `types` module
- Update `mod.rs` to re-export public types

**Testing**:
- Run `cargo test --lib` to verify no breakage
- Run `cargo clippy` to check for issues
- Run `cargo fmt` for formatting

**Success Criteria**:
- [ ] `types.rs` created with ~250 lines
- [ ] All data structures moved and properly documented
- [ ] All tests pass without modification
- [ ] Clean compilation with no warnings

### Phase 2: Extract Storage Implementation

**Goal**: Move checkpoint storage logic to dedicated `storage.rs` module

**Changes**:
- Create `src/cook/execution/mapreduce/checkpoint/storage.rs`
- Move storage-related code:
  - `CheckpointStorage` trait definition
  - `CompressionAlgorithm` enum with compress/decompress methods (~90 lines)
  - `FileCheckpointStorage` struct and implementation (~200 lines)
- Add proper module documentation
- Update `manager.rs` imports
- Update `mod.rs` to expose storage types

**Testing**:
- Run `cargo test --lib checkpoint::storage` to verify storage tests
- Verify compression algorithm tests pass
- Check file operations work correctly

**Success Criteria**:
- [ ] `storage.rs` created with ~350 lines
- [ ] Storage trait and implementations cleanly separated
- [ ] All storage-related tests pass
- [ ] Compression tests pass for all algorithms (None, Gzip, Zstd, Lz4)

### Phase 3: Extract Manager Core Logic

**Goal**: Keep only core checkpoint management logic in `manager.rs`

**Changes**:
- Refactor `CheckpointManager` to focus on core responsibilities:
  - Checkpoint creation and metadata management
  - Resume state building
  - Checkpoint validation
  - Retention policy application
- Move reduce phase checkpoint methods to a new `reduce_checkpoint.rs` helper:
  - `save_reduce_checkpoint()`
  - `load_reduce_checkpoint()`
  - `can_resume_reduce()`
  - `get_reduce_checkpoint_dir()`
- Extract complex helper methods to pure functions:
  - `select_checkpoints_for_deletion()` → pure function
  - `prepare_work_items_for_resume()` → pure function with pattern matching
  - `determine_resume_strategy()` → pure function based on phase
- Reduce `manager.rs` to ~400-500 lines of focused logic

**Testing**:
- Run full test suite: `cargo test --lib checkpoint`
- Verify checkpoint creation/loading works
- Verify resume strategies work correctly
- Verify retention policies function properly

**Success Criteria**:
- [ ] `manager.rs` reduced to ~400-500 lines
- [ ] `reduce_checkpoint.rs` created with ~150 lines
- [ ] All core checkpoint operations work
- [ ] All resume strategy tests pass
- [ ] All validation tests pass

### Phase 4: Reorganize and Document Tests

**Goal**: Move tests to a dedicated test module and improve organization

**Changes**:
- Create `src/cook/execution/mapreduce/checkpoint/tests/` directory
- Split tests into focused files:
  - `tests/checkpoint_ops.rs` - checkpoint creation, loading, listing
  - `tests/resume_strategies.rs` - resume strategy tests
  - `tests/storage.rs` - storage and compression tests
  - `tests/validation.rs` - validation and integrity tests
  - `tests/retention.rs` - retention policy tests
- Move helper functions (`create_test_checkpoint()`, `create_test_checkpoint_with_work_items()`) to `tests/helpers.rs`
- Keep integration-style tests that verify cross-module behavior
- Reduce duplication in test setup

**Testing**:
- Run `cargo test --lib checkpoint` to verify all tests still pass
- Ensure no tests were lost in the migration
- Verify test organization improves clarity

**Success Criteria**:
- [ ] Tests moved to dedicated `tests/` directory (~800 lines)
- [ ] `manager.rs` no longer contains test code
- [ ] All tests pass in new organization
- [ ] Test helper functions properly shared

### Phase 5: Final Integration and Verification

**Goal**: Ensure all modules integrate properly and document the new structure

**Changes**:
- Update `src/cook/execution/mapreduce/checkpoint/mod.rs`:
  - Add proper module documentation explaining the organization
  - Re-export public types from appropriate modules
  - Document the purpose of each sub-module
- Add module-level documentation to each file:
  - `types.rs` - "Checkpoint data structures and state types"
  - `storage.rs` - "Checkpoint storage implementations and compression"
  - `manager.rs` - "Core checkpoint management and coordination"
  - `reduce_checkpoint.rs` - "Reduce phase checkpoint utilities"
- Run full verification:
  - `cargo test --all` - all tests pass
  - `cargo clippy -- -D warnings` - no warnings
  - `cargo fmt --check` - proper formatting
  - `just ci` - full CI checks

**Testing**:
- Run complete test suite across all modules
- Verify no functionality regression
- Check that public API remains stable
- Validate documentation coverage

**Success Criteria**:
- [ ] All modules properly documented
- [ ] Module structure clearly explained in `mod.rs`
- [ ] All tests pass (100% of original tests)
- [ ] No clippy warnings
- [ ] Proper formatting throughout
- [ ] Public API unchanged (no breaking changes)

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib checkpoint` after changes
2. Run `cargo clippy` to catch potential issues
3. Run `cargo fmt` to ensure consistent formatting
4. Verify compilation with `cargo check`

**Final verification**:
1. Run `just ci` - Full CI checks
2. Run `cargo tarpaulin` - Regenerate coverage (should improve from 0%)
3. Run `debtmap analyze` - Verify god object score improvement
4. Compare before/after metrics:
   - File sizes should be <500 lines each
   - Cyclomatic complexity should be distributed
   - Struct ratio should normalize across modules

## Rollback Plan

If a phase fails:
1. Use `git diff` to review changes
2. Revert the phase with `git restore <files>` or `git reset --hard HEAD~1` if committed
3. Review the failure and adjust approach
4. Consider breaking the phase into smaller steps
5. Retry with refined plan

## Module Structure (After Refactoring)

```
src/cook/execution/mapreduce/checkpoint/
├── mod.rs                    (~50 lines - module organization and re-exports)
├── types.rs                  (~250 lines - all data structures)
├── storage.rs                (~350 lines - storage trait and implementations)
├── manager.rs                (~450 lines - core checkpoint management)
├── reduce_checkpoint.rs      (~150 lines - reduce phase helpers)
└── tests/
    ├── checkpoint_ops.rs     (~200 lines)
    ├── resume_strategies.rs  (~150 lines)
    ├── storage.rs            (~150 lines)
    ├── validation.rs         (~150 lines)
    ├── retention.rs          (~100 lines)
    └── helpers.rs            (~50 lines)

Total: ~1900 lines (vs 2283 original)
Reduction: ~383 lines (16.7%) through test deduplication and better organization
```

## Notes

- **Preserve all functionality**: This is a refactoring, not a rewrite. Every test must continue to pass.
- **No breaking changes**: The public API (`CheckpointManager`, public types) must remain stable.
- **Incremental commits**: Commit after each successful phase to enable easy rollback.
- **Focus on separation of concerns**: Each module should have one clear purpose.
- **Improve testability**: Extracted pure functions are easier to test independently.
- **Documentation is critical**: Each module needs clear documentation explaining its role.
