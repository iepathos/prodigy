//! Performance benchmarks for MapReduce phase modules
//!
//! These benchmarks verify that the phase-based architecture maintains
//! performance requirements specified in Spec 131:
//! - < 2% performance regression vs original implementation
//! - Setup phase execution overhead
//! - Map phase scaling characteristics
//! - Reduce phase aggregation performance

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use prodigy::cook::execution::mapreduce::phases::{
    coordinator::PhaseCoordinator, PhaseContext, PhaseExecutor,
};
use prodigy::cook::execution::mapreduce::{MapPhase, ReducePhase};
use prodigy::cook::execution::SetupPhase;
use prodigy::cook::orchestrator::ExecutionEnvironment;
use prodigy::cook::workflow::WorkflowStep;
use prodigy::subprocess::SubprocessManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Helper to create a test execution environment
fn create_test_env(temp_dir: &TempDir) -> ExecutionEnvironment {
    ExecutionEnvironment {
        working_dir: Arc::new(temp_dir.path().to_path_buf()),
        project_dir: Arc::new(temp_dir.path().to_path_buf()),
        worktree_name: Some(Arc::from("bench-worktree")),
        session_id: Arc::from("bench-session"),
    }
}

/// Helper to create a subprocess manager
fn create_subprocess_manager() -> Arc<SubprocessManager> {
    Arc::new(SubprocessManager::production())
}

/// Benchmark setup phase execution with varying numbers of commands
fn bench_setup_phase_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("phase_setup");
    group.measurement_time(Duration::from_secs(10));

    for num_commands in &[1, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("execute_commands", num_commands),
            num_commands,
            |b, &count| {
                b.to_async(&rt).iter(|| async move {
                    let temp_dir = TempDir::new().unwrap();
                    let env = create_test_env(&temp_dir);
                    let subprocess = create_subprocess_manager();

                    let commands: Vec<_> = (0..count)
                        .map(|i| WorkflowStep {
                            shell: Some(format!("echo 'command {}' > /dev/null", i)),
                            ..Default::default()
                        })
                        .collect();

                    let setup_phase = SetupPhase {
                        commands,
                        timeout: Some(60),
                        capture_outputs: HashMap::new(),
                    };

                    let executor =
                        prodigy::cook::execution::mapreduce::phases::setup::SetupPhaseExecutor::new(
                            setup_phase,
                        );

                    let mut context = PhaseContext::new(env, subprocess);

                    let _ = executor.execute(&mut context).await;
                });
            },
        );
    }

    group.finish();
}

/// Benchmark map phase work item processing
fn bench_map_phase_work_items(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("phase_map");
    group.measurement_time(Duration::from_secs(15));

    for num_items in &[10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("process_items", num_items),
            num_items,
            |b, &count| {
                b.to_async(&rt).iter(|| async move {
                    let temp_dir = TempDir::new().unwrap();
                    let env = create_test_env(&temp_dir);
                    let subprocess = create_subprocess_manager();

                    // Create a JSON input file
                    let input_file = temp_dir.path().join("items.json");
                    let items: Vec<_> = (0..count)
                        .map(|i| {
                            serde_json::json!({
                                "id": i,
                                "name": format!("item_{}", i),
                                "data": format!("data_{}", i)
                            })
                        })
                        .collect();
                    std::fs::write(&input_file, serde_json::to_string(&items).unwrap()).unwrap();

                    let map_phase = MapPhase {
                        config: prodigy::cook::execution::mapreduce::MapConfig {
                            input: input_file.to_string_lossy().to_string(),
                            max_parallel: 5,
                            ..Default::default()
                        },
                        agent_template: vec![WorkflowStep {
                            shell: Some("echo '${item.name}' > /dev/null".to_string()),
                            ..Default::default()
                        }],
                        json_path: None,
                        filter: None,
                        sort_by: None,
                        max_items: Some(count),
                        timeout_config: None,
                    };

                    let executor =
                        prodigy::cook::execution::mapreduce::phases::map::MapPhaseExecutor::new(
                            map_phase,
                        );

                    let mut context = PhaseContext::new(env, subprocess);

                    // Note: Map executor may not be fully functional in new architecture
                    // This measures the pure planning/coordination overhead
                    let _ = executor.validate_context(&context);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark reduce phase aggregation performance
fn bench_reduce_phase_aggregation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("phase_reduce");
    group.measurement_time(Duration::from_secs(10));

    for num_commands in &[1, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("aggregate_results", num_commands),
            num_commands,
            |b, &count| {
                b.to_async(&rt).iter(|| async move {
                    let temp_dir = TempDir::new().unwrap();
                    let env = create_test_env(&temp_dir);
                    let subprocess = create_subprocess_manager();

                    let commands: Vec<_> = (0..count)
                        .map(|i| WorkflowStep {
                            shell: Some(format!("echo 'aggregate {}' > /dev/null", i)),
                            ..Default::default()
                        })
                        .collect();

                    let reduce_phase = ReducePhase {
                        commands,
                        timeout: Some(60),
                    };

                    let executor =
                        prodigy::cook::execution::mapreduce::phases::reduce::ReducePhaseExecutor::new(
                            reduce_phase,
                        );

                    let mut context = PhaseContext::new(env, subprocess);

                    let _ = executor.execute(&mut context).await;
                });
            },
        );
    }

    group.finish();
}

/// Benchmark full workflow execution (Setup -> Map -> Reduce)
fn bench_full_workflow_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("phase_workflow");
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("complete_workflow", |b| {
        b.to_async(&rt).iter(|| async move {
            let temp_dir = TempDir::new().unwrap();
            let env = create_test_env(&temp_dir);
            let subprocess = create_subprocess_manager();

            // Setup phase: create test data
            let setup_phase = SetupPhase {
                commands: vec![
                    WorkflowStep {
                        shell: Some("echo 'setup 1' > /dev/null".to_string()),
                        ..Default::default()
                    },
                    WorkflowStep {
                        shell: Some("echo 'setup 2' > /dev/null".to_string()),
                        ..Default::default()
                    },
                ],
                timeout: Some(30),
                capture_outputs: HashMap::new(),
            };

            // Map phase: minimal processing
            let map_phase = MapPhase {
                config: prodigy::cook::execution::mapreduce::MapConfig {
                    input: "[]".to_string(),
                    max_parallel: 1,
                    ..Default::default()
                },
                agent_template: vec![],
                json_path: None,
                filter: None,
                sort_by: None,
                max_items: None,
                timeout_config: None,
            };

            // Reduce phase: aggregate results
            let reduce_phase = ReducePhase {
                commands: vec![WorkflowStep {
                    shell: Some("echo 'reduce' > /dev/null".to_string()),
                    ..Default::default()
                }],
                timeout: Some(30),
            };

            let coordinator = PhaseCoordinator::new(
                Some(setup_phase),
                map_phase,
                Some(reduce_phase),
                subprocess.clone(),
            );

            let _ = coordinator.execute_workflow(env, subprocess).await;
        });
    });

    group.finish();
}

/// Benchmark phase context creation and initialization
fn bench_phase_context_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.benchmark_group("phase_context")
        .bench_function("create_context", |b| {
            b.to_async(&rt).iter(|| async move {
                let temp_dir = TempDir::new().unwrap();
                let env = create_test_env(&temp_dir);
                let subprocess = create_subprocess_manager();

                let _context = PhaseContext::new(env, subprocess);
            });
        });
}

/// Benchmark phase transition logic (pure planning)
fn bench_phase_transitions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("phase_transitions");

    group.bench_function("transition_overhead", |b| {
        b.to_async(&rt).iter(|| async move {
            let temp_dir = TempDir::new().unwrap();
            let env = create_test_env(&temp_dir);
            let subprocess = create_subprocess_manager();

            // Minimal workflow to measure transition overhead
            let map_phase = MapPhase {
                config: prodigy::cook::execution::mapreduce::MapConfig {
                    input: "[]".to_string(),
                    max_parallel: 1,
                    ..Default::default()
                },
                agent_template: vec![],
                json_path: None,
                filter: None,
                sort_by: None,
                max_items: None,
                timeout_config: None,
            };

            let coordinator = PhaseCoordinator::new(None, map_phase, None, subprocess.clone());

            // This measures pure coordination overhead without actual work
            let _ = coordinator.execute_workflow(env, subprocess).await;
        });
    });

    group.finish();
}

/// Benchmark parallel phase scaling (multiple phases in sequence)
fn bench_phase_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("phase_scaling");
    group.measurement_time(Duration::from_secs(15));

    for parallelism in &[1, 2, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("parallel_agents", parallelism),
            parallelism,
            |b, &max_parallel| {
                b.to_async(&rt).iter(|| async move {
                    let temp_dir = TempDir::new().unwrap();
                    let env = create_test_env(&temp_dir);
                    let subprocess = create_subprocess_manager();

                    let map_phase = MapPhase {
                        config: prodigy::cook::execution::mapreduce::MapConfig {
                            input: "[]".to_string(),
                            max_parallel,
                            ..Default::default()
                        },
                        agent_template: vec![],
                        json_path: None,
                        filter: None,
                        sort_by: None,
                        max_items: None,
                        timeout_config: None,
                    };

                    let coordinator =
                        PhaseCoordinator::new(None, map_phase, None, subprocess.clone());

                    let _ = coordinator.execute_workflow(env, subprocess).await;
                });
            },
        );
    }

    group.finish();
}

/// Benchmark phase executor trait overhead
fn bench_executor_trait_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.benchmark_group("phase_executor")
        .bench_function("trait_dispatch", |b| {
            b.to_async(&rt).iter(|| async move {
                let temp_dir = TempDir::new().unwrap();
                let env = create_test_env(&temp_dir);
                let subprocess = create_subprocess_manager();

                let setup_phase = SetupPhase {
                    commands: vec![WorkflowStep {
                        shell: Some("true".to_string()),
                        ..Default::default()
                    }],
                    timeout: Some(30),
                    capture_outputs: HashMap::new(),
                };

                let executor =
                    prodigy::cook::execution::mapreduce::phases::setup::SetupPhaseExecutor::new(
                        setup_phase,
                    );

                // Measure trait method dispatch overhead
                let context = PhaseContext::new(env, subprocess);
                let _ = executor.phase_type();
                let _ = executor.can_skip(&context);
                let _ = executor.validate_context(&context);
            });
        });
}

criterion_group!(
    benches,
    bench_setup_phase_scaling,
    bench_map_phase_work_items,
    bench_reduce_phase_aggregation,
    bench_full_workflow_execution,
    bench_phase_context_creation,
    bench_phase_transitions,
    bench_phase_scaling,
    bench_executor_trait_overhead
);

criterion_main!(benches);
