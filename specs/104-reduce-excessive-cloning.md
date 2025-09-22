---
number: 104
title: Reduce Excessive Cloning in Hot Paths
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-09-21
---

# Specification 104: Reduce Excessive Cloning in Hot Paths

## Context

The codebase contains 1,664 `.clone()` calls across 202 files, with particularly heavy usage in hot paths like MapReduce execution (198 clones), workflow orchestration (112 clones), and normalization (63 clones). This excessive cloning impacts performance, increases memory usage, and indicates potential architectural issues. Many of these clones could be eliminated through better use of references, `Arc`, `Cow`, and improved ownership patterns.

## Objective

Reduce unnecessary cloning throughout the codebase, particularly in performance-critical paths, while maintaining code clarity and safety. Target a 50% reduction in clone operations in hot paths.

## Requirements

### Functional Requirements

1. Identify and eliminate unnecessary clones in hot execution paths
2. Replace owned strings with `Arc<str>` or `Cow<'_, str>` where appropriate
3. Use references instead of cloning for read-only access
4. Implement efficient data sharing patterns for concurrent operations
5. Focus on high-clone modules:
   - `/src/cook/execution/mapreduce/mod.rs` (198 clones)
   - `/src/cook/workflow/executor.rs` (123 clones)
   - `/src/cook/orchestrator.rs` (112 clones)
   - `/src/cook/workflow/normalized.rs` (63 clones)

### Non-Functional Requirements

- Maintain thread safety and data race freedom
- No change in external behavior
- Improve memory usage and performance
- Keep code readable and maintainable
- Follow Rust ownership best practices

## Acceptance Criteria

- [ ] 50% reduction in clone operations in identified hot paths
- [ ] Memory usage reduced by at least 20% during MapReduce operations
- [ ] Performance benchmarks show measurable improvement
- [ ] All tests continue to pass
- [ ] No new lifetime complexity that hurts maintainability
- [ ] Documentation explains cloning strategies

## Technical Details

### Implementation Approach

1. **Audit Clone Usage**
   - Categorize clones by necessity (required vs convenience)
   - Identify shared immutable data candidates
   - Find clones in loops and recursive functions

2. **Refactoring Strategies**

   **Strategy 1: Use Arc for Shared Immutable Data**
   ```rust
   // Before: Cloning configuration repeatedly
   struct Executor {
       config: Config,
   }
   impl Executor {
       fn spawn_worker(&self) -> Worker {
           Worker::new(self.config.clone())
       }
   }

   // After: Share via Arc
   struct Executor {
       config: Arc<Config>,
   }
   impl Executor {
       fn spawn_worker(&self) -> Worker {
           Worker::new(Arc::clone(&self.config))
       }
   }
   ```

   **Strategy 2: Use Cow for Sometimes-Modified Data**
   ```rust
   // Before: Always cloning strings
   fn process_path(path: String) -> String {
       if path.starts_with("~/") {
           expand_home(path)
       } else {
           path
       }
   }

   // After: Clone only when needed
   fn process_path(path: &str) -> Cow<'_, str> {
       if path.starts_with("~/") {
           Cow::Owned(expand_home(path))
       } else {
           Cow::Borrowed(path)
       }
   }
   ```

   **Strategy 3: Use References with Lifetime Parameters**
   ```rust
   // Before: Cloning for temporary use
   fn validate(&self, config: Config) -> Result<()> {
       let validator = Validator::new(config.clone());
       validator.check()
   }

   // After: Borrow for temporary use
   fn validate(&self, config: &Config) -> Result<()> {
       let validator = Validator::new(config);
       validator.check()
   }
   ```

   **Strategy 4: Intern Common Strings**
   ```rust
   // Before: Cloning command names repeatedly
   commands.push(Command {
       name: "test".to_string(),
       args: args.clone(),
   });

   // After: Use interned strings
   static TEST_CMD: &str = "test";
   commands.push(Command {
       name: TEST_CMD,
       args: &args,
   });
   ```

### Specific Optimization Targets

1. **MapReduce Executor**
   - Share work items via Arc instead of cloning
   - Use Cow for agent configurations
   - Pool command templates

2. **Workflow Normalization**
   - Cache normalized commands
   - Share common workflow metadata
   - Use references for temporary transformations

3. **Configuration Management**
   - Load configuration once, share via Arc
   - Intern frequently used configuration keys
   - Use Cow for path manipulations

### Memory Profiling Approach

1. Use `valgrind --tool=massif` to measure heap usage
2. Benchmark before and after with `criterion`
3. Track allocations with custom allocator in tests
4. Monitor peak memory during MapReduce operations

## Dependencies

- No external dependencies
- May require API changes for lifetime parameters
- Impacts multiple modules simultaneously

## Testing Strategy

1. **Correctness Tests**
   - Ensure no use-after-free or data races
   - Verify concurrent access patterns work correctly
   - Test edge cases with empty/single/large collections

2. **Performance Tests**
   - Benchmark clone-heavy operations before/after
   - Measure memory usage reduction
   - Profile allocation patterns

3. **Stress Tests**
   - Run MapReduce with 1000+ work items
   - Test concurrent workflow execution
   - Verify no memory leaks over long runs

## Documentation Requirements

- Document when cloning is necessary vs convenient
- Provide guidelines for choosing between Arc, Rc, and Cow
- Create examples of efficient data sharing patterns
- Update performance documentation with improvements