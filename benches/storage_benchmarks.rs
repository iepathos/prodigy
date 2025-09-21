//! Performance benchmarks for storage operations

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use prodigy::storage::GlobalStorage;
use serde_json::json;
use std::hint::black_box;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create test JSON data of varying sizes
fn create_test_json(size: usize) -> serde_json::Value {
    let mut data = vec![];
    for i in 0..size {
        data.push(json!({
            "id": i,
            "name": format!("item_{}", i),
            "data": "x".repeat(100),
            "metadata": {
                "created": "2024-01-01",
                "tags": vec!["test", "benchmark"],
            }
        }));
    }
    json!(data)
}

fn bench_storage_write(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("storage_write");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("write_json", size), size, |b, &size| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = GlobalStorage::new().unwrap();
                    let data = create_test_json(size);
                    (storage, data, temp_dir)
                },
                |(storage, data, _temp_dir)| async move {
                    let path = storage
                        .get_state_dir("test-repo", "test-job")
                        .await
                        .unwrap()
                        .join("test.json");
                    tokio::fs::create_dir_all(path.parent().unwrap())
                        .await
                        .unwrap();
                    tokio::fs::write(path, serde_json::to_string(&data).unwrap())
                        .await
                        .unwrap();
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_storage_read(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("storage_read");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("read_json", size), size, |b, &size| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = GlobalStorage::new().unwrap();
                    let data = create_test_json(size);
                    let rt_local = Runtime::new().unwrap();

                    // Pre-write data
                    rt_local.block_on(async {
                        let path = storage
                            .get_state_dir("test-repo", "test-job")
                            .await
                            .unwrap()
                            .join("test.json");
                        tokio::fs::create_dir_all(path.parent().unwrap())
                            .await
                            .unwrap();
                        tokio::fs::write(path, serde_json::to_string(&data).unwrap())
                            .await
                            .unwrap();
                    });

                    (storage, temp_dir)
                },
                |(storage, _temp_dir)| async move {
                    let path = storage
                        .get_state_dir("test-repo", "test-job")
                        .await
                        .unwrap()
                        .join("test.json");
                    let content = tokio::fs::read_to_string(path).await.unwrap();
                    let data: serde_json::Value = serde_json::from_str(&content).unwrap();
                    black_box(data);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_directory_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.benchmark_group("directory_operations")
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .bench_function("create_directory_tree", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = GlobalStorage::new().unwrap();
                    (storage, temp_dir)
                },
                |(storage, _temp_dir)| async move {
                    // Create typical directory structure
                    let dirs = vec![
                        storage.get_events_dir("test-repo", "job1").await.unwrap(),
                        storage.get_dlq_dir("test-repo", "job1").await.unwrap(),
                        storage.get_state_dir("test-repo", "job1").await.unwrap(),
                    ];

                    for dir in dirs {
                        tokio::fs::create_dir_all(dir).await.unwrap();
                    }
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("list_directory_contents", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = GlobalStorage::new().unwrap();
                    let rt_local = Runtime::new().unwrap();

                    // Create files to list
                    rt_local.block_on(async {
                        let dir = storage.get_state_dir("test-repo", "job1").await.unwrap();
                        tokio::fs::create_dir_all(&dir).await.unwrap();
                        for i in 0..20 {
                            let path = dir.join(format!("file_{}.json", i));
                            tokio::fs::write(path, format!("{{\"id\": {}}}", i))
                                .await
                                .unwrap();
                        }
                    });

                    (storage, temp_dir)
                },
                |(storage, _temp_dir)| async move {
                    let dir = storage.get_state_dir("test-repo", "job1").await.unwrap();
                    let mut entries = tokio::fs::read_dir(dir).await.unwrap();
                    let mut files = vec![];
                    while let Some(entry) = entries.next_entry().await.unwrap() {
                        files.push(entry.file_name());
                    }
                    black_box(files);
                },
                BatchSize::SmallInput,
            );
        });
}

criterion_group!(
    benches,
    bench_storage_write,
    bench_storage_read,
    bench_directory_operations
);

criterion_main!(benches);
