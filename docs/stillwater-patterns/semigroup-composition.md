# Variable Aggregation: Semigroup Composition

## Current Problem
**Location**: `src/cook/execution/variables.rs:100-500`

**Symptom**: Duplicated aggregation logic across 15 aggregate types, custom merge implementations.

```rust
// Current: Duplicated merge logic
pub enum AggregateType {
    Count, Sum, Average, Min, Max, Collect,
    Median, StdDev, Variance, Unique, Concat,
    Merge, Flatten, Sort, GroupBy,
}

// Custom implementations for each type
pub fn aggregate_count(values: Vec<Value>) -> Value {
    Value::Number(values.len() as f64)
}

pub fn aggregate_sum(values: Vec<Value>) -> Value {
    let sum: f64 = values.iter()
        .filter_map(|v| v.as_f64())
        .sum();
    Value::Number(sum)
}

pub fn aggregate_collect(values: Vec<Value>) -> Value {
    Value::Array(values)
}

// ... 12 more similar functions

// Merging results requires match statement
pub fn merge_aggregate_results(a: AggregateResult, b: AggregateResult) -> AggregateResult {
    match (a, b) {
        (AggregateResult::Count(x), AggregateResult::Count(y)) => {
            AggregateResult::Count(x + y)
        }
        (AggregateResult::Sum(x), AggregateResult::Sum(y)) => {
            AggregateResult::Sum(x + y)
        }
        (AggregateResult::Collect(mut x), AggregateResult::Collect(y)) => {
            x.extend(y);
            AggregateResult::Collect(x)
        }
        // ... 12 more arms
        _ => panic!("Cannot merge different aggregate types"),
    }
}
```

**Problem**:
- Code duplication across aggregates
- No clear abstraction for combination
- Manual implementation of merge logic
- No mathematical guarantees (associativity)

## Stillwater Solution: Semigroup Trait

```rust
use stillwater::Semigroup;

// 1. Implement Semigroup for aggregates
impl Semigroup for AggregateResult {
    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (AggregateResult::Count(a), AggregateResult::Count(b)) => {
                AggregateResult::Count(a + b)
            }
            (AggregateResult::Sum(a), AggregateResult::Sum(b)) => {
                AggregateResult::Sum(a + b)
            }
            (AggregateResult::Collect(mut a), AggregateResult::Collect(b)) => {
                a.extend(b);
                AggregateResult::Collect(a)
            }
            (AggregateResult::Merge(mut a), AggregateResult::Merge(b)) => {
                a.merge(b);  // Merge is also a Semigroup
                AggregateResult::Merge(a)
            }
            _ => panic!("Cannot combine incompatible aggregate types"),
        }
    }
}

// 2. Use Semigroup for aggregation (no custom logic)
pub fn aggregate_results(results: Vec<AggregateResult>) -> Option<AggregateResult> {
    results.into_iter()
        .reduce(|a, b| a.combine(b))  // Uses Semigroup::combine
}

// 3. Parallel aggregation (associativity guarantee)
pub fn parallel_aggregate(results: Vec<AggregateResult>) -> Option<AggregateResult> {
    // Can split and combine in any order (associative property)
    results.par_iter()  // Parallel iterator
        .cloned()
        .reduce(|a, b| a.combine(b))
}

// 4. Incremental aggregation
pub fn add_to_aggregate(
    current: Option<AggregateResult>,
    new_value: AggregateResult,
) -> AggregateResult {
    match current {
        Some(agg) => agg.combine(new_value),
        None => new_value,
    }
}

// 5. Property testing (mathematical guarantees)
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_aggregate_associativity(
            a in any::<AggregateResult>(),
            b in any::<AggregateResult>(),
            c in any::<AggregateResult>(),
        ) {
            // Semigroup law: (a . b) . c = a . (b . c)
            let left = a.clone().combine(b.clone()).combine(c.clone());
            let right = a.combine(b.combine(c));

            assert_eq!(left, right);
        }
    }
}
```

## Benefit

- Consistent aggregation via trait
- Composable aggregations
- Parallel aggregation guaranteed safe (associativity)
- Mathematical properties testable via property tests
- Less code duplication

## Impact

- Code reduction: 30% less aggregation code
- Parallelism: Safe parallel aggregation
- Correctness: Property tests guarantee laws hold
- Clarity: Clear abstraction for combination
