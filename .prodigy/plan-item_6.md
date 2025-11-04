# Implementation Plan: Modularize Data Pipeline Test File

## Problem Summary

**Location**: ./src/cook/execution/data_pipeline.rs:file:0
**Priority Score**: 52.04
**Debt Type**: File-level - Large test file with low modularity
**Current Metrics**:
- Lines of Code: 2843
- Functions: 133
- Cyclomatic Complexity: 321 (avg 2.41 per function, max 22)
- Coverage: 0% (test code)
- File Type: Unit Test Code

**Issue**: Large test file (2843 lines) with mixed concerns should be split into focused test modules. The file contains comprehensive tests for JSON path parsing, filtering, sorting, and data pipeline operations, but all tests are in a single monolithic file making it difficult to navigate and maintain.

## Target State

**Expected Impact**:
- Complexity Reduction: 64.2 points
- Maintainability Improvement: 5.2 points
- Test Effort: 284.3 (already test code, no new tests needed)

**Success Criteria**:
- [ ] File reduced from 2843 lines to <500 lines (main module)
- [ ] Tests split into logical sub-modules by feature area
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Clear module organization reflecting the data pipeline components

## Implementation Phases

### Phase 1: Analyze and Plan Module Structure

**Goal**: Understand the current test organization and design the module split strategy

**Changes**:
- Analyze the file to identify distinct testing domains:
  - JsonPath tests (compilation, selection, recursive descent, filters)
  - FilterExpression tests (comparison, logical operators, functions, IN expressions)
  - Sorter tests (field parsing, sorting logic, null handling)
  - DataPipeline tests (integration, processing, deduplication, field mapping)
- Create module structure plan
- Document which tests go into which modules

**Testing**:
- No code changes yet, only analysis
- Verify current tests pass: `cargo test data_pipeline`

**Success Criteria**:
- [ ] Clear mapping of test functions to modules documented
- [ ] Module structure designed (4-5 focused modules)
- [ ] All current tests passing

### Phase 2: Extract JsonPath Tests to Submodule

**Goal**: Move all JsonPath-related tests to a dedicated submodule

**Changes**:
- Create `tests/json_path.rs` submodule within the test module
- Move ~30-40 JsonPath test functions to the new module
- Keep imports and test helpers minimal and focused
- Ensure tests can access necessary types from parent module

**Testing**:
- Run `cargo test json_path` to verify extracted tests pass
- Run `cargo test data_pipeline` to ensure nothing broke

**Success Criteria**:
- [ ] JsonPath tests isolated in dedicated submodule
- [ ] All JsonPath tests passing
- [ ] Main file reduced by ~400-600 lines
- [ ] Ready to commit

### Phase 3: Extract FilterExpression Tests to Submodule

**Goal**: Move all FilterExpression-related tests to a dedicated submodule

**Changes**:
- Create `tests/filter_expression.rs` submodule
- Move ~40-50 FilterExpression test functions including:
  - Comparison tests
  - Logical operator tests (AND, OR, NOT)
  - IN expression tests
  - Function tests
  - Complex nested expression tests
- Keep test data fixtures organized

**Testing**:
- Run `cargo test filter_expression` to verify extracted tests pass
- Run `cargo test data_pipeline` to ensure all tests still pass

**Success Criteria**:
- [ ] FilterExpression tests isolated in dedicated submodule
- [ ] All FilterExpression tests passing
- [ ] Main file reduced by additional ~500-800 lines
- [ ] Ready to commit

### Phase 4: Extract Sorter Tests to Submodule

**Goal**: Move all Sorter-related tests to a dedicated submodule

**Changes**:
- Create `tests/sorter.rs` submodule
- Move ~15-20 Sorter test functions including:
  - Sort field parsing tests
  - Sort order tests (ASC/DESC)
  - Null position handling tests
  - Multi-field sorting tests
- Organize test fixtures for sorting scenarios

**Testing**:
- Run `cargo test sorter` to verify extracted tests pass
- Run `cargo test data_pipeline` to ensure all tests still pass

**Success Criteria**:
- [ ] Sorter tests isolated in dedicated submodule
- [ ] All Sorter tests passing
- [ ] Main file reduced by additional ~200-400 lines
- [ ] Ready to commit

### Phase 5: Organize DataPipeline Integration Tests and Finalize

**Goal**: Keep integration tests in main module, finalize structure

**Changes**:
- Keep ~20-30 DataPipeline integration tests in main `tests` module
- Add module documentation explaining the structure
- Add `mod` declarations for the submodules at the top of the test module
- Verify main file is now <500 lines
- Run final formatting and linting

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Run `cargo clippy` to check for warnings
- Run `cargo fmt` to ensure formatting

**Success Criteria**:
- [ ] Main test module <500 lines
- [ ] Clear module structure with 4 focused test submodules:
  - `tests/json_path.rs`
  - `tests/filter_expression.rs`
  - `tests/sorter.rs`
  - Main `tests` module for integration tests
- [ ] All 133 tests still passing
- [ ] No clippy warnings
- [ ] Properly formatted
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib data_pipeline` to verify existing tests pass before changes
2. After extraction, run `cargo test --lib <module_name>` to verify moved tests work
3. Run `cargo test --lib data_pipeline` again to ensure nothing broke
4. Run `cargo clippy -- -D warnings` to catch any issues
5. Run `cargo fmt` to ensure consistent formatting

**Final verification**:
1. `cargo test --lib` - All tests pass (133 tests should still pass)
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Properly formatted
4. Verify file size: `wc -l src/cook/execution/data_pipeline.rs` - Should be <500 lines

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure - likely import or visibility issues
3. Adjust the module structure (make types pub(crate) if needed)
4. Retry the extraction

## Notes

**Key Considerations**:
- This is test code, so we're not changing production behavior
- The goal is better organization, not new functionality
- Module boundaries should follow domain boundaries (JsonPath, Filter, Sort, Pipeline)
- Some shared test utilities may need to be accessible across modules
- Rust test module organization: use `#[cfg(test)] mod tests { mod submodule { ... } }`

**Module Structure Pattern**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests stay here

    mod json_path {
        use super::*;
        // JsonPath tests
    }

    mod filter_expression {
        use super::*;
        // FilterExpression tests
    }

    mod sorter {
        use super::*;
        // Sorter tests
    }
}
```

**Why This Matters**:
- 2843 lines is difficult to navigate and maintain
- Splitting by domain makes tests easier to find
- Reduces cognitive load when working on specific features
- Follows single-responsibility principle at module level
- Makes it easier to add new tests in the right place
