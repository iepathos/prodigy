---
number: 103
title: Separate I/O Operations from Business Logic
category: foundation
priority: high
status: draft
dependencies: [102]
created: 2025-09-21
---

# Specification 103: Separate I/O Operations from Business Logic

## Context

The codebase contains numerous functions that mix I/O operations (file system, network, database) with pure business logic, violating the functional programming principle of "Pure functions for business logic, I/O at boundaries" from VISION.md. This mixing makes code harder to test, reason about, and maintain. Functions that both calculate and perform side effects cannot be easily unit tested without mocking I/O operations.

## Objective

Refactor the codebase to follow the "functional core, imperative shell" pattern, separating pure business logic from I/O operations to improve testability, maintainability, and adherence to functional programming principles.

## Requirements

### Functional Requirements

1. Extract pure business logic into separate functions that take inputs and return outputs
2. Move I/O operations to boundary functions that orchestrate pure functions
3. Ensure business logic functions have no side effects
4. Create clear interfaces between pure and I/O layers
5. Focus on critical modules:
   - Analytics engine (mixed calculation and persistence)
   - Configuration loading (parsing mixed with file I/O)
   - MapReduce execution (logic mixed with file operations)
   - Session management (state calculation with file writes)

### Non-Functional Requirements

- No change in external behavior
- Improved testability without mocking
- Better separation of concerns
- Easier reasoning about business logic
- Maintain or improve performance

## Acceptance Criteria

- [ ] All identified mixed functions separated into pure and I/O components
- [ ] Pure functions have no file system, network, or database dependencies
- [ ] Pure functions are fully tested without mocks
- [ ] I/O functions are thin wrappers around pure logic
- [ ] Clear module boundaries between pure and I/O code
- [ ] Documentation explains the separation pattern

## Technical Details

### Implementation Approach

1. **Identify Mixed Functions**
   - Functions that read files and process data
   - Functions that calculate and write results
   - Functions with both business logic and logging

2. **Refactoring Pattern**
   ```rust
   // Before: Mixed I/O and logic
   fn process_workflow(path: &Path) -> Result<()> {
       let content = fs::read_to_string(path)?;
       let parsed = serde_yaml::from_str(&content)?;
       let validated = validate_workflow(parsed)?;
       let result = optimize_workflow(validated);
       fs::write(path.with_extension("optimized"), result)?;
       Ok(())
   }

   // After: Separated concerns
   // Pure business logic
   fn transform_workflow(content: &str) -> Result<String> {
       let parsed = parse_workflow_pure(content)?;
       let validated = validate_workflow_pure(parsed)?;
       let optimized = optimize_workflow_pure(validated);
       Ok(serialize_workflow_pure(optimized))
   }

   // I/O shell
   fn process_workflow(path: &Path) -> Result<()> {
       let content = fs::read_to_string(path)?;
       let result = transform_workflow(&content)?;
       fs::write(path.with_extension("optimized"), result)?;
       Ok(())
   }
   ```

### Target Patterns for Refactoring

1. **Configuration Loading**
   ```rust
   // Before:
   fn load_and_validate_config(path: &Path) -> Result<Config> {
       let content = fs::read_to_string(path)?;
       let mut config: Config = toml::from_str(&content)?;
       config.resolve_paths()?;  // Modifies paths
       config.validate()?;       // Checks file existence
       Ok(config)
   }

   // After:
   fn parse_config(content: &str) -> Result<Config> {
       toml::from_str(content).map_err(Into::into)
   }

   fn validate_config_pure(config: Config, exists_fn: impl Fn(&Path) -> bool) -> Result<Config> {
       // Pure validation using exists_fn instead of direct I/O
       config.validate_with(exists_fn)
   }

   fn load_config(path: &Path) -> Result<Config> {
       let content = fs::read_to_string(path)?;
       let config = parse_config(&content)?;
       validate_config_pure(config, |p| p.exists())
   }
   ```

2. **Analytics Processing**
   ```rust
   // Before:
   fn analyze_metrics(&self, session_id: &str) -> Result<Report> {
       let data = self.db.load_metrics(session_id)?;
       let aggregated = self.aggregate(data);
       let report = self.generate_report(aggregated);
       self.db.save_report(&report)?;
       report
   }

   // After:
   fn calculate_metrics(data: Vec<Metric>) -> Report {
       let aggregated = aggregate_pure(data);
       generate_report_pure(aggregated)
   }

   fn analyze_metrics(&self, session_id: &str) -> Result<Report> {
       let data = self.db.load_metrics(session_id)?;
       let report = calculate_metrics(data);
       self.db.save_report(&report)?;
       Ok(report)
   }
   ```

### Module Organization

Create clear separation:
```
src/
  core/           # Pure business logic
    workflow/     # Pure workflow transformations
    metrics/      # Pure metric calculations
    validation/   # Pure validation logic
  io/             # I/O operations
    fs/           # File system operations
    git/          # Git operations
    storage/      # Storage backends
  app/            # Application orchestration
    commands/     # CLI command handlers
```

## Dependencies

- Depends on Spec 102 for functional patterns
- No external library dependencies
- Requires refactoring of module structure

## Testing Strategy

1. **Pure Function Tests**
   - Test all business logic without any mocks
   - Property-based testing for transformations
   - Exhaustive edge case testing

2. **I/O Integration Tests**
   - Test I/O wrappers with real file system
   - Verify correct orchestration of pure functions
   - Test error handling at boundaries

3. **Performance Tests**
   - Ensure separation doesn't impact performance
   - Benchmark pure functions independently
   - Measure I/O overhead separately

## Documentation Requirements

- Document the functional core/imperative shell pattern
- Provide examples of proper separation
- Create guidelines for identifying mixed functions
- Update architecture documentation with layer separation