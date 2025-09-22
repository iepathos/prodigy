//! Performance benchmarks for cloning optimizations
//!
//! Tests the performance improvements from spec 104 implementation

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use prodigy::cook::environment::path_resolver::{PathResolver, Platform};
use std::sync::Arc;
use std::collections::HashMap;

/// Benchmark path resolution with Cow optimizations
fn bench_path_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_resolution");

    let test_paths = vec![
        "/home/user/project/src/main.rs",
        "~/Documents/code/test.txt",
        "$HOME/Downloads/file.zip",
        "${USER}/workspace/build",
        "C:\\Users\\John\\Desktop\\file.txt",
    ];

    let resolver = PathResolver::new();

    group.bench_function("path_no_expansion", |b| {
        b.iter(|| {
            for path in &test_paths[0..1] {
                black_box(resolver.resolve(path));
            }
        });
    });

    group.bench_function("path_with_home", |b| {
        b.iter(|| {
            black_box(resolver.resolve(&test_paths[1]));
        });
    });

    group.bench_function("path_with_env_vars", |b| {
        b.iter(|| {
            for path in &test_paths[2..4] {
                black_box(resolver.resolve(path));
            }
        });
    });

    group.finish();
}

/// Benchmark Arc<str> vs String cloning performance
fn bench_string_cloning(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_cloning");

    // Test different string sizes
    let small_str = "small";
    let medium_str = "This is a medium length string for testing";
    let large_str = "a".repeat(1000);

    for (name, test_str) in [
        ("small", small_str),
        ("medium", medium_str),
        ("large", &large_str),
    ] {
        // Benchmark String cloning
        let string_version = test_str.to_string();
        group.bench_with_input(
            BenchmarkId::new("String_clone", name),
            &string_version,
            |b, s| b.iter(|| black_box(s.clone()))
        );

        // Benchmark Arc<str> cloning
        let arc_version: Arc<str> = Arc::from(test_str);
        group.bench_with_input(
            BenchmarkId::new("Arc_str_clone", name),
            &arc_version,
            |b, s| b.iter(|| black_box(Arc::clone(s)))
        );
    }

    group.finish();
}

/// Benchmark HashMap<String, String> vs Arc<HashMap> performance
fn bench_hashmap_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_operations");

    // Create test data
    let mut map = HashMap::new();
    for i in 0..100 {
        map.insert(format!("key_{}", i), format!("value_{}", i));
    }

    // Benchmark HashMap clone
    group.bench_function("HashMap_clone", |b| {
        b.iter(|| black_box(map.clone()));
    });

    // Benchmark Arc<HashMap> clone
    let arc_map = Arc::new(map.clone());
    group.bench_function("Arc_HashMap_clone", |b| {
        b.iter(|| black_box(Arc::clone(&arc_map)));
    });

    // Benchmark shared access patterns
    group.bench_function("HashMap_multiple_readers", |b| {
        b.iter(|| {
            let map1 = map.clone();
            let map2 = map.clone();
            let map3 = map.clone();
            black_box((map1.get("key_50"), map2.get("key_50"), map3.get("key_50")));
        });
    });

    group.bench_function("Arc_HashMap_multiple_readers", |b| {
        b.iter(|| {
            let map1 = Arc::clone(&arc_map);
            let map2 = Arc::clone(&arc_map);
            let map3 = Arc::clone(&arc_map);
            black_box((map1.get("key_50"), map2.get("key_50"), map3.get("key_50")));
        });
    });

    group.finish();
}

/// Memory allocation benchmarks for different cloning strategies
fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    // Simulate workflow data structure
    #[derive(Clone)]
    struct WorkflowData {
        id: String,
        name: String,
        description: String,
        variables: HashMap<String, String>,
    }

    struct OptimizedWorkflowData {
        id: Arc<str>,
        name: Arc<str>,
        description: Arc<str>,
        variables: Arc<HashMap<String, String>>,
    }

    impl Clone for OptimizedWorkflowData {
        fn clone(&self) -> Self {
            Self {
                id: Arc::clone(&self.id),
                name: Arc::clone(&self.name),
                description: Arc::clone(&self.description),
                variables: Arc::clone(&self.variables),
            }
        }
    }

    let mut vars = HashMap::new();
    for i in 0..20 {
        vars.insert(format!("var_{}", i), format!("value_{}", i));
    }

    let original = WorkflowData {
        id: "workflow-123".to_string(),
        name: "Test Workflow".to_string(),
        description: "A test workflow for benchmarking".to_string(),
        variables: vars.clone(),
    };

    let optimized = OptimizedWorkflowData {
        id: Arc::from("workflow-123"),
        name: Arc::from("Test Workflow"),
        description: Arc::from("A test workflow for benchmarking"),
        variables: Arc::new(vars),
    };

    group.bench_function("original_clone", |b| {
        b.iter(|| black_box(original.clone()));
    });

    group.bench_function("optimized_clone", |b| {
        b.iter(|| black_box(optimized.clone()));
    });

    // Simulate multiple agents sharing data
    group.bench_function("original_10_agents", |b| {
        b.iter(|| {
            let agents: Vec<_> = (0..10).map(|_| original.clone()).collect();
            black_box(agents);
        });
    });

    group.bench_function("optimized_10_agents", |b| {
        b.iter(|| {
            let agents: Vec<_> = (0..10).map(|_| optimized.clone()).collect();
            black_box(agents);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_path_resolution,
    bench_string_cloning,
    bench_hashmap_operations,
    bench_memory_allocation
);
criterion_main!(benches);