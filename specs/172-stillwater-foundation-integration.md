---
number: 172
title: Stillwater Foundation Integration
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-11-24
---

# Specification 172: Stillwater Foundation Integration

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None (requires Stillwater 0.2.0 with specs 012, 014, 015, 016, 021)

## Context

Prodigy currently uses a mixed imperative/functional architecture with critical production bugs:
- **Runtime panics** in aggregation due to type mismatches (`panic!("Cannot combine different AggregateResult types")`)
- **Manual async coordination** that is error-prone and hard to maintain
- **Mixed I/O and business logic** making testing difficult
- **Limited error accumulation** that fails fast instead of reporting all errors

This specification covers Phase 1 of the Stillwater migration plan: integrating Stillwater as a dependency and eliminating the most critical production bug (aggregation panics) using homogeneous validation.

## Objective

Establish the foundation for Prodigy's functional programming migration by:
1. Integrating Stillwater as a core dependency
2. Eliminating panic-based type checking in aggregation using homogeneous validation
3. Ensuring all existing functionality works with no regression
4. Validating that Stillwater patterns solve real production problems

## Requirements

### Functional Requirements

#### FR1: Stillwater Dependency Integration
- **MUST** add Stillwater dependency to Cargo.toml
- **MUST** support both crates.io and path dependency configurations
- **MUST** pin specific Stillwater version for stability
- **MUST** update imports in core modules
- **MUST** compile successfully with Stillwater integrated
- **MUST** pass all existing tests with no regression

#### FR2: Homogeneous Validation for AggregateResult
- **MUST** use `combine_homogeneous` from Stillwater for aggregation
- **MUST** validate type homogeneity before calling `Semigroup::combine`
- **MUST** eliminate all `panic!` calls in aggregation code
- **MUST** return `Validation<T, Vec<E>>` for all aggregation operations
- **MUST** accumulate ALL type mismatch errors (not just first)
- **MUST** preserve existing `Semigroup` implementation for pure combining

#### FR3: Error Handling and Reporting
- **MUST** create new `AggregationError::TypeMismatch` variant
- **MUST** include context: agent index, expected type, actual type
- **MUST** log all aggregation errors to DLQ with full context
- **MUST** preserve error location and stack trace information
- **MUST** provide user-friendly error messages
- **MUST** integrate with existing `ContextError` pattern

#### FR4: MapReduce Integration Points
- **MUST** update `finalize_aggregation` in map phase to use validation
- **MUST** integrate validation with existing checkpoint system
- **MUST** preserve aggregation results in DLQ for failed items
- **MUST** maintain backward compatibility with existing workflows
- **MUST** handle validation failures gracefully without data loss

### Non-Functional Requirements

#### NFR1: Performance
- **MUST** maintain or improve aggregation performance
- **MUST** have < 5% overhead from validation
- **MUST** run existing performance benchmarks with no regression
- **MUST** use zero-cost abstractions where possible

#### NFR2: Stability
- **MUST** pass all existing tests
- **MUST** maintain backward compatibility with workflows
- **MUST** not break existing error handling
- **MUST** preserve checkpoint and resume functionality

#### NFR3: Code Quality
- **MUST** follow existing Prodigy code conventions
- **MUST** add inline documentation for Stillwater patterns
- **MUST** provide examples of validation usage
- **MUST** pass clippy with no new warnings

## Acceptance Criteria

- [ ] Stillwater dependency added and compiles successfully
- [ ] All existing tests pass with no modification
- [ ] `AggregateResult` uses `combine_homogeneous` for validation
- [ ] No `panic!` calls remain in aggregation code
- [ ] Type mismatch errors accumulated and reported to DLQ
- [ ] Integration tests verify ALL errors reported (not just first)
- [ ] Property tests verify homogeneous aggregation always succeeds
- [ ] Regression tests confirm existing MapReduce workflows work
- [ ] Performance benchmarks show < 5% overhead
- [ ] Documentation updated with Stillwater validation patterns

## Technical Details

### Implementation Approach

#### 1. Dependency Integration

```toml
# Cargo.toml
[dependencies]
stillwater = "0.2.0"  # or path = "../stillwater" for development
```

#### 2. AggregateResult Validation

**Before (Current - with panic):**
```rust
impl Semigroup for AggregateResult {
    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Count(a), Count(b)) => Count(a.saturating_add(b)),
            // ... other variants
            _ => panic!("Cannot combine different AggregateResult types"),
        }
    }
}
```

**After (With validation):**
```rust
use stillwater::validation::homogeneous::combine_homogeneous;
use stillwater::Validation;

// Semigroup stays pure - only called after validation
impl Semigroup for AggregateResult {
    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Count(a), Count(b)) => Count(a.saturating_add(b)),
            (Sum(a), Sum(b)) => Sum(a + b),
            (Average(sum_a, count_a), Average(sum_b, count_b)) =>
                Average(sum_a + sum_b, count_a + count_b),
            // ... all other variants

            // Safe to unreachable - validated before calling
            _ => unreachable!("Call validate_homogeneous before combining"),
        }
    }
}

// Validation at boundary
pub fn aggregate_map_results(
    results: Vec<AggregateResult>
) -> Validation<AggregateResult, Vec<AggregationError>> {
    combine_homogeneous(
        results,
        |r| std::mem::discriminant(r),
        |idx, got, expected| AggregationError::TypeMismatch {
            agent_index: idx,
            expected: discriminant_name(expected),
            got: discriminant_name(got),
        },
    )
}

// Helper for error messages
impl AggregateResult {
    fn discriminant_name(&self) -> &'static str {
        match self {
            Count(_) => "Count",
            Sum(_) => "Sum",
            Average(_, _) => "Average",
            Median(_) => "Median",
            StdDev(_) => "StdDev",
            Variance(_) => "Variance",
            Unique(_) => "Unique",
            Collect(_) => "Collect",
            Concat(_) => "Concat",
            Merge(_) => "Merge",
            Flatten(_) => "Flatten",
            Sort(_) => "Sort",
            GroupBy(_) => "GroupBy",
        }
    }
}
```

#### 3. MapReduce Integration

```rust
// src/cook/execution/mapreduce/phases/map.rs
async fn finalize_aggregation(
    &self,
    results: Vec<AggregateResult>,
) -> Result<AggregateResult, PhaseError> {
    // Validate before combining
    match aggregate_map_results(results) {
        Validation::Success(combined) => Ok(combined),
        Validation::Failure(errors) => {
            // Add ALL errors to DLQ, not just first
            for error in &errors {
                error!("Aggregation type mismatch: {:?}", error);
                self.dlq.add_error(error).await?;
            }
            Err(PhaseError::AggregationFailed {
                count: errors.len(),
                errors
            })
        }
    }
}
```

### Architecture Changes

**New Error Types:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationError {
    TypeMismatch {
        agent_index: usize,
        expected: &'static str,
        got: &'static str,
    },
    // ... other variants
}
```

**Files Modified:**
```
Cargo.toml                                            # Add Stillwater dep
src/lib.rs                                            # Add Stillwater imports
src/cook/execution/variables/semigroup.rs            # Validation integration
src/cook/execution/mapreduce/phases/map.rs           # Use validation
src/cook/execution/errors.rs                         # New error variants
```

**Files Added:**
```
src/cook/execution/variables/validation.rs           # Validation helpers
```

### Data Structures

**No changes to existing data structures** - Stillwater integrates cleanly with current `AggregateResult` enum.

### APIs and Interfaces

**New Public API:**
```rust
pub fn aggregate_map_results(
    results: Vec<AggregateResult>
) -> Validation<AggregateResult, Vec<AggregationError>>;

pub fn aggregate_with_initial(
    initial: AggregateResult,
    results: Vec<AggregateResult>,
) -> Validation<AggregateResult, Vec<AggregationError>>;
```

**Existing APIs unchanged** - internal implementation changes only.

## Dependencies

### Prerequisites
- **Stillwater 0.2.0** published with specs 012, 014, 015, 016, 021
- All Stillwater examples and tests passing
- Property tests verifying Semigroup associativity
- Documentation complete with migration guides

### Affected Components
- `src/cook/execution/variables/` - Aggregation and semigroup
- `src/cook/execution/mapreduce/phases/map.rs` - Finalization
- `src/cook/execution/errors.rs` - Error types
- DLQ integration for error reporting

### External Dependencies
- `stillwater = "0.2.0"` (new)

## Testing Strategy

### Unit Tests

**Homogeneous Validation:**
```rust
#[test]
fn test_homogeneous_aggregation_succeeds() {
    let results = vec![
        AggregateResult::Count(5),
        AggregateResult::Count(3),
        AggregateResult::Count(2),
    ];

    let result = aggregate_map_results(results);

    assert!(matches!(result, Validation::Success(AggregateResult::Count(10))));
}

#[test]
fn test_heterogeneous_aggregation_returns_all_errors() {
    let results = vec![
        AggregateResult::Count(5),
        AggregateResult::Sum(10.0),      // Error at index 1
        AggregateResult::Average(6.0, 2), // Error at index 2
        AggregateResult::Count(3),
    ];

    let result = aggregate_map_results(results);

    match result {
        Validation::Failure(errors) => {
            assert_eq!(errors.len(), 2); // Both errors reported
            assert!(errors.iter().any(|e| matches!(e,
                AggregationError::TypeMismatch { agent_index: 1, .. })));
            assert!(errors.iter().any(|e| matches!(e,
                AggregationError::TypeMismatch { agent_index: 2, .. })));
        }
        _ => panic!("Expected validation failure"),
    }
}
```

### Integration Tests

**MapReduce DLQ Integration:**
```rust
#[tokio::test]
async fn test_type_mismatch_added_to_dlq() {
    let workflow = create_test_workflow_with_mismatched_types();

    let result = execute_mapreduce_workflow(workflow).await;

    assert!(result.is_err());

    let dlq_items = get_dlq_items(&workflow.job_id).await;
    assert!(!dlq_items.is_empty());

    // Verify error details in DLQ
    let error = &dlq_items[0].failure_history[0];
    assert!(matches!(error.reason,
        AggregationError::TypeMismatch { .. }));
}
```

### Property Tests

**Associativity:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_homogeneous_aggregation_is_associative(
        values in prop::collection::vec(1usize..1000, 3..20)
    ) {
        let results: Vec<_> = values.iter()
            .map(|&v| AggregateResult::Count(v))
            .collect();

        // Split into two groups differently
        let mid1 = results.len() / 2;
        let mid2 = results.len() / 3;

        let result1 = aggregate_map_results(results.clone());

        let mut split1 = results.clone();
        let split1_right = split1.split_off(mid1);
        let combined1_left = aggregate_map_results(split1).unwrap();
        let combined1_right = aggregate_map_results(split1_right).unwrap();
        let result2 = aggregate_map_results(vec![combined1_left, combined1_right]);

        // Different grouping should give same result
        prop_assert_eq!(result1, result2);
    }
}
```

### Performance Tests

**Benchmark aggregation overhead:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_aggregation_with_validation(c: &mut Criterion) {
    let results: Vec<_> = (0..1000)
        .map(|i| AggregateResult::Count(i))
        .collect();

    c.bench_function("aggregate_with_validation", |b| {
        b.iter(|| {
            aggregate_map_results(black_box(results.clone()))
        })
    });
}

criterion_group!(benches, bench_aggregation_with_validation);
criterion_main!(benches);
```

### User Acceptance

**Scenario: Type mismatch in production workflow**
1. Setup MapReduce workflow with agents returning different aggregate types
2. Execute workflow and let aggregation fail
3. Verify ALL type mismatches reported (not just first)
4. Verify errors logged to DLQ with full context
5. Verify user can see all errors in single report

## Documentation Requirements

### Code Documentation

**Inline documentation for validation pattern:**
```rust
/// Aggregates multiple `AggregateResult` values using homogeneous validation.
///
/// This function validates that all results have the same variant type before
/// combining them. If any result has a different type, ALL mismatches are
/// accumulated and returned as errors.
///
/// # Example
///
/// ```rust
/// use prodigy::cook::execution::variables::aggregate_map_results;
/// use prodigy::cook::execution::variables::AggregateResult;
///
/// let results = vec![
///     AggregateResult::Count(5),
///     AggregateResult::Count(3),
/// ];
///
/// match aggregate_map_results(results) {
///     Validation::Success(combined) => {
///         // All types matched, aggregation succeeded
///     }
///     Validation::Failure(errors) => {
///         // Type mismatches found - ALL errors reported
///         for error in errors {
///             eprintln!("Aggregation error: {:?}", error);
///         }
///     }
/// }
/// ```
pub fn aggregate_map_results(
    results: Vec<AggregateResult>
) -> Validation<AggregateResult, Vec<AggregationError>>
```

### User Documentation

**Update CLAUDE.md:**
- Document Stillwater validation pattern
- Provide examples of error accumulation
- Explain benefits over panic-based approach
- Migration guide for custom aggregations

### Architecture Updates

**Update ARCHITECTURE.md:**
- Add section on Stillwater integration
- Document validation pattern usage
- Explain pure core / imperative shell separation
- Show data flow with validation boundaries

## Implementation Notes

### Critical Success Factors
1. **No panics in production** - All type errors handled gracefully
2. **All errors reported** - Accumulate all validation failures
3. **Zero regression** - Existing tests must pass unchanged
4. **Performance** - < 5% overhead from validation

### Gotchas and Pitfalls
- **Semigroup trait**: Keep pure, only call after validation
- **Error context**: Preserve agent index for debugging
- **Checkpoint integration**: Ensure validation works with resume
- **DLQ integration**: Don't lose errors in batch processing

### Best Practices
- Use `combine_homogeneous` at boundaries (MapReduce phases)
- Keep validation logic separate from combining logic
- Accumulate errors with context, not just counts
- Test both success and failure paths thoroughly

### Migration Path
1. Add Stillwater dependency
2. Create validation helpers
3. Update error types
4. Integrate in map phase finalization
5. Add comprehensive tests
6. Run benchmarks
7. Update documentation

## Migration and Compatibility

### Breaking Changes
- **None** - Internal implementation only
- All public APIs remain unchanged
- Existing workflows work without modification

### Backward Compatibility
- Checkpoint format unchanged
- Resume functionality preserved
- DLQ format extended (backward compatible)
- Error types extended (non-breaking)

### Migration Steps for Developers
1. Update `Cargo.toml` with Stillwater dependency
2. Run `cargo build` to verify compilation
3. Run test suite: `cargo test`
4. Run benchmarks: `cargo bench`
5. Test MapReduce workflows manually
6. Deploy to staging environment
7. Monitor for validation errors in logs

### Rollback Strategy
If critical issues arise:
1. Revert Stillwater dependency removal
2. Restore panic-based aggregation (original code)
3. Redeploy previous version
4. Analyze failure cause
5. Fix and re-attempt migration

**Rollback impact:** Lose improved error reporting, return to panic risk.
