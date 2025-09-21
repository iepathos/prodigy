---
number: 103
title: Fix Critical Unwrap Calls
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-21
---

# Specification 103: Fix Critical Unwrap Calls

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The codebase contains 140+ instances of `.unwrap()` and `panic!()` calls that can cause the application to crash unexpectedly. These represent poor error handling practices and create reliability issues. Critical paths in session management, storage, and subprocess execution are particularly affected, making the tool fragile in production use.

## Objective

Replace all `.unwrap()` calls in critical code paths with proper error handling, ensuring graceful degradation and helpful error messages instead of panics.

## Requirements

### Functional Requirements
- Identify and categorize all unwrap() calls by criticality
- Replace unwrap() in critical paths with proper Result handling
- Add context to errors for better debugging
- Ensure no functionality is lost during refactoring
- Provide meaningful error messages to users

### Non-Functional Requirements
- Zero panics in normal operation scenarios
- Improved debuggability with error context
- Consistent error handling patterns
- No performance degradation from error handling

## Acceptance Criteria

- [ ] No unwrap() calls in main execution paths
- [ ] All public API functions return Result types
- [ ] Error messages include context about what failed and why
- [ ] Integration tests pass without panics
- [ ] Stress tests don't trigger panics
- [ ] Error handling guidelines documented
- [ ] Static analysis shows < 10 unwrap() calls (only in tests)

## Technical Details

### Implementation Approach

1. **Phase 1: Audit and Categorize**
   ```rust
   // Categorize by risk level:
   // CRITICAL: Main execution paths, storage, subprocess
   // HIGH: Session management, worktree operations
   // MEDIUM: Configuration parsing, validation
   // LOW: Test code, examples
   ```

2. **Phase 2: Fix Critical Paths**
   Priority modules:
   - `src/session/` - 60+ unwraps
   - `src/subprocess/` - Core execution
   - `src/storage/` - Data persistence
   - `src/cook/execution/` - Workflow execution

3. **Phase 3: Add Error Context**
   ```rust
   // Before:
   let file = File::open(path).unwrap();

   // After:
   let file = File::open(path)
       .with_context(|| format!("Failed to open session file: {}", path.display()))?;
   ```

4. **Phase 4: Establish Patterns**
   - Use `anyhow::Result` for application errors
   - Use `thiserror` for library errors
   - Add `.context()` for debugging information
   - Use `?` operator for propagation

### Architecture Changes

No architectural changes, but establish error handling patterns:

```rust
// For recoverable errors:
match operation() {
    Ok(result) => process(result),
    Err(e) => {
        log::warn!("Operation failed, using fallback: {}", e);
        use_fallback()
    }
}

// For critical errors:
operation()
    .context("Critical operation failed")?;

// For optional operations:
if let Err(e) = optional_operation() {
    log::debug!("Optional operation skipped: {}", e);
}
```

### Data Structures

No changes to data structures, but ensure all public methods return `Result<T>`.

### APIs and Interfaces

Update function signatures:
```rust
// Before:
pub fn load_config(path: &Path) -> Config {
    let content = fs::read_to_string(path).unwrap();
    serde_yaml::from_str(&content).unwrap()
}

// After:
pub fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
    Ok(config)
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: All modules with unwrap() calls
- **External Dependencies**: Already uses `anyhow`, may add `thiserror`

## Testing Strategy

- **Unit Tests**: Verify error cases are handled gracefully
- **Integration Tests**: Test error propagation through call stack
- **Chaos Tests**: Inject failures to verify no panics
- **Error Path Coverage**: Ensure all error branches are tested

## Documentation Requirements

- **Code Documentation**: Document error conditions for each function
- **Error Handling Guide**: Create guide for consistent error handling
- **User Documentation**: Document common error messages and solutions

## Implementation Notes

Priority order for fixing unwraps:
1. Session and storage operations (data loss risk)
2. Subprocess execution (operation failure)
3. Worktree management (git corruption risk)
4. Configuration parsing (startup failure)
5. Analytics and optional features (low impact)

Keep unwrap() only in:
- Test code where panic is acceptable
- Impossible error cases with clear comments
- Example code for clarity

## Migration and Compatibility

No breaking changes for users, but library consumers must handle Result types:

```rust
// Library users before:
let state = prodigy::load_state(path);

// Library users after:
let state = prodigy::load_state(path)?;
// or
let state = prodigy::load_state(path)
    .unwrap_or_else(|e| {
        eprintln!("Warning: Could not load state: {}", e);
        Default::default()
    });
```