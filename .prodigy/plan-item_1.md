# Implementation Plan: Improve Coverage and Refactor ExpressionOptimizer::constant_folding

## Problem Summary

**Location**: ./src/cook/execution/expression/optimizer.rs:ExpressionOptimizer::constant_folding:178
**Priority Score**: 24.77
**Debt Type**: TestingGap (77% coverage gap with high complexity)

**Current Metrics**:
- Lines of Code: 251
- Cyclomatic Complexity: 95
- Cognitive Complexity: 172
- Coverage: 23.77% (93 uncovered lines out of 251)

**Issue**: Complex pure business logic with 77% coverage gap. Cyclomatic complexity of 95 requires at least 95 test cases for full path coverage. The function handles constant folding for 20+ expression types with deeply nested match patterns. Current tests only cover 4 basic scenarios (AND, OR, NOT, double negation), leaving 73 of 95 branches uncovered.

## Target State

**Expected Impact**:
- Complexity Reduction: 28.5 (from 95 to ~67)
- Coverage Improvement: 38.11% (from 23.77% to ~62%)
- Risk Reduction: 10.41

**Success Criteria**:
- [ ] Test coverage for `constant_folding` increases to 60%+ (cover 73+ uncovered branches)
- [ ] Extract 10-15 pure helper functions with complexity ≤3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting via `cargo fmt`
- [ ] Function complexity reduced by at least 25%

## Implementation Phases

### Phase 1: Add Tests for Comparison Operators (Lines 228-337)

**Goal**: Cover the 6 comparison operators (Equal, NotEqual, GreaterThan, LessThan, GreaterEqual, LessEqual) which have 0% coverage currently.

**Changes**:
- Add tests for `Equal` with Number/String/Boolean/Null constants (lines 228-258)
- Add tests for `NotEqual` with Number/String/Boolean constants (lines 261-287)
- Add tests for numeric comparisons: GreaterThan, LessThan, GreaterEqual, LessEqual (lines 290-337)
- Add tests for expressions_equal optimization in Equal/NotEqual (lines 251-253, 280-282)

**Testing**:
```rust
#[test]
fn test_constant_folding_equal_numbers() { /* ... */ }

#[test]
fn test_constant_folding_equal_strings() { /* ... */ }

#[test]
fn test_constant_folding_not_equal() { /* ... */ }

#[test]
fn test_constant_folding_greater_than() { /* ... */ }

#[test]
fn test_constant_folding_comparisons() { /* ... */ }
```

**Success Criteria**:
- [ ] 15+ new test cases covering lines 228-337
- [ ] Coverage for comparison operators reaches 80%+
- [ ] All tests pass with `cargo test`
- [ ] Ready to commit

### Phase 2: Add Tests for Type Checking Operators (Lines 339-367)

**Goal**: Cover IsNull and IsNotNull type checking which currently have 0% coverage.

**Changes**:
- Add tests for `IsNull` with Null constant (line 343-345)
- Add tests for `IsNull` with non-null constants (lines 347-349)
- Add tests for `IsNotNull` with Null (lines 357-359)
- Add tests for `IsNotNull` with non-null constants (lines 361-363)

**Testing**:
```rust
#[test]
fn test_constant_folding_is_null() {
    // Test IsNull(Null) => true
    // Test IsNull(Number) => false
    // Test IsNull(String) => false
}

#[test]
fn test_constant_folding_is_not_null() {
    // Test IsNotNull(Null) => false
    // Test IsNotNull(Boolean) => true
}
```

**Success Criteria**:
- [ ] 8+ test cases covering lines 339-367
- [ ] Coverage for type checking reaches 90%+
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Add Tests for String and Pattern Operators (Lines 369-393)

**Goal**: Cover Contains, StartsWith, EndsWith, Matches, Index, and ArrayWildcard which have 0% coverage.

**Changes**:
- Add tests for `Contains` recursive folding (lines 370-373)
- Add tests for `StartsWith` recursive folding (lines 374-377)
- Add tests for `EndsWith` recursive folding (lines 378-381)
- Add tests for `Matches` recursive folding (lines 382-385)
- Add tests for `Index` recursive folding (lines 386-389)
- Add tests for `ArrayWildcard` recursive folding (lines 390-393)

**Testing**:
```rust
#[test]
fn test_constant_folding_string_operations() {
    // Test Contains with nested constants
    // Test StartsWith with boolean folding inside
    // Test EndsWith with comparison folding inside
}

#[test]
fn test_constant_folding_index_operations() {
    // Test Index with constant expressions
    // Test ArrayWildcard with folding
}
```

**Success Criteria**:
- [ ] 12+ test cases covering lines 369-393
- [ ] Coverage for string/pattern operators reaches 80%+
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Add Tests for Aggregate and Type Check Functions (Lines 395-422)

**Goal**: Cover Length, Sum, Count, Min, Max, Avg and type check functions which have 0% coverage.

**Changes**:
- Add tests for `Length` recursive folding (lines 396-398)
- Add tests for `Sum`, `Count`, `Min`, `Max`, `Avg` (lines 399-405)
- Add tests for `IsNumber`, `IsString`, `IsBool`, `IsArray`, `IsObject` (lines 407-422)

**Testing**:
```rust
#[test]
fn test_constant_folding_aggregate_functions() {
    // Test Length with constant expressions
    // Test Sum/Count/Min/Max/Avg with nested folding
}

#[test]
fn test_constant_folding_type_checks() {
    // Test IsNumber/IsString/IsBool/IsArray/IsObject
    // with recursive constant folding
}
```

**Success Criteria**:
- [ ] 12+ test cases covering lines 395-422
- [ ] Coverage for aggregate functions reaches 80%+
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Extract Pure Helper Functions - Comparison Logic

**Goal**: Reduce complexity by extracting comparison operator logic into pure helper functions.

**Changes**:
- Extract `fold_equal_comparison(left, right) -> Result<Expression>`
- Extract `fold_not_equal_comparison(left, right) -> Result<Expression>`
- Extract `fold_numeric_comparison(op, left, right) -> Result<Expression>`
- Update `constant_folding` to call extracted functions

**Before (lines 228-337)**:
```rust
Expression::Equal(left, right) => {
    let left = self.constant_folding(*left)?;
    let right = self.constant_folding(*right)?;
    match (&left, &right) {
        // 30 lines of matching...
    }
}
```

**After**:
```rust
Expression::Equal(left, right) => {
    let left = self.constant_folding(*left)?;
    let right = self.constant_folding(*right)?;
    fold_equal_comparison(&mut self.stats, left, right)
}
```

**Testing**:
- Existing tests should continue to pass
- Add unit tests for each extracted function

**Success Criteria**:
- [ ] 3 new pure functions extracted, each with complexity ≤3
- [ ] Each helper function has 3-5 dedicated unit tests
- [ ] All existing tests pass
- [ ] Cyclomatic complexity reduced by ~15 points
- [ ] Ready to commit

### Phase 6: Extract Pure Helper Functions - Type Checking Logic

**Goal**: Extract type checking and aggregate function logic to reduce complexity further.

**Changes**:
- Extract `fold_is_null(inner) -> Result<Expression>`
- Extract `fold_is_not_null(inner) -> Result<Expression>`
- Extract `fold_aggregate_function(fn_type, inner) -> Result<Expression>`
- Extract `fold_type_check(check_type, inner) -> Result<Expression>`
- Update `constant_folding` to call extracted functions

**Testing**:
- Existing tests should continue to pass
- Add unit tests for each extracted function

**Success Criteria**:
- [ ] 4 new pure functions extracted, each with complexity ≤3
- [ ] Each helper function has 3-5 dedicated unit tests
- [ ] All existing tests pass
- [ ] Cyclomatic complexity reduced by another ~10 points
- [ ] Ready to commit

### Phase 7: Final Integration and Validation

**Goal**: Verify all improvements, run full test suite, and generate new coverage report.

**Changes**:
- Run full test suite: `cargo test --lib`
- Run clippy: `cargo clippy --all-targets`
- Format code: `cargo fmt`
- Generate coverage: `cargo tarpaulin --lib`
- Verify debtmap improvement

**Testing**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Verify coverage improvement
3. Manual review of extracted functions for clarity

**Success Criteria**:
- [ ] All tests pass (existing + ~60 new tests)
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Coverage for `constant_folding` reaches 60%+
- [ ] Cyclomatic complexity reduced to ~67 or lower
- [ ] Ready for final commit

## Testing Strategy

**For each phase (1-4)**:
1. Write tests first (TDD approach)
2. Run `cargo test optimizer::tests::test_constant_folding*` to verify new tests
3. Check coverage of specific lines with `cargo tarpaulin --lib`
4. Commit after each phase with clear message

**For refactoring phases (5-6)**:
1. Extract one function at a time
2. Run full test suite after each extraction
3. Add unit tests for extracted function
4. Run clippy to ensure no new warnings
5. Commit after extracting each function

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Regenerate coverage report
3. Verify coverage improvement in optimizer.rs
4. Check that complexity metrics improved

## Rollback Plan

If a phase fails:
1. Review the error message carefully
2. Revert the phase with `git reset --hard HEAD~1`
3. Break the phase into smaller sub-phases if needed
4. Retry with adjusted approach

If tests fail after refactoring:
1. Verify extracted function logic matches original
2. Check that stats counting is preserved
3. Ensure recursive calls are maintained
4. Add debug logging if needed to trace the issue

## Notes

**Key Insights from Code Analysis**:
- The function is already pure (no I/O, just transformations)
- Stats tracking (`self.stats.constants_folded += 1`) must be preserved in extracted functions
- Many branches follow similar patterns (binary operators, unary operators)
- Extracted functions can be module-level pure functions (not methods)
- The function uses recursive descent pattern - extracted helpers must maintain this

**Extraction Strategy**:
- Group related match arms (e.g., all comparisons, all type checks)
- Pass `&mut stats` to extracted functions to preserve tracking
- Keep extracted functions small (≤20 lines, complexity ≤3)
- Use descriptive names that match the operation (fold_equal_comparison, etc.)

**Coverage Priority**:
- Focus on uncovered branches first (lines 210, 220-422)
- Test both the "fold to constant" path AND the "no fold" path
- Include edge cases (e.g., NaN handling, epsilon comparisons)
- Test recursive folding (nested expressions)

**Complexity Reduction Math**:
- Current: 95 cyclomatic complexity
- After extracting 7 functions with ~13 branches each: 95 - (7 × 13) + 7 = 11
- Main function calls 7 helpers (adds 7), helpers are extracted (removes ~91)
- Target: Main function ~30, helpers ~3 each = Total managed complexity significantly reduced
