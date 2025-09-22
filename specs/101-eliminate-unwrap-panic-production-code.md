---
number: 101
title: Eliminate unwrap() and panic!() from Production Code
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-09-22
---

# Specification 101: Eliminate unwrap() and panic!() from Production Code

## Context

The codebase currently contains 2,583 unwrap() calls and 109 panic!() calls in production code, which directly violates VISION.md principles of reliability ("Zero panics in production code") and graceful degradation. These calls can cause the application to crash unexpectedly, leading to data loss and poor user experience.

Critical locations identified:
- `storage/lock.rs:41,47` - Time conversion unwraps could fail
- `main.rs:690,783,854` - Current directory unwraps
- `worktree/manager.rs:2255` - Session lookup with unwrap_or_else + panic

## Objective

Replace all unwrap() and panic!() calls in production code with proper error handling using Result<T, E> types, ensuring the application never crashes unexpectedly and always provides meaningful error messages to users.

## Requirements

### Functional Requirements
- All unwrap() calls in production code must be replaced with proper error handling
- All panic!() calls in production code must be replaced with Result returns
- Error messages must be user-friendly and actionable
- Critical paths (storage, main, worktree) must be prioritized first
- Test code may retain unwrap()/panic!() for assertion failures

### Non-Functional Requirements
- No performance degradation from error handling overhead
- Maintain backward compatibility of public APIs
- All error types must implement std::error::Error
- Error context must be preserved through the call stack

## Acceptance Criteria

- [ ] Zero unwrap() calls in src/main.rs
- [ ] Zero unwrap() calls in src/storage/ modules
- [ ] Zero unwrap() calls in src/worktree/ modules
- [ ] Zero panic!() calls in production code paths
- [ ] All errors provide clear, actionable messages
- [ ] Comprehensive error handling tests for critical paths
- [ ] Documentation updated to reflect error handling patterns

## Technical Details

### Implementation Approach

1. **Phase 1: Critical Path Cleanup**
   - Replace unwrap() in main.rs with proper error propagation
   - Fix storage module unwraps with StorageError types
   - Address worktree manager panics with WorktreeError returns

2. **Phase 2: Systematic Replacement**
   - Use `?` operator for error propagation where appropriate
   - Convert remaining unwrap() calls to match expressions with error handling
   - Replace panic!() with custom error types

3. **Phase 3: Error Context Enhancement**
   - Add contextual error information using anyhow::Context
   - Implement From<T> for error types to enable `?` operator
   - Ensure error messages guide users toward resolution

### Error Handling Patterns

```rust
// Before (problematic)
let current_dir = std::env::current_dir().unwrap();
let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

// After (proper error handling)
let current_dir = std::env::current_dir()
    .context("Failed to get current directory")?;
let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .context("System clock error")?;
```

## Dependencies

No dependencies - this is foundational work that other improvements depend on.

## Testing Strategy

- Unit tests for all new error paths
- Integration tests for error propagation through critical workflows
- Property-based tests using proptest for error scenarios
- Stress testing to ensure no panics under load

## Documentation Requirements

- Update error handling guidelines in CLAUDE.md
- Document common error patterns and resolution steps
- Add troubleshooting guide for user-facing errors
- Update API documentation with error return types