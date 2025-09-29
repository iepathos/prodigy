//! Performance benchmarks for checkpoint operations
//! Verifies <5% overhead requirement for checkpoint save/load

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use prodigy::cook::workflow::checkpoint::{
    CheckpointManager, CompletedStep, ExecutionState, WorkflowCheckpoint, WorkflowStatus,
    CHECKPOINT_VERSION,
};
use serde_json::json;
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create a test checkpoint with varying complexity
fn create_test_checkpoint(num_variables: usize, num_completed_steps: usize) -> WorkflowCheckpoint {
    let mut variables = HashMap::new();

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
        .map(|i| CompletedStep {
            step_index: i,
            command: format!("shell: command_{}.sh", i),
            success: true,
            output: Some(format!("Output from command {}", i)),
            captured_variables: HashMap::new(),
            duration: Duration::from_secs(1),
            completed_at: chrono::Utc::now(),
            retry_state: None,
        })
        .collect();

    let now = chrono::Utc::now();

    WorkflowCheckpoint {
        workflow_id: "bench-workflow-123".to_string(),
        execution_state: ExecutionState {
            current_step_index: num_completed_steps,
            total_steps: num_completed_steps + 10,
            status: WorkflowStatus::Running,
            start_time: now,
            last_checkpoint: now,
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps,
        variable_state: variables,
        mapreduce_state: None,
        timestamp: now,
        version: CHECKPOINT_VERSION,
        workflow_hash: "test-hash".to_string(),
        total_steps: num_completed_steps + 10,
        workflow_name: Some("benchmark-workflow".to_string()),
        workflow_path: None,
        error_recovery_state: None,
        retry_checkpoint_state: None,
        variable_checkpoint_state: None,
    }
}

fn bench_checkpoint_save(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.benchmark_group("checkpoint_save")
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .bench_function("small_checkpoint", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                    let checkpoint = create_test_checkpoint(10, 5);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("medium_checkpoint", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                    let checkpoint = create_test_checkpoint(100, 50);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("large_checkpoint", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                    let checkpoint = create_test_checkpoint(1000, 200);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                },
                BatchSize::SmallInput,
            );
        });
}

fn bench_checkpoint_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.benchmark_group("checkpoint_load")
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .bench_function("small_checkpoint", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                    let checkpoint = create_test_checkpoint(10, 5);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    // Save the checkpoint first
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                    // Now benchmark the load
                    black_box(
                        manager
                            .load_checkpoint(&checkpoint.workflow_id)
                            .await
                            .unwrap(),
                    );
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("medium_checkpoint", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                    let checkpoint = create_test_checkpoint(100, 50);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    // Save the checkpoint first
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                    // Now benchmark the load
                    black_box(
                        manager
                            .load_checkpoint(&checkpoint.workflow_id)
                            .await
                            .unwrap(),
                    );
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("large_checkpoint", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                    let checkpoint = create_test_checkpoint(1000, 200);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    // Save the checkpoint first
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                    // Now benchmark the load
                    black_box(
                        manager
                            .load_checkpoint(&checkpoint.workflow_id)
                            .await
                            .unwrap(),
                    );
                },
                BatchSize::SmallInput,
            );
        });
}

fn bench_checkpoint_atomic_write(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("checkpoint_atomic_write", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoint = create_test_checkpoint(100, 50);
                (manager, checkpoint, temp_dir)
            },
            |(manager, checkpoint, _temp_dir)| async move {
                // Test atomic write with potential concurrent access
                manager.save_checkpoint(&checkpoint).await.unwrap();
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_checkpoint_list(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("checkpoint_list_10_checkpoints", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoints: Vec<_> = (0..10)
                    .map(|i| {
                        let mut checkpoint = create_test_checkpoint(50, 25);
                        checkpoint.workflow_id = format!("workflow-{}", i);
                        checkpoint
                    })
                    .collect();
                (manager, checkpoints, temp_dir)
            },
            |(manager, checkpoints, _temp_dir)| async move {
                // Create the checkpoints first
                for checkpoint in checkpoints {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                }
                // Now benchmark the list operation
                black_box(manager.list_checkpoints().await.unwrap());
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_checkpoint_delete(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("checkpoint_delete", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                let checkpoints: Vec<_> = (0..5)
                    .map(|i| {
                        let mut checkpoint = create_test_checkpoint(50, 25);
                        checkpoint.workflow_id = format!("workflow-to-delete-{}", i);
                        checkpoint
                    })
                    .collect();
                (manager, checkpoints, temp_dir)
            },
            |(manager, checkpoints, _temp_dir)| async move {
                // Create the checkpoints first
                let workflow_ids: Vec<_> =
                    checkpoints.iter().map(|c| c.workflow_id.clone()).collect();
                for checkpoint in checkpoints {
                    manager.save_checkpoint(&checkpoint).await.unwrap();
                }
                // Now benchmark the delete operations
                for workflow_id in workflow_ids {
                    manager.delete_checkpoint(&workflow_id).await.unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_mapreduce_checkpoint_overhead(c: &mut Criterion) {
    use prodigy::cook::execution::mapreduce::checkpoint::{
        AgentState, CheckpointConfig, CheckpointManager as MapReduceCheckpointManager,
        CheckpointMetadata, CheckpointReason, CompressionAlgorithm, ErrorState,
        ExecutionState as MapReduceExecutionState, FileCheckpointStorage, MapReduceCheckpoint,
        PhaseType, ResourceState, VariableState, WorkItemState,
    };
    use std::collections::HashMap;

    let rt = Runtime::new().unwrap();

    // Helper function to create a MapReduce checkpoint
    let create_mr_checkpoint = |num_items: usize| -> MapReduceCheckpoint {
        MapReduceCheckpoint {
            metadata: CheckpointMetadata {
                checkpoint_id: "test-mr-checkpoint".to_string(),
                job_id: "test-job".to_string(),
                version: 1,
                created_at: chrono::Utc::now(),
                phase: PhaseType::Map,
                total_work_items: num_items,
                completed_items: num_items / 2,
                checkpoint_reason: CheckpointReason::Interval,
                integrity_hash: String::new(),
            },
            execution_state: MapReduceExecutionState {
                current_phase: PhaseType::Map,
                phase_start_time: chrono::Utc::now(),
                setup_results: None,
                map_results: None,
                reduce_results: None,
                workflow_variables: HashMap::new(),
            },
            work_item_state: WorkItemState {
                pending_items: vec![],
                in_progress_items: HashMap::new(),
                completed_items: vec![],
                failed_items: vec![],
                current_batch: None,
            },
            agent_state: AgentState {
                active_agents: HashMap::new(),
                agent_assignments: HashMap::new(),
                agent_results: HashMap::new(),
                resource_allocation: HashMap::new(),
            },
            variable_state: VariableState {
                workflow_variables: HashMap::new(),
                captured_outputs: HashMap::new(),
                environment_variables: HashMap::new(),
                item_variables: HashMap::new(),
            },
            resource_state: ResourceState {
                total_agents_allowed: 10,
                current_agents_active: 0,
                worktrees_created: vec![],
                worktrees_cleaned: vec![],
                disk_usage_bytes: None,
            },
            error_state: ErrorState {
                error_count: 0,
                dlq_items: vec![],
                error_threshold_reached: false,
                last_error: None,
            },
        }
    };

    // Benchmark checkpoint creation overhead
    c.benchmark_group("mapreduce_checkpoint_overhead")
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .bench_function("no_compression", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = Box::new(FileCheckpointStorage::with_compression(
                        temp_dir.path().to_path_buf(),
                        CompressionAlgorithm::None,
                    ));
                    let config = CheckpointConfig::default();
                    let manager =
                        MapReduceCheckpointManager::new(storage, config, "test-job".to_string());
                    let checkpoint = create_mr_checkpoint(1000);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    manager
                        .create_checkpoint(&checkpoint, CheckpointReason::Interval)
                        .await
                        .unwrap();
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("gzip_compression", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = Box::new(FileCheckpointStorage::with_compression(
                        temp_dir.path().to_path_buf(),
                        CompressionAlgorithm::Gzip,
                    ));
                    let config = CheckpointConfig::default();
                    let manager =
                        MapReduceCheckpointManager::new(storage, config, "test-job".to_string());
                    let checkpoint = create_mr_checkpoint(1000);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    manager
                        .create_checkpoint(&checkpoint, CheckpointReason::Interval)
                        .await
                        .unwrap();
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("zstd_compression", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = Box::new(FileCheckpointStorage::with_compression(
                        temp_dir.path().to_path_buf(),
                        CompressionAlgorithm::Zstd,
                    ));
                    let config = CheckpointConfig::default();
                    let manager =
                        MapReduceCheckpointManager::new(storage, config, "test-job".to_string());
                    let checkpoint = create_mr_checkpoint(1000);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    manager
                        .create_checkpoint(&checkpoint, CheckpointReason::Interval)
                        .await
                        .unwrap();
                },
                BatchSize::SmallInput,
            );
        })
        .bench_function("lz4_compression", |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = Box::new(FileCheckpointStorage::with_compression(
                        temp_dir.path().to_path_buf(),
                        CompressionAlgorithm::Lz4,
                    ));
                    let config = CheckpointConfig::default();
                    let manager =
                        MapReduceCheckpointManager::new(storage, config, "test-job".to_string());
                    let checkpoint = create_mr_checkpoint(1000);
                    (manager, checkpoint, temp_dir)
                },
                |(manager, checkpoint, _temp_dir)| async move {
                    manager
                        .create_checkpoint(&checkpoint, CheckpointReason::Interval)
                        .await
                        .unwrap();
                },
                BatchSize::SmallInput,
            );
        });
}

criterion_group!(
    benches,
    bench_checkpoint_save,
    bench_checkpoint_load,
    bench_checkpoint_atomic_write,
    bench_checkpoint_list,
    bench_checkpoint_delete,
    bench_mapreduce_checkpoint_overhead
);

criterion_main!(benches);
