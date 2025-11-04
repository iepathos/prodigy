# Implementation Plan: Refactor data_pipeline.rs - Extract Modules and Reduce File Size

## Problem Summary

**Location**: ./src/cook/execution/data_pipeline.rs:file:0
**Priority Score**: 52.19
**Debt Type**: File-level complexity - Oversized test file
**Current Metrics**:
- Lines of Code: 2859
- Functions: 133
- Cyclomatic Complexity: 321 (avg: 2.41, max: 22)
- Coverage: 0% (test file - coverage measured externally)

**Issue**: This file is a massive test file (2859 lines) that combines multiple concerns:
1. Data pipeline orchestration (`DataPipeline` struct)
2. JSON path parsing and evaluation (`JsonPath` implementation)
3. Filter expression parsing and evaluation (`FilterExpression` implementation)
4. Sorting logic (`Sorter` implementation)
5. Extensive test suite (starting at line 1462, ~1400 lines of tests)

The recommendation is to extract complex functions and reduce the file to under 500 lines by breaking it into focused, single-responsibility modules.

## Target State

**Expected Impact**:
- Complexity Reduction: 64.2 points
- Maintainability Improvement: 5.22 points
- Reduced Test Effort: 285.9 points saved

**Success Criteria**:
- [ ] File size reduced from 2859 lines to under 800 lines (main module)
- [ ] Extract JsonPath to separate module (~300 lines)
- [ ] Extract FilterExpression to separate module (~700 lines)
- [ ] Extract Sorter to separate module (~200 lines)
- [ ] All 133 tests continue to pass
- [ ] No clippy warnings
- [ ] Proper module organization under `src/cook/execution/data_pipeline/`

## Implementation Phases

This refactoring will be done in 5 incremental phases, each extracting a focused component to its own module.

### Phase 1: Create Module Structure and Extract JsonPath

**Goal**: Extract the JSON path parsing and evaluation logic to a dedicated module.

**Changes**:
1. Create `src/cook/execution/data_pipeline/` directory
2. Create `src/cook/execution/data_pipeline/json_path.rs` with:
   - `JsonPath` struct and implementation (~200 lines)
   - `PathComponent` enum
   - `PathPart` enum (helper for parsing)
   - All JSON path related helper functions
3. Create `src/cook/execution/data_pipeline/mod.rs` as new entry point
4. Update main `data_pipeline.rs` to re-export from the new module structure
5. Move relevant test helper functions to the json_path module

**Testing**:
- Run `cargo test --lib data_pipeline` to verify all tests pass
- Run `cargo clippy -- -D warnings` to ensure no new warnings

**Success Criteria**:
- [ ] `src/cook/execution/data_pipeline/json_path.rs` exists with ~200-300 lines
- [ ] All JSON path tests pass (grep for `test.*json.*path` in test section)
- [ ] No build errors or clippy warnings
- [ ] Code compiles and imports work correctly

### Phase 2: Extract FilterExpression to Separate Module

**Goal**: Extract the filter expression parsing and evaluation logic to its own module.

**Changes**:
1. Create `src/cook/execution/data_pipeline/filter.rs` with:
   - `FilterExpression` enum (~700 lines total)
   - `ComparisonOp` enum
   - `LogicalOp` enum
   - All parsing logic (parse, try_parse_* methods)
   - All evaluation logic (evaluate method)
   - Helper functions for operator detection and value parsing
2. Update `mod.rs` to include and re-export filter module
3. Update imports in main data_pipeline module

**Testing**:
- Run `cargo test --lib data_pipeline::filter` for filter-specific tests
- Run full test suite to ensure integration works
- Verify complex filter expressions still work

**Success Criteria**:
- [ ] `filter.rs` exists with ~700 lines (the bulk of the parsing logic)
- [ ] All filter tests pass (grep for `test.*filter` patterns)
- [ ] Complex expressions (AND/OR/NOT/IN/functions) still work
- [ ] No clippy warnings
- [ ] Clean module boundaries with clear public API

### Phase 3: Extract Sorter to Separate Module

**Goal**: Extract sorting logic to its own focused module.

**Changes**:
1. Create `src/cook/execution/data_pipeline/sorter.rs` with:
   - `Sorter` struct (~200 lines)
   - `SortField` struct
   - `SortOrder` enum
   - `NullPosition` enum
   - All parsing and comparison logic
2. Update `mod.rs` to include sorter module
3. Move sorting-related helper functions

**Testing**:
- Run sorter-specific tests
- Test multi-field sorting
- Test null handling in various positions
- Verify DESC/ASC ordering works correctly

**Success Criteria**:
- [ ] `sorter.rs` exists with ~200 lines
- [ ] All sorting tests pass
- [ ] Multi-field sort works correctly
- [ ] Null position handling verified
- [ ] No clippy warnings

### Phase 4: Reorganize DataPipeline Core and Tests

**Goal**: Clean up the main data_pipeline module and organize tests by module.

**Changes**:
1. Keep only `DataPipeline` struct and its core methods in `mod.rs`:
   - `from_config`
   - `from_full_config`
   - `process`
   - `process_streaming`
   - Helper methods (deduplicate, apply_field_mapping, extract_field_value)
2. Move tests to module-specific test files:
   - `json_path/tests.rs` - JSON path tests
   - `filter/tests.rs` - Filter expression tests
   - `sorter/tests.rs` - Sorter tests
   - Keep integration tests in main `mod.rs`
3. Update module declarations to include test submodules

**Testing**:
- Run full test suite: `cargo test --lib data_pipeline`
- Verify test count remains the same (133 tests)
- Check that integration tests still work correctly

**Success Criteria**:
- [ ] Main `mod.rs` reduced to ~300-400 lines (core orchestration only)
- [ ] Tests organized by module (easier to navigate)
- [ ] All 133 tests still pass
- [ ] Test output shows clear module organization
- [ ] Integration tests verify end-to-end pipeline functionality

### Phase 5: Final Cleanup and Documentation

**Goal**: Polish the refactored modules, add documentation, and verify quality.

**Changes**:
1. Add module-level documentation to each file:
   - Explain module purpose
   - Document public API
   - Add usage examples
2. Review and improve function documentation
3. Ensure consistent error messages and context
4. Run full CI checks
5. Update any outdated comments

**Testing**:
- Run `just ci` for full CI validation
- Run `cargo doc --no-deps --open` to verify documentation
- Run `cargo clippy -- -D warnings` for strict linting
- Spot-check complex test cases manually

**Success Criteria**:
- [ ] All modules have comprehensive rustdoc comments
- [ ] Public API is well-documented with examples
- [ ] CI passes cleanly (no warnings, all tests pass)
- [ ] Documentation builds without errors
- [ ] Code follows Rust API guidelines
- [ ] No TODO comments or dead code

## Testing Strategy

**For each phase**:
1. Run module-specific tests: `cargo test --lib data_pipeline::<module>`
2. Run full data_pipeline tests: `cargo test --lib data_pipeline`
3. Check for warnings: `cargo clippy -- -D warnings`
4. Verify formatting: `cargo fmt --check`

**Final verification**:
1. `just ci` - Full CI checks (build, test, clippy, fmt)
2. `cargo test --all` - Ensure no regressions in other modules
3. Manual spot-check of complex scenarios:
   - Nested filter expressions with AND/OR/NOT
   - Multi-field sorting with nulls
   - Recursive JSON path descent
   - Large dataset processing

**Test organization validation**:
- Verify test count: `cargo test --lib data_pipeline -- --list | wc -l` should show ~133 tests
- Check module coverage: Tests should be distributed across modules
- Integration tests should remain in main mod.rs

## Rollback Plan

If a phase fails:
1. **Identify the failure**: Check compiler errors, test failures, or clippy warnings
2. **Revert the phase**: `git reset --hard HEAD~1` to undo the phase commit
3. **Analyze the issue**:
   - Missing imports or exports?
   - Visibility issues (pub vs private)?
   - Test dependencies not moved?
   - Circular dependencies?
4. **Adjust the plan**: Update the phase plan based on the failure
5. **Retry with fixes**: Implement the corrected approach

**Common issues to watch for**:
- Forgetting to make types/functions `pub` when moving to a module
- Missing re-exports in `mod.rs`
- Test helpers that need to be moved with tests
- Circular dependencies between modules
- Use of internal types in public APIs

## Notes

**File Structure After Refactoring**:
```
src/cook/execution/data_pipeline/
├── mod.rs              (~300-400 lines - DataPipeline core + integration tests)
├── json_path.rs        (~200-300 lines - JSON path parsing/evaluation)
├── filter.rs           (~700 lines - Filter expression parsing/evaluation)
└── sorter.rs           (~200 lines - Sorting logic)
```

**Total line reduction**: From 2859 lines in one file to ~1400-1600 lines across 4 focused modules, with much better organization.

**Key principles for this refactoring**:
1. **No behavior changes**: This is purely structural - all tests must pass
2. **Clear module boundaries**: Each module has a single, well-defined responsibility
3. **Preserve test coverage**: All 133 tests move with their corresponding code
4. **Incremental commits**: Each phase is independently valuable and committable

**Why this is better**:
- **Navigability**: Easier to find specific functionality
- **Testing**: Faster to run module-specific test suites
- **Maintenance**: Changes are scoped to relevant modules
- **Compile times**: Incremental compilation benefits from smaller modules
- **Cognitive load**: Each module is easier to understand in isolation
