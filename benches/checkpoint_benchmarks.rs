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
                    let rt_local = Runtime::new().unwrap();
                    rt_local.block_on(async {
                        manager.save_checkpoint(&checkpoint).await.unwrap();
                    });
                    (manager, checkpoint.workflow_id, temp_dir)
                },
                |(manager, workflow_id, _temp_dir)| async move {
                    black_box(manager.load_checkpoint(&workflow_id).await.unwrap());
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
                    let rt_local = Runtime::new().unwrap();
                    rt_local.block_on(async {
                        manager.save_checkpoint(&checkpoint).await.unwrap();
                    });
                    (manager, checkpoint.workflow_id, temp_dir)
                },
                |(manager, workflow_id, _temp_dir)| async move {
                    black_box(manager.load_checkpoint(&workflow_id).await.unwrap());
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
                    let rt_local = Runtime::new().unwrap();
                    rt_local.block_on(async {
                        manager.save_checkpoint(&checkpoint).await.unwrap();
                    });
                    (manager, checkpoint.workflow_id, temp_dir)
                },
                |(manager, workflow_id, _temp_dir)| async move {
                    black_box(manager.load_checkpoint(&workflow_id).await.unwrap());
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
                let rt_local = Runtime::new().unwrap();

                // Create 10 checkpoints
                for i in 0..10 {
                    let mut checkpoint = create_test_checkpoint(50, 25);
                    checkpoint.workflow_id = format!("workflow-{}", i);
                    rt_local.block_on(async {
                        manager.save_checkpoint(&checkpoint).await.unwrap();
                    });
                }
                (manager, temp_dir)
            },
            |(manager, _temp_dir)| async move {
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
                let rt_local = Runtime::new().unwrap();

                // Create checkpoints to delete
                let mut workflow_ids = Vec::new();
                for i in 0..5 {
                    let mut checkpoint = create_test_checkpoint(50, 25);
                    checkpoint.workflow_id = format!("workflow-to-delete-{}", i);
                    workflow_ids.push(checkpoint.workflow_id.clone());
                    rt_local.block_on(async {
                        manager.save_checkpoint(&checkpoint).await.unwrap();
                    });
                }
                (manager, workflow_ids, temp_dir)
            },
            |(manager, workflow_ids, _temp_dir)| async move {
                for workflow_id in workflow_ids {
                    manager.delete_checkpoint(&workflow_id).await.unwrap();
                }
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
    bench_checkpoint_delete
);

criterion_main!(benches);
