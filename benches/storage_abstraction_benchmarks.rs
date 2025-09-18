//! Performance benchmarks comparing storage abstraction vs direct access

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use prodigy::storage::{
    backends::{FileBackend, MemoryBackend},
    config::{BackendConfig, BackendType, FileConfig, MemoryConfig, StorageConfig},
    factory::StorageFactory,
    traits::{EventStorage, SessionStorage, StateStorage, UnifiedStorage},
    types::{
        CheckpointData, EventEntry, JobState, JobStatus, SessionState, SessionStatus,
        WorkflowCheckpoint,
    },
};
use serde_json::json;
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use uuid::Uuid;

/// Create a test session
fn create_test_session(id: &str) -> SessionState {
    SessionState {
        session_id: id.to_string(),
        repository: "bench-repo".to_string(),
        status: SessionStatus::InProgress,
        started_at: chrono::Utc::now(),
        completed_at: None,
        workflow_path: Some("/test/workflow.yaml".to_string()),
        git_branch: Some("test-branch".to_string()),
        iterations_completed: 0,
        files_changed: 0,
        worktree_name: Some(format!("worktree-{}", id)),
        iteration_timings: HashMap::new(),
        command_timings: HashMap::new(),
        metadata: HashMap::new(),
    }
}

/// Create a test event
fn create_test_event(job_id: &str, size: usize) -> EventEntry {
    EventEntry {
        timestamp: chrono::Utc::now(),
        event_type: "benchmark".to_string(),
        job_id: job_id.to_string(),
        work_item_id: Some(format!("item-{}", Uuid::new_v4())),
        agent_id: Some(format!("agent-{}", Uuid::new_v4())),
        correlation_id: Some(Uuid::new_v4()),
        message: Some("x".repeat(size)),
        data: json!({"test": "x".repeat(size / 2)}),
        error: None,
    }
}

/// Benchmark session operations comparing abstraction vs direct
fn bench_session_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("session_operations");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    // Benchmark via abstraction layer
    group.bench_function("abstraction_save_load", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig {
                    backend: BackendType::File,
                    backend_config: BackendConfig::File(FileConfig {
                        base_dir: temp_dir.path().to_path_buf(),
                        use_global: false,
                        enable_file_locks: false, // Disable for fair comparison
                        max_file_size: 10 * 1024 * 1024,
                        enable_compression: false,
                    }),
                    connection_pool_size: 10,
                    retry_policy: Default::default(),
                    timeout: Duration::from_secs(30),
                    enable_locking: false,
                    enable_cache: false,
                    cache_config: Default::default(),
                };
                let rt_local = Runtime::new().unwrap();
                let backend = rt_local.block_on(FileBackend::new(&config)).unwrap();
                let session = create_test_session(&Uuid::new_v4().to_string());
                (backend, session, temp_dir)
            },
            |(backend, session, _temp_dir)| async move {
                // Save and load session
                backend
                    .session_storage()
                    .save_session(&session)
                    .await
                    .unwrap();
                let loaded = backend
                    .session_storage()
                    .load_session(&session.session_id)
                    .await
                    .unwrap();
                black_box(loaded);
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark direct file access
    group.bench_function("direct_file_save_load", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let session = create_test_session(&Uuid::new_v4().to_string());
                (temp_dir, session)
            },
            |(temp_dir, session)| async move {
                // Direct file operations
                let file_path = temp_dir
                    .path()
                    .join("sessions")
                    .join(format!("{}.json", session.session_id));

                tokio::fs::create_dir_all(file_path.parent().unwrap())
                    .await
                    .unwrap();

                let json = serde_json::to_string(&session).unwrap();
                tokio::fs::write(&file_path, &json).await.unwrap();

                let content = tokio::fs::read_to_string(&file_path).await.unwrap();
                let loaded: SessionState = serde_json::from_str(&content).unwrap();
                black_box(loaded);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark event storage operations
fn bench_event_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("event_operations");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Elements(100)); // We'll append 100 events

    for size in [100, 1000, 10000].iter() {
        // Via abstraction
        group.bench_with_input(
            BenchmarkId::new("abstraction_append", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = StorageConfig {
                            backend: BackendType::File,
                            backend_config: BackendConfig::File(FileConfig {
                                base_dir: temp_dir.path().to_path_buf(),
                                use_global: false,
                                enable_file_locks: false,
                                max_file_size: 10 * 1024 * 1024,
                                enable_compression: false,
                            }),
                            connection_pool_size: 10,
                            retry_policy: Default::default(),
                            timeout: Duration::from_secs(30),
                            enable_locking: false,
                            enable_cache: false,
                            cache_config: Default::default(),
                        };
                        let rt_local = Runtime::new().unwrap();
                        let backend = rt_local.block_on(FileBackend::new(&config)).unwrap();
                        let events: Vec<EventEntry> = (0..100)
                            .map(|_| create_test_event("bench-job", size))
                            .collect();
                        (backend, events, temp_dir)
                    },
                    |(backend, events, _temp_dir)| async move {
                        for event in events {
                            backend
                                .event_storage()
                                .append_event("bench-repo", "bench-job", &event)
                                .await
                                .unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // Direct file access
        group.bench_with_input(BenchmarkId::new("direct_append", size), size, |b, &size| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let events: Vec<EventEntry> = (0..100)
                        .map(|_| create_test_event("bench-job", size))
                        .collect();
                    (temp_dir, events)
                },
                |(temp_dir, events)| async move {
                    let file_path = temp_dir
                        .path()
                        .join("events")
                        .join("bench-repo")
                        .join("bench-job.jsonl");

                    tokio::fs::create_dir_all(file_path.parent().unwrap())
                        .await
                        .unwrap();

                    let mut content = String::new();
                    for event in events {
                        content.push_str(&serde_json::to_string(&event).unwrap());
                        content.push('\n');
                    }

                    tokio::fs::write(&file_path, content).await.unwrap();
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark memory backend operations
fn bench_memory_backend(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_backend");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("memory_session_ops", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let config = StorageConfig {
                    backend: BackendType::Memory,
                    backend_config: BackendConfig::Memory(MemoryConfig::default()),
                    connection_pool_size: 10,
                    retry_policy: Default::default(),
                    timeout: Duration::from_secs(30),
                    enable_locking: true,
                    enable_cache: false,
                    cache_config: Default::default(),
                };
                let backend = MemoryBackend::new(&config).unwrap();
                let sessions: Vec<SessionState> = (0..100)
                    .map(|i| create_test_session(&format!("session-{}", i)))
                    .collect();
                (backend, sessions)
            },
            |(backend, sessions)| async move {
                // Save all sessions
                for session in &sessions {
                    backend
                        .session_storage()
                        .save_session(session)
                        .await
                        .unwrap();
                }

                // Load all sessions
                for session in &sessions {
                    let loaded = backend
                        .session_storage()
                        .load_session(&session.session_id)
                        .await
                        .unwrap();
                    black_box(loaded);
                }

                // List sessions
                let list = backend
                    .session_storage()
                    .list_sessions("bench-repo")
                    .await
                    .unwrap();
                black_box(list);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark factory creation overhead
fn bench_factory_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("factory_overhead");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("factory_create_file_backend", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig {
                    backend: BackendType::File,
                    backend_config: BackendConfig::File(FileConfig {
                        base_dir: temp_dir.path().to_path_buf(),
                        use_global: false,
                        enable_file_locks: true,
                        max_file_size: 10 * 1024 * 1024,
                        enable_compression: false,
                    }),
                    connection_pool_size: 10,
                    retry_policy: Default::default(),
                    timeout: Duration::from_secs(30),
                    enable_locking: true,
                    enable_cache: false,
                    cache_config: Default::default(),
                };
                (config, temp_dir)
            },
            |(config, _temp_dir)| async move {
                let storage = StorageFactory::from_config(&config).await.unwrap();
                black_box(storage);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("factory_create_memory_backend", |b| {
        b.iter(|| {
            let storage = StorageFactory::create_test_storage();
            black_box(storage);
        });
    });

    group.finish();
}

/// Calculate overhead percentage
fn calculate_overhead(abstraction_time: f64, direct_time: f64) -> f64 {
    ((abstraction_time - direct_time) / direct_time) * 100.0
}

/// Main benchmark to verify <5% overhead requirement
fn bench_overhead_verification(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    println!("\n=== Storage Abstraction Overhead Verification ===\n");

    let mut group = c.benchmark_group("overhead_verification");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(200);

    // Test different operation types
    let operations = vec![
        ("small_writes", 100),
        ("medium_writes", 1000),
        ("large_writes", 10000),
    ];

    for (op_name, size) in operations {
        let abstraction_id = format!("abstraction_{}", op_name);
        let direct_id = format!("direct_{}", op_name);

        // Measure abstraction layer
        group.bench_with_input(
            BenchmarkId::from_parameter(&abstraction_id),
            &size,
            |b, &size| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = StorageConfig {
                            backend: BackendType::File,
                            backend_config: BackendConfig::File(FileConfig {
                                base_dir: temp_dir.path().to_path_buf(),
                                use_global: false,
                                enable_file_locks: false,
                                max_file_size: 10 * 1024 * 1024,
                                enable_compression: false,
                            }),
                            connection_pool_size: 10,
                            retry_policy: Default::default(),
                            timeout: Duration::from_secs(30),
                            enable_locking: false,
                            enable_cache: false,
                            cache_config: Default::default(),
                        };
                        let rt_local = Runtime::new().unwrap();
                        let backend = rt_local.block_on(FileBackend::new(&config)).unwrap();
                        let data = "x".repeat(size);
                        (backend, data, temp_dir)
                    },
                    |(backend, data, _temp_dir)| async move {
                        let session = SessionState {
                            session_id: Uuid::new_v4().to_string(),
                            repository: "bench".to_string(),
                            status: SessionStatus::InProgress,
                            started_at: chrono::Utc::now(),
                            completed_at: None,
                            workflow_path: Some(data.clone()),
                            git_branch: Some(data),
                            iterations_completed: 0,
                            files_changed: 0,
                            worktree_name: None,
                            iteration_timings: HashMap::new(),
                            command_timings: HashMap::new(),
                            metadata: HashMap::new(),
                        };
                        backend
                            .session_storage()
                            .save_session(&session)
                            .await
                            .unwrap();
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // Measure direct access
        group.bench_with_input(
            BenchmarkId::from_parameter(&direct_id),
            &size,
            |b, &size| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let data = "x".repeat(size);
                        (temp_dir, data)
                    },
                    |(temp_dir, data)| async move {
                        let session = SessionState {
                            session_id: Uuid::new_v4().to_string(),
                            repository: "bench".to_string(),
                            status: SessionStatus::InProgress,
                            started_at: chrono::Utc::now(),
                            completed_at: None,
                            workflow_path: Some(data.clone()),
                            git_branch: Some(data),
                            iterations_completed: 0,
                            files_changed: 0,
                            worktree_name: None,
                            iteration_timings: HashMap::new(),
                            command_timings: HashMap::new(),
                            metadata: HashMap::new(),
                        };

                        let file_path = temp_dir
                            .path()
                            .join("sessions")
                            .join(format!("{}.json", session.session_id));

                        tokio::fs::create_dir_all(file_path.parent().unwrap())
                            .await
                            .unwrap();

                        let json = serde_json::to_string(&session).unwrap();
                        tokio::fs::write(&file_path, json).await.unwrap();
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();

    println!("\n=== Overhead Summary ===");
    println!("Target: <5% overhead for abstraction layer");
    println!("See HTML report for detailed results");
    println!("========================\n");
}

criterion_group!(
    benches,
    bench_session_operations,
    bench_event_operations,
    bench_memory_backend,
    bench_factory_overhead,
    bench_overhead_verification
);

criterion_main!(benches);
