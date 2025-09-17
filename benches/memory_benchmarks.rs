//! Memory usage benchmarks for Prodigy
//! Tracks memory allocation, resource cleanup efficiency, and detects potential leaks

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use prodigy::cook::workflow::checkpoint::CheckpointManager;
use prodigy::storage::GlobalStorage;
use serde_json::json;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Structure to track memory statistics
#[derive(Debug, Clone)]
struct MemoryStats {
    allocated: usize,
    resident: usize,
    virtual_memory: usize,
}

/// Get current memory statistics for the process
fn get_memory_stats() -> MemoryStats {
    // Using simple approximation - in production would use memory-stats crate
    let rusage = unsafe {
        let mut usage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        usage
    };

    MemoryStats {
        allocated: (rusage.ru_maxrss as usize) * 1024, // Convert to bytes on Linux
        resident: (rusage.ru_maxrss as usize) * 1024,
        virtual_memory: (rusage.ru_ixrss as usize) * 1024,
    }
}

/// Track memory usage delta for a given operation
async fn measure_memory_delta<F, Fut, R>(operation: F) -> (R, MemoryStats)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let before = get_memory_stats();
    let result = operation().await;
    let after = get_memory_stats();

    let delta = MemoryStats {
        allocated: after.allocated.saturating_sub(before.allocated),
        resident: after.resident.saturating_sub(before.resident),
        virtual_memory: after.virtual_memory.saturating_sub(before.virtual_memory),
    };

    (result, delta)
}

fn bench_storage_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_memory");

    // Test memory usage for different storage sizes
    for size_mb in &[1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("event_storage", size_mb),
            size_mb,
            |b, &mb| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = GlobalStorage::new(temp_dir.path()).unwrap();
                        let num_events = mb * 1000; // Approximate number of events for target size
                        (storage, num_events, temp_dir)
                    },
                    |(storage, num_events, _temp_dir)| async move {
                        let (_, mem_delta) = measure_memory_delta(|| async {
                            for i in 0..num_events {
                                let _ = storage.get_events_dir("bench-job").await;
                                let event = json!({
                                    "event_id": i,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                    "data": vec![0u8; 1024] // 1KB per event
                                });
                                black_box(event);
                            }
                        })
                        .await;

                        black_box(mem_delta);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_checkpoint_memory_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("checkpoint_memory");

    for num_steps in &[10, 100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("checkpoint_size", num_steps),
            num_steps,
            |b, &steps| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                        (manager, steps, temp_dir)
                    },
                    |(manager, steps, _temp_dir)| async move {
                        let (_, mem_delta) = measure_memory_delta(|| async {
                            let checkpoint = create_large_checkpoint(steps);
                            manager.save_checkpoint(&checkpoint).await.unwrap();
                            manager
                                .load_checkpoint(&checkpoint.workflow_id)
                                .await
                                .unwrap()
                        })
                        .await;

                        black_box(mem_delta);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_worktree_cleanup_efficiency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("worktree_cleanup");

    group.bench_function("cleanup_simulation", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                // Simulate worktree resources
                let worktree_paths: Vec<_> = (0..10)
                    .map(|i| temp_dir.path().join(format!("worktree-{}", i)))
                    .collect();

                // Create dummy directories
                for path in &worktree_paths {
                    std::fs::create_dir_all(path).unwrap();
                }

                (worktree_paths, temp_dir)
            },
            |(worktree_paths, _temp_dir)| async move {
                // Measure memory before and after cleanup
                let (_, mem_delta) = measure_memory_delta(|| async {
                    for path in worktree_paths {
                        // Simulate cleanup
                        if path.exists() {
                            std::fs::remove_dir_all(path).unwrap();
                        }
                    }
                })
                .await;

                black_box(mem_delta);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_memory_leak_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("leak_detection");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("repeated_allocations", |b| {
        b.to_async(&rt).iter_batched(
            || {},
            |_| async move {
                let initial_memory = get_memory_stats();

                // Perform repeated allocations and deallocations
                for _ in 0..100 {
                    let temp_dir = TempDir::new().unwrap();
                    let storage = GlobalStorage::new(temp_dir.path()).unwrap();

                    // Write and read events
                    for i in 0..10 {
                        let _ = storage.get_events_dir("bench-job").await;
                        black_box(json!({ "id": i }));
                    }

                    // Drop everything explicitly
                    drop(storage);
                    drop(temp_dir);
                }

                let final_memory = get_memory_stats();
                let leak_indicator = final_memory
                    .resident
                    .saturating_sub(initial_memory.resident);

                // Check if memory growth is reasonable (< 10MB for this test)
                assert!(
                    leak_indicator < 10 * 1024 * 1024,
                    "Potential memory leak detected: {} bytes growth",
                    leak_indicator
                );

                black_box(leak_indicator);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_concurrent_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_memory");

    for num_tasks in &[10, 50, 100, 200] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_operations", num_tasks),
            num_tasks,
            |b, &tasks| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = Arc::new(GlobalStorage::new(temp_dir.path()).unwrap());
                        (storage, tasks, temp_dir)
                    },
                    |(storage, tasks, _temp_dir)| async move {
                        let (_, mem_delta) = measure_memory_delta(|| async {
                            let mut handles = Vec::new();

                            for i in 0..tasks {
                                let storage_clone = Arc::clone(&storage);
                                let handle = tokio::spawn(async move {
                                    for j in 0..10 {
                                        let _ = storage_clone.get_events_dir("bench-job").await;
                                        black_box(json!({ "task": i, "event": j }));
                                    }
                                });
                                handles.push(handle);
                            }

                            // Wait for all tasks to complete
                            for handle in handles {
                                handle.await.unwrap();
                            }
                        })
                        .await;

                        black_box(mem_delta);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_large_workflow_memory(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("workflow_memory");

    group.bench_function("large_workflow_parsing", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let workflow_content = generate_large_workflow(100); // 100 steps
                (workflow_content, temp_dir)
            },
            |(workflow_content, _temp_dir)| async move {
                let (_, mem_delta) = measure_memory_delta(|| async {
                    // Parse and validate workflow
                    let workflow: prodigy::config::WorkflowConfig =
                        serde_yaml::from_str(&workflow_content).unwrap();

                    // Simulate execution planning (without actually running commands)
                    let step_count = workflow.commands.len();
                    let mut variables = std::collections::HashMap::new();
                    variables.insert("step_count".to_string(), json!(step_count));

                    // Simulate variable state for each step
                    for i in 0..step_count {
                        variables.insert(format!("step_{}_status", i), json!("pending"));
                    }

                    black_box((workflow, variables));
                })
                .await;

                black_box(mem_delta);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Helper function to create a large checkpoint
fn create_large_checkpoint(
    num_steps: usize,
) -> prodigy::cook::workflow::checkpoint::WorkflowCheckpoint {
    use prodigy::cook::workflow::checkpoint::{
        CompletedStep, ExecutionState, WorkflowCheckpoint, WorkflowStatus, CHECKPOINT_VERSION,
    };
    use std::collections::HashMap;

    let mut variables = HashMap::new();
    for i in 0..num_steps {
        variables.insert(
            format!("var_{}", i),
            json!({
                "data": vec![0u8; 1024], // 1KB per variable
                "metadata": format!("step_{}", i)
            }),
        );
    }

    let completed_steps = (0..num_steps)
        .map(|i| CompletedStep {
            step_index: i,
            command: format!("command_{}", i),
            success: true,
            output: Some(vec![0u8; 1024].into_iter().map(|_| 'x').collect()),
            captured_variables: HashMap::new(),
            duration: Duration::from_secs(1),
            completed_at: chrono::Utc::now(),
            retry_state: None,
        })
        .collect();

    WorkflowCheckpoint {
        workflow_id: "large-workflow".to_string(),
        execution_state: ExecutionState {
            current_step_index: num_steps,
            total_steps: num_steps,
            status: WorkflowStatus::Completed,
            start_time: chrono::Utc::now(),
            last_checkpoint: chrono::Utc::now(),
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps,
        variable_state: variables,
        mapreduce_state: None,
        timestamp: chrono::Utc::now(),
        version: CHECKPOINT_VERSION,
        workflow_hash: "test-hash".to_string(),
        total_steps: num_steps,
        workflow_name: Some("large-workflow".to_string()),
        workflow_path: None,
        error_recovery_state: None,
        retry_checkpoint_state: None,
        variable_checkpoint_state: None,
    }
}

/// Generate a large workflow YAML string
fn generate_large_workflow(num_steps: usize) -> String {
    let mut yaml = String::from("name: large-benchmark-workflow\nmode: sequential\nsteps:\n");
    for i in 0..num_steps {
        yaml.push_str(&format!("  - shell: echo 'Step {}'\n", i));
        if i % 10 == 0 {
            yaml.push_str(&format!("    capture: step_{}_output\n", i));
        }
    }
    yaml
}

criterion_group!(
    benches,
    bench_storage_memory_usage,
    bench_checkpoint_memory_overhead,
    bench_worktree_cleanup_efficiency,
    bench_memory_leak_detection,
    bench_concurrent_memory_usage,
    bench_large_workflow_memory
);

criterion_main!(benches);
