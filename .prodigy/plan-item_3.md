# Implementation Plan: Split Expression Optimizer God Module

## Problem Summary

**Location**: ./src/cook/execution/expression/optimizer.rs:file:0
**Priority Score**: 63.90
**Debt Type**: God Object (God Module)
**Current Metrics**:
- Lines of Code: 1701
- Functions: 70
- Cyclomatic Complexity: 210 (avg 3.0, max 76)
- Coverage: 0.0% (tests are in same file as production code)

**Issue**: This file contains both production code (lines 1-858) and all unit tests (lines 859-1701) in a single 1701-line module. The debtmap analysis correctly identifies this as a God Module that needs to be split into focused modules. The primary issue is not just size, but the mixing of production code with 842 lines of test code in the same file.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 42.0 points
- Maintainability Improvement: 6.39 points
- Test Effort: 170.1 (complexity of testing after split)

**Success Criteria**:
- [ ] Production code separated from test code
- [ ] Tests moved to proper test directory following Rust conventions
- [ ] Helper functions extracted into focused utility modules
- [ ] All existing tests continue to pass unchanged
- [ ] No clippy warnings
- [ ] Proper module structure with clear boundaries
- [ ] Code remains functionally identical (pure refactoring)

## Implementation Phases

This refactoring will be done in 5 incremental phases, each independently testable and committable.

### Phase 1: Extract Comparison Folding Functions

**Goal**: Extract the comparison folding helper functions into a dedicated `folding` submodule to reduce the main file size and improve organization.

**Changes**:
1. Create new file: `src/cook/execution/expression/optimizer/folding.rs`
2. Move these functions to the new module:
   - `fold_equal_comparison` (lines 595-628)
   - `fold_not_equal_comparison` (lines 630-659)
   - `NumericComparisonOp` enum (lines 661-668)
   - `fold_numeric_comparison` (lines 670-706)
   - `fold_is_null` (lines 708-721)
   - `fold_is_not_null` (lines 723-736)
3. Make functions `pub(crate)` or `pub(super)` as needed
4. Update imports in main optimizer.rs
5. Create `src/cook/execution/expression/optimizer/mod.rs` if needed

**Testing**:
- Run `cargo test --lib optimizer` to ensure all tests pass
- Verify the functions are accessible from the main optimizer

**Success Criteria**:
- [ ] ~142 lines moved to new folding.rs module
- [ ] All optimizer tests pass unchanged
- [ ] No clippy warnings
- [ ] Code compiles successfully

### Phase 2: Extract Expression Utility Functions

**Goal**: Extract the expression hashing and equality utility functions into a utilities submodule.

**Changes**:
1. Create new file: `src/cook/execution/expression/optimizer/utils.rs`
2. Move these functions to the new module:
   - `hash_expression` (lines 738-745)
   - `hash_expression_recursive` (lines 747-811)
   - `expressions_equal` (lines 813-857)
3. Make functions `pub(crate)` or `pub(super)` as needed
4. Update imports in main optimizer.rs

**Testing**:
- Run `cargo test --lib optimizer` to ensure all tests pass
- Verify CSE and algebraic simplification still work

**Success Criteria**:
- [ ] ~120 lines moved to new utils.rs module
- [ ] All optimizer tests pass unchanged
- [ ] No clippy warnings
- [ ] Code compiles successfully

### Phase 3: Move Tests to Separate Test Module

**Goal**: Move all unit tests out of the production code file into a proper test module, following Rust best practices.

**Changes**:
1. Create new directory: `src/cook/execution/expression/optimizer/tests/`
2. Move test module (lines 859-1701) to `tests/mod.rs` or split into focused test files:
   - `tests/constant_folding_tests.rs` - Basic constant folding tests
   - `tests/comparison_tests.rs` - Comparison operator tests
   - `tests/type_check_tests.rs` - Type checking tests
   - `tests/aggregate_tests.rs` - Aggregate function tests
   - `tests/optimization_tests.rs` - Full optimization pipeline tests
3. Update test imports to use `super::*` or specific imports
4. Remove `#[cfg(test)]` block from optimizer.rs

**Testing**:
- Run `cargo test --lib optimizer` to ensure all tests still pass
- Verify test organization matches module structure

**Success Criteria**:
- [ ] All 842 lines of tests moved to separate test files
- [ ] All 70 test functions pass unchanged
- [ ] Production code file reduced to ~859 lines
- [ ] No clippy warnings
- [ ] Clear test organization by functionality

### Phase 4: Extract Cache-Related Code

**Goal**: Extract the sub-expression cache implementation into its own module for better separation of concerns.

**Changes**:
1. Create new file: `src/cook/execution/expression/optimizer/cache.rs`
2. Move these structures and implementations:
   - `SubExpressionCache` struct (lines 46-53)
   - `SubExpressionCache` impl (lines 55-82)
   - `CachedExpression` struct (lines 84-93)
3. Make types `pub(crate)` as needed
4. Update optimizer.rs to use the new cache module

**Testing**:
- Run `cargo test --lib optimizer` to ensure CSE tests pass
- Verify cache functionality is unchanged

**Success Criteria**:
- [ ] ~48 lines moved to cache.rs module
- [ ] All optimizer tests pass unchanged
- [ ] No clippy warnings
- [ ] Clear separation of cache concerns

### Phase 5: Organize Module Structure and Documentation

**Goal**: Finalize the module organization with proper exports, documentation, and a clean public API.

**Changes**:
1. Update `src/cook/execution/expression/optimizer/mod.rs`:
   - Re-export public types: `OptimizerConfig`, `OptimizationStats`, `ExpressionOptimizer`
   - Keep internal modules private: `folding`, `utils`, `cache`
   - Add module-level documentation
2. Verify the public API remains unchanged:
   - `ExpressionOptimizer::new()`
   - `ExpressionOptimizer::with_config()`
   - `ExpressionOptimizer::optimize()`
   - All configuration and stats types
3. Add inline documentation for internal modules
4. Run full test suite and CI checks

**Testing**:
- Run `cargo test --all` to ensure all tests pass
- Run `cargo clippy` to check for warnings
- Run `just ci` for full CI validation
- Verify public API accessibility from other modules

**Success Criteria**:
- [ ] Clean module structure with clear boundaries
- [ ] Public API unchanged and fully accessible
- [ ] All tests pass (70 test functions)
- [ ] No clippy warnings
- [ ] Module documentation complete
- [ ] Ready to commit final changes

## Final Module Structure

After all phases:

```
src/cook/execution/expression/optimizer/
├── mod.rs                    # Main optimizer, ~500 lines
├── folding.rs                # Comparison folding functions, ~142 lines
├── utils.rs                  # Expression utilities, ~120 lines
├── cache.rs                  # Sub-expression cache, ~48 lines
└── tests/
    ├── mod.rs                # Test module setup
    ├── constant_folding_tests.rs
    ├── comparison_tests.rs
    ├── type_check_tests.rs
    ├── aggregate_tests.rs
    └── optimization_tests.rs
```

**Total reduction**: From 1 file (1701 lines) to 9 focused modules (avg ~190 lines/module for production code)

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib optimizer` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Verify code compiles: `cargo build`
4. Manual inspection of module boundaries

**Final verification**:
1. `cargo test --all` - All tests pass
2. `cargo clippy --all-targets` - No warnings
3. `cargo build --release` - Production build succeeds
4. `just ci` - Full CI checks pass
5. Verify optimizer can still be imported and used from other modules

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the compilation or test errors
3. Adjust the module boundaries or imports
4. Retry the phase with fixes

**Common issues to watch for**:
- Import visibility (pub vs pub(crate) vs private)
- Circular dependencies between modules
- Missing re-exports in mod.rs
- Test imports not finding the right symbols

## Notes

**Why this approach**:
- **Phase 1-2**: Extract helper functions first to reduce complexity gradually
- **Phase 3**: Moving tests is the biggest win (reduces file size by 50%)
- **Phase 4**: Cache extraction further separates concerns
- **Phase 5**: Ensures clean final state with proper documentation

**Important considerations**:
- This is pure refactoring - no behavior changes
- All 70 existing tests must pass after each phase
- Module boundaries follow natural function grouping
- Public API remains unchanged to avoid breaking other code

**Alignment with debtmap recommendation**:
The debtmap suggested splitting by "data flow" into 3 modules. This plan refines that to 5 modules based on actual responsibility analysis:
1. **Main optimizer** (mod.rs) - Core optimization logic and orchestration
2. **Folding** - Constant folding and comparison helpers
3. **Utils** - Expression hashing and equality checking
4. **Cache** - Sub-expression caching infrastructure
5. **Tests** - All unit tests in organized submodules

This creates focused modules with <30 functions each (main has 13 methods, folding has 6 functions, utils has 3 functions, cache has 4 methods).
