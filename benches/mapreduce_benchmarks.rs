//! Performance benchmarks for MapReduce operations
//! Measures work item distribution, agent coordination, and cross-worktree synchronization

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION;
use prodigy::storage::GlobalStorage;
use serde_json::json;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Simulated work item structure for benchmarking
#[derive(Clone, Debug)]
struct WorkItem {
    id: String,
    data: serde_json::Value,
    retries: u32,
    max_retries: u32,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    status: String,
    error: Option<String>,
    result: Option<serde_json::Value>,
}

/// Create test work items with varying complexity
fn create_work_items(count: usize, size: &str) -> Vec<WorkItem> {
    (0..count)
        .map(|i| {
            let data = match size {
                "small" => json!({
                    "id": i,
                    "type": "small",
                    "data": format!("item_{}", i)
                }),
                "medium" => json!({
                    "id": i,
                    "type": "medium",
                    "data": format!("item_{}", i),
                    "nested": {
                        "field1": format!("value_{}", i),
                        "field2": i * 2,
                        "field3": vec![i; 10]
                    }
                }),
                "large" => json!({
                    "id": i,
                    "type": "large",
                    "data": format!("item_{}", i),
                    "nested": {
                        "field1": format!("value_{}", i),
                        "field2": i * 2,
                        "field3": vec![i; 100],
                        "field4": (0..50).map(|j| json!({"sub": j})).collect::<Vec<_>>()
                    },
                    "metadata": {
                        "created_at": "2024-01-01",
                        "tags": vec![format!("tag_{}", i % 10); 20]
                    }
                }),
                _ => panic!("Invalid size"),
            };
            WorkItem {
                id: format!("item_{}", i),
                data,
                retries: 0,
                max_retries: 3,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                status: "pending".to_string(),
                error: None,
                result: None,
            }
        })
        .collect()
}

/// Simulated MapReduce job structure
#[derive(Clone, Debug)]
struct MapReduceJob {
    job_id: String,
    workflow_id: String,
    input_file: PathBuf,
    json_path: String,
    filter: Option<String>,
    sort_by: Option<String>,
    max_items: Option<usize>,
    max_parallel: usize,
    agent_commands: Vec<String>,
    reduce_commands: Vec<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    status: String,
    completed_agents: usize,
    total_agents: usize,
    successful_items: usize,
    failed_items: usize,
    error: Option<String>,
    checkpoint_version: u32,
    results: Vec<serde_json::Value>,
    correlation_id: String,
    timeout_minutes: Option<u32>,
    retry_on_failure: bool,
}

/// Create a test MapReduce job
fn create_mapreduce_job(num_items: usize, max_parallel: usize) -> MapReduceJob {
    MapReduceJob {
        job_id: format!("bench-job-{}", uuid::Uuid::new_v4()),
        workflow_id: "benchmark-workflow".to_string(),
        input_file: PathBuf::from("benchmark-input.json"),
        json_path: "$.items[*]".to_string(),
        filter: None,
        sort_by: None,
        max_items: Some(num_items),
        max_parallel,
        agent_commands: vec!["echo 'Processing ${item.id}'".to_string()],
        reduce_commands: vec!["echo 'Reducing results'".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: "pending".to_string(),
        completed_agents: 0,
        total_agents: num_items,
        successful_items: 0,
        failed_items: 0,
        error: None,
        checkpoint_version: CHECKPOINT_VERSION,
        results: Vec::new(),
        correlation_id: uuid::Uuid::new_v4().to_string(),
        timeout_minutes: None,
        retry_on_failure: false,
    }
}

fn bench_work_item_distribution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("work_item_distribution");

    for size in &[10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("distribute", size),
            size,
            |b, &num_items| {
                b.to_async(&rt).iter_batched(
                    || {
                        let items = create_work_items(num_items, "medium");
                        let temp_dir = TempDir::new().unwrap();
                        let storage = GlobalStorage::new(temp_dir.path()).unwrap();
                        (items, storage, temp_dir)
                    },
                    |(items, _storage, _temp_dir)| async move {
                        // Simulate work item distribution
                        let chunks: Vec<_> = items
                            .chunks(items.len() / 10.max(1))
                            .map(|chunk| chunk.to_vec())
                            .collect();
                        black_box(chunks);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_agent_coordination(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("agent_coordination");

    for parallel in &[2, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("coordinate", parallel),
            parallel,
            |b, &max_parallel| {
                b.to_async(&rt).iter_batched(
                    || {
                        let job = create_mapreduce_job(100, max_parallel);
                        let temp_dir = TempDir::new().unwrap();
                        let storage = GlobalStorage::new(temp_dir.path()).unwrap();
                        (job, storage, temp_dir)
                    },
                    |(job, _storage, temp_dir)| async move {
                        // Simulate context creation
                        let context = (
                            job.job_id.clone(),
                            job.workflow_id.clone(),
                            job.correlation_id.clone(),
                            0,
                            job.total_agents,
                            temp_dir.path().to_path_buf(),
                        );
                        // Simulate agent coordination logic
                        black_box(context);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_event_logging_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("event_logging");

    for batch_size in &[1, 10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("log_events", batch_size),
            batch_size,
            |b, &num_events| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = GlobalStorage::new(temp_dir.path()).unwrap();
                        let events: Vec<_> = (0..num_events)
                            .map(|i| {
                                json!({
                                    "event_type": "agent_progress",
                                    "agent_id": format!("agent_{}", i % 10),
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                    "data": {
                                        "items_processed": i,
                                        "items_remaining": num_events - i
                                    }
                                })
                            })
                            .collect();
                        (storage, events, temp_dir)
                    },
                    |(storage, events, _temp_dir)| async move {
                        // Simulate event logging to storage
                        for event in events {
                            // Write to events directory
                            let _ = storage.get_events_dir("bench-job").await;
                            black_box(event);
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_cross_worktree_sync(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("cross_worktree_sync");

    for num_worktrees in &[2, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("sync_events", num_worktrees),
            num_worktrees,
            |b, &num_trees| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let base_storage = GlobalStorage::new(temp_dir.path()).unwrap();
                        let job_id = format!("sync-job-{}", uuid::Uuid::new_v4());

                        // Create multiple storages simulating different worktrees
                        let storages: Vec<_> = (0..num_trees)
                            .map(|_| GlobalStorage::new(temp_dir.path()).unwrap())
                            .collect();

                        (base_storage, storages, job_id, temp_dir)
                    },
                    |(base_storage, storages, job_id, _temp_dir)| async move {
                        // Simulate cross-worktree event aggregation
                        for (i, storage) in storages.iter().enumerate() {
                            let _ = storage.get_events_dir(&job_id).await;
                            black_box(i);
                        }

                        // Read aggregated events
                        let _ = base_storage.get_events_dir(&job_id).await;
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_work_item_filtering(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.benchmark_group("work_item_filtering")
        .bench_function("filter_1000_items", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let items = create_work_items(1000, "medium");
                    // Simulate filter
                    (items, "item.nested.field2 > 500".to_string())
                },
                |(items, _filter)| async move {
                    let filtered: Vec<_> = items
                        .into_iter()
                        .filter(|item| {
                            // Simple filtering logic
                            item.data["nested"]["field2"]
                                .as_i64()
                                .map(|v| v > 500)
                                .unwrap_or(false)
                        })
                        .collect();
                    black_box(filtered);
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("sort_1000_items", |b| {
            b.to_async(&rt).iter_batched(
                || create_work_items(1000, "medium"),
                |mut items| async move {
                    items.sort_by_key(|item| item.data["id"].as_i64().unwrap_or(0));
                    black_box(items);
                },
                BatchSize::SmallInput,
            );
        });
}

fn bench_dlq_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dlq_operations");

    group.bench_function("add_to_dlq", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let storage = GlobalStorage::new(temp_dir.path()).unwrap();
                let items = create_work_items(100, "small");
                (storage, items, temp_dir)
            },
            |(storage, items, _temp_dir)| async move {
                // Simulate DLQ operations
                for item in items {
                    let _ = storage.get_dlq_dir("bench-job").await;
                    black_box(item.data);
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("read_from_dlq", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let storage = GlobalStorage::new(temp_dir.path()).unwrap();
                let items = create_work_items(100, "small");

                let rt_local = Runtime::new().unwrap();
                rt_local.block_on(async {
                    for _item in items {
                        let _ = storage.get_dlq_dir("bench-job").await;
                    }
                });

                (storage, temp_dir)
            },
            |(storage, _temp_dir)| async move {
                let _ = storage.get_dlq_dir("bench-job").await;
                let _ = storage.list_dlq_job_ids().await;
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_agent_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("agent_scaling");
    group.measurement_time(Duration::from_secs(10));

    for agents in &[1, 5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("scale_agents", agents),
            agents,
            |b, &num_agents| {
                b.to_async(&rt).iter_batched(
                    || {
                        let job = create_mapreduce_job(num_agents * 10, num_agents);
                        let temp_dir = TempDir::new().unwrap();
                        let storage = GlobalStorage::new(temp_dir.path()).unwrap();
                        (job, storage, temp_dir)
                    },
                    |(job, _storage, temp_dir)| async move {
                        // Simulate agent lifecycle management
                        let contexts: Vec<_> = (0..job.max_parallel)
                            .map(|i| {
                                (
                                    job.job_id.clone(),
                                    job.workflow_id.clone(),
                                    job.correlation_id.clone(),
                                    i,
                                    job.total_agents,
                                    temp_dir.path().to_path_buf(),
                                )
                            })
                            .collect();
                        black_box(contexts);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_work_item_distribution,
    bench_agent_coordination,
    bench_event_logging_throughput,
    bench_cross_worktree_sync,
    bench_work_item_filtering,
    bench_dlq_operations,
    bench_agent_scaling
);

criterion_main!(benches);
