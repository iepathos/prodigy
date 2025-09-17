//! Performance benchmarks for expression evaluation
//!
//! This benchmark suite measures the performance of filter and sort
//! expression evaluation to ensure we meet the spec requirements
//! of filtering 10,000 items in under 100ms.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use prodigy::cook::execution::expression::{
    evaluator::ExpressionEvaluator,
    parser::{parse_expression, parse_sort_expression},
};
use serde_json::json;
use std::hint::black_box;

/// Benchmark filtering performance with various dataset sizes
fn bench_filter_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_performance");

    // Test different dataset sizes
    for size in &[100, 1000, 5000, 10000, 50000] {
        // Create test data
        let items: Vec<serde_json::Value> = (0..*size)
            .map(|i| {
                json!({
                    "id": i,
                    "name": format!("item_{}", i),
                    "score": i % 100,
                    "active": i % 2 == 0,
                    "category": match i % 3 {
                        0 => "A",
                        1 => "B",
                        _ => "C"
                    },
                    "price": (i as f64) * 1.5,
                    "tags": vec![format!("tag{}", i % 5), format!("tag{}", i % 7)]
                })
            })
            .collect();

        // Simple numeric filter: score > 50
        group.bench_with_input(
            BenchmarkId::new("numeric_filter", size),
            &items,
            |b, items| {
                let expr = parse_expression("score > 50").unwrap();
                let evaluator = ExpressionEvaluator::new(expr);
                b.iter(|| {
                    let filtered: Vec<_> = items
                        .iter()
                        .filter(|item| evaluator.evaluate(black_box(item)).unwrap_or(false))
                        .collect();
                    black_box(filtered);
                });
            },
        );

        // Complex filter: (score > 50 AND active == true) OR category == "A"
        group.bench_with_input(
            BenchmarkId::new("complex_filter", size),
            &items,
            |b, items| {
                let expr = parse_expression("(score > 50 AND active == true) OR category == \"A\"").unwrap();
                let evaluator = ExpressionEvaluator::new(expr);
                b.iter(|| {
                    let filtered: Vec<_> = items
                        .iter()
                        .filter(|item| evaluator.evaluate(black_box(item)).unwrap_or(false))
                        .collect();
                    black_box(filtered);
                });
            },
        );

        // String contains filter: name CONTAINS "5"
        group.bench_with_input(
            BenchmarkId::new("string_contains_filter", size),
            &items,
            |b, items| {
                let expr = parse_expression("name CONTAINS \"5\"").unwrap();
                let evaluator = ExpressionEvaluator::new(expr);
                b.iter(|| {
                    let filtered: Vec<_> = items
                        .iter()
                        .filter(|item| evaluator.evaluate(black_box(item)).unwrap_or(false))
                        .collect();
                    black_box(filtered);
                });
            },
        );

        // Array membership filter: tags CONTAINS "tag3"
        group.bench_with_input(
            BenchmarkId::new("array_contains_filter", size),
            &items,
            |b, items| {
                let expr = parse_expression("tags CONTAINS \"tag3\"").unwrap();
                let evaluator = ExpressionEvaluator::new(expr);
                b.iter(|| {
                    let filtered: Vec<_> = items
                        .iter()
                        .filter(|item| evaluator.evaluate(black_box(item)).unwrap_or(false))
                        .collect();
                    black_box(filtered);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark sorting performance with various dataset sizes
fn bench_sort_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("sort_performance");

    for size in &[100, 1000, 5000, 10000] {
        // Create test data
        let mut items: Vec<serde_json::Value> = (0..*size)
            .map(|i| {
                json!({
                    "id": i,
                    "name": format!("item_{}", i),
                    "score": (i * 7) % 100,  // Scrambled order
                    "price": ((i * 13) % 1000) as f64 / 10.0,
                    "category": match (i * 3) % 4 {
                        0 => "A",
                        1 => "B",
                        2 => "C",
                        _ => "D"
                    },
                    "created_at": format!("2024-{:02}-{:02}", 1 + (i % 12), 1 + (i % 28))
                })
            })
            .collect();

        // Simple numeric sort: score DESC
        group.bench_with_input(
            BenchmarkId::new("numeric_sort", size),
            &items,
            |b, items| {
                let sort_expr = parse_sort_expression("score DESC").unwrap();
                b.iter(|| {
                    let mut sorted = items.clone();
                    sorted.sort_by(|a, b| {
                        sort_expr.compare(black_box(a), black_box(b))
                    });
                    black_box(sorted);
                });
            },
        );

        // Multi-field sort: category ASC, score DESC
        group.bench_with_input(
            BenchmarkId::new("multi_field_sort", size),
            &items,
            |b, items| {
                let sort_expr = parse_sort_expression("category ASC, score DESC").unwrap();
                b.iter(|| {
                    let mut sorted = items.clone();
                    sorted.sort_by(|a, b| {
                        sort_expr.compare(black_box(a), black_box(b))
                    });
                    black_box(sorted);
                });
            },
        );

        // String sort: name ASC
        group.bench_with_input(
            BenchmarkId::new("string_sort", size),
            &items,
            |b, items| {
                let sort_expr = parse_sort_expression("name ASC").unwrap();
                b.iter(|| {
                    let mut sorted = items.clone();
                    sorted.sort_by(|a, b| {
                        sort_expr.compare(black_box(a), black_box(b))
                    });
                    black_box(sorted);
                });
            },
        );

        // Date string sort: created_at DESC
        group.bench_with_input(
            BenchmarkId::new("date_string_sort", size),
            &items,
            |b, items| {
                let sort_expr = parse_sort_expression("created_at DESC").unwrap();
                b.iter(|| {
                    let mut sorted = items.clone();
                    sorted.sort_by(|a, b| {
                        sort_expr.compare(black_box(a), black_box(b))
                    });
                    black_box(sorted);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark combined filter and sort operations (typical real-world usage)
fn bench_filter_and_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_and_sort");

    // Create 10,000 items (spec requirement)
    let items: Vec<serde_json::Value> = (0..10000)
        .map(|i| {
            json!({
                "id": i,
                "name": format!("product_{}", i),
                "price": ((i * 17) % 10000) as f64 / 100.0,
                "stock": (i * 13) % 100,
                "available": i % 3 != 0,
                "category": match i % 5 {
                    0 => "electronics",
                    1 => "clothing",
                    2 => "books",
                    3 => "home",
                    _ => "other"
                },
                "rating": ((i * 7) % 50) as f64 / 10.0,
                "reviews": (i * 11) % 1000,
                "tags": vec![
                    format!("tag{}", i % 10),
                    format!("brand{}", i % 20),
                    format!("style{}", i % 15)
                ]
            })
        })
        .collect();

    // Scenario 1: Filter available items and sort by price
    group.bench_function("filter_available_sort_price", |b| {
        let filter = parse_expression("available == true AND stock > 0").unwrap();
        let evaluator = ExpressionEvaluator::new(filter);
        let sort = parse_sort_expression("price ASC").unwrap();

        b.iter(|| {
            let mut result: Vec<_> = items
                .iter()
                .filter(|item| evaluator.evaluate(black_box(item)).unwrap_or(false))
                .cloned()
                .collect();
            result.sort_by(|a, b| sort.compare(a, b));
            black_box(result);
        });
    });

    // Scenario 2: Complex filter with multi-field sort
    group.bench_function("complex_filter_multi_sort", |b| {
        let filter = parse_expression("(category == \"electronics\" OR category == \"books\") AND rating >= 4.0 AND price < 100").unwrap();
        let evaluator = ExpressionEvaluator::new(filter);
        let sort = parse_sort_expression("rating DESC, reviews DESC, price ASC").unwrap();

        b.iter(|| {
            let mut result: Vec<_> = items
                .iter()
                .filter(|item| evaluator.evaluate(black_box(item)).unwrap_or(false))
                .cloned()
                .collect();
            result.sort_by(|a, b| sort.compare(a, b));
            black_box(result);
        });
    });

    // Scenario 3: Tag-based filter with string sort
    group.bench_function("tag_filter_name_sort", |b| {
        let filter = parse_expression("tags CONTAINS \"tag5\" AND reviews > 50").unwrap();
        let evaluator = ExpressionEvaluator::new(filter);
        let sort = parse_sort_expression("name ASC").unwrap();

        b.iter(|| {
            let mut result: Vec<_> = items
                .iter()
                .filter(|item| evaluator.evaluate(black_box(item)).unwrap_or(false))
                .cloned()
                .collect();
            result.sort_by(|a, b| sort.compare(a, b));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark expression parsing performance
fn bench_expression_parsing(c: &mut Criterion) {
    c.bench_function("parse_simple_expression", |b| {
        b.iter(|| {
            let _expr = parse_expression(black_box("score > 50"));
        });
    });

    c.bench_function("parse_complex_expression", |b| {
        b.iter(|| {
            let _expr = parse_expression(black_box(
                "(category == \"electronics\" AND price < 1000 AND rating >= 4.5) OR (featured == true AND stock > 0)"
            ));
        });
    });

    c.bench_function("parse_simple_sort", |b| {
        b.iter(|| {
            let _expr = parse_sort_expression(black_box("price DESC"));
        });
    });

    c.bench_function("parse_multi_field_sort", |b| {
        b.iter(|| {
            let _expr = parse_sort_expression(black_box("category ASC, rating DESC, price ASC, name DESC"));
        });
    });
}

criterion_group!(
    benches,
    bench_filter_performance,
    bench_sort_performance,
    bench_filter_and_sort,
    bench_expression_parsing
);

criterion_main!(benches);