---
number: 171
title: Stillwater Semigroup-Based Variable Aggregation
category: optimization
priority: low
status: draft
dependencies: []
created: 2025-11-23
---

# Specification 171: Stillwater Semigroup-Based Variable Aggregation

**Category**: optimization
**Priority**: low
**Status**: draft
**Dependencies**: None

## Context

The variable aggregation system in `src/cook/execution/variables.rs` (2,286 lines) implements 15 different aggregate types (Count, Sum, Average, Min, Max, Collect, Median, StdDev, Variance, Unique, Concat, Merge, Flatten, Sort, GroupBy) with duplicated combination logic.

**Current Problems**:
- Custom merge implementations for each aggregate type
- Code duplication across similar aggregations
- No mathematical guarantees (associativity)
- Cannot safely parallelize aggregations
- Unclear composition semantics

## Objective

Implement `Semigroup` trait for aggregate results using Stillwater's pattern, providing consistent composition semantics, enabling safe parallel aggregation, and reducing code duplication.

## Requirements

### Functional Requirements

1. **Semigroup Implementation**
   - Implement `Semigroup` trait for `AggregateResult`
   - Define associative `combine()` operation for all aggregate types
   - Enable result composition via standard interface

2. **Consistent Aggregation**
   - Replace custom merge logic with `Semigroup::combine()`
   - Use `.reduce()` for aggregating multiple results
   - Clear mathematical properties (associativity)

3. **Parallel Aggregation**
   - Enable safe parallel result combination
   - No assumptions about combination order
   - Guarantee correct results regardless of grouping

### Non-Functional Requirements

1. **Performance**: No degradation vs current implementation
2. **Correctness**: Property tests verify semigroup laws
3. **Code Quality**: 30% reduction in aggregation code
4. **Clarity**: Clear composition semantics

## Acceptance Criteria

- [ ] `Semigroup` trait implemented for `AggregateResult`
- [ ] `combine()` method implemented for all 15 aggregate types
- [ ] Custom merge logic replaced with `.combine()` calls
- [ ] Multiple result aggregation uses `.reduce()` pattern
- [ ] Property tests verify associativity for all types
- [ ] Parallel aggregation tests demonstrate correctness
- [ ] Performance benchmarks show no regression
- [ ] Documentation updated with semigroup semantics
- [ ] 15+ unit tests for semigroup operations
- [ ] Migration guide for custom aggregations

## Technical Details

### Implementation Approach

**Phase 1: Semigroup Trait Implementation**
```rust
// src/cook/execution/variables/semigroup.rs

use stillwater::Semigroup;
use super::AggregateResult;

impl Semigroup for AggregateResult {
    fn combine(self, other: Self) -> Self {
        use AggregateResult::*;

        match (self, other) {
            // Numeric aggregations
            (Count(a), Count(b)) => Count(a + b),
            (Sum(a), Sum(b)) => Sum(a + b),

            // Collection aggregations
            (Collect(mut a), Collect(b)) => {
                a.extend(b);
                Collect(a)
            }
            (Concat(mut a), Concat(b)) => {
                a.push_str(&b);
                Concat(a)
            }
            (Unique(mut a), Unique(b)) => {
                a.extend(b);
                Unique(a)
            }

            // Map aggregations
            (Merge(mut a), Merge(b)) => {
                for (k, v) in b {
                    a.entry(k).or_insert(v);
                }
                Merge(a)
            }

            // Nested collections
            (Flatten(mut a), Flatten(b)) => {
                a.extend(b);
                Flatten(a)
            }

            // Statistical aggregations (require state tracking)
            (Average(sum_a, count_a), Average(sum_b, count_b)) => {
                Average(sum_a + sum_b, count_a + count_b)
            }

            // Min/Max
            (Min(a), Min(b)) => Min(a.min(b)),
            (Max(a), Max(b)) => Max(a.max(b)),

            // Incompatible types
            (a, b) => panic!(
                "Cannot combine incompatible aggregate types: {:?} and {:?}",
                std::mem::discriminant(&a),
                std::mem::discriminant(&b)
            ),
        }
    }
}
```

**Phase 2: Aggregate Function Updates**
```rust
// src/cook/execution/variables/aggregation.rs

use stillwater::Semigroup;

/// Aggregate multiple results using Semigroup
pub fn aggregate_results(
    results: Vec<AggregateResult>
) -> Option<AggregateResult> {
    results.into_iter()
        .reduce(|a, b| a.combine(b))  // Uses Semigroup::combine
}

/// Aggregate with fold (more control)
pub fn aggregate_with_initial(
    initial: AggregateResult,
    results: Vec<AggregateResult>,
) -> AggregateResult {
    results.into_iter()
        .fold(initial, |acc, r| acc.combine(r))
}

/// Parallel aggregation (safe due to associativity)
pub fn parallel_aggregate(
    results: Vec<AggregateResult>
) -> Option<AggregateResult> {
    use rayon::prelude::*;

    results.into_par_iter()
        .reduce(|| AggregateResult::Count(0), |a, b| a.combine(b))
}
```

**Phase 3: Statistical Aggregations with State**
```rust
// Some aggregations need to track intermediate state

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateResult {
    Count(usize),
    Sum(f64),
    Average(f64, usize),  // (sum, count) - state for combining
    Min(f64),
    Max(f64),
    Median(Vec<f64>),     // Collect all values, compute median later
    StdDev(Vec<f64>),     // Collect all values, compute stddev later
    Variance(Vec<f64>),   // Collect all values, compute variance later
    Collect(Vec<Value>),
    Unique(HashSet<Value>),
    Concat(String),
    Merge(HashMap<String, Value>),
    Flatten(Vec<Value>),
    Sort(Vec<Value>),
    GroupBy(HashMap<String, Vec<Value>>),
}

impl AggregateResult {
    /// Finalize aggregate (compute final value from state)
    pub fn finalize(self) -> Value {
        match self {
            AggregateResult::Average(sum, count) => {
                Value::Number((sum / count as f64).into())
            }
            AggregateResult::Median(mut values) => {
                values.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mid = values.len() / 2;
                Value::Number(values[mid].into())
            }
            AggregateResult::StdDev(values) => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter()
                    .map(|v| (v - mean).powi(2))
                    .sum::<f64>() / values.len() as f64;
                Value::Number(variance.sqrt().into())
            }
            // ... other finalizations
            _ => self.into_value(),
        }
    }
}
```

**Phase 4: Integration**
```rust
// src/cook/execution/variables/mod.rs

pub use semigroup::*;

/// Process aggregate expression
pub fn process_aggregate(
    expr: &str,
    items: &[Value],
) -> Result<Value> {
    let agg_type = parse_aggregate_type(expr)?;

    // Map items to intermediate results
    let results: Vec<AggregateResult> = items.iter()
        .map(|item| compute_partial_aggregate(agg_type, item))
        .collect();

    // Combine using Semigroup
    let combined = aggregate_results(results)
        .ok_or_else(|| anyhow::anyhow!("No results to aggregate"))?;

    // Finalize to get final value
    Ok(combined.finalize())
}
```

### Architecture Changes

**New Module Structure**:
```
src/cook/execution/variables/
├── mod.rs              (public API)
├── semigroup.rs        (NEW - Semigroup impl)
├── aggregation.rs      (updated - uses Semigroup)
├── types.rs            (AggregateResult enum)
└── parser.rs           (expression parsing)
```

**Composition Pattern**:
```
Parse Expression → Compute Partials → Combine (Semigroup) → Finalize
```

### Data Structures

```rust
/// Aggregate result (internal representation)
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateResult {
    // Simple aggregations (combinable directly)
    Count(usize),
    Sum(f64),
    Min(f64),
    Max(f64),
    Collect(Vec<Value>),
    Concat(String),

    // Stateful aggregations (combine state, finalize later)
    Average(f64, usize),      // (sum, count)
    Median(Vec<f64>),         // all values
    StdDev(Vec<f64>),         // all values
    Variance(Vec<f64>),       // all values

    // Complex aggregations
    Unique(HashSet<Value>),
    Merge(HashMap<String, Value>),
    Flatten(Vec<Value>),
    GroupBy(HashMap<String, Vec<Value>>),
}
```

### APIs and Interfaces

**Semigroup Interface** (from Stillwater):
```rust
pub trait Semigroup: Sized {
    fn combine(self, other: Self) -> Self;
}
```

**Aggregation API** (updated):
```rust
// Combine multiple results
pub fn aggregate_results(results: Vec<AggregateResult>) -> Option<AggregateResult>;

// Parallel aggregation
pub fn parallel_aggregate(results: Vec<AggregateResult>) -> Option<AggregateResult>;

// Finalize aggregate to value
impl AggregateResult {
    pub fn finalize(self) -> Value;
}
```

## Dependencies

### Prerequisites
- Stillwater library with `Semigroup` trait
- Understanding of associative operations

### Affected Components
- `src/cook/execution/variables.rs` - Main aggregation logic
- Variable interpolation throughout codebase

### External Dependencies
- `stillwater = "0.1"` (Semigroup trait)
- `rayon = "1.7"` (optional, for parallel aggregation)

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_combine() {
        let a = AggregateResult::Count(5);
        let b = AggregateResult::Count(3);

        assert_eq!(a.combine(b), AggregateResult::Count(8));
    }

    #[test]
    fn test_sum_combine() {
        let a = AggregateResult::Sum(10.5);
        let b = AggregateResult::Sum(5.5);

        assert_eq!(a.combine(b), AggregateResult::Sum(16.0));
    }

    #[test]
    fn test_collect_combine() {
        let a = AggregateResult::Collect(vec![Value::Number(1.into())]);
        let b = AggregateResult::Collect(vec![Value::Number(2.into())]);

        let result = a.combine(b);

        match result {
            AggregateResult::Collect(values) => {
                assert_eq!(values.len(), 2);
            }
            _ => panic!("Expected Collect"),
        }
    }

    #[test]
    fn test_average_combine_and_finalize() {
        let a = AggregateResult::Average(10.0, 2);  // avg: 5.0
        let b = AggregateResult::Average(20.0, 3);  // avg: 6.67

        let combined = a.combine(b);  // sum: 30.0, count: 5

        match combined {
            AggregateResult::Average(sum, count) => {
                assert_eq!(sum, 30.0);
                assert_eq!(count, 5);

                let final_value = AggregateResult::Average(sum, count).finalize();
                assert_eq!(final_value, Value::Number(6.0.into()));  // 30/5
            }
            _ => panic!("Expected Average"),
        }
    }
}
```

### Property Tests (Associativity)

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Generator for arbitrary AggregateResult
    fn arb_aggregate_result() -> impl Strategy<Value = AggregateResult> {
        prop_oneof![
            any::<usize>().prop_map(AggregateResult::Count),
            any::<f64>().prop_map(AggregateResult::Sum),
            any::<f64>().prop_map(AggregateResult::Min),
            any::<f64>().prop_map(AggregateResult::Max),
        ]
    }

    proptest! {
        #[test]
        fn test_semigroup_associativity(
            a in arb_aggregate_result(),
            b in arb_aggregate_result(),
            c in arb_aggregate_result(),
        ) {
            // Semigroup law: (a · b) · c = a · (b · c)
            let left = a.clone().combine(b.clone()).combine(c.clone());
            let right = a.combine(b.combine(c));

            prop_assert_eq!(left, right);
        }
    }
}
```

### Integration Tests

```rust
#[test]
fn test_aggregate_multiple_results() {
    let results = vec![
        AggregateResult::Count(1),
        AggregateResult::Count(2),
        AggregateResult::Count(3),
    ];

    let combined = aggregate_results(results).unwrap();

    assert_eq!(combined, AggregateResult::Count(6));
}

#[test]
fn test_parallel_aggregation() {
    let results: Vec<AggregateResult> = (0..1000)
        .map(|i| AggregateResult::Count(i))
        .collect();

    let sequential = aggregate_results(results.clone()).unwrap();
    let parallel = parallel_aggregate(results).unwrap();

    assert_eq!(sequential, parallel);
}
```

### Performance Tests

```rust
#[test]
fn benchmark_semigroup_vs_custom() {
    let results: Vec<AggregateResult> = (0..10_000)
        .map(|i| AggregateResult::Sum(i as f64))
        .collect();

    // Semigroup approach
    let start = Instant::now();
    let _ = aggregate_results(results.clone());
    let semigroup_duration = start.elapsed();

    // Custom approach (for comparison)
    let start = Instant::now();
    let _ = custom_aggregate(results);
    let custom_duration = start.elapsed();

    // Should be comparable or faster
    assert!(semigroup_duration <= custom_duration * 1.1);
}
```

## Documentation Requirements

### Code Documentation

```rust
/// Semigroup implementation for aggregate results
///
/// This enables consistent combination semantics across all aggregate types.
/// The `combine` operation is associative, allowing safe parallel aggregation.
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::variables::{AggregateResult, Semigroup};
///
/// let a = AggregateResult::Count(5);
/// let b = AggregateResult::Count(3);
/// let c = a.combine(b);
///
/// assert_eq!(c, AggregateResult::Count(8));
/// ```
///
/// # Associativity
///
/// The semigroup law guarantees: `(a · b) · c = a · (b · c)`
///
/// This property is verified by property tests.
impl Semigroup for AggregateResult { ... }
```

### User Documentation

Update `CLAUDE.md`:
```markdown
## Variable Aggregation

Prodigy uses Semigroup-based aggregation for consistent, composable results:

### Supported Aggregates

- **count**: Count items
- **sum**: Sum numeric values
- **avg**: Average (combines as sum+count, finalizes to average)
- **min/max**: Minimum/maximum values
- **collect**: Collect all values
- **unique**: Collect unique values
- **concat**: Concatenate strings

### Parallel Aggregation

Aggregates can be safely combined in parallel due to associativity guarantees.
```

### Architecture Updates

Add to `ARCHITECTURE.md`:
```markdown
## Variable Aggregation Architecture

### Semigroup-Based Composition

All aggregate types implement `Semigroup` trait for consistent combination:

- **Associative**: `(a · b) · c = a · (b · c)` (guaranteed by tests)
- **Composable**: Results combine via standard `.combine()` method
- **Parallel-Safe**: Can aggregate in any order or grouping

### Two-Phase Aggregation

1. **Partial Phase**: Compute intermediate results (state)
2. **Combine Phase**: Merge results using Semigroup
3. **Finalize Phase**: Compute final value from combined state

### Example

```rust
let results = vec![Count(1), Count(2), Count(3)];
let combined = results.into_iter().reduce(|a, b| a.combine(b));
// Result: Count(6)
```
```

## Implementation Notes

### Mathematical Guarantees

**Associativity**: Property tests verify semigroup law for all types
**Correctness**: Parallel and sequential aggregation produce identical results
**Composition**: Results can be combined incrementally or in batches

### Edge Cases

- **Empty results**: `.reduce()` returns `None`
- **Single result**: No combination needed, identity preserved
- **Type mismatch**: Panic with clear error (incompatible types)

### Performance Considerations

- **No overhead**: Semigroup trait compiles to direct method calls
- **Parallel opportunity**: Associativity enables safe parallelism
- **State tracking**: Stateful aggregations collect all data for finalization

## Migration and Compatibility

### Breaking Changes
None - internal refactoring only.

### Code Reduction

Estimated 30% reduction in aggregation code:
- Before: 15 custom merge implementations (~200 lines)
- After: Single `Semigroup::combine()` (~50 lines)

### Migration Path

1. Implement Semigroup for AggregateResult
2. Replace custom merge calls with `.combine()`
3. Update tests to use property tests
4. Add parallel aggregation benchmarks
5. Update documentation

### Rollback Strategy

Semigroup is additive - can keep custom implementations if needed.
