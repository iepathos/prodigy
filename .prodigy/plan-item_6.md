# Implementation Plan: Refactor GenericResourcePool::acquire for Clarity

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/resources/pool.rs:GenericResourcePool::acquire:121
**Priority Score**: 28.5
**Debt Type**: ComplexityHotspot (Cognitive: 32, Cyclomatic: 8)
**Current Metrics**:
- Lines of Code: 78 (function spans lines 121-198)
- Cyclomatic Complexity: 8
- Cognitive Complexity: 32
- Test Coverage: Unknown (transitive_coverage: null)
- Function Role: PureLogic (purity: 80%)

**Issue**: Complexity 8 is manageable but at the threshold. The function has cognitive complexity of 32, suggesting it's doing too much. The function handles:
1. Resource reuse from pool (lines 124-155)
2. Semaphore acquisition (lines 157-165)
3. New resource creation (lines 167-168)
4. Metrics tracking (appears in both branches)
5. ResourceGuard creation with cleanup closure (duplicated logic)

**Recommendation**: Current structure is acceptable - prioritize test coverage. Consider extracting guard clauses for precondition checks.

## Target State

**Expected Impact**:
- Complexity Reduction: 4.0 points (from 8 to ~4)
- Coverage Improvement: 0.0% (no coverage data available)
- Risk Reduction: 9.975 points

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 8 to ≤4
- [ ] Cognitive complexity reduced from 32 to ≤16
- [ ] Duplicated logic (ResourceGuard creation) extracted to pure function
- [ ] Metrics tracking logic extracted to pure function
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Metrics Update Logic

**Goal**: Remove duplicated metrics tracking logic by extracting it to a pure function

**Changes**:
- Create `update_acquisition_metrics()` helper function that takes metrics, start time, and whether it's a reuse
- Replace duplicated metrics update code in both branches (lines 126-136 and 170-179)
- This reduces cognitive load and eliminates code duplication

**Testing**:
- Run `cargo test --lib` to ensure existing tests pass
- Run `cargo clippy` to verify no warnings
- Verify metrics are still tracked correctly (if tests exist)

**Success Criteria**:
- [ ] Metrics update logic extracted to single function
- [ ] Both code paths use the extracted function
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract ResourceGuard Creation Logic

**Goal**: Remove duplicated ResourceGuard creation by extracting cleanup closure logic

**Changes**:
- Create `create_resource_guard()` helper function that takes resource, weak pool reference, and cleanup function
- Replace duplicated guard creation code in both branches (lines 140-154 and 183-197)
- This eliminates the largest block of duplicated code

**Testing**:
- Run `cargo test --lib` to ensure resource guards work correctly
- Verify resources are properly returned to pool on drop
- Check cleanup is called when pool is gone

**Success Criteria**:
- [ ] ResourceGuard creation extracted to single function
- [ ] Both code paths use the extracted function
- [ ] Resource lifecycle works correctly (return to pool or cleanup)
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Simplify Main Function Flow

**Goal**: Reduce cognitive complexity by improving control flow clarity

**Changes**:
- Add early guard clause comment to clarify "try reuse first" intent
- Consider extracting "create new resource" path to separate method if complexity is still high
- Ensure function reads top-to-bottom: try reuse → acquire permit → create new

**Testing**:
- Run `cargo test --lib` for functional correctness
- Review code for improved readability
- Verify control flow is clear and linear

**Success Criteria**:
- [ ] Main function flow is clear and easy to follow
- [ ] Cognitive complexity reduced significantly
- [ ] No nested closures or complex logic in main function
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Add Test Coverage (if needed)

**Goal**: Improve test coverage for resource pool acquisition logic

**Changes**:
- Add unit tests for extracted helper functions
- Add integration tests for:
  - Resource reuse from pool
  - New resource creation when pool is empty
  - Semaphore limiting concurrent acquisitions
  - Metrics tracking accuracy
  - ResourceGuard cleanup behavior

**Testing**:
- Run `cargo test` to verify all new tests pass
- Run `cargo tarpaulin` to measure coverage improvement
- Ensure edge cases are covered

**Success Criteria**:
- [ ] Helper functions have unit tests
- [ ] Main acquisition paths have integration tests
- [ ] Edge cases (pool empty, semaphore exhausted, cleanup) tested
- [ ] Coverage improved for this function
- [ ] All tests pass
- [ ] Ready to commit

## Implementation Approach

### Extracting Pure Functions

For Phase 1 (Metrics Update):
```rust
fn update_acquisition_metrics(
    metrics: &mut PoolMetrics,
    start: Instant,
    is_reuse: bool,
) {
    metrics.in_use += 1;
    metrics.total_acquisitions += 1;

    if is_reuse {
        metrics.reuse_count += 1;
        metrics.available = metrics.available.saturating_sub(1);
    } else {
        metrics.total_created += 1;
    }

    let wait_time = start.elapsed();
    metrics.avg_wait_time_ms = ((metrics.avg_wait_time_ms
        * (metrics.total_acquisitions - 1) as u64)
        + wait_time.as_millis() as u64)
        / metrics.total_acquisitions as u64;
}
```

For Phase 2 (ResourceGuard Creation):
```rust
fn create_resource_guard<T>(
    resource: T,
    pool: Arc<Mutex<VecDeque<T>>>,
    cleanup: Arc<dyn Fn(T) + Send + Sync>,
) -> super::ResourceGuard<T>
where
    T: Send + 'static,
{
    let pool_weak = Arc::downgrade(&pool);
    super::ResourceGuard::new(resource, move |r| {
        if let Some(pool) = pool_weak.upgrade() {
            tokio::spawn(async move {
                let mut available = pool.lock().await;
                available.push_back(r);
            });
        } else {
            cleanup(r);
        }
    })
}
```

### Key Principles

1. **Extract Pure Logic**: Metrics calculation is pure (given inputs → deterministic output)
2. **Eliminate Duplication**: Both helper functions remove exact duplicates
3. **Maintain Behavior**: No functional changes, only structural refactoring
4. **Incremental Progress**: Each phase is independently testable and committable

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure formatting
4. Manual review of changes for correctness

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Regenerate coverage (if Phase 4 executed)
3. `debtmap analyze` - Verify complexity reduction

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure - likely cause:
   - Incorrect function signature
   - Missed parameter in extraction
   - Lifetime or ownership issues
3. Adjust the plan:
   - May need to pass additional parameters
   - May need different ownership model (Arc vs clone)
4. Retry with fixes

## Notes

**Why this approach works:**
- The function has clear duplication (metrics update, guard creation) that can be extracted
- Extractions are mechanical and low-risk
- Each phase reduces both cyclomatic and cognitive complexity
- No behavioral changes needed - pure refactoring

**Potential gotchas:**
- Helper functions need proper lifetime annotations for generic T
- Arc/weak reference patterns must be preserved in extracted guard creation
- Metrics locking order must be maintained to avoid deadlocks

**After this refactor:**
- Main `acquire()` function will be ~30-40 lines instead of 78
- Complexity should drop from 8 to ~4 (meeting target)
- Code will be more testable and maintainable
- Future changes to metrics or guard creation will be centralized
