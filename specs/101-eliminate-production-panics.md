---
number: 101
title: Eliminate Production Panic Calls
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-09-21
---

# Specification 101: Eliminate Production Panic Calls

## Context

The codebase currently contains 48 `panic!()` calls in production code paths, violating the VISION.md requirement of "Zero panics in production code". These panics can cause the application to crash unexpectedly, leading to data loss and poor user experience. Additionally, there are 3,004 `unwrap()` calls that can also trigger panics when encountering None or Err values.

## Objective

Replace all production panic calls and high-risk unwrap calls with proper error handling to ensure the application never crashes due to preventable panics.

## Requirements

### Functional Requirements

1. Replace all 48 production `panic!()` calls with proper Result-based error handling
2. Replace critical path `unwrap()` calls with error propagation using the `?` operator
3. Implement graceful error recovery for all failure scenarios
4. Preserve error context for debugging while preventing crashes
5. Focus on critical modules first:
   - Session management (32 unwraps in tracker.rs)
   - Git operations (23 unwraps in git/mod.rs)
   - Analytics engine (8 unwraps in engine.rs)
   - API server (6 unwraps in api_server.rs)

### Non-Functional Requirements

- No performance degradation from error handling changes
- Maintain or improve error message clarity
- Ensure all error paths are tested
- Follow Rust idioms for error handling

## Acceptance Criteria

- [ ] Zero `panic!()` calls in production code (tests excluded)
- [ ] All critical path `unwrap()` calls replaced with proper error handling
- [ ] Comprehensive error tests for all modified code paths
- [ ] No regressions in existing functionality
- [ ] Error messages provide sufficient context for debugging
- [ ] All modified functions return `Result<T, E>` where appropriate

## Technical Details

### Implementation Approach

1. **Phase 1: Audit and Categorize**
   - Identify all panic sites with context
   - Categorize by risk level (critical path vs edge cases)
   - Create tracking issue for each module

2. **Phase 2: Replace Panics**
   - Convert `panic!()` to `anyhow::bail!()` or custom errors
   - Replace `unwrap()` with `?` operator or `ok_or_else()`
   - Add context using `.context()` or `.with_context()`

3. **Phase 3: Test Coverage**
   - Add unit tests for error paths
   - Verify error propagation works correctly
   - Test error recovery mechanisms

### Example Transformations

```rust
// Before: Panic on error
let config = load_config().unwrap();
if !config.is_valid() {
    panic!("Invalid configuration");
}

// After: Proper error handling
let config = load_config()
    .context("Failed to load configuration")?;
if !config.is_valid() {
    anyhow::bail!("Invalid configuration: {}", config.validation_error());
}
```

```rust
// Before: Multiple unwraps
let path = args.get(0).unwrap();
let content = fs::read_to_string(path).unwrap();
let parsed = parse_yaml(&content).unwrap();

// After: Error propagation with context
let path = args.get(0)
    .ok_or_else(|| anyhow!("Missing path argument"))?;
let content = fs::read_to_string(path)
    .with_context(|| format!("Failed to read file: {}", path))?;
let parsed = parse_yaml(&content)
    .context("Failed to parse YAML content")?;
```

## Dependencies

- No external dependencies
- May require updates to function signatures to return Result types
- Tests will need updates to handle new error paths

## Testing Strategy

1. **Unit Tests**
   - Test each error path explicitly
   - Verify error messages contain expected context
   - Ensure no panics occur in any test scenario

2. **Integration Tests**
   - Test error propagation across module boundaries
   - Verify application continues running after errors
   - Test recovery mechanisms work correctly

3. **Property-Based Tests**
   - Generate random invalid inputs
   - Verify no panics occur regardless of input

## Documentation Requirements

- Update function documentation to describe possible errors
- Add error handling guidelines to CONTRIBUTING.md
- Create examples of proper error handling patterns
- Document recovery strategies for different error types