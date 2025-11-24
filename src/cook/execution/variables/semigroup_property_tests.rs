//! Property-based tests for Semigroup laws
//!
//! These tests verify that the Semigroup implementation for AggregateResult
//! satisfies the associativity law: (a · b) · c = a · (b · c)

#[cfg(test)]
mod property_tests {
    use super::super::semigroup::AggregateResult;
    use proptest::prelude::*;
    use serde_json::Value;
    use stillwater::Semigroup;

    // Generators for arbitrary AggregateResult values

    fn arb_count() -> impl Strategy<Value = AggregateResult> {
        any::<usize>().prop_map(AggregateResult::Count)
    }

    fn arb_sum() -> impl Strategy<Value = AggregateResult> {
        any::<f64>()
            .prop_filter("must be finite", |f| f.is_finite())
            .prop_map(AggregateResult::Sum)
    }

    fn arb_min() -> impl Strategy<Value = AggregateResult> {
        any::<i32>().prop_map(|n| AggregateResult::Min(Value::Number(n.into())))
    }

    fn arb_max() -> impl Strategy<Value = AggregateResult> {
        any::<i32>().prop_map(|n| AggregateResult::Max(Value::Number(n.into())))
    }

    fn arb_collect() -> impl Strategy<Value = AggregateResult> {
        prop::collection::vec(any::<i32>(), 0..10).prop_map(|nums| {
            let values = nums.into_iter().map(|n| Value::Number(n.into())).collect();
            AggregateResult::Collect(values)
        })
    }

    fn arb_average() -> impl Strategy<Value = AggregateResult> {
        (
            any::<f64>().prop_filter("must be finite", |f| f.is_finite()),
            1usize..100,
        )
            .prop_map(|(sum, count)| AggregateResult::Average(sum, count))
    }

    fn arb_median() -> impl Strategy<Value = AggregateResult> {
        prop::collection::vec(
            any::<f64>().prop_filter("must be finite", |f| f.is_finite()),
            0..10,
        )
        .prop_map(AggregateResult::Median)
    }

    fn arb_concat() -> impl Strategy<Value = AggregateResult> {
        any::<String>().prop_map(AggregateResult::Concat)
    }

    fn arb_unique() -> impl Strategy<Value = AggregateResult> {
        prop::collection::hash_set(any::<String>(), 0..10).prop_map(AggregateResult::Unique)
    }

    fn arb_flatten() -> impl Strategy<Value = AggregateResult> {
        prop::collection::vec(any::<i32>(), 0..10).prop_map(|nums| {
            let values = nums.into_iter().map(|n| Value::Number(n.into())).collect();
            AggregateResult::Flatten(values)
        })
    }

    // Macro to generate associativity tests for each aggregate type
    macro_rules! test_associativity {
        ($name:ident, $generator:expr) => {
            proptest! {
                #[test]
                fn $name(
                    a in $generator,
                    b in $generator,
                    c in $generator,
                ) {
                    // Semigroup law: (a · b) · c = a · (b · c)
                    let left = a.clone().combine(b.clone()).combine(c.clone());
                    let right = a.combine(b.combine(c));

                    // For Sum type, use approximate equality due to floating-point precision
                    match (&left, &right) {
                        (AggregateResult::Sum(l), AggregateResult::Sum(r)) => {
                            // Use relative epsilon tolerance for very small or very large values
                            let max_val = l.abs().max(r.abs());
                            let tolerance = if !(1e-100..=1e100).contains(&max_val) {
                                // For extreme values, use relative tolerance
                                max_val * 1e-10 + 1e-300
                            } else {
                                // For normal values, use absolute tolerance
                                1e-10
                            };
                            prop_assert!(
                                (l - r).abs() < tolerance || (l.is_nan() && r.is_nan()),
                                "Sum values differ: left={}, right={}, diff={}",
                                l, r, (l - r).abs()
                            );
                        }
                        _ => {
                            prop_assert_eq!(left, right);
                        }
                    }
                }
            }
        };
    }

    test_associativity!(test_count_associativity, arb_count());
    test_associativity!(test_sum_associativity, arb_sum());
    test_associativity!(test_min_associativity, arb_min());
    test_associativity!(test_max_associativity, arb_max());
    test_associativity!(test_collect_associativity, arb_collect());
    test_associativity!(test_average_associativity, arb_average());
    test_associativity!(test_median_associativity, arb_median());
    test_associativity!(test_concat_associativity, arb_concat());
    test_associativity!(test_unique_associativity, arb_unique());
    test_associativity!(test_flatten_associativity, arb_flatten());

    // Additional tests for specific edge cases

    proptest! {
        #[test]
        fn test_count_multiple_combines(counts in prop::collection::vec(0usize..1000, 1..20)) {
            // Test that combining N counts in any grouping gives same result
            // Use smaller values to avoid overflow in test itself
            let total: usize = counts.iter().sum();

            let result = counts
                .into_iter()
                .map(AggregateResult::Count)
                .reduce(|a, b| a.combine(b))
                .unwrap();

            prop_assert_eq!(result, AggregateResult::Count(total));
        }
    }

    proptest! {
        #[test]
        fn test_sum_multiple_combines(
            sums in prop::collection::vec(
                any::<f64>().prop_filter("must be finite", |f| f.is_finite()),
                1..20
            )
        ) {
            let total: f64 = sums.iter().sum();

            let result = sums
                .into_iter()
                .map(AggregateResult::Sum)
                .reduce(|a, b| a.combine(b))
                .unwrap();

            match result {
                AggregateResult::Sum(s) => {
                    prop_assert!((s - total).abs() < 0.0001 || (s.is_nan() && total.is_nan()));
                }
                _ => panic!("Expected Sum"),
            }
        }
    }

    proptest! {
        #[test]
        fn test_concat_associativity_with_strings(
            a in any::<String>(),
            b in any::<String>(),
            c in any::<String>(),
        ) {
            let ar_a = AggregateResult::Concat(a.clone());
            let ar_b = AggregateResult::Concat(b.clone());
            let ar_c = AggregateResult::Concat(c.clone());

            let left = ar_a.clone().combine(ar_b.clone()).combine(ar_c.clone());
            let right = ar_a.combine(ar_b.combine(ar_c));

            // Both should produce a + b + c
            let expected = format!("{}{}{}", a, b, c);

            match (left, right) {
                (AggregateResult::Concat(l), AggregateResult::Concat(r)) => {
                    prop_assert_eq!(l, expected.clone());
                    prop_assert_eq!(r, expected);
                }
                _ => panic!("Expected Concat"),
            }
        }
    }

    proptest! {
        #[test]
        fn test_average_combine_preserves_correctness(
            values in prop::collection::vec(
                // Use smaller range to avoid floating point precision issues
                -1000.0f64..1000.0f64,
                1..20
            )
        ) {
            // Split values into two groups and combine
            let mid = values.len() / 2;
            let (left_vals, right_vals) = values.split_at(mid);

            let left_sum: f64 = left_vals.iter().sum();
            let left_count = left_vals.len();

            let right_sum: f64 = right_vals.iter().sum();
            let right_count = right_vals.len();

            let left = AggregateResult::Average(left_sum, left_count);
            let right = AggregateResult::Average(right_sum, right_count);

            let combined = left.combine(right);

            match combined {
                AggregateResult::Average(total_sum, total_count) => {
                    let expected_sum: f64 = values.iter().sum();
                    let expected_count = values.len();

                    prop_assert_eq!(total_count, expected_count);
                    // Use relative tolerance for floating point comparison
                    let tolerance = expected_sum.abs() * 0.0001 + 0.0001;
                    prop_assert!((total_sum - expected_sum).abs() < tolerance);
                }
                _ => panic!("Expected Average"),
            }
        }
    }

    proptest! {
        #[test]
        fn test_unique_idempotence(strings in prop::collection::hash_set(any::<String>(), 0..10)) {
            // Combining unique sets should preserve uniqueness
            let a = AggregateResult::Unique(strings.clone());
            let b = AggregateResult::Unique(strings.clone());

            let combined = a.combine(b);

            match combined {
                AggregateResult::Unique(result_set) => {
                    // Should have same elements (set union is idempotent)
                    prop_assert!(strings.is_subset(&result_set));
                    prop_assert!(result_set.is_subset(&strings));
                }
                _ => panic!("Expected Unique"),
            }
        }
    }
}
