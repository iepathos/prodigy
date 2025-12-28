# Error Context Migration Guide

## Overview

This guide explains how to migrate existing error handling code to use the new error context system introduced in Spec 165. The context system provides rich error diagnostics while maintaining user-friendly error messages.

!!! info "Two Error Context Patterns"
    Prodigy has **two distinct error context patterns** for different use cases:

    1. **`ProdigyError.context()`** - General error handling throughout the codebase
    2. **Stillwater's `ContextError<E>`** - Used in the `cook` module for effect-based operations

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
// Source: src/error/mod.rs:538-557
use prodigy::error::ProdigyError;

// Basic usage
operation()
    .map_err(|e| e.context("Failed to perform operation"))?;

// With dynamic context
operation()
    .map_err(|e| e.context(format!("Failed to process item {}", id)))?;
```

!!! tip "Preferred Method"
    Use `.context()` for new code. It adds context to a chain that can be inspected programmatically. The legacy `.with_context()` method modifies the message string directly and is kept for compatibility.

### The `.context_at()` Method

For enhanced debugging, use `.context_at()` which automatically captures the caller's source file location:

```rust
// Source: src/error/mod.rs:559-580
operation()
    .map_err(|e| e.context_at("Failed to perform operation"))?;

// The error will include the file location where context_at was called,
// making it easier to trace the error origin in logs.
```

### Stillwater's ContextError Pattern (Cook Module)

The `cook` module uses Stillwater's `ContextError<E>` for effect-based operations:

```rust
// Source: src/cook/error/ext.rs:20-81
use prodigy::cook::error::{ResultExt, ContextResult};

fn process_file(path: &str) -> ContextResult<String, std::io::Error> {
    let content = std::fs::read_to_string(path)
        .context("Reading input file")?;
    Ok(content)
}

// With dynamic context (lazy evaluation)
fn process_item(id: &str) -> ContextResult<Data, ProcessError> {
    load_item(id)
        .with_context(|| format!("Processing item {}", id))?;
    Ok(data)
}
```

!!! note "When to Use Each Pattern"
    | Pattern | Import | Use Case |
    |---------|--------|----------|
    | `ProdigyError.context()` | `use prodigy::error::ProdigyError` | General error handling |
    | `ResultExt.context()` | `use prodigy::cook::error::ResultExt` | Effect-based cook operations |

### ErrorExt Trait

The `ErrorExt` trait provides convenient methods for converting errors to specific `ProdigyError` types:

```rust
// Source: src/error/helpers.rs:5-39
use prodigy::error::ErrorExt;

// Convert any error to a ProdigyError with context
file_operation()
    .to_prodigy("Failed to process file")?;

// Convert to specific error types
config_parse()
    .to_config_error("Invalid configuration format")?;

database_query()
    .to_storage_error("Failed to query database")?;

subprocess_run()
    .to_execution_error("Command failed")?;

session_load()
    .to_session_error("Failed to restore session")?;
```

### The `prodigy_error!` Macro

For quick error creation, use the `prodigy_error!` macro:

```rust
// Source: src/error/helpers.rs:121-152

// Create errors without a source
let err = prodigy_error!(config: "Invalid configuration");
let err = prodigy_error!(storage: "File not found");
let err = prodigy_error!(execution: "Command failed");
let err = prodigy_error!(session: "Session expired");
let err = prodigy_error!(workflow: "Workflow validation failed");

// Create errors with a source error
let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file.txt");
let err = prodigy_error!(storage: "Failed to read config", io_err);
```

## Migration Patterns

### Pattern 1: Simple Error Propagation

=== "Before"
    ```rust
    fn read_config(path: &Path) -> Result<Config, ProdigyError> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }
    ```

=== "After"
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

=== "Before"
    ```rust
    fn deploy_workflow(name: &str) -> Result<(), ProdigyError> {
        let workflow = load_workflow(name)?;
        validate_workflow(&workflow)?;
        execute_workflow(&workflow)?;
        Ok(())
    }
    ```

=== "After"
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

=== "Before"
    ```rust
    fn process_items(items: Vec<WorkItem>) -> Result<Vec<String>, ProdigyError> {
        items.iter()
            .map(|item| process_item(item))
            .collect()
    }
    ```

=== "After"
    ```rust
    fn process_items(items: Vec<WorkItem>) -> Result<Vec<String>, ProdigyError> {
        items.iter()
            .map(|item| {
                // Effect boundary: per-item processing
                process_item(item)
                    .map_err(|e| e.context(format!("Failed to process item {}", item.id)))
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

=== "Before"
    ```rust
    fn save_checkpoint(checkpoint: &Checkpoint) -> Result<(), ProdigyError> {
        let json = serde_json::to_string(checkpoint)?;
        std::fs::write(&checkpoint.path, json)?;
        Ok(())
    }
    ```

=== "After"
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

=== "Before"
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

=== "After"
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

=== "Good"
    ```rust
    .map_err(|e| e.context("Failed to load workflow configuration"))
    ```

=== "Bad"
    ```rust
    .map_err(|e| e.context("std::fs::read_to_string returned error"))
    ```

### 2. Include Relevant Identifiers in Context

=== "Good"
    ```rust
    .map_err(|e| e.context(format!("Failed to process item {}", item.id)))
    ```

=== "Bad"
    ```rust
    .map_err(|e| e.context("Failed to process item"))
    ```

### 3. Don't Repeat Information from Source Error

=== "Good"
    ```rust
    .map_err(|e| e.context("Failed to parse workflow"))
    ```

=== "Bad"
    ```rust
    .map_err(|e| e.context("Failed to parse workflow: invalid JSON syntax"))
    // The "invalid JSON syntax" is already in the source error
    ```

### 4. Use Closures for Lazy Evaluation

For expensive string formatting, use a closure:

```rust
// In cook module (Stillwater pattern)
.with_context(|| format!("Failed to process {:#?}", expensive_to_debug))

// For ProdigyError, the format! is always evaluated, so keep it simple
.map_err(|e| e.context(format!("Failed for item {}", id)))
```

!!! tip "Performance Consideration"
    The Stillwater `with_context` closure is only called on error. For ProdigyError, if the format string is expensive, consider extracting identifiers beforehand.

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

!!! warning "Mistake 1: Adding Context Without Mapping Error Type"
    ```rust
    // This won't compile - std::io::Error doesn't have .context()
    std::fs::read_to_string(path)
        .context("Failed to read file")?;
    ```

    **Fix:**
    ```rust
    std::fs::read_to_string(path)
        .map_err(ProdigyError::from)
        .map_err(|e| e.context("Failed to read file"))?;
    ```

!!! warning "Mistake 2: Swallowing the Source Error"
    ```rust
    // Loses the original error information
    std::fs::read_to_string(path)
        .map_err(|_| ProdigyError::storage("Failed to read file"))?;
    ```

    **Fix:**
    ```rust
    std::fs::read_to_string(path)
        .map_err(ProdigyError::from)
        .map_err(|e| e.context("Failed to read file"))?;
    ```

!!! warning "Mistake 3: Adding Too Much Context"
    ```rust
    // Context every single line
    let x = foo().context("Getting foo")?;
    let y = bar(x).context("Calling bar with foo")?;
    let z = baz(y).context("Calling baz with bar result")?;
    ```

    **Fix:**
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
// Source: tests/error_context_preservation_test.rs
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
- For cook module patterns, see `src/cook/error/ext.rs`
- Run `cargo doc --open` to view the full API documentation

## Summary

The key to successful migration:

1. Identify effect boundaries in your code
2. Add `.context()` calls at those boundaries
3. Use descriptive, user-friendly context messages
4. Include relevant identifiers (IDs, paths, names)
5. Test that context is preserved through the error chain

!!! example "Quick Reference"
    | Need | Solution |
    |------|----------|
    | Add context to ProdigyError | `.map_err(\|e\| e.context("message"))` |
    | Add context with location tracking | `.map_err(\|e\| e.context_at("message"))` |
    | Convert error to ProdigyError | `.map_err(ProdigyError::from)` |
    | Convert with specific type | `.to_storage_error("message")` |
    | Quick error creation | `prodigy_error!(config: "message")` |
    | Cook module context | `.context("message")` (via ResultExt) |

With these patterns, your error handling will provide excellent diagnostics for developers while remaining user-friendly.
