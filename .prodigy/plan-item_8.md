# Implementation Plan: Reduce Complexity in ExpressionOptimizer::constant_folding

## Problem Summary

**Location**: ./src/cook/execution/expression/optimizer/mod.rs:ExpressionOptimizer::constant_folding:128
**Priority Score**: 32.5
**Debt Type**: ComplexityHotspot (Cyclomatic: 76, Cognitive: 127)
**Current Metrics**:
- Function Length: 169 lines
- Cyclomatic Complexity: 76
- Cognitive Complexity: 127
- Pattern Repetition: 0.978 (high repetition suggests extractable patterns)

**Issue**: The `constant_folding` method has cyclomatic complexity of 76 and cognitive complexity of 127, making it extremely difficult to test and maintain. The function handles too many Expression variant types in a single massive match statement.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 38.0 (from 76 to ~38)
- Coverage Improvement: 0.0
- Risk Reduction: 11.375

**Success Criteria**:
- [ ] Reduce cyclomatic complexity from 76 to ≤20
- [ ] Extract at least 4 focused helper functions
- [ ] Each helper function has cyclomatic complexity ≤10
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Logical Operators (And, Or, Not)

**Goal**: Extract boolean logic folding into a dedicated function to reduce complexity by ~10

**Changes**:
- Create `fold_logical_operators()` function in `folding.rs`
- Move And, Or, Not pattern matching from `constant_folding` to the new function
- Update `constant_folding` to delegate to `fold_logical_operators` for these cases
- Ensure recursive calls to `constant_folding` work correctly

**Testing**:
- Run `cargo test optimizer` to verify boolean logic still works
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] `fold_logical_operators()` function created and tested
- [ ] And/Or/Not cases removed from `constant_folding`
- [ ] All tests pass
- [ ] Complexity reduced by ~10
- [ ] Ready to commit

### Phase 2: Extract String Operators (Contains, StartsWith, EndsWith, Matches)

**Goal**: Extract string operation folding into a dedicated function to reduce complexity by ~8

**Changes**:
- Create `fold_string_operators()` function in `folding.rs`
- Move Contains, StartsWith, EndsWith, Matches pattern matching
- Update `constant_folding` to delegate string operations
- Maintain recursive folding of sub-expressions

**Testing**:
- Run `cargo test optimizer` to verify string operations
- Check that regex matching still works correctly

**Success Criteria**:
- [ ] `fold_string_operators()` function created
- [ ] String operation cases removed from `constant_folding`
- [ ] All tests pass
- [ ] Complexity reduced by ~8 additional points
- [ ] Ready to commit

### Phase 3: Extract Type Check Operators (IsNumber, IsString, IsBool, IsArray, IsObject)

**Goal**: Extract type checking folding into a dedicated function to reduce complexity by ~10

**Changes**:
- Create `fold_type_checks()` function in `folding.rs`
- Move IsNumber, IsString, IsBool, IsArray, IsObject pattern matching
- Consolidate with existing `fold_is_null` and `fold_is_not_null` functions
- Update `constant_folding` to delegate type checks

**Testing**:
- Run `cargo test optimizer` to verify type checks work
- Test edge cases for each type check

**Success Criteria**:
- [ ] `fold_type_checks()` function created
- [ ] Type check cases removed from `constant_folding`
- [ ] All tests pass
- [ ] Complexity reduced by ~10 additional points
- [ ] Ready to commit

### Phase 4: Extract Aggregate Functions (Length, Sum, Count, Min, Max, Avg)

**Goal**: Extract aggregate function folding into a dedicated function to reduce complexity by ~12

**Changes**:
- Create `fold_aggregate_functions()` function in `folding.rs`
- Move Length, Sum, Count, Min, Max, Avg pattern matching
- Update `constant_folding` to delegate aggregate functions
- Maintain recursive folding pattern

**Testing**:
- Run `cargo test optimizer` to verify aggregates
- Check that recursive folding still works for nested expressions

**Success Criteria**:
- [ ] `fold_aggregate_functions()` function created
- [ ] Aggregate function cases removed from `constant_folding`
- [ ] All tests pass
- [ ] Complexity reduced by ~12 additional points
- [ ] Ready to commit

### Phase 5: Extract Array/Index Operators (Index, ArrayWildcard)

**Goal**: Extract array access folding to complete the refactoring, reducing complexity by ~8

**Changes**:
- Create `fold_array_operators()` function in `folding.rs`
- Move Index and ArrayWildcard pattern matching
- Update `constant_folding` to delegate array operations
- Verify final complexity is ≤20

**Testing**:
- Run `cargo test optimizer` to verify all functionality
- Run `just ci` for full test suite
- Measure final cyclomatic complexity

**Success Criteria**:
- [ ] `fold_array_operators()` function created
- [ ] Array operator cases removed from `constant_folding`
- [ ] Final cyclomatic complexity ≤20
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Final Structure

The `constant_folding` function after Phase 5 should look like:

```rust
fn constant_folding(&mut self, expr: Expression) -> Result<Expression> {
    match expr {
        // Delegate to specialized folding functions
        Expression::And(_, _) | Expression::Or(_, _) | Expression::Not(_) => {
            fold_logical_operators(self, expr)
        }
        Expression::Equal(left, right) => {
            let left = self.constant_folding(*left)?;
            let right = self.constant_folding(*right)?;
            fold_equal_comparison(&mut self.stats, left, right)
        }
        Expression::NotEqual(left, right) => {
            let left = self.constant_folding(*left)?;
            let right = self.constant_folding(*right)?;
            fold_not_equal_comparison(&mut self.stats, left, right)
        }
        Expression::GreaterThan(_, _) | Expression::LessThan(_, _)
        | Expression::GreaterEqual(_, _) | Expression::LessEqual(_, _) => {
            fold_comparison_operators(self, expr)
        }
        Expression::Contains(_, _) | Expression::StartsWith(_, _)
        | Expression::EndsWith(_, _) | Expression::Matches(_, _) => {
            fold_string_operators(self, expr)
        }
        Expression::IsNull(_) | Expression::IsNotNull(_)
        | Expression::IsNumber(_) | Expression::IsString(_)
        | Expression::IsBool(_) | Expression::IsArray(_) | Expression::IsObject(_) => {
            fold_type_checks(self, expr)
        }
        Expression::Length(_) | Expression::Sum(_) | Expression::Count(_)
        | Expression::Min(_) | Expression::Max(_) | Expression::Avg(_) => {
            fold_aggregate_functions(self, expr)
        }
        Expression::Index(_, _) | Expression::ArrayWildcard(_, _) => {
            fold_array_operators(self, expr)
        }
        // Literals pass through
        _ => Ok(expr),
    }
}
```

This reduces the function to ~30 lines with a cyclomatic complexity of ~10.

## Testing Strategy

**For each phase**:
1. Run `cargo test optimizer` to verify optimizer tests pass
2. Run `cargo test --lib` to verify no regressions
3. Run `cargo clippy` to check for warnings
4. Manually inspect extracted function for clarity

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo clippy -- -D warnings` - Zero warnings
3. Visual inspection of `constant_folding` - should be simple dispatch function
4. Check line count: `constant_folding` should be ≤30 lines

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or clippy warning
3. Adjust the extraction strategy
4. Consider whether the function needs to remain in `mod.rs` vs `folding.rs`
5. Retry with updated approach

## Notes

### Key Insights:
- High pattern repetition (0.978) indicates the code is already well-structured for extraction
- The function is pure logic with no I/O, making it safe to refactor
- Each extracted function should take `&mut ExpressionOptimizer` and `Expression` and return `Result<Expression>`
- The folding functions need access to `optimizer.stats` for metrics tracking

### Function Signatures:
All extracted functions should follow this pattern:
```rust
pub(super) fn fold_xxx_operators(
    optimizer: &mut ExpressionOptimizer,
    expr: Expression,
) -> Result<Expression>
```

This allows them to:
- Access and update `optimizer.stats`
- Recursively call `optimizer.constant_folding()` for nested expressions
- Return optimized expressions

### Dependencies:
The extracted functions will be added to `folding.rs` and imported in `mod.rs` similar to the existing:
- `fold_equal_comparison`
- `fold_not_equal_comparison`
- `fold_numeric_comparison`
- `fold_is_null`
- `fold_is_not_null`

### Comparison Operators Note:
The comparison operators (GreaterThan, LessThan, GreaterEqual, LessEqual) are already extracted to `fold_numeric_comparison`, so we just need to ensure `constant_folding` delegates to it consistently.
