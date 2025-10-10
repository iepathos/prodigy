# Implementation Plan: Reduce Cognitive Complexity in CookSessionAdapter::update_session

## Problem Summary

**Location**: ./src/unified_session/cook_adapter.rs:CookSessionAdapter::update_session:184
**Priority Score**: 48.64
**Debt Type**: ComplexityHotspot (cognitive: 17, cyclomatic: 5)
**Current Metrics**:
- Lines of Code: 26
- Function Length: 26
- Cognitive Complexity: 17
- Cyclomatic Complexity: 5
- Coverage: Heavily tested (27 upstream callers)

**Issue**: While cyclomatic complexity of 5 is manageable, the cognitive complexity of 17 indicates nested logic and debug logging that makes the function harder to understand. The function has excessive debug logging statements (7 debug calls) interspersed with business logic, creating mental overhead when reading the code.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 2.5
- Coverage Improvement: 0.0 (already well-tested)
- Risk Reduction: 17.02

**Success Criteria**:
- [ ] Cognitive complexity reduced from 17 to ≤10
- [ ] Cyclomatic complexity maintained at ≤5
- [ ] All 27 existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Debug logging separated from business logic

## Implementation Phases

### Phase 1: Extract Debug Logging to a Tracing Span

**Goal**: Replace inline debug! calls with a structured tracing span that automatically logs function entry/exit and key operations.

**Changes**:
- Add `#[tracing::instrument(skip(self))]` attribute to the function
- Remove the 7 inline `debug!()` calls
- Add strategic span events for the update loop only

**Testing**:
- Run `cargo test --lib -- cook_adapter` to verify all adapter tests pass
- Run `cargo clippy` to check for warnings
- Verify logging behavior with `RUST_LOG=debug cargo test test_update_session_with_timing -- --nocapture`

**Success Criteria**:
- [ ] Function has instrument attribute
- [ ] Inline debug calls removed (except for critical state transitions)
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Pure Function for Update Conversion and Application

**Goal**: Separate the pure logic of converting and applying updates from the I/O operations (locking, async calls).

**Changes**:
- Extract a pure function: `fn apply_unified_updates(manager: &UnifiedSessionManager, id: &SessionId, updates: Vec<UnifiedSessionUpdate>) -> impl Future<Output = Result<()>>`
- Move the update loop into this new function
- Keep only the essential I/O operations in `update_session`: lock acquisition, conversion, delegating to pure function, cache update

**Testing**:
- Run `cargo test --lib -- cook_adapter` to verify all adapter tests pass
- Unit test the new pure function with mock scenarios
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] New pure function extracted and tested
- [ ] `update_session` is simplified to: lock → convert → apply → update cache
- [ ] All existing tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Simplify Early Return Pattern

**Goal**: Use early return to reduce nesting and improve readability.

**Changes**:
- Replace `if let Some(id) = ...` with early return guard clause:
  ```rust
  let Some(id) = &*self.current_session.lock().await else {
      return Ok(());
  };
  ```
- This eliminates one level of nesting and makes the happy path clearer

**Testing**:
- Run `cargo test --lib -- cook_adapter::test_update_session_no_active_session` specifically
- Run full test suite: `cargo test --lib -- cook_adapter`
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] Early return pattern implemented
- [ ] Nesting depth reduced by 1 level
- [ ] All tests pass (especially no-active-session case)
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib -- cook_adapter` to verify all 12+ adapter tests pass
2. Run `cargo clippy -- -D warnings` to catch any issues
3. Run `cargo fmt` to ensure proper formatting
4. Review the specific test cases affected:
   - Phase 1: `test_update_session_with_timing` (checks logging)
   - Phase 2: All sequential update tests
   - Phase 3: `test_update_session_no_active_session`

**Final verification**:
1. `just ci` - Full CI checks
2. Review cognitive complexity improvement (target: 17 → ≤10)
3. Verify all 27 upstream callers still work correctly

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure details
3. Adjust the implementation approach:
   - Phase 1: Consider keeping minimal debug logging if tracing span isn't sufficient
   - Phase 2: Ensure async boundaries are correct (await placement)
   - Phase 3: Verify early return doesn't skip cache updates
4. Retry with adjustments

## Notes

**Why this function?**
- High cognitive complexity (17) despite moderate cyclomatic complexity (5)
- The mismatch suggests readability issues from logging noise
- Already has excellent test coverage (27 callers)
- Pure logic can be extracted without breaking the adapter pattern

**Key insights from code analysis**:
- The 7 debug! calls create cognitive load without adding complexity
- The nested `if let` can be simplified with early return
- The update loop is pure logic that can be extracted
- The function is essentially: lock → convert → apply → cache update

**Refactoring approach**:
- Use tracing spans instead of inline debug calls (reduces cognitive load)
- Extract the update application loop (pure function)
- Simplify control flow with early returns (reduces nesting)
- Keep the adapter pattern intact (no architectural changes)
