//! Performance benchmarks for command execution and workflow processing

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use prodigy::config::WorkflowConfig;
use prodigy::cook::workflow::checkpoint::CheckpointManager;
use serde_json::json;
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create a simple workflow YAML with varying complexity
fn create_test_workflow_yaml(num_steps: usize) -> String {
    let mut commands = String::new();
    for i in 0..num_steps {
        commands.push_str(&format!("  - claude: echo 'Step {}'\n", i));
    }
    format!("commands:\n{}", commands)
}

/// Create test variable context with varying complexity
fn create_variable_context(num_vars: usize) -> HashMap<String, serde_json::Value> {
    let mut context = HashMap::new();

    for i in 0..num_vars {
        context.insert(format!("var_{}", i), json!(format!("value_{}", i)));
    }

    // Add nested structures
    context.insert(
        "shell".to_string(),
        json!({
            "output": "Command output with multiple lines\n".repeat(5),
            "exit_code": 0,
        }),
    );

    context.insert(
        "map".to_string(),
        json!({
            "results": (0..10).map(|i| json!({"item": i, "status": "completed"})).collect::<Vec<_>>(),
            "successful": 10,
            "failed": 0,
        }),
    );

    context
}

fn bench_workflow_parsing(c: &mut Criterion) {
    c.benchmark_group("workflow_parsing")
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .bench_function("small_workflow", |b| {
            let yaml = create_test_workflow_yaml(5);
            b.iter(|| {
                let workflow: WorkflowConfig = serde_yaml::from_str(&yaml).unwrap();
                black_box(workflow);
            });
        })
        .bench_function("medium_workflow", |b| {
            let yaml = create_test_workflow_yaml(50);
            b.iter(|| {
                let workflow: WorkflowConfig = serde_yaml::from_str(&yaml).unwrap();
                black_box(workflow);
            });
        })
        .bench_function("large_workflow", |b| {
            let yaml = create_test_workflow_yaml(200);
            b.iter(|| {
                let workflow: WorkflowConfig = serde_yaml::from_str(&yaml).unwrap();
                black_box(workflow);
            });
        })
        .bench_function("complex_workflow", |b| {
            let yaml = r#"
name: complex-workflow
env:
  GLOBAL_VAR: "value"
retry_defaults:
  attempts: 3
  backoff: exponential
steps:
  - shell: echo "Setup"
    on_failure:
      - shell: echo "Cleanup"
  - foreach:
      foreach: "find . -name '*.txt'"
      parallel: 10
      do:
        - shell: "process ${item}"
  - goal_seek:
      goal: "All tests pass"
      command: "cargo test"
      validate: "cargo test"
      threshold: 100
"#;
            b.iter(|| {
                let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
                black_box(workflow);
            });
        });
}

fn bench_variable_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("variable_operations");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for num_vars in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("variable_context_creation", num_vars),
            num_vars,
            |b, &num_vars| {
                b.iter(|| {
                    let context = create_variable_context(num_vars);
                    black_box(context);
                });
            },
        );
    }

    group.finish();
}

fn bench_workflow_validation(c: &mut Criterion) {
    c.benchmark_group("workflow_validation")
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .bench_function("validate_simple", |b| {
            let yaml = create_test_workflow_yaml(10);
            b.iter(|| {
                let workflow: WorkflowConfig = serde_yaml::from_str(&yaml).unwrap();
                // Validate workflow structure
                assert!(!workflow.commands.is_empty());
                black_box(workflow);
            });
        })
        .bench_function("validate_with_env", |b| {
            let yaml = r#"
name: env-workflow
env:
  VAR1: value1
  VAR2: value2
steps:
  - shell: echo "${VAR1}"
  - shell: echo "${VAR2}"
"#;
            b.iter(|| {
                let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
                assert!(workflow.env.is_some());
                black_box(workflow);
            });
        });
}

fn bench_real_world_scenarios(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("real_world_scenarios");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));

    // Scenario 1: Large codebase processing (e.g., linting/formatting)
    group.bench_function("codebase_processing", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let yaml = r#"
name: codebase-processing
mode: mapreduce
map:
  input: "files.json"
  json_path: "$.files[*]"
  agent_template:
    - shell: "echo 'Linting ${item.path}'"
    - shell: "echo 'Formatting ${item.path}'"
  max_parallel: 10
  max_items: 100
reduce:
  - shell: "echo 'Processed ${map.successful} files'"
  - shell: "echo 'Failed: ${map.failed} files'"
"#;
                let temp_dir = TempDir::new().unwrap();
                let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
                (workflow, temp_dir)
            },
            |(workflow, _temp_dir)| async move {
                // Simulate workflow execution planning
                black_box(workflow);
            },
            BatchSize::SmallInput,
        );
    });

    // Scenario 2: Complex deployment pipeline
    group.bench_function("deployment_pipeline", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let yaml = r#"
name: deployment-pipeline
env:
  ENV: production
  REGION: us-west-2
steps:
  - shell: "cargo test"
    on_failure:
      - shell: "echo 'Tests failed'"
      - shell: "exit 1"
  - shell: "cargo build --release"
  - shell: "docker build -t app:latest ."
  - shell: "docker push app:latest"
  - foreach:
      foreach: "aws ec2 describe-instances --query 'Reservations[*].Instances[*].InstanceId'"
      parallel: 5
      do:
        - shell: "aws ssm send-command --instance-ids ${item}"
  - shell: "kubectl rollout restart deployment/app"
  - shell: "kubectl rollout status deployment/app"
"#;
                let temp_dir = TempDir::new().unwrap();
                let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
                (workflow, temp_dir)
            },
            |(workflow, _temp_dir)| async move {
                // Simulate validation and planning
                assert!(!workflow.commands.is_empty());
                black_box(workflow);
            },
            BatchSize::SmallInput,
        );
    });

    // Scenario 3: Data processing pipeline with checkpointing
    group.bench_function("data_pipeline_with_checkpoint", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let yaml = r#"
name: data-processing-pipeline
checkpoint:
  enabled: true
  interval_minutes: 5
steps:
  - shell: "curl -o data.csv https://example.com/large-dataset.csv"
    capture: raw_data
  - shell: "python preprocess.py data.csv"
    capture: preprocessed_data
  - shell: "python analyze.py ${preprocessed_data.output}"
    capture: analysis_results
  - shell: "python generate_report.py ${analysis_results.output}"
    capture: report
  - shell: "aws s3 cp ${report.output} s3://bucket/reports/"
"#;
                let temp_dir = TempDir::new().unwrap();
                let workflow: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
                #[allow(deprecated)]
                let checkpoint_manager = CheckpointManager::new(temp_dir.path().to_path_buf());
                (workflow, checkpoint_manager, temp_dir)
            },
            |(workflow, checkpoint_manager, _temp_dir)| async move {
                // Simulate checkpoint operations during workflow
                let checkpoint = prodigy::cook::workflow::checkpoint::WorkflowCheckpoint {
                    workflow_id: "benchmark-workflow".to_string(),
                    execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
                        current_step_index: 0,
                        total_steps: workflow.commands.len(),
                        status: prodigy::cook::workflow::checkpoint::WorkflowStatus::Running,
                        start_time: chrono::Utc::now(),
                        last_checkpoint: chrono::Utc::now(),
                        current_iteration: None,
                        total_iterations: None,
                    },
                    completed_steps: vec![],
                    variable_state: HashMap::new(),
                    mapreduce_state: None,
                    timestamp: chrono::Utc::now(),
                    version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
                    workflow_hash: "test-hash".to_string(),
                    total_steps: workflow.commands.len(),
                    workflow_name: Some("benchmark-workflow".to_string()),
                    workflow_path: None,
                    error_recovery_state: None,
                    retry_checkpoint_state: None,
                    variable_checkpoint_state: None,
                };

                checkpoint_manager
                    .save_checkpoint(&checkpoint)
                    .await
                    .unwrap();
                black_box(checkpoint);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_workflow_resume_operation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("workflow_resume");

    group.bench_function("resume_from_checkpoint", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                #[allow(deprecated)]
                let checkpoint_manager = CheckpointManager::new(temp_dir.path().to_path_buf());

                // Create a checkpoint representing a partially completed workflow
                let checkpoint = prodigy::cook::workflow::checkpoint::WorkflowCheckpoint {
                    workflow_id: "resume-workflow".to_string(),
                    execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
                        current_step_index: 50,
                        total_steps: 100,
                        status: prodigy::cook::workflow::checkpoint::WorkflowStatus::Running,
                        start_time: chrono::Utc::now(),
                        last_checkpoint: chrono::Utc::now(),
                        current_iteration: None,
                        total_iterations: None,
                    },
                    completed_steps: (0..50)
                        .map(|i| prodigy::cook::workflow::checkpoint::CompletedStep {
                            step_index: i,
                            command: format!("command_{}", i),
                            success: true,
                            output: Some(format!("Output {}", i)),
                            captured_variables: HashMap::new(),
                            duration: Duration::from_secs(1),
                            completed_at: chrono::Utc::now(),
                            retry_state: None,
                        })
                        .collect(),
                    variable_state: create_variable_context(100),
                    mapreduce_state: None,
                    timestamp: chrono::Utc::now(),
                    version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
                    workflow_hash: "test-hash".to_string(),
                    total_steps: 100,
                    workflow_name: Some("resume-workflow".to_string()),
                    workflow_path: None,
                    error_recovery_state: None,
                    retry_checkpoint_state: None,
                    variable_checkpoint_state: None,
                };

                let rt_local = Runtime::new().unwrap();
                rt_local.block_on(async {
                    checkpoint_manager
                        .save_checkpoint(&checkpoint)
                        .await
                        .unwrap();
                });

                (checkpoint_manager, checkpoint.workflow_id, temp_dir)
            },
            |(checkpoint_manager, workflow_id, _temp_dir)| async move {
                let checkpoint = checkpoint_manager
                    .load_checkpoint(&workflow_id)
                    .await
                    .unwrap();

                // Simulate resume operations
                assert_eq!(checkpoint.execution_state.current_step_index, 50);
                black_box(checkpoint);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_validation_performance(c: &mut Criterion) {
    use prodigy::core::validation::{
        validate_command, validate_environment, validate_iteration_count, validate_json_schema,
        validate_paths, validate_resource_limits, ResourceLimits,
    };
    use std::path::Path;

    let mut group = c.benchmark_group("validation_performance");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    // Benchmark: Command validation
    group.bench_function("validate_single_command", |b| {
        let command = "cargo test --all";
        b.iter(|| {
            let result = validate_command(command);
            black_box(result);
        });
    });

    group.bench_function("validate_dangerous_command", |b| {
        let command = "sudo rm -rf / && dd if=/dev/zero";
        b.iter(|| {
            let result = validate_command(command);
            black_box(result);
        });
    });

    // Benchmark: Environment variable validation
    group.bench_function("validate_environment_small", |b| {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());
        let required = vec!["PATH", "HOME"];

        b.iter(|| {
            let result = validate_environment(&required, &env);
            black_box(result);
        });
    });

    group.bench_function("validate_environment_large", |b| {
        let mut env = HashMap::new();
        for i in 0..100 {
            env.insert(format!("VAR_{}", i), format!("value_{}", i));
        }
        let required = vec!["VAR_0", "VAR_50", "VAR_99"];

        b.iter(|| {
            let result = validate_environment(&required, &env);
            black_box(result);
        });
    });

    group.bench_function("validate_environment_missing_vars", |b| {
        let env = HashMap::new();
        let required = vec!["VAR1", "VAR2", "VAR3", "VAR4", "VAR5"];

        b.iter(|| {
            let result = validate_environment(&required, &env);
            black_box(result);
        });
    });

    // Benchmark: Path validation
    group.bench_function("validate_paths_small", |b| {
        let paths: Vec<&Path> = vec![
            Path::new("/tmp/file1.txt"),
            Path::new("/tmp/file2.txt"),
            Path::new("/tmp/file3.txt"),
        ];
        let exists_check = |_: &Path| true;

        b.iter(|| {
            let result = validate_paths(&paths, exists_check);
            black_box(result);
        });
    });

    group.bench_function("validate_paths_large", |b| {
        // Create owned paths to avoid lifetime issues
        let path_strings: Vec<String> = (0..100)
            .map(|i| format!("/tmp/file{}.txt", i))
            .collect();
        let paths: Vec<&Path> = path_strings.iter().map(|s| Path::new(s.as_str())).collect();
        let exists_check = |_: &Path| true;

        b.iter(|| {
            let result = validate_paths(&paths, exists_check);
            black_box(result);
        });
    });

    // Benchmark: JSON schema validation
    group.bench_function("validate_json_simple", |b| {
        let json = json!({
            "name": "test",
            "value": 42,
            "description": "A test object"
        });
        let required = vec!["name", "value"];

        b.iter(|| {
            let result = validate_json_schema(&json, &required);
            black_box(result);
        });
    });

    group.bench_function("validate_json_complex", |b| {
        let mut obj = serde_json::Map::new();
        for i in 0..50 {
            obj.insert(format!("field_{}", i), json!(format!("value_{}", i)));
        }
        let json = json!(obj);
        let required: Vec<&str> = (0..25).map(|i| Box::leak(format!("field_{}", i).into_boxed_str()) as &str).collect();

        b.iter(|| {
            let result = validate_json_schema(&json, &required);
            black_box(result);
        });
    });

    group.bench_function("validate_json_missing_fields", |b| {
        let json = json!({
            "existing": "value"
        });
        let required = vec!["field1", "field2", "field3", "field4", "field5"];

        b.iter(|| {
            let result = validate_json_schema(&json, &required);
            black_box(result);
        });
    });

    // Benchmark: Iteration count validation
    group.bench_function("validate_iteration_count", |b| {
        b.iter(|| {
            let result = validate_iteration_count(50, 100);
            black_box(result);
        });
    });

    // Benchmark: Resource limits validation
    group.bench_function("validate_resource_limits_valid", |b| {
        let limits = ResourceLimits {
            memory_mb: 2048,
            cpu_cores: 8,
            timeout_seconds: 300,
        };

        b.iter(|| {
            let result = validate_resource_limits(&limits);
            black_box(result);
        });
    });

    group.bench_function("validate_resource_limits_invalid", |b| {
        let limits = ResourceLimits {
            memory_mb: 0,
            cpu_cores: 0,
            timeout_seconds: 0,
        };

        b.iter(|| {
            let result = validate_resource_limits(&limits);
            black_box(result);
        });
    });

    // Benchmark: Error accumulation (multiple validations in sequence)
    group.bench_function("error_accumulation_multiple_validations", |b| {
        let command = "sudo rm -rf / && dd if=/dev/zero";
        let env = HashMap::new();
        let required = vec!["VAR1", "VAR2", "VAR3"];
        let json = json!({"existing": "value"});
        let required_fields = vec!["field1", "field2", "field3"];

        b.iter(|| {
            // Accumulate errors from multiple validators
            let cmd_result = validate_command(command);
            let env_result = validate_environment(&required, &env);
            let json_result = validate_json_schema(&json, &required_fields);

            black_box((cmd_result, env_result, json_result));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_workflow_parsing,
    bench_variable_operations,
    bench_workflow_validation,
    bench_real_world_scenarios,
    bench_workflow_resume_operation,
    bench_validation_performance
);

criterion_main!(benches);
