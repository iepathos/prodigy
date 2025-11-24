//! Performance benchmarks for semigroup-based variable aggregation
//! Verifies that semigroup pattern has no significant performance regression vs custom implementations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use prodigy::cook::execution::variables::{
    aggregate_results, aggregate_with_initial, parallel_aggregate, AggregateResult,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use stillwater::Semigroup;

/// Baseline: Custom count merge (pre-semigroup approach)
fn custom_merge_count(counts: Vec<usize>) -> usize {
    counts.into_iter().sum()
}

/// Baseline: Custom collection merge (pre-semigroup approach)
fn custom_merge_collect(collections: Vec<Vec<Value>>) -> Vec<Value> {
    collections.into_iter().flatten().collect()
}

/// Baseline: Custom average merge (pre-semigroup approach)
fn custom_merge_averages(averages: Vec<(f64, usize)>) -> f64 {
    let (total_sum, total_count) = averages
        .into_iter()
        .fold((0.0, 0), |(sum, count), (avg_sum, avg_count)| {
            (sum + avg_sum, count + avg_count)
        });
    if total_count == 0 {
        0.0
    } else {
        total_sum / total_count as f64
    }
}

/// Benchmark count aggregation
fn bench_count_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("count_aggregation");

    for size in [10, 100, 1000, 10000] {
        // Baseline: Custom implementation
        group.bench_with_input(BenchmarkId::new("custom", size), &size, |b, &size| {
            b.iter(|| {
                let counts: Vec<usize> = (0..size).map(|_| 1).collect();
                black_box(custom_merge_count(counts))
            });
        });

        // Semigroup: Sequential
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> =
                    (0..size).map(|_| AggregateResult::Count(1)).collect();
                black_box(aggregate_results(results))
            });
        });

        // Semigroup: Parallel (for large datasets)
        if size >= 1000 {
            group.bench_with_input(
                BenchmarkId::new("semigroup_parallel", size),
                &size,
                |b, &size| {
                    b.iter(|| {
                        let results: Vec<AggregateResult> =
                            (0..size).map(|_| AggregateResult::Count(1)).collect();
                        black_box(parallel_aggregate(results))
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark sum aggregation
fn bench_sum_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum_aggregation");

    for size in [10, 100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> =
                    (0..size).map(|i| AggregateResult::Sum(i as f64)).collect();
                black_box(aggregate_results(results))
            });
        });
    }

    group.finish();
}

/// Benchmark collection aggregation
fn bench_collect_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("collect_aggregation");

    for size in [10, 100, 1000] {
        // Baseline: Custom implementation
        group.bench_with_input(BenchmarkId::new("custom", size), &size, |b, &size| {
            b.iter(|| {
                let collections: Vec<Vec<Value>> = (0..size).map(|i| vec![json!(i)]).collect();
                black_box(custom_merge_collect(collections))
            });
        });

        // Semigroup implementation
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> = (0..size)
                    .map(|i| AggregateResult::Collect(vec![json!(i)]))
                    .collect();
                black_box(aggregate_results(results))
            });
        });
    }

    group.finish();
}

/// Benchmark average aggregation
fn bench_average_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("average_aggregation");

    for size in [10, 100, 1000, 10000] {
        // Baseline: Custom implementation
        group.bench_with_input(BenchmarkId::new("custom", size), &size, |b, &size| {
            b.iter(|| {
                let averages: Vec<(f64, usize)> = (0..size).map(|i| (i as f64, 1)).collect();
                black_box(custom_merge_averages(averages))
            });
        });

        // Semigroup implementation
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> = (0..size)
                    .map(|i| AggregateResult::Average(i as f64, 1))
                    .collect();
                let combined = aggregate_results(results);
                black_box(combined.map(|r| r.finalize()))
            });
        });
    }

    group.finish();
}

/// Benchmark unique aggregation
fn bench_unique_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("unique_aggregation");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> = (0..size)
                    .map(|i| {
                        let mut set = HashSet::new();
                        set.insert(format!("item_{}", i % (size / 2))); // Some duplicates
                        AggregateResult::Unique(set)
                    })
                    .collect();
                black_box(aggregate_results(results))
            });
        });
    }

    group.finish();
}

/// Benchmark merge (object) aggregation
fn bench_merge_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_aggregation");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> = (0..size)
                    .map(|i| {
                        let mut map = HashMap::new();
                        map.insert(format!("key_{}", i), json!(i));
                        AggregateResult::Merge(map)
                    })
                    .collect();
                black_box(aggregate_results(results))
            });
        });
    }

    group.finish();
}

/// Benchmark concat aggregation
fn bench_concat_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("concat_aggregation");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> = (0..size)
                    .map(|_| AggregateResult::Concat("x".to_string()))
                    .collect();
                black_box(aggregate_results(results))
            });
        });
    }

    group.finish();
}

/// Benchmark min/max aggregation
fn bench_min_max_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("min_max_aggregation");

    for size in [10, 100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("min", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> =
                    (0..size).map(|i| AggregateResult::Min(json!(i))).collect();
                black_box(aggregate_results(results))
            });
        });

        group.bench_with_input(BenchmarkId::new("max", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> =
                    (0..size).map(|i| AggregateResult::Max(json!(i))).collect();
                black_box(aggregate_results(results))
            });
        });
    }

    group.finish();
}

/// Benchmark median aggregation (stateful)
fn bench_median_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("median_aggregation");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("semigroup", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<AggregateResult> = (0..size)
                    .map(|i| AggregateResult::Median(vec![i as f64]))
                    .collect();
                let combined = aggregate_results(results);
                black_box(combined.map(|r| r.finalize()))
            });
        });
    }

    group.finish();
}

/// Benchmark aggregate_with_initial
fn bench_aggregate_with_initial(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregate_with_initial");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("count", size), &size, |b, &size| {
            b.iter(|| {
                let initial = AggregateResult::Count(100);
                let results: Vec<AggregateResult> =
                    (0..size).map(|_| AggregateResult::Count(1)).collect();
                black_box(aggregate_with_initial(initial, results))
            });
        });
    }

    group.finish();
}

/// Benchmark direct combine calls (low-level)
fn bench_direct_combine(c: &mut Criterion) {
    let mut group = c.benchmark_group("direct_combine");

    group.bench_function("count", |b| {
        b.iter(|| {
            let a = AggregateResult::Count(100);
            let b = AggregateResult::Count(50);
            black_box(a.combine(b))
        });
    });

    group.bench_function("sum", |b| {
        b.iter(|| {
            let a = AggregateResult::Sum(100.5);
            let b = AggregateResult::Sum(50.3);
            black_box(a.combine(b))
        });
    });

    group.bench_function("average", |b| {
        b.iter(|| {
            let a = AggregateResult::Average(100.0, 10);
            let b = AggregateResult::Average(50.0, 5);
            black_box(a.combine(b))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_count_aggregation,
    bench_sum_aggregation,
    bench_collect_aggregation,
    bench_average_aggregation,
    bench_unique_aggregation,
    bench_merge_aggregation,
    bench_concat_aggregation,
    bench_min_max_aggregation,
    bench_median_aggregation,
    bench_aggregate_with_initial,
    bench_direct_combine,
);

criterion_main!(benches);
