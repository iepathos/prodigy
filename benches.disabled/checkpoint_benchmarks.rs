// Performance benchmarks for checkpoint operations
// Verifies <5% overhead requirement for checkpoint save/load

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use prodigy::cook::workflow::{Checkpoint, CheckpointManager, ExecutionState, WorkflowStatus};
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create a test checkpoint with varying complexity
fn create_test_checkpoint(num_variables: usize, num_completed_steps: usize) -> Checkpoint {
    let mut variables = serde_json::Map::new();

    // Add variables of varying sizes
    for i in 0..num_variables {
        variables.insert(
            format!("var_{}", i),
            json!(format!(
                "value_{}_with_some_content_to_make_it_realistic",
                i
            )),
        );
    }

    // Add some nested structures
    variables.insert(
        "shell".to_string(),
        json!({
            "output": "Sample command output with multiple lines\n".repeat(10),
            "exit_code": 0,
            "duration_ms": 1234
        }),
    );

    // Create completed steps
    let completed_steps = (0..num_completed_steps)
        .map(|i| prodigy::cook::workflow::CompletedStep {
            step_index: i,
            command: format!("shell: command_{}.sh", i),
            success: true,
            timestamp: chrono::Utc::now(),
            output: Some(format!("Output from command {}", i)),
            retry_state: if i % 3 == 0 {
                Some(prodigy::cook::workflow::RetryState {
                    current_attempt: 1,
                    max_attempts: 3,
                    last_error: None,
                })
            } else {
                None
            },
        })
        .collect();

    Checkpoint {
        workflow_id: "benchmark-workflow-12345".to_string(),
        workflow_path: PathBuf::from("benchmark.yaml"),
        timestamp: chrono::Utc::now(),
        execution_state: ExecutionState {
            status: WorkflowStatus::InProgress,
            current_step_index: num_completed_steps,
            total_steps: num_completed_steps + 10,
        },
        variables: json!(variables),
        completed_steps,
        parallel_state: None,
        mapreduce_state: None,
        retry_state: Some(prodigy::cook::workflow::RetryState {
            current_attempt: 1,
            max_attempts: 3,
            last_error: None,
        }),
    }
}

/// Benchmark checkpoint save operation
fn benchmark_checkpoint_save(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("checkpoint_save");
    group.measurement_time(Duration::from_secs(10));

    // Small checkpoint (5 variables, 10 completed steps)
    group.bench_function("small_checkpoint", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(5, 10);
                (manager, checkpoint, temp_dir)
            },
            |(manager, checkpoint, _temp_dir)| {
                rt.block_on(async {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Medium checkpoint (50 variables, 100 completed steps)
    group.bench_function("medium_checkpoint", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(50, 100);
                (manager, checkpoint, temp_dir)
            },
            |(manager, checkpoint, _temp_dir)| {
                rt.block_on(async {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Large checkpoint (200 variables, 500 completed steps)
    group.bench_function("large_checkpoint", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(200, 500);
                (manager, checkpoint, temp_dir)
            },
            |(manager, checkpoint, _temp_dir)| {
                rt.block_on(async {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark checkpoint load operation
fn benchmark_checkpoint_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("checkpoint_load");
    group.measurement_time(Duration::from_secs(10));

    // Small checkpoint
    group.bench_function("small_checkpoint", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(5, 10);
                rt.block_on(async {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                });
                (manager, checkpoint.workflow_id, temp_dir)
            },
            |(manager, workflow_id, _temp_dir)| {
                rt.block_on(async {
                    let _checkpoint = manager.load_checkpoint(&workflow_id).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Medium checkpoint
    group.bench_function("medium_checkpoint", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(50, 100);
                rt.block_on(async {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                });
                (manager, checkpoint.workflow_id, temp_dir)
            },
            |(manager, workflow_id, _temp_dir)| {
                rt.block_on(async {
                    let _checkpoint = manager.load_checkpoint(&workflow_id).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Large checkpoint
    group.bench_function("large_checkpoint", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(200, 500);
                rt.block_on(async {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                });
                (manager, checkpoint.workflow_id, temp_dir)
            },
            |(manager, workflow_id, _temp_dir)| {
                rt.block_on(async {
                    let _checkpoint = manager.load_checkpoint(&workflow_id).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark checkpoint cleanup operation
fn benchmark_checkpoint_cleanup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("checkpoint_cleanup");

    group.bench_function("cleanup_single", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(50, 100);
                rt.block_on(async {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                });
                (manager, checkpoint.workflow_id, temp_dir)
            },
            |(manager, workflow_id, _temp_dir)| {
                rt.block_on(async {
                    manager.cleanup_checkpoint(&workflow_id).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("cleanup_multiple", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let workflow_ids: Vec<String> = (0..10)
                    .map(|i| {
                        let checkpoint = create_test_checkpoint(10, 20);
                        let id = format!("workflow_{}", i);
                        let mut checkpoint_with_id = checkpoint;
                        checkpoint_with_id.workflow_id = id.clone();
                        rt.block_on(async {
                            manager.save_checkpoint(&checkpoint_with_id).await.unwrap();
                        });
                        id
                    })
                    .collect();
                (manager, workflow_ids, temp_dir)
            },
            |(manager, workflow_ids, _temp_dir)| {
                rt.block_on(async {
                    for id in workflow_ids {
                        manager.cleanup_checkpoint(&id).await.unwrap();
                    }
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark workflow execution with vs without checkpointing
fn benchmark_execution_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("execution_overhead");
    group.measurement_time(Duration::from_secs(15));

    // Simulate workflow execution without checkpoints
    group.bench_function("without_checkpoints", |b| {
        b.iter(|| {
            // Simulate 100 command executions
            for i in 0..100 {
                // Simulate command work
                std::thread::sleep(Duration::from_micros(100));
                black_box(format!("Command {} executed", i));
            }
        });
    });

    // Simulate workflow execution with checkpoints
    group.bench_function("with_checkpoints", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                (manager, temp_dir)
            },
            |(manager, _temp_dir)| {
                rt.block_on(async {
                    let mut checkpoint = create_test_checkpoint(10, 0);

                    // Simulate 100 command executions with checkpoint saves
                    for i in 0..100 {
                        // Simulate command work
                        std::thread::sleep(Duration::from_micros(100));
                        black_box(format!("Command {} executed", i));

                        // Save checkpoint every 10 commands
                        if i % 10 == 0 {
                            checkpoint.execution_state.current_step_index = i;
                            checkpoint.completed_steps.push(
                                prodigy::cook::workflow::CompletedStep {
                                    step_index: i,
                                    command: format!("command_{}", i),
                                    success: true,
                                    timestamp: chrono::Utc::now(),
                                    output: Some(format!("Output {}", i)),
                                    retry_state: None,
                                },
                            );
                            manager.save_checkpoint(&checkpoint).await.unwrap();
                        }
                    }
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark checkpoint size impact
fn benchmark_checkpoint_size_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("size_impact");

    // Measure serialization time for different checkpoint sizes
    group.bench_function("serialize_small", |b| {
        let checkpoint = create_test_checkpoint(10, 20);
        b.iter(|| {
            let _json = serde_json::to_string(&checkpoint).unwrap();
        });
    });

    group.bench_function("serialize_medium", |b| {
        let checkpoint = create_test_checkpoint(100, 200);
        b.iter(|| {
            let _json = serde_json::to_string(&checkpoint).unwrap();
        });
    });

    group.bench_function("serialize_large", |b| {
        let checkpoint = create_test_checkpoint(500, 1000);
        b.iter(|| {
            let _json = serde_json::to_string(&checkpoint).unwrap();
        });
    });

    // Measure deserialization time
    group.bench_function("deserialize_small", |b| {
        let checkpoint = create_test_checkpoint(10, 20);
        let json = serde_json::to_string(&checkpoint).unwrap();
        b.iter(|| {
            let _checkpoint: Checkpoint = serde_json::from_str(&json).unwrap();
        });
    });

    group.bench_function("deserialize_medium", |b| {
        let checkpoint = create_test_checkpoint(100, 200);
        let json = serde_json::to_string(&checkpoint).unwrap();
        b.iter(|| {
            let _checkpoint: Checkpoint = serde_json::from_str(&json).unwrap();
        });
    });

    group.bench_function("deserialize_large", |b| {
        let checkpoint = create_test_checkpoint(500, 1000);
        let json = serde_json::to_string(&checkpoint).unwrap();
        b.iter(|| {
            let _checkpoint: Checkpoint = serde_json::from_str(&json).unwrap();
        });
    });

    group.finish();
}

/// Benchmark concurrent checkpoint operations
fn benchmark_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");

    // Concurrent saves
    group.bench_function("concurrent_saves", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoints: Vec<Checkpoint> = (0..10)
                    .map(|i| {
                        let mut checkpoint = create_test_checkpoint(20, 50);
                        checkpoint.workflow_id = format!("concurrent_{}", i);
                        checkpoint
                    })
                    .collect();
                (manager, checkpoints, temp_dir)
            },
            |(manager, checkpoints, _temp_dir)| {
                rt.block_on(async {
                    let futures: Vec<_> = checkpoints
                        .into_iter()
                        .map(|checkpoint| {
                            let manager = manager.clone();
                            async move {
                                manager.save_checkpoint(&checkpoint).await.unwrap();
                            }
                        })
                        .collect();

                    futures::future::join_all(futures).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Concurrent loads
    group.bench_function("concurrent_loads", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let workflow_ids: Vec<String> = (0..10)
                    .map(|i| {
                        let mut checkpoint = create_test_checkpoint(20, 50);
                        checkpoint.workflow_id = format!("concurrent_{}", i);
                        rt.block_on(async {
                            manager.save_checkpoint(&checkpoint).await.unwrap();
                        });
                        checkpoint.workflow_id
                    })
                    .collect();
                (manager, workflow_ids, temp_dir)
            },
            |(manager, workflow_ids, _temp_dir)| {
                rt.block_on(async {
                    let futures: Vec<_> = workflow_ids
                        .into_iter()
                        .map(|id| {
                            let manager = manager.clone();
                            async move {
                                manager.load_checkpoint(&id).await.unwrap();
                            }
                        })
                        .collect();

                    futures::future::join_all(futures).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_checkpoint_save,
    benchmark_checkpoint_load,
    benchmark_checkpoint_cleanup,
    benchmark_execution_overhead,
    benchmark_checkpoint_size_impact,
    benchmark_concurrent_operations
);

criterion_main!(benches);
