//! Performance benchmarks for command execution and workflow processing

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use prodigy::cook::workflow::parser::WorkflowParser;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create a simple workflow YAML with varying complexity
fn create_test_workflow_yaml(num_steps: usize) -> String {
    let mut steps = String::new();
    for i in 0..num_steps {
        steps.push_str(&format!("  - shell: echo 'Step {}'\n", i));
    }
    format!("name: benchmark-workflow\nsteps:\n{}", steps)
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
                let parser = WorkflowParser::new();
                black_box(parser.parse(&yaml).unwrap());
            });
        })
        .bench_function("medium_workflow", |b| {
            let yaml = create_test_workflow_yaml(50);
            b.iter(|| {
                let parser = WorkflowParser::new();
                black_box(parser.parse(&yaml).unwrap());
            });
        })
        .bench_function("large_workflow", |b| {
            let yaml = create_test_workflow_yaml(200);
            b.iter(|| {
                let parser = WorkflowParser::new();
                black_box(parser.parse(&yaml).unwrap());
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
                let parser = WorkflowParser::new();
                black_box(parser.parse(yaml).unwrap());
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
                let parser = WorkflowParser::new();
                let workflow = parser.parse(&yaml).unwrap();
                // Validate workflow structure
                assert!(!workflow.name.is_empty());
                assert!(!workflow.steps.is_empty());
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
                let parser = WorkflowParser::new();
                let workflow = parser.parse(yaml).unwrap();
                assert!(workflow.env.is_some());
                black_box(workflow);
            });
        });
}

criterion_group!(
    benches,
    bench_workflow_parsing,
    bench_variable_operations,
    bench_workflow_validation
);

criterion_main!(benches);