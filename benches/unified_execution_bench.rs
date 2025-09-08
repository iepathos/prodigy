//! Performance benchmarks for the unified execution path
//!
//! This benchmark suite measures the performance of the new unified
//! command execution pipeline to ensure no regressions are introduced.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use prodigy::cook::execution::{
    command::*, executor::*, output::*, process::*, CommandExecutor as UnifiedExecutor,
    UnifiedCommandExecutor,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark simple command execution
fn bench_simple_command_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("simple_echo_command", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor();
                let request = create_echo_request();
                let _result = executor.execute(black_box(request)).await;
            });
        });
    });
}

/// Benchmark command with large output
fn bench_large_output_command(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("large_output_processing", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor();
                let request = create_large_output_request();
                let _result = executor.execute(black_box(request)).await;
            });
        });
    });
}

/// Benchmark command with complex environment setup
fn bench_complex_environment_command(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("complex_environment_setup", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor();
                let request = create_complex_env_request();
                let _result = executor.execute(black_box(request)).await;
            });
        });
    });
}

/// Benchmark concurrent command execution
fn bench_concurrent_commands(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("concurrent_10_commands", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = Arc::new(create_test_executor());
                let mut handles = vec![];

                for _ in 0..10 {
                    let executor = executor.clone();
                    let handle = tokio::spawn(async move {
                        let request = create_echo_request();
                        executor.execute(request).await
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    let _ = handle.await;
                }
            });
        });
    });
}

/// Benchmark output parsing and processing
fn bench_output_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("json_output_parsing", |b| {
        b.iter(|| {
            rt.block_on(async {
                let processor = OutputProcessor::new();
                let raw_output = create_json_output();
                let _result = processor
                    .process_output(
                        black_box(raw_output),
                        CommandType::Shell,
                        Some(OutputFormat::Json),
                    )
                    .await;
            });
        });
    });

    c.bench_function("yaml_output_parsing", |b| {
        b.iter(|| {
            rt.block_on(async {
                let processor = OutputProcessor::new();
                let raw_output = create_yaml_output();
                let _result = processor
                    .process_output(
                        black_box(raw_output),
                        CommandType::Shell,
                        Some(OutputFormat::Yaml),
                    )
                    .await;
            });
        });
    });
}

/// Benchmark command validation
fn bench_command_validation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("command_validation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor();
                let request = create_echo_request();
                let _result = executor.validate(black_box(&request)).await;
            });
        });
    });
}

/// Benchmark resource estimation
fn bench_resource_estimation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("resource_estimation_claude", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor();
                let request = create_claude_request();
                let _result = executor.estimate_resources(black_box(&request)).await;
            });
        });
    });

    c.bench_function("resource_estimation_shell", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor();
                let request = create_echo_request();
                let _result = executor.estimate_resources(black_box(&request)).await;
            });
        });
    });
}

/// Benchmark command spec to executable conversion
fn bench_command_conversion(c: &mut Criterion) {
    c.bench_function("spec_to_executable_claude", |b| {
        b.iter(|| {
            let spec = CommandSpec::Claude {
                command: "test command".to_string(),
                context: None,
                tools: None,
                output_format: None,
            };
            let context = ExecutionContext::default();
            let _result = spec.to_executable_command(black_box(&context));
        });
    });

    c.bench_function("spec_to_executable_shell", |b| {
        b.iter(|| {
            let spec = CommandSpec::Shell {
                command: "echo test".to_string(),
                shell: Some("bash".to_string()),
                working_dir: Some(PathBuf::from("/tmp")),
                env: Some(HashMap::new()),
            };
            let context = ExecutionContext::default();
            let _result = spec.to_executable_command(black_box(&context));
        });
    });
}

/// Benchmark variable substitution
fn bench_variable_substitution(c: &mut Criterion) {
    c.bench_function("variable_substitution_simple", |b| {
        let mut context = ExecutionContext::default();
        context.variables.insert("name".to_string(), "world".to_string());
        let input = "Hello ${name}!";

        b.iter(|| {
            context.substitute_variables(black_box(input));
        });
    });

    c.bench_function("variable_substitution_complex", |b| {
        let mut context = ExecutionContext::default();
        for i in 0..20 {
            context
                .variables
                .insert(format!("var{}", i), format!("value{}", i));
        }
        let input = "Test ${var0} ${var1} ${var2} ${var3} ${var4} ${var5} end";

        b.iter(|| {
            context.substitute_variables(black_box(input));
        });
    });
}

// Helper functions to create test data

fn create_test_executor() -> UnifiedCommandExecutor {
    struct MockObservability;
    
    #[async_trait::async_trait]
    impl executor::ObservabilityCollector for MockObservability {
        async fn record_command_start(&self, _context: &executor::ExecutionContextInternal) {}
        async fn record_command_complete(&self, _result: &anyhow::Result<executor::CommandResult>) {}
    }

    UnifiedCommandExecutor::new(
        Arc::new(ProcessManager::new()),
        Arc::new(OutputProcessor::new()),
        Arc::new(MockObservability),
        Arc::new(executor::ResourceMonitor),
    )
}

fn create_echo_request() -> CommandRequest {
    CommandRequest {
        spec: CommandSpec::Shell {
            command: "echo test".to_string(),
            shell: None,
            working_dir: None,
            env: None,
        },
        execution_config: ExecutionConfig::default(),
        context: ExecutionContext::default(),
        metadata: CommandMetadata::new("bench"),
    }
}

fn create_claude_request() -> CommandRequest {
    CommandRequest {
        spec: CommandSpec::Claude {
            command: "test command".to_string(),
            context: Some("test context".to_string()),
            tools: None,
            output_format: Some(OutputFormat::Json),
        },
        execution_config: ExecutionConfig::default(),
        context: ExecutionContext::default(),
        metadata: CommandMetadata::new("bench"),
    }
}

fn create_large_output_request() -> CommandRequest {
    CommandRequest {
        spec: CommandSpec::Shell {
            command: "seq 1 10000".to_string(), // Generate large output
            shell: None,
            working_dir: None,
            env: None,
        },
        execution_config: ExecutionConfig {
            capture_output: CaptureOutputMode::Both,
            ..ExecutionConfig::default()
        },
        context: ExecutionContext::default(),
        metadata: CommandMetadata::new("bench"),
    }
}

fn create_complex_env_request() -> CommandRequest {
    let mut env = HashMap::new();
    for i in 0..50 {
        env.insert(format!("VAR_{}", i), format!("value_{}", i));
    }

    CommandRequest {
        spec: CommandSpec::Shell {
            command: "env".to_string(),
            shell: None,
            working_dir: Some(PathBuf::from("/tmp")),
            env: Some(env.clone()),
        },
        execution_config: ExecutionConfig {
            env,
            timeout: Some(Duration::from_secs(10)),
            capture_output: CaptureOutputMode::Both,
            ..ExecutionConfig::default()
        },
        context: ExecutionContext::default(),
        metadata: CommandMetadata::new("bench"),
    }
}

fn create_json_output() -> ProcessOutput {
    ProcessOutput::new().with_stdout(
        r#"{
            "status": "success",
            "data": {
                "items": [1, 2, 3, 4, 5],
                "metadata": {
                    "count": 5,
                    "timestamp": "2024-01-01T12:00:00Z"
                }
            }
        }"#
        .to_string(),
    )
}

fn create_yaml_output() -> ProcessOutput {
    ProcessOutput::new().with_stdout(
        r#"status: success
data:
  items:
    - 1
    - 2
    - 3
    - 4
    - 5
  metadata:
    count: 5
    timestamp: 2024-01-01T12:00:00Z"#
            .to_string(),
    )
}

criterion_group!(
    benches,
    bench_simple_command_execution,
    bench_large_output_command,
    bench_complex_environment_command,
    bench_concurrent_commands,
    bench_output_processing,
    bench_command_validation,
    bench_resource_estimation,
    bench_command_conversion,
    bench_variable_substitution
);

criterion_main!(benches);