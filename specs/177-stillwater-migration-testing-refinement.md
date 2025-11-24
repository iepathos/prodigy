---
number: 177
title: Stillwater Migration Testing and Refinement
category: testing
priority: critical
status: draft
dependencies: [172, 173, 174, 175, 176]
created: 2025-11-24
---

# Specification 177: Stillwater Migration Testing and Refinement

**Category**: testing
**Priority**: critical
**Status**: draft
**Dependencies**: Specs 172, 173, 174, 175, 176 (all Stillwater migration phases)

## Context

After implementing Phases 1-5 of the Stillwater migration, we need comprehensive testing and refinement to ensure:

**Testing Requirements:**
- **Pure function testing** - Verify no-mock testing works as expected
- **Property-based testing** - Validate functional programming laws hold
- **Effect testing** - Verify effect composition and execution
- **Integration testing** - Ensure end-to-end workflows work correctly
- **Performance testing** - Validate no regression from functional patterns

**Refinement Requirements:**
- **Performance optimization** - Identify and fix any bottlenecks
- **Code quality** - Achieve quality metrics and zero warnings
- **Documentation** - Comprehensive guides and examples
- **Migration validation** - Confirm all goals achieved

This specification covers Phase 6 of the Stillwater migration: comprehensive testing, performance validation, and final refinement.

## Objective

Validate and refine the Stillwater migration by:
1. **Implementing comprehensive test suites** for all migration components
2. **Running performance benchmarks** to validate < 5% overhead
3. **Achieving quality metrics** (LOC reduction, test coverage, clippy clean)
4. **Creating property-based tests** for functional laws
5. **Documenting patterns** for future development
6. **Validating migration success** against original goals

## Requirements

### Functional Requirements

#### FR1: Pure Function Test Suite
- **MUST** test all pure functions without I/O mocking
- **MUST** achieve 100% coverage on pure functions
- **MUST** use simple assertions (no complex test setup)
- **MUST** run tests in < 1ms per pure function
- **MUST** verify determinism and referential transparency

#### FR2: Property-Based Testing
- **MUST** verify Semigroup associativity for aggregation
- **MUST** verify Validation accumulation properties
- **MUST** verify Effect composition laws
- **MUST** test with randomly generated inputs
- **MUST** use proptest or quickcheck for all property tests

#### FR3: Effect Testing with Mocks
- **MUST** test all effects with mock environments
- **MUST** verify effect composition chains
- **MUST** test error propagation through effects
- **MUST** verify context preservation
- **MUST** make mock environments simple to construct

#### FR4: Integration Testing
- **MUST** test end-to-end workflows with all phases
- **MUST** verify MapReduce parallelism works correctly
- **MUST** test checkpoint and resume with new effects
- **MUST** verify DLQ integration with validation
- **MUST** ensure backward compatibility with existing workflows

#### FR5: Performance Benchmarking
- **MUST** benchmark all critical paths
- **MUST** compare against baseline (pre-migration)
- **MUST** verify < 5% overhead from functional patterns
- **MUST** validate parallel speedup > 0.7 * num_agents
- **MUST** profile with cargo flamegraph for bottlenecks

### Non-Functional Requirements

#### NFR1: Code Quality Metrics
- **MUST** reduce orchestrator from 2,884 LOC to < 500 LOC
- **MUST** reduce workflow executor from 2,243 LOC to < 300 LOC (+ effects)
- **MUST** increase pure function LOC to > 1,000 LOC
- **MUST** achieve > 90% test coverage (100% on pure functions)
- **MUST** pass clippy with zero warnings

#### NFR2: Performance Metrics
- **MUST** maintain execution time ≤ 105% of baseline
- **MUST** maintain memory usage ≤ 110% of baseline
- **MUST** achieve parallel speedup > 0.7 * num_agents
- **MUST** have no deadlocks or race conditions
- **MUST** verify no performance regression in benchmarks

#### NFR3: Documentation Quality
- **MUST** update CLAUDE.md with all Stillwater patterns
- **MUST** update ARCHITECTURE.md with new design
- **MUST** provide comprehensive inline documentation
- **MUST** include examples for each pattern
- **MUST** document migration lessons learned

## Acceptance Criteria

- [ ] All pure functions have unit tests (no mocking)
- [ ] 100% test coverage on pure functions
- [ ] Property tests verify Semigroup, Validation, Effect laws
- [ ] Effect tests use mock environments successfully
- [ ] Integration tests cover all workflow types
- [ ] Performance benchmarks show < 5% overhead
- [ ] Parallel speedup verified (> 0.7 * num_agents)
- [ ] Memory usage within 110% of baseline
- [ ] Orchestrator reduced to < 500 LOC
- [ ] Workflow executor reduced to < 300 LOC (+ effects)
- [ ] Pure functions exceed 1,000 LOC
- [ ] Test coverage > 90%
- [ ] Zero clippy warnings
- [ ] Documentation complete and comprehensive
- [ ] All existing workflows pass without modification

## Technical Details

### Implementation Approach

#### 1. Pure Function Test Suite

```rust
// tests/pure/execution_planning.rs

#[test]
fn test_plan_execution_mapreduce() {
    let config = CookConfig {
        mapreduce: Some(MapReduceConfig {
            max_parallel: 10,
            setup: vec![Command::Shell { cmd: "echo setup".into() }],
            // ...
        }),
        ..Default::default()
    };

    let plan = plan_execution(&config);

    assert_eq!(plan.mode, ExecutionMode::MapReduce);
    assert_eq!(plan.parallel_budget, 10);
    assert_eq!(plan.resource_needs.worktrees, 11);
    assert_eq!(plan.phases.len(), 3); // setup, map, reduce

    // Pure function - no I/O, no mocks, deterministic!
}

#[test]
fn test_detect_execution_mode_all_variants() {
    // Test all mode detection branches
    assert_eq!(
        detect_execution_mode(&config_with_dry_run()),
        ExecutionMode::DryRun
    );

    assert_eq!(
        detect_execution_mode(&config_with_mapreduce()),
        ExecutionMode::MapReduce
    );

    assert_eq!(
        detect_execution_mode(&config_with_arguments()),
        ExecutionMode::Iterative
    );

    assert_eq!(
        detect_execution_mode(&config_standard()),
        ExecutionMode::Standard
    );
}

#[test]
fn test_calculate_resources_accuracy() {
    let mr_config = config_with_mapreduce(max_parallel = 20);

    let resources = calculate_resources(&mr_config, &ExecutionMode::MapReduce);

    assert_eq!(resources.worktrees, 21); // 20 agents + 1 parent
    assert_eq!(resources.max_concurrent_commands, 20);
    assert!(resources.memory_estimate > 0);
}

// tests/pure/command_builder.rs

#[test]
fn test_build_command_simple() {
    let template = "echo ${name} ${value}";
    let vars = [
        ("name".into(), "test".into()),
        ("value".into(), "123".into()),
    ]
    .iter()
    .cloned()
    .collect();

    let result = build_command(template, &vars);

    assert_eq!(result, "echo test 123");
}

#[test]
fn test_expand_variables_preserves_missing() {
    let template = "echo ${exists} ${missing}";
    let vars = [("exists".into(), "value".into())].iter().cloned().collect();

    let result = expand_variables(template, &vars);

    assert_eq!(result, "echo value ${missing}");
}

#[test]
fn test_extract_variable_references() {
    let template = "echo ${VAR1} $VAR2 ${VAR3}";

    let refs = extract_variable_references(template);

    assert_eq!(refs.len(), 3);
    assert!(refs.contains("VAR1"));
    assert!(refs.contains("VAR2"));
    assert!(refs.contains("VAR3"));
}

// tests/pure/session_updates.rs

#[test]
fn test_apply_status_update_valid_transition() {
    let session = UnifiedSession {
        status: SessionStatus::Running,
        ..Default::default()
    };

    let result = apply_session_update(
        session,
        SessionUpdate::Status(SessionStatus::Completed),
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap().status, SessionStatus::Completed);
}

#[test]
fn test_apply_status_update_invalid_transition() {
    let session = UnifiedSession {
        status: SessionStatus::Completed,
        ..Default::default()
    };

    let result = apply_session_update(
        session,
        SessionUpdate::Status(SessionStatus::Running),
    );

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        SessionError::InvalidTransition { .. }
    ));
}
```

#### 2. Property-Based Testing

```rust
// tests/property/semigroup.rs

use proptest::prelude::*;
use stillwater::Semigroup;

proptest! {
    /// Semigroup law: Associativity
    /// (a <> b) <> c == a <> (b <> c)
    #[test]
    fn prop_aggregate_result_associative(
        a in arb_aggregate_result(),
        b in arb_aggregate_result(),
        c in arb_aggregate_result(),
    ) {
        // Only test homogeneous results (same variant)
        if std::mem::discriminant(&a) == std::mem::discriminant(&b)
            && std::mem::discriminant(&b) == std::mem::discriminant(&c)
        {
            let left = a.clone().combine(b.clone()).combine(c.clone());
            let right = a.combine(b.combine(c));

            prop_assert_eq!(left, right);
        }
    }

    #[test]
    fn prop_execution_planning_deterministic(
        max_parallel in 1usize..100,
        dry_run: bool,
    ) {
        let config = create_test_config(max_parallel, dry_run);

        let plan1 = plan_execution(&config);
        let plan2 = plan_execution(&config);

        // Pure function - same input, same output
        prop_assert_eq!(plan1, plan2);
    }

    #[test]
    fn prop_variable_expansion_idempotent(
        template in ".*",
        vars in prop::collection::hash_map(".*", ".*", 0..10),
    ) {
        let result1 = expand_variables(&template, &vars);
        let result2 = expand_variables(&result1, &vars);

        // Should be idempotent after first expansion
        prop_assert_eq!(result1, result2);
    }
}

// tests/property/validation.rs

proptest! {
    /// Validation accumulates all errors
    #[test]
    fn prop_validation_accumulates_all_errors(
        items in prop::collection::vec(arb_invalid_work_item(), 1..20)
    ) {
        let schema = simple_schema();

        let result = validate_all_work_items(items.clone(), &schema);

        match result {
            Validation::Failure(errors) => {
                // Should have at least one error per invalid item
                prop_assert!(errors.len() >= items.len());
            }
            _ => prop_assert!(false, "Expected validation failure"),
        }
    }

    /// Valid items always validate successfully
    #[test]
    fn prop_validation_success_identity(
        items in prop::collection::vec(arb_valid_work_item(), 1..20)
    ) {
        let schema = simple_schema();

        let result = validate_all_work_items(items.clone(), &schema);

        prop_assert!(matches!(result, Validation::Success(_)));

        if let Validation::Success(validated) = result {
            prop_assert_eq!(
                validated.iter().map(|v| &v.data).collect::<Vec<_>>(),
                items.iter().collect::<Vec<_>>()
            );
        }
    }
}
```

#### 3. Effect Testing with Mocks

```rust
// tests/effects/workflow_executor.rs

struct MockWorkflowEnv {
    claude_outputs: HashMap<String, String>,
    shell_outputs: HashMap<String, String>,
}

impl MockWorkflowEnv {
    fn with_claude_output(mut self, cmd: &str, output: &str) -> Self {
        self.claude_outputs.insert(cmd.into(), output.into());
        self
    }
}

#[tokio::test]
async fn test_execute_claude_command_effect() {
    let env = MockWorkflowEnv::default()
        .with_claude_output("/test", "success");

    let effect = execute_claude_command("/test", &HashMap::new());

    let result = effect.run_async(&env).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().stdout, "success");
}

#[tokio::test]
async fn test_effect_composition_error_propagation() {
    let env = MockWorkflowEnv::default()
        .with_claude_output("/step1", "output1")
        .with_claude_output("/step2", "error: failed");

    let effect = execute_claude_command("/step1", &HashMap::new())
        .and_then(|output1| {
            // Use output1 in step2
            execute_claude_command("/step2", &output1.variables)
        });

    let result = effect.run_async(&env).await;

    // Error in step2 should propagate
    assert!(result.is_err());
}

// tests/effects/mapreduce.rs

#[tokio::test]
async fn test_execute_agent_with_mock_environment() {
    let mock_env = MapEnvBuilder::new()
        .with_mock_worktree_manager()
        .with_mock_executor()
        .with_mock_storage()
        .build();

    let assignment = WorkAssignment {
        id: 0,
        item: json!({"test": true}),
        worktree_name: "agent-0".into(),
    };

    let effect = execute_agent(assignment);
    let result = effect.run_async(&mock_env).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, 0);
}
```

#### 4. Integration Testing

```rust
// tests/integration/mapreduce.rs

#[tokio::test]
async fn test_end_to_end_mapreduce_workflow() {
    // Setup test environment
    let temp_dir = TempDir::new().unwrap();
    let repo = init_test_repo(&temp_dir);

    // Create test workflow
    let workflow_file = create_mapreduce_workflow(&temp_dir, 10);

    // Execute workflow
    let result = execute_workflow(&workflow_file).await;

    assert!(result.is_ok());

    let execution_result = result.unwrap();
    assert_eq!(execution_result.successful_agents, 10);
    assert_eq!(execution_result.failed_agents, 0);

    // Verify all agents created commits
    let commits = get_recent_commits(&repo);
    assert_eq!(commits.len(), 10);

    // Verify cleanup
    let worktrees = list_worktrees(&repo);
    assert!(worktrees.is_empty());
}

#[tokio::test]
async fn test_parallel_execution_timing() {
    // 10 work items, each takes 1 second, max_parallel = 5
    let workflow = create_slow_workflow(10, Duration::from_secs(1), 5);

    let start = Instant::now();
    let result = execute_workflow(&workflow).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());

    // Should complete in ~2 seconds (10 items / 5 parallel)
    // Allow 30% overhead for scheduling
    assert!(elapsed < Duration::from_secs(3));
    assert!(elapsed > Duration::from_secs(2));

    // Verify speedup
    let sequential_time = Duration::from_secs(10);
    let speedup = sequential_time.as_secs_f64() / elapsed.as_secs_f64();

    // Should have > 70% parallel efficiency
    assert!(speedup > 0.7 * 5.0);
}

#[tokio::test]
async fn test_checkpoint_resume_with_effects() {
    let workflow = create_resumable_workflow(20);

    // Start execution
    let handle = tokio::spawn(async move {
        execute_workflow(&workflow).await
    });

    // Wait for 5 seconds, then interrupt
    tokio::time::sleep(Duration::from_secs(5)).await;
    handle.abort();

    // Wait for checkpoint to flush
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Resume workflow
    let result = resume_workflow(&workflow.session_id).await;

    assert!(result.is_ok());

    // Verify all work completed
    let execution_result = result.unwrap();
    assert_eq!(execution_result.successful_agents, 20);
}
```

#### 5. Performance Benchmarking

```rust
// benches/execution_planning.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_execution_planning(c: &mut Criterion) {
    let configs = vec![
        create_config(1),
        create_config(10),
        create_config(100),
    ];

    let mut group = c.benchmark_group("execution_planning");

    for config in &configs {
        group.bench_with_input(
            BenchmarkId::from_parameter(config.mapreduce.as_ref().unwrap().max_parallel),
            config,
            |b, config| {
                b.iter(|| {
                    plan_execution(black_box(config))
                })
            },
        );
    }

    group.finish();
}

fn bench_variable_expansion(c: &mut Criterion) {
    let template = "${var1} ${var2} ${var3} ${var4} ${var5}";
    let vars = create_var_map(100);

    c.bench_function("variable_expansion", |b| {
        b.iter(|| {
            expand_variables(black_box(template), black_box(&vars))
        })
    });
}

fn bench_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregation");

    for size in [10, 100, 1000] {
        let results: Vec<_> = (0..size)
            .map(|i| AggregateResult::Count(i))
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &results,
            |b, results| {
                b.iter(|| {
                    aggregate_map_results(black_box(results.clone()))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_execution_planning,
    bench_variable_expansion,
    bench_aggregation
);
criterion_main!(benches);

// benches/parallel_execution.rs

fn bench_parallel_speedup(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_execution");

    let items = create_work_items(100);

    for parallelism in [1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(parallelism),
            &parallelism,
            |b, &parallelism| {
                b.to_async(&runtime).iter(|| {
                    distribute_work(black_box(items.clone()), parallelism)
                })
            },
        );
    }

    group.finish();
}
```

#### 6. Code Quality Measurement

```bash
#!/bin/bash
# scripts/measure_migration_success.sh

echo "=== Stillwater Migration Success Metrics ==="
echo

echo "1. Lines of Code Analysis"
echo "  Orchestrator core:"
wc -l src/cook/orchestrator/core.rs | awk '{print "    " $1 " LOC (target: < 500)"}'

echo "  Workflow executor:"
wc -l src/cook/workflow/executor.rs 2>/dev/null || echo "    N/A (split into pure + effects)"

echo "  Pure functions:"
find src/core src/cook -name "pure.rs" -o -path "*/pure/*.rs" | xargs wc -l | tail -1 | awk '{print "    " $1 " LOC (target: > 1000)"}'

echo
echo "2. Test Coverage"
cargo tarpaulin --out Stdout | grep "Coverage"

echo
echo "3. Clippy Warnings"
cargo clippy --all-targets -- -D warnings

echo
echo "4. Performance Benchmarks"
cargo bench --bench execution_planning
cargo bench --bench parallel_execution

echo
echo "5. Test Execution Time"
time cargo test --release
```

### Architecture Changes

**New Test Modules:**
```
tests/
├── pure/                          # Pure function tests (no mocking)
│   ├── execution_planning.rs
│   ├── command_builder.rs
│   ├── output_parser.rs
│   └── session_updates.rs
├── property/                      # Property-based tests
│   ├── semigroup.rs
│   ├── validation.rs
│   └── effect.rs
├── effects/                       # Effect tests with mocks
│   ├── workflow_executor.rs
│   ├── mapreduce.rs
│   └── session_manager.rs
├── integration/                   # End-to-end tests
│   ├── mapreduce.rs
│   ├── checkpoint_resume.rs
│   └── validation_dlq.rs
└── helpers/                       # Test utilities
    ├── mock_environments.rs
    ├── test_workflows.rs
    └── assertions.rs

benches/
├── execution_planning.rs
├── variable_expansion.rs
├── aggregation.rs
└── parallel_execution.rs
```

### APIs and Interfaces

**Test Helpers:**
```rust
// tests/helpers/mock_environments.rs

pub struct MapEnvBuilder {
    config: MapConfig,
    worktree_manager: Option<Arc<MockWorktreeManager>>,
    executor: Option<Arc<MockExecutor>>,
    storage: Option<Arc<MockStorage>>,
    // ...
}

impl MapEnvBuilder {
    pub fn new() -> Self { ... }
    pub fn with_config(mut self, config: MapConfig) -> Self { ... }
    pub fn with_mock_worktree_manager(mut self) -> Self { ... }
    pub fn with_mock_executor(mut self) -> Self { ... }
    pub fn build(self) -> MapEnv { ... }
}

// tests/helpers/test_workflows.rs

pub fn create_mapreduce_workflow(
    items: usize,
    max_parallel: usize,
) -> WorkflowFile;

pub fn create_slow_workflow(
    items: usize,
    per_item_duration: Duration,
    max_parallel: usize,
) -> WorkflowFile;

pub fn create_workflow_with_invalid_items() -> WorkflowFile;

// tests/helpers/assertions.rs

pub fn assert_parallel_speedup(
    sequential_time: Duration,
    parallel_time: Duration,
    parallelism: usize,
    min_efficiency: f64,
);

pub fn assert_loc_reduction(
    before: usize,
    after: usize,
    target_reduction: f64,
);
```

## Dependencies

### Prerequisites
- All migration phases completed (Specs 172-176)
- Test framework setup (tokio-test, proptest, criterion)
- Benchmark baseline captured before migration

### Affected Components
- All migrated code
- All existing tests
- Documentation

### External Dependencies
- `proptest = "*"` (property-based testing)
- `criterion = "*"` (benchmarking)
- `cargo-tarpaulin` (coverage)
- `cargo-flamegraph` (profiling)

## Testing Strategy

### Test Coverage Goals
- **Pure functions**: 100% coverage
- **Effects**: > 90% coverage
- **Integration**: All critical paths covered
- **Overall**: > 90% coverage

### Performance Goals
- **Execution time**: ≤ 105% of baseline
- **Memory usage**: ≤ 110% of baseline
- **Parallel speedup**: > 0.7 * num_agents
- **Aggregation overhead**: < 5%

### Quality Goals
- **Orchestrator**: < 500 LOC
- **Workflow executor**: < 300 LOC (+ effects)
- **Pure functions**: > 1,000 LOC
- **Clippy warnings**: 0

## Documentation Requirements

### CLAUDE.md Updates
- Add "Stillwater Migration Patterns" section
- Document all new patterns (Effect, Reader, Validation)
- Provide comprehensive examples
- Include testing patterns

### ARCHITECTURE.md Updates
- Add "Functional Architecture" section
- Document pure core / imperative shell pattern
- Show module hierarchy
- Explain data flow

### Migration Guide
- Document lessons learned
- Provide pattern catalog
- Include anti-patterns to avoid
- Show before/after examples

## Implementation Notes

### Critical Success Factors
1. **100% pure function coverage** - No excuses
2. **Performance validation** - Must meet all metrics
3. **Quality metrics** - All targets achieved
4. **Comprehensive documentation** - Patterns well-documented

### Measurement Approach
1. Capture baseline metrics before migration
2. Measure after each phase
3. Track trends and identify regressions early
4. Optimize hot paths identified by flamegraph

### Refinement Process
1. Run full test suite
2. Run benchmarks and compare to baseline
3. Profile with flamegraph if performance issues
4. Optimize hot paths
5. Re-test and re-benchmark
6. Repeat until all metrics met

## Migration and Compatibility

### Success Validation
- [ ] All existing workflows pass
- [ ] All new tests pass
- [ ] All benchmarks within targets
- [ ] All quality metrics achieved
- [ ] Documentation complete

### Sign-off Criteria
- Technical lead review
- Performance validation
- Quality metrics achieved
- User acceptance testing passed

### Post-Migration Monitoring
- Track production performance for 2 weeks
- Monitor error rates
- Collect user feedback
- Address any issues promptly

## Rollback Strategy

If migration fails validation:
1. Identify specific issue
2. Fix in place if possible
3. If unfixable, rollback specific phase
4. Re-attempt with fixes

**Note**: By Phase 6, rollback should be rare - issues should be caught earlier.
