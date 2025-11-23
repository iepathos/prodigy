# Error Context Migration Guide

## Overview

This guide explains how to migrate existing error handling code to use the new error context system introduced in Spec 165. The context system provides rich error diagnostics while maintaining user-friendly error messages.

## Why Add Context?

Context chaining provides:

- **Better debugging**: Full error chain shows exactly where and why failures occurred
- **User-friendly messages**: End users see clean, actionable error messages
- **Developer diagnostics**: Developers get detailed context for troubleshooting
- **Audit trail**: Complete history of operations leading to failure

## Key Concepts

### Effect Boundaries

An "effect boundary" is where your code transitions between different layers or performs operations that can fail:

1. **I/O Operations**: File reads/writes, network calls, database operations
2. **External Calls**: Subprocess execution, library functions
3. **Layer Transitions**: Moving between architectural layers (UI → Service → Storage)
4. **Error Propagation**: Calling functions that return Results

### The `.context()` Method

The `.context()` method adds a layer of context to an error:

```rust
use prodigy::error::ProdigyError;

// Basic usage
operation()
    .map_err(|e| e.context("Failed to perform operation"))?;

// With dynamic context
operation()
    .map_err(|e| e.context(format!("Failed to process item {}", id)))?;
```

## Migration Patterns

### Pattern 1: Simple Error Propagation

**Before:**
```rust
fn read_config(path: &Path) -> Result<Config, ProdigyError> {
    let content = std::fs::read_to_string(path)?;
    let config = serde_json::from_str(&content)?;
    Ok(config)
}
```

**After:**
```rust
fn read_config(path: &Path) -> Result<Config, ProdigyError> {
    // Effect boundary: file I/O
    let content = std::fs::read_to_string(path)
        .map_err(ProdigyError::from)
        .map_err(|e| e.context(format!("Failed to read config from {}", path.display())))?;

    // Effect boundary: parsing
    let config = serde_json::from_str(&content)
        .map_err(ProdigyError::from)
        .map_err(|e| e.context("Failed to parse configuration JSON"))?;

    Ok(config)
}
```

**Result:**
```
Error: Failed to parse configuration JSON
  Context:
    - Failed to read config from /path/to/config.json
  Source: expected value at line 1 column 1
```

### Pattern 2: Nested Function Calls

**Before:**
```rust
fn deploy_workflow(name: &str) -> Result<(), ProdigyError> {
    let workflow = load_workflow(name)?;
    validate_workflow(&workflow)?;
    execute_workflow(&workflow)?;
    Ok(())
}
```

**After:**
```rust
fn deploy_workflow(name: &str) -> Result<(), ProdigyError> {
    // Effect boundary: loading
    let workflow = load_workflow(name)
        .map_err(|e| e.context(format!("Failed to load workflow '{}'", name)))?;

    // Effect boundary: validation
    validate_workflow(&workflow)
        .map_err(|e| e.context(format!("Validation failed for workflow '{}'", name)))?;

    // Effect boundary: execution
    execute_workflow(&workflow)
        .map_err(|e| e.context(format!("Execution failed for workflow '{}'", name)))?;

    Ok(())
}
```

**Result:**
```
Error: Execution failed for workflow 'my-workflow'
  Context:
    - Validation failed for workflow 'my-workflow'
    - Failed to load workflow 'my-workflow'
  Source: Workflow file not found
```

### Pattern 3: Batch Operations

**Before:**
```rust
fn process_items(items: Vec<WorkItem>) -> Result<Vec<String>, ProdigyError> {
    items.iter()
        .map(|item| process_item(item))
        .collect()
}
```

**After:**
```rust
fn process_items(items: Vec<WorkItem>) -> Result<Vec<String>, ProdigyError> {
    items.iter()
        .map(|item| {
            // Effect boundary: per-item processing
            process_item(item)
                .map_err(|e| e.with_context(format!("Failed to process item {}", item.id)))
        })
        .collect::<Result<Vec<_>, _>>()
        // Effect boundary: batch-level
        .map_err(|e| e.context(format!("Failed to process batch of {} items", items.len())))
}
```

**Result:**
```
Error: Failed to process batch of 10 items
  Context:
    - Failed to process item item-5
  Source: Invalid item data format
```

### Pattern 4: Storage Operations

**Before:**
```rust
fn save_checkpoint(checkpoint: &Checkpoint) -> Result<(), ProdigyError> {
    let json = serde_json::to_string(checkpoint)?;
    std::fs::write(&checkpoint.path, json)?;
    Ok(())
}
```

**After:**
```rust
fn save_checkpoint(checkpoint: &Checkpoint) -> Result<(), ProdigyError> {
    // Effect boundary: serialization
    let json = serde_json::to_string(checkpoint)
        .map_err(ProdigyError::from)
        .map_err(|e| e.context("Failed to serialize checkpoint to JSON"))?;

    // Effect boundary: file write
    std::fs::write(&checkpoint.path, json)
        .map_err(ProdigyError::from)
        .map_err(|e| e.context(format!(
            "Failed to write checkpoint to {}",
            checkpoint.path.display()
        )))?;

    Ok(())
}
```

### Pattern 5: Command Execution

**Before:**
```rust
fn run_git_command(args: &[&str]) -> Result<String, ProdigyError> {
    let output = Command::new("git")
        .args(args)
        .output()?;

    if !output.status.success() {
        return Err(ProdigyError::execution("Git command failed"));
    }

    Ok(String::from_utf8(output.stdout)?)
}
```

**After:**
```rust
fn run_git_command(args: &[&str]) -> Result<String, ProdigyError> {
    // Effect boundary: process execution
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(ProdigyError::from)
        .map_err(|e| e.context(format!("Failed to execute: git {}", args.join(" "))))?;

    if !output.status.success() {
        return Err(ProdigyError::execution("Git command failed")
            .with_exit_code(output.status.code().unwrap_or(-1))
            .context(format!("Command exited with error: git {}", args.join(" "))));
    }

    // Effect boundary: UTF-8 conversion
    Ok(String::from_utf8(output.stdout)
        .map_err(ProdigyError::from)
        .map_err(|e| e.context("Failed to parse command output as UTF-8"))?)
}
```

## Best Practices

### 1. Context Messages Should Explain "What" Not "How"

**Good:**
```rust
.map_err(|e| e.context("Failed to load workflow configuration"))
```

**Bad:**
```rust
.map_err(|e| e.context("std::fs::read_to_string returned error"))
```

### 2. Include Relevant Identifiers in Context

**Good:**
```rust
.map_err(|e| e.context(format!("Failed to process item {}", item.id)))
```

**Bad:**
```rust
.map_err(|e| e.context("Failed to process item"))
```

### 3. Don't Repeat Information from Source Error

**Good:**
```rust
.map_err(|e| e.context("Failed to parse workflow"))
```

**Bad:**
```rust
.map_err(|e| e.context("Failed to parse workflow: invalid JSON syntax"))
// The "invalid JSON syntax" is already in the source error
```

### 4. Use `.with_context()` for Lazy Evaluation

For expensive string formatting, use `.with_context()` with a closure:

```rust
.map_err(|e| e.with_context(|| {
    format!("Failed to process {:#?}", expensive_to_debug)
}))
```

This only formats the string if there's actually an error.

### 5. Layer Context from Specific to General

```rust
// Specific context at the operation level
read_file()
    .map_err(|e| e.context("Failed to read config.json"))?;

// General context at the feature level
load_config()
    .map_err(|e| e.context("Failed to initialize application configuration"))?;
```

## Common Mistakes to Avoid

### ❌ Mistake 1: Adding Context Without Mapping Error Type

```rust
// This won't compile - std::io::Error doesn't have .context()
std::fs::read_to_string(path)
    .context("Failed to read file")?;
```

**✅ Fix:**
```rust
std::fs::read_to_string(path)
    .map_err(ProdigyError::from)
    .map_err(|e| e.context("Failed to read file"))?;
```

### ❌ Mistake 2: Swallowing the Source Error

```rust
// Loses the original error information
std::fs::read_to_string(path)
    .map_err(|_| ProdigyError::storage("Failed to read file"))?;
```

**✅ Fix:**
```rust
std::fs::read_to_string(path)
    .map_err(ProdigyError::from)
    .map_err(|e| e.context("Failed to read file"))?;
```

### ❌ Mistake 3: Adding Too Much Context

```rust
// Context every single line
let x = foo().context("Getting foo")?;
let y = bar(x).context("Calling bar with foo")?;
let z = baz(y).context("Calling baz with bar result")?;
```

**✅ Fix:**
```rust
// Context only at effect boundaries
let x = foo()?;  // Pure function, no context needed
let y = bar(x)?;  // Pure function, no context needed
let z = baz(y).context("Failed to persist result")?;  // I/O boundary
```

## Gradual Migration Strategy

You don't need to migrate everything at once. Follow this priority:

### Phase 1: High-Impact Areas (Start Here)
1. User-facing commands (CLI handlers)
2. Storage operations (file I/O, database)
3. External integrations (subprocess, network)

### Phase 2: Core Business Logic
1. Workflow execution
2. MapReduce coordination
3. Session management

### Phase 3: Utilities and Helpers
1. String parsing
2. Configuration loading
3. Internal utilities

## Testing Context Preservation

Verify your migration with tests:

```rust
#[test]
fn test_error_context_preserved() {
    let result = my_operation();
    assert!(result.is_err());

    let error = result.unwrap_err();
    let dev_message = error.developer_message();

    // Verify context includes expected information
    assert!(dev_message.contains("Failed to load workflow"));
    assert!(dev_message.contains("Failed to read file"));
}
```

## Checking Your Work

After migration, errors should:

1. **Show clear user messages**: End users shouldn't see stack traces or technical jargon
2. **Include full context in verbose mode**: Developers should see the complete error chain
3. **Serialize properly**: Errors should convert to JSON for APIs and logging
4. **Preserve information**: No error information should be lost in conversion

## Getting Help

- Review the module documentation in `src/error/mod.rs`
- Check examples in `src/error/migration_example.rs`
- Look at integration tests in `tests/error_context_preservation_test.rs`
- Run `cargo doc --open` to view the full API documentation

## Summary

The key to successful migration:

1. Identify effect boundaries in your code
2. Add `.context()` calls at those boundaries
3. Use descriptive, user-friendly context messages
4. Include relevant identifiers (IDs, paths, names)
5. Test that context is preserved through the error chain

With these patterns, your error handling will provide excellent diagnostics for developers while remaining user-friendly.
