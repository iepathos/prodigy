---
number: 75
title: Benchmark Infrastructure Modernization
category: testing
priority: medium
status: draft
dependencies: []
created: 2025-01-16
---

# Specification 75: Benchmark Infrastructure Modernization

**Category**: testing
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The benchmark infrastructure in Prodigy is completely broken and disabled (moved to `benches.disabled/`). The benchmarks use outdated APIs, reference non-existent modules, and fail to compile with current codebase architecture. This is problematic because performance testing is crucial for a workflow orchestration tool that needs to handle large-scale MapReduce operations efficiently.

Key issues with current benchmarks:
1. Import statements reference non-existent modules and structures
2. API calls use deprecated or removed methods
3. Benchmark setup code doesn't match current architecture
4. No integration with current execution pipeline
5. Missing benchmarks for critical new features (MapReduce, global storage, streaming)

## Objective

Modernize and restore the benchmark infrastructure to work with the current Prodigy architecture, add comprehensive benchmarks for all critical performance paths, and establish a continuous performance monitoring system to prevent regressions.

## Requirements

### Functional Requirements

1. **Infrastructure Restoration**
   - Fix all compilation errors in existing benchmarks
   - Update API calls to use current interfaces
   - Modernize benchmark setup and execution code
   - Integrate with current build system
   - Enable benchmark execution in CI/CD pipeline

2. **Core Execution Benchmarks**
   - Command execution pipeline performance
   - Workflow parsing and validation
   - Variable interpolation performance
   - Error handling overhead
   - Resume operation performance

3. **MapReduce Performance Benchmarks**
   - Work item distribution efficiency
   - Agent coordination overhead
   - Cross-worktree synchronization performance
   - Event logging throughput
   - Large-scale job execution (1000+ items)

4. **Storage System Benchmarks**
   - Global storage read/write performance
   - Checkpoint save/load operations
   - Event log write throughput
   - DLQ operations performance
   - Storage migration performance

5. **Memory and Resource Benchmarks**
   - Memory usage under load
   - Resource cleanup efficiency
   - Concurrent execution scaling
   - Memory leak detection
   - Resource contention measurement

6. **Real-World Scenario Benchmarks**
   - Typical workflow execution profiles
   - Large codebase processing scenarios
   - Complex variable interpolation workloads
   - Error-heavy workflow performance
   - Resume operation overhead

### Non-Functional Requirements

1. **Reliability**
   - Benchmarks run consistently across environments
   - Deterministic performance measurements
   - Robust to system load variations
   - Clear performance regression detection

2. **Maintainability**
   - Easy to add new benchmarks
   - Clear benchmark organization and naming
   - Automated benchmark result collection
   - Performance trend tracking

3. **Performance**
   - Fast benchmark execution (< 5 minutes total)
   - Minimal benchmark overhead
   - Accurate timing measurements
   - Statistical significance validation

## Acceptance Criteria

- [ ] All existing benchmarks compile and run successfully
- [ ] Comprehensive benchmarks cover all critical execution paths
- [ ] MapReduce performance benchmarks validate scalability
- [ ] Storage system benchmarks measure throughput accurately
- [ ] Memory usage benchmarks detect resource leaks
- [ ] Real-world scenario benchmarks reflect actual usage patterns
- [ ] Benchmark execution time is under 5 minutes
- [ ] Performance regression detection works reliably
- [ ] Benchmark results are automatically collected and stored
- [ ] Performance trends are tracked over time

## Technical Details

### Implementation Approach

```rust
// Updated benchmark structure using current APIs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use prodigy::{
    cook::{
        execution::{
            executor::UnifiedCommandExecutor,
            mapreduce::MapReduceExecutor,
            interpolation::VariableInterpolator,
        },
        workflow::{
            parser::WorkflowParser,
            checkpoint::CheckpointManager,
        },
    },
    storage::global::GlobalStorage,
};
use tokio::runtime::Runtime;
use std::sync::Arc;

fn bench_command_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("simple_shell_command", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor().await;
                let command = create_simple_shell_command();
                executor.execute(command).await.unwrap();
            });
        });
    });
}

fn bench_mapreduce_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("mapreduce_scaling");

    for item_count in [10, 100, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("items", item_count),
            item_count,
            |b, &item_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let executor = create_mapreduce_executor().await;
                        let workflow = create_test_mapreduce_workflow(item_count);
                        executor.execute(workflow).await.unwrap();
                    });
                });
            },
        );
    }
    group.finish();
}

fn bench_variable_interpolation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("complex_variable_interpolation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let interpolator = create_variable_interpolator().await;
                let template = create_complex_template();
                let context = create_large_variable_context();
                interpolator.interpolate(&template, &context).unwrap();
            });
        });
    });
}

fn bench_storage_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("storage_operations");

    group.bench_function("checkpoint_save", |b| {
        b.iter(|| {
            rt.block_on(async {
                let storage = create_test_storage().await;
                let checkpoint = create_large_checkpoint();
                storage.save_checkpoint(&checkpoint).await.unwrap();
            });
        });
    });

    group.bench_function("event_log_write", |b| {
        b.iter(|| {
            rt.block_on(async {
                let storage = create_test_storage().await;
                let events = create_test_events(100);
                storage.write_events(&events).await.unwrap();
            });
        });
    });

    group.finish();
}

fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("workflow_memory_overhead", |b| {
        b.iter(|| {
            rt.block_on(async {
                let initial_memory = get_memory_usage();

                let executor = create_test_executor().await;
                let workflow = create_memory_intensive_workflow();
                executor.execute(workflow).await.unwrap();

                let final_memory = get_memory_usage();
                let overhead = final_memory - initial_memory;

                // Assert memory usage is within acceptable bounds
                assert!(overhead < MAX_ACCEPTABLE_MEMORY_OVERHEAD);
            });
        });
    });
}

fn bench_real_world_scenarios(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("real_world_scenarios");

    group.bench_function("codebase_analysis_workflow", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_test_executor().await;
                let workflow = load_codebase_analysis_workflow();
                executor.execute(workflow).await.unwrap();
            });
        });
    });

    group.bench_function("large_file_processing", |b| {
        b.iter(|| {
            rt.block_on(async {
                let executor = create_mapreduce_executor().await;
                let workflow = create_file_processing_workflow(1000);
                executor.execute(workflow).await.unwrap();
            });
        });
    });

    group.finish();
}

// Helper functions for benchmark setup
async fn create_test_executor() -> UnifiedCommandExecutor {
    // Implementation using current APIs
    todo!("Create test executor with current architecture")
}

async fn create_mapreduce_executor() -> MapReduceExecutor {
    // Implementation using current MapReduce APIs
    todo!("Create MapReduce executor with current architecture")
}

fn create_test_mapreduce_workflow(item_count: usize) -> Workflow {
    // Generate workflow with specified number of items
    todo!("Create test workflow with {} items", item_count)
}

criterion_group!(
    benches,
    bench_command_execution,
    bench_mapreduce_scaling,
    bench_variable_interpolation,
    bench_storage_operations,
    bench_memory_usage,
    bench_real_world_scenarios
);

criterion_main!(benches);
```

### Architecture Changes

1. **Benchmark Framework Updates**
   - Update Criterion usage to latest version
   - Fix API compatibility issues
   - Modernize benchmark structure

2. **Test Data Generation**
   - Create realistic test workflows
   - Generate large datasets for scaling tests
   - Mock external dependencies appropriately

3. **Performance Monitoring Integration**
   - Automated benchmark execution
   - Performance regression detection
   - Result storage and trending

4. **Resource Measurement**
   - Memory usage tracking
   - CPU utilization monitoring
   - I/O performance measurement

### Data Structures

```rust
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub test_data_size: usize,
    pub iterations: u32,
    pub warmup_iterations: u32,
    pub timeout: Duration,
    pub memory_limit: usize,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub execution_time: Duration,
    pub memory_usage: usize,
    pub cpu_utilization: f64,
    pub io_operations: u64,
    pub throughput: f64,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub benchmark_name: String,
    pub metrics: PerformanceMetrics,
    pub baseline_comparison: Option<f64>, // Percentage change
    pub regression_detected: bool,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BenchmarkSuite {
    pub name: String,
    pub benchmarks: Vec<Box<dyn Benchmark>>,
    pub config: BenchmarkConfig,
    pub baseline_results: Option<Vec<BenchmarkResult>>,
}
```

### Integration Points

1. **Build System Integration**
   - Add benchmark targets to Cargo.toml
   - Configure benchmark execution environments
   - Integrate with CI/CD pipeline

2. **Performance Monitoring Integration**
   - Automated result collection
   - Performance trend analysis
   - Regression alert system

3. **Test Infrastructure Integration**
   - Shared test utilities with unit tests
   - Common test data generation
   - Mock service integration

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `benches/` directory (restored from `benches.disabled/`)
  - `Cargo.toml` benchmark configuration
  - CI/CD pipeline configuration
- **External Dependencies**:
  - `criterion` (performance benchmarking)
  - `memory-stats` (memory usage tracking)

## Testing Strategy

- **Benchmark Validation**:
  - Verify benchmark compilation and execution
  - Validate performance measurement accuracy
  - Test benchmark stability across runs

- **Performance Regression Testing**:
  - Baseline establishment for all benchmarks
  - Automated regression detection
  - Performance trend analysis

- **Resource Usage Testing**:
  - Memory leak detection
  - Resource cleanup validation
  - Scalability testing

- **Real-World Validation**:
  - Compare benchmark results with actual usage
  - Validate benchmark representativeness
  - Adjust benchmarks based on real performance data

## Documentation Requirements

- **Code Documentation**:
  - Benchmark implementation guide
  - Performance measurement methodology
  - Benchmark result interpretation

- **User Documentation**:
  - Performance characteristics guide
  - Benchmark execution instructions
  - Performance tuning recommendations

- **Architecture Updates**:
  - Performance monitoring architecture
  - Benchmark infrastructure overview
  - Continuous performance testing strategy

## Implementation Notes

1. **Deterministic Testing**: Ensure benchmarks produce consistent results across runs
2. **Resource Isolation**: Isolate benchmarks to prevent interference
3. **Realistic Scenarios**: Create benchmarks that reflect actual usage patterns
4. **Automated Monitoring**: Set up automated performance regression detection
5. **Gradual Rollout**: Start with core benchmarks and expand coverage incrementally

## Migration and Compatibility

- Restore disabled benchmarks in `benches.disabled/` directory
- Migrate to current API interfaces
- Maintain backward compatibility for performance tracking
- Establish new performance baselines for all benchmarks