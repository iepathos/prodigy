//! Semigroup implementation for aggregate results
//!
//! This module provides the `AggregateResult` type and its `Semigroup` implementation,
//! enabling consistent combination semantics across all aggregate types.
//! The `combine` operation is associative, allowing safe parallel aggregation.

use rayon::prelude::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use stillwater::Semigroup;

/// Internal representation of aggregate results that can be combined
///
/// This enum represents intermediate aggregate states that can be combined
/// using the Semigroup trait. Some aggregations (like Average, Median, etc.)
/// track state that gets finalized into a final value later.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateResult {
    /// Count of items
    Count(usize),

    /// Sum of numeric values
    Sum(f64),

    /// Minimum value
    Min(Value),

    /// Maximum value
    Max(Value),

    /// Collection of all values
    Collect(Vec<Value>),

    /// Average state (sum, count) - finalized to average later
    Average(f64, usize),

    /// Median state - collects all values, computes median on finalize
    Median(Vec<f64>),

    /// Standard deviation state - collects all values
    StdDev(Vec<f64>),

    /// Variance state - collects all values
    Variance(Vec<f64>),

    /// Unique values set
    Unique(HashSet<String>),

    /// Concatenated strings
    Concat(String),

    /// Merged object/map
    Merge(HashMap<String, Value>),

    /// Flattened nested arrays
    Flatten(Vec<Value>),

    /// Sorted values (collected, sorted on finalize)
    Sort(Vec<Value>, bool), // values, descending

    /// Grouped values by key
    GroupBy(HashMap<String, Vec<Value>>),
}

impl Semigroup for AggregateResult {
    fn combine(self, other: Self) -> Self {
        use AggregateResult::*;

        match (self, other) {
            // Numeric aggregations (use saturating arithmetic to prevent overflow)
            (Count(a), Count(b)) => Count(a.saturating_add(b)),
            (Sum(a), Sum(b)) => Sum(a + b),

            // Min/Max - compare and keep appropriate value
            (Min(a), Min(b)) => {
                if compare_values(&a, &b) == std::cmp::Ordering::Less {
                    Min(a)
                } else {
                    Min(b)
                }
            }
            (Max(a), Max(b)) => {
                if compare_values(&a, &b) == std::cmp::Ordering::Greater {
                    Max(a)
                } else {
                    Max(b)
                }
            }

            // Collection aggregations
            (Collect(mut a), Collect(b)) => {
                a.extend(b);
                Collect(a)
            }

            // Stateful aggregations (combine state, finalize later)
            (Average(sum_a, count_a), Average(sum_b, count_b)) => {
                Average(sum_a + sum_b, count_a + count_b)
            }

            (Median(mut a), Median(b)) => {
                a.extend(b);
                Median(a)
            }

            (StdDev(mut a), StdDev(b)) => {
                a.extend(b);
                StdDev(a)
            }

            (Variance(mut a), Variance(b)) => {
                a.extend(b);
                Variance(a)
            }

            // String aggregations
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

            // Sort - combine collections, preserve descending flag
            (Sort(mut a, desc_a), Sort(b, desc_b)) => {
                a.extend(b);
                // If flags differ, default to first one (could also panic)
                Sort(a, desc_a && desc_b)
            }

            // GroupBy - merge groups
            (GroupBy(mut a), GroupBy(b)) => {
                for (key, mut values) in b {
                    a.entry(key).or_insert_with(Vec::new).append(&mut values);
                }
                GroupBy(a)
            }

            // Incompatible types - this is a programming error
            (a, b) => panic!(
                "Cannot combine incompatible aggregate types: {:?} and {:?}",
                discriminant_name(&a),
                discriminant_name(&b)
            ),
        }
    }
}

impl AggregateResult {
    /// Finalize aggregate result into a JSON value
    ///
    /// This converts the internal aggregate state into the final computed value.
    /// For simple aggregates (Count, Sum), this is straightforward.
    /// For stateful aggregates (Average, Median, StdDev, Variance), this performs
    /// the final computation.
    pub fn finalize(self) -> Value {
        match self {
            AggregateResult::Count(n) => Value::Number(serde_json::Number::from(n)),

            AggregateResult::Sum(s) => serde_json::Number::from_f64(s)
                .map(Value::Number)
                .unwrap_or(Value::Null),

            AggregateResult::Min(v) | AggregateResult::Max(v) => v,

            AggregateResult::Collect(values) => Value::Array(values),

            AggregateResult::Average(sum, count) => {
                if count == 0 {
                    Value::Null
                } else {
                    let avg = sum / count as f64;
                    serde_json::Number::from_f64(avg)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
            }

            AggregateResult::Median(mut values) => {
                if values.is_empty() {
                    Value::Null
                } else {
                    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let mid = values.len() / 2;
                    serde_json::Number::from_f64(values[mid])
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
            }

            AggregateResult::StdDev(values) => {
                if values.is_empty() {
                    Value::Null
                } else {
                    let mean = values.iter().sum::<f64>() / values.len() as f64;
                    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                        / values.len() as f64;
                    serde_json::Number::from_f64(variance.sqrt())
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
            }

            AggregateResult::Variance(values) => {
                if values.is_empty() {
                    Value::Null
                } else {
                    let mean = values.iter().sum::<f64>() / values.len() as f64;
                    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                        / values.len() as f64;
                    serde_json::Number::from_f64(variance)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
            }

            AggregateResult::Unique(set) => {
                let values: Vec<Value> = set.into_iter().map(Value::String).collect();
                Value::Array(values)
            }

            AggregateResult::Concat(s) => Value::String(s),

            AggregateResult::Merge(map) => Value::Object(map.into_iter().collect()),

            AggregateResult::Flatten(values) => Value::Array(values),

            AggregateResult::Sort(mut values, descending) => {
                values.sort_by(|a, b| {
                    let cmp = compare_values(a, b);
                    if descending {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                });
                Value::Array(values)
            }

            AggregateResult::GroupBy(groups) => {
                let obj: serde_json::Map<String, Value> = groups
                    .into_iter()
                    .map(|(k, v)| (k, Value::Array(v)))
                    .collect();
                Value::Object(obj)
            }
        }
    }
}

/// Compare two JSON values (numeric if possible, otherwise string)
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    if let (Some(a_num), Some(b_num)) = (a.as_f64(), b.as_f64()) {
        a_num
            .partial_cmp(&b_num)
            .unwrap_or(std::cmp::Ordering::Equal)
    } else {
        a.to_string().cmp(&b.to_string())
    }
}

/// Get a string representation of the discriminant for error messages
fn discriminant_name(result: &AggregateResult) -> &'static str {
    match result {
        AggregateResult::Count(_) => "Count",
        AggregateResult::Sum(_) => "Sum",
        AggregateResult::Min(_) => "Min",
        AggregateResult::Max(_) => "Max",
        AggregateResult::Collect(_) => "Collect",
        AggregateResult::Average(_, _) => "Average",
        AggregateResult::Median(_) => "Median",
        AggregateResult::StdDev(_) => "StdDev",
        AggregateResult::Variance(_) => "Variance",
        AggregateResult::Unique(_) => "Unique",
        AggregateResult::Concat(_) => "Concat",
        AggregateResult::Merge(_) => "Merge",
        AggregateResult::Flatten(_) => "Flatten",
        AggregateResult::Sort(_, _) => "Sort",
        AggregateResult::GroupBy(_) => "GroupBy",
    }
}

/// Aggregate multiple results in parallel using rayon
///
/// This function leverages rayon's parallel iterator to combine aggregate results
/// concurrently. The semigroup combine operation's associativity guarantees that
/// parallel aggregation produces the same result as sequential aggregation.
///
/// # Arguments
/// * `results` - Vector of aggregate results to combine in parallel
///
/// # Returns
/// * `Some(AggregateResult)` if the vector is non-empty
/// * `None` if the vector is empty
///
/// # Performance
/// Parallel aggregation is beneficial for large datasets (typically >1000 items).
/// For smaller datasets, the overhead of parallelization may outweigh the benefits.
/// Use `aggregate_results` for small datasets.
///
/// # Example
/// ```
/// use prodigy::cook::execution::variables::semigroup::{AggregateResult, parallel_aggregate};
///
/// let results: Vec<_> = (0..10000)
///     .map(|_| AggregateResult::Count(1))
///     .collect();
///
/// let combined = parallel_aggregate(results).unwrap();
/// assert_eq!(combined, AggregateResult::Count(10000));
/// ```
pub fn parallel_aggregate(results: Vec<AggregateResult>) -> Option<AggregateResult> {
    results.into_par_iter().reduce_with(|a, b| a.combine(b))
}

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
                assert_eq!(values[0], Value::Number(1.into()));
                assert_eq!(values[1], Value::Number(2.into()));
            }
            _ => panic!("Expected Collect"),
        }
    }

    #[test]
    fn test_average_combine_and_finalize() {
        let a = AggregateResult::Average(10.0, 2); // avg: 5.0
        let b = AggregateResult::Average(20.0, 3); // avg: 6.67
        let combined = a.combine(b); // sum: 30.0, count: 5

        match combined {
            AggregateResult::Average(sum, count) => {
                assert_eq!(sum, 30.0);
                assert_eq!(count, 5);

                let final_value = AggregateResult::Average(sum, count).finalize();
                assert_eq!(
                    final_value,
                    Value::Number(serde_json::Number::from_f64(6.0).unwrap())
                );
            }
            _ => panic!("Expected Average"),
        }
    }

    #[test]
    fn test_min_combine() {
        let a = AggregateResult::Min(Value::Number(5.into()));
        let b = AggregateResult::Min(Value::Number(3.into()));
        let result = a.combine(b);

        match result {
            AggregateResult::Min(v) => assert_eq!(v, Value::Number(3.into())),
            _ => panic!("Expected Min"),
        }
    }

    #[test]
    fn test_max_combine() {
        let a = AggregateResult::Max(Value::Number(5.into()));
        let b = AggregateResult::Max(Value::Number(8.into()));
        let result = a.combine(b);

        match result {
            AggregateResult::Max(v) => assert_eq!(v, Value::Number(8.into())),
            _ => panic!("Expected Max"),
        }
    }

    #[test]
    fn test_concat_combine() {
        let a = AggregateResult::Concat("Hello, ".to_string());
        let b = AggregateResult::Concat("World!".to_string());
        let result = a.combine(b);

        match result {
            AggregateResult::Concat(s) => assert_eq!(s, "Hello, World!"),
            _ => panic!("Expected Concat"),
        }
    }

    #[test]
    fn test_unique_combine() {
        let mut set_a = HashSet::new();
        set_a.insert("a".to_string());
        set_a.insert("b".to_string());

        let mut set_b = HashSet::new();
        set_b.insert("b".to_string());
        set_b.insert("c".to_string());

        let a = AggregateResult::Unique(set_a);
        let b = AggregateResult::Unique(set_b);
        let result = a.combine(b);

        match result {
            AggregateResult::Unique(set) => {
                assert_eq!(set.len(), 3);
                assert!(set.contains("a"));
                assert!(set.contains("b"));
                assert!(set.contains("c"));
            }
            _ => panic!("Expected Unique"),
        }
    }

    #[test]
    fn test_merge_combine() {
        let mut map_a = HashMap::new();
        map_a.insert("a".to_string(), Value::Number(1.into()));
        map_a.insert("b".to_string(), Value::Number(2.into()));

        let mut map_b = HashMap::new();
        map_b.insert("b".to_string(), Value::Number(999.into())); // Should not override
        map_b.insert("c".to_string(), Value::Number(3.into()));

        let a = AggregateResult::Merge(map_a);
        let b = AggregateResult::Merge(map_b);
        let result = a.combine(b);

        match result {
            AggregateResult::Merge(map) => {
                assert_eq!(map.len(), 3);
                assert_eq!(map.get("a"), Some(&Value::Number(1.into())));
                assert_eq!(map.get("b"), Some(&Value::Number(2.into()))); // First value wins
                assert_eq!(map.get("c"), Some(&Value::Number(3.into())));
            }
            _ => panic!("Expected Merge"),
        }
    }

    #[test]
    fn test_flatten_combine() {
        let a = AggregateResult::Flatten(vec![Value::Number(1.into()), Value::Number(2.into())]);
        let b = AggregateResult::Flatten(vec![Value::Number(3.into())]);
        let result = a.combine(b);

        match result {
            AggregateResult::Flatten(values) => {
                assert_eq!(values.len(), 3);
            }
            _ => panic!("Expected Flatten"),
        }
    }

    #[test]
    fn test_median_combine_and_finalize() {
        let a = AggregateResult::Median(vec![1.0, 3.0, 5.0]);
        let b = AggregateResult::Median(vec![2.0, 4.0]);
        let combined = a.combine(b);

        let finalized = combined.finalize();
        // Median of [1, 2, 3, 4, 5] = 3
        assert_eq!(
            finalized,
            Value::Number(serde_json::Number::from_f64(3.0).unwrap())
        );
    }

    #[test]
    fn test_variance_combine_and_finalize() {
        let a = AggregateResult::Variance(vec![1.0, 2.0, 3.0]);
        let b = AggregateResult::Variance(vec![4.0, 5.0]);
        let combined = a.combine(b);

        let finalized = combined.finalize();
        // Variance of [1, 2, 3, 4, 5]: mean = 3, variance = 2
        match finalized {
            Value::Number(n) => {
                let variance = n.as_f64().unwrap();
                assert!((variance - 2.0).abs() < 0.01);
            }
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_group_by_combine() {
        let mut groups_a = HashMap::new();
        groups_a.insert("group1".to_string(), vec![Value::Number(1.into())]);

        let mut groups_b = HashMap::new();
        groups_b.insert("group1".to_string(), vec![Value::Number(2.into())]);
        groups_b.insert("group2".to_string(), vec![Value::Number(3.into())]);

        let a = AggregateResult::GroupBy(groups_a);
        let b = AggregateResult::GroupBy(groups_b);
        let result = a.combine(b);

        match result {
            AggregateResult::GroupBy(groups) => {
                assert_eq!(groups.len(), 2);
                assert_eq!(groups.get("group1").unwrap().len(), 2);
                assert_eq!(groups.get("group2").unwrap().len(), 1);
            }
            _ => panic!("Expected GroupBy"),
        }
    }

    #[test]
    fn test_multiple_combines() {
        let results = vec![
            AggregateResult::Count(1),
            AggregateResult::Count(2),
            AggregateResult::Count(3),
            AggregateResult::Count(4),
        ];

        let combined = results.into_iter().reduce(|a, b| a.combine(b)).unwrap();
        assert_eq!(combined, AggregateResult::Count(10));
    }

    #[test]
    fn test_finalize_count() {
        let result = AggregateResult::Count(42);
        let finalized = result.finalize();
        assert_eq!(finalized, Value::Number(42.into()));
    }

    #[test]
    fn test_finalize_sum() {
        let result = AggregateResult::Sum(123.45);
        let finalized = result.finalize();
        match finalized {
            Value::Number(n) => assert_eq!(n.as_f64().unwrap(), 123.45),
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_finalize_concat() {
        let result = AggregateResult::Concat("test string".to_string());
        let finalized = result.finalize();
        assert_eq!(finalized, Value::String("test string".to_string()));
    }

    #[test]
    #[should_panic(expected = "Cannot combine incompatible aggregate types")]
    fn test_incompatible_types_panic() {
        let a = AggregateResult::Count(5);
        let b = AggregateResult::Sum(3.0);
        let _ = a.combine(b);
    }
}
