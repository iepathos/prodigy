# Implementation Plan: Refactor event_store.rs God Module

## Problem Summary

**Location**: ./src/cook/execution/events/event_store.rs:file:0
**Priority Score**: 50.41
**Debt Type**: GodModule
**Current Metrics**:
- Lines of Code: 2737 (706 production, 2031 tests)
- Functions: 66 total (11 production, 55 test functions)
- Cyclomatic Complexity: 153 total, avg 2.32
- Coverage: 0.0%
- God Object Score: 1.0 (maximum)

**Issue**: The file is a massive God Module combining multiple responsibilities: file I/O, event filtering, indexing, statistics aggregation, validation, and 2000+ lines of tests. The debtmap recommends: "URGENT: 2737 lines, 66 functions! Split by data flow: 1) Input/parsing functions 2) Core logic/transformation 3) Output/formatting. Create 3 focused modules with <30 functions each."

## Target State

**Expected Impact**:
- Complexity Reduction: 30.6 points
- Maintainability Improvement: 5.04 points
- Test Effort Reduction: 273.7 units

**Success Criteria**:
- [ ] Split production code into 3-4 focused modules (<200 lines each)
- [ ] Separate test code into dedicated test module
- [ ] All helper functions are pure (no side effects)
- [ ] Each module has a single, clear responsibility
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting (rustfmt)
- [ ] Test file reduced from 2031 lines to manageable size
- [ ] Production modules are <200 lines each

## Implementation Phases

### Phase 1: Extract Pure Index Logic

**Goal**: Extract pure index calculation and validation functions into a new `index.rs` module.

**Changes**:
1. Create `src/cook/execution/events/index.rs` with:
   - `EventIndex` struct (move from event_store.rs)
   - `FileOffset` struct (move from event_store.rs)
   - `calculate_time_range()` - pure function
   - `build_index_from_events()` - pure function
   - `update_time_range()` - pure function
   - `increment_event_count()` - pure function
   - `validate_job_id()` - pure validation
   - `validate_index_consistency()` - pure validation

2. Update `event_store.rs`:
   - Remove moved structs and functions
   - Add `use super::index::*;`
   - Update references to use new module

3. Update `mod.rs`:
   - Add `pub mod index;`
   - Re-export public types

**Testing**:
```bash
cargo test --lib cook::execution::events::index
cargo test --lib cook::execution::events::event_store
cargo clippy -- -D warnings
cargo fmt --check
```

**Success Criteria**:
- [ ] `index.rs` created with 8 functions (~150 lines)
- [ ] All index functions are pure (no I/O)
- [ ] Existing event_store tests pass
- [ ] No clippy warnings
- [ ] Production code in event_store.rs reduced by ~150 lines

### Phase 2: Extract I/O Operations

**Goal**: Extract file I/O operations into a new `io.rs` module.

**Changes**:
1. Create `src/cook/execution/events/io.rs` with:
   - `save_index()` - async I/O for saving index
   - `read_events_from_file_with_offsets()` - async file reading with offset tracking
   - File path utilities (if any)

2. Update `event_store.rs`:
   - Remove moved I/O functions
   - Add `use super::io::*;`
   - Keep only core EventStore trait implementation

3. Ensure all I/O functions handle errors properly:
   - Use `.context()` for all file operations
   - Return `Result<T>` consistently
   - No `unwrap()` or `panic!()`

**Testing**:
```bash
cargo test --lib cook::execution::events::io
cargo test --lib cook::execution::events::event_store
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] `io.rs` created with I/O functions (~100 lines)
- [ ] All I/O operations use proper error handling
- [ ] Event store implementation uses new io module
- [ ] All tests pass
- [ ] Production code in event_store.rs reduced by ~100 lines

### Phase 3: Extract Statistics and Filtering

**Goal**: Extract statistics aggregation and event filtering into focused modules.

**Changes**:
1. Create `src/cook/execution/events/stats.rs` with:
   - `EventStats` struct (move from event_store.rs)
   - Statistics aggregation logic (pure functions)
   - Time range calculations for stats

2. Create `src/cook/execution/events/filter.rs` with:
   - `EventFilter` struct (move from event_store.rs)
   - `matches_filter()` - pure predicate function
   - Filter validation functions

3. Update `event_store.rs`:
   - Remove moved structs and functions
   - Add imports for new modules
   - Use new modules in EventStore implementation

4. Update `mod.rs`:
   - Add `pub mod stats;`
   - Add `pub mod filter;`
   - Re-export public types

**Testing**:
```bash
cargo test --lib cook::execution::events::stats
cargo test --lib cook::execution::events::filter
cargo test --lib cook::execution::events::event_store
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] `stats.rs` created with statistics logic (~80 lines)
- [ ] `filter.rs` created with filtering logic (~60 lines)
- [ ] All functions are testable and focused
- [ ] All tests pass
- [ ] Production code in event_store.rs reduced by ~140 lines

### Phase 4: Reorganize Tests into Dedicated Module

**Goal**: Move the 2031 lines of tests out of event_store.rs into a focused test module.

**Changes**:
1. Create `src/cook/execution/events/event_store_tests.rs` with:
   - All tests from event_store.rs
   - Organized into logical groups:
     - Index tests
     - Validation tests
     - I/O tests
     - Query tests
     - Statistics tests

2. Update `event_store.rs`:
   - Remove `#[cfg(test)] mod tests { ... }` section
   - Keep file focused on production code only

3. Update test module structure:
   - Use `#[cfg(test)]` at module level
   - Import necessary types from parent modules
   - Use test helpers from `test_pure_functions.rs`

4. Ensure test organization:
   - Group related tests together
   - Use descriptive test names
   - Document complex test scenarios

**Testing**:
```bash
cargo test --lib cook::execution::events
cargo test event_store_tests
cargo clippy --tests
```

**Success Criteria**:
- [ ] `event_store_tests.rs` created with all 55 tests (~2000 lines)
- [ ] Tests are well-organized and grouped logically
- [ ] All tests pass
- [ ] event_store.rs is now <200 lines of production code
- [ ] Test code is maintainable and focused

### Phase 5: Final Verification and Documentation

**Goal**: Verify the refactoring is complete and documentation is updated.

**Changes**:
1. Verify module structure:
   ```
   src/cook/execution/events/
   ├── mod.rs                    # Module exports (~50 lines)
   ├── event_store.rs            # Core trait + impl (~200 lines)
   ├── index.rs                  # Index logic (~150 lines)
   ├── io.rs                     # I/O operations (~100 lines)
   ├── stats.rs                  # Statistics (~80 lines)
   ├── filter.rs                 # Filtering (~60 lines)
   ├── event_store_tests.rs      # Tests (~2000 lines)
   └── [existing modules...]
   ```

2. Update documentation:
   - Add module-level docs for each new file
   - Document public APIs
   - Add examples where appropriate

3. Run full verification:
   ```bash
   just ci                       # Full CI suite
   cargo tarpaulin               # Coverage check
   cargo clippy -- -D warnings   # Linting
   cargo fmt --check             # Formatting
   ```

4. Verify metrics:
   - Run `debtmap analyze` to confirm improvement
   - Check God Object score is reduced
   - Verify complexity reduction achieved

**Testing**:
```bash
just ci
cargo doc --no-deps --open
```

**Success Criteria**:
- [ ] All modules are focused and <200 lines
- [ ] All public APIs are documented
- [ ] Full CI suite passes
- [ ] God Object score reduced significantly
- [ ] Complexity metrics improved
- [ ] Code is more maintainable

## Testing Strategy

**For each phase**:
1. Run focused module tests: `cargo test --lib cook::execution::events::{module}`
2. Run all event tests: `cargo test --lib cook::execution::events`
3. Check formatting: `cargo fmt --check`
4. Check linting: `cargo clippy -- -D warnings`
5. Verify no regressions: `cargo test --lib`

**Final verification**:
1. `just ci` - Full CI suite (build, test, lint, format)
2. `cargo tarpaulin` - Generate coverage report
3. `cargo doc --no-deps` - Verify documentation builds
4. Manual review of new module structure

**Error Handling Verification**:
- No `unwrap()` or `panic!()` in production code
- All file operations use `.context()` or `.with_context()`
- All functions return `Result<T>` for operations that can fail
- Test error paths explicitly

## Rollback Plan

If a phase fails:
1. **Identify the issue**:
   - Review test failures
   - Check compiler errors
   - Analyze clippy warnings

2. **Revert the phase**:
   ```bash
   git reset --hard HEAD~1
   ```

3. **Adjust approach**:
   - Review the failing tests
   - Check for missing imports
   - Verify function signatures match
   - Ensure all types are properly exported

4. **Retry with fixes**:
   - Make targeted fixes
   - Re-run tests incrementally
   - Commit when stable

## Notes

### Why This Approach Works

1. **Incremental**: Each phase is independently valuable and testable
2. **Focused**: Each new module has a single, clear responsibility
3. **Safe**: All tests continue to pass after each phase
4. **Measurable**: Clear metrics for success (lines, complexity, tests)

### Key Principles

- **Separate I/O from logic**: Pure functions in `index.rs`, I/O in `io.rs`
- **Single responsibility**: Each module does one thing well
- **Testability**: Pure functions are easy to test
- **Error handling**: No unwrap(), proper Result propagation

### Potential Challenges

1. **Import cycles**: Ensure modules don't depend on each other circularly
2. **Type visibility**: Make sure structs are properly exported in mod.rs
3. **Test organization**: Tests may need to import from multiple modules
4. **Async boundaries**: Keep async code (I/O) separate from pure logic

### Expected Metrics After Refactoring

- **event_store.rs**: ~200 lines (down from 706)
- **Total production code**: ~640 lines across 6 focused modules
- **God Object score**: <0.5 (down from 1.0)
- **Average module size**: ~110 lines per module
- **Test organization**: Clear separation, easier to maintain
- **Complexity**: Reduced by targeting 30.6 point reduction

This plan addresses the God Module problem by splitting responsibilities into focused modules while maintaining all existing functionality and tests.
