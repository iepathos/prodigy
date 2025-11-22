---
number: 165
title: Stillwater Error Context Enhancement
category: foundation
priority: medium
status: draft
dependencies: [163, 164]
created: 2025-11-22
---

# Specification 165: Stillwater Error Context Enhancement

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 163 (Stillwater Validation Migration), Spec 164 (Stillwater Effect Composition)

## Context

Prodigy's error handling currently suffers from context loss as errors propagate through call stacks. Key problems:

1. **Manual context addition**: `src/error/mod.rs:248-283` requires pattern matching every error variant to add context
2. **Lost operation context**: Errors often lack information about what operation was being performed
3. **Poor debugging experience**: Stack traces don't preserve semantic operation chains
4. **Inconsistent context**: Some errors have rich context, others are bare
5. **Difficult error chain inspection**: No standard way to walk error causes

**Current approach** (src/error/mod.rs):
```rust
pub fn with_context(self, context: String) -> Self {
    match self {
        ProdigyError::WorkflowNotFound(id) => {
            ProdigyError::WorkflowNotFound(id).with_message(context)
        }
        ProdigyError::ValidationFailed(msg) => {
            ProdigyError::ValidationFailed(format!("{}: {}", context, msg))
        }
        // ... 30+ more variants
    }
}
```

This is:
- **Boilerplate-heavy**: Must update every time error enum changes
- **Error-prone**: Easy to forget variants or handle incorrectly
- **Limited flexibility**: Can only add one context string, not a chain

**Stillwater approach**:
```rust
fetch_user(id)
    .context("Loading user profile")
    .and_then(|user| process_data(user))
    .context("Processing user data")
    .run(&env)?;

// Error output:
// Error: UserNotFound(12345)
//   -> Processing user data
//   -> Loading user profile
```

Benefits:
- **Automatic context chain**: Each `.context()` adds to chain
- **No pattern matching**: Works with any error type
- **Preserved causality**: Full operation chain in errors
- **Inspectable**: Can programmatically walk error chain

## Objective

Enhance prodigy's error handling to use stillwater's context chaining, eliminating manual pattern matching, preserving full operation context, and improving debugging experience.

## Requirements

### Functional Requirements

- **FR1**: Replace manual `with_context()` pattern matching with automatic context chaining
- **FR2**: Preserve full operation context chain in all errors
- **FR3**: Support programmatic error chain inspection
- **FR4**: Maintain existing error types and variants (no breaking changes)
- **FR5**: Add context at every Effect boundary using `.context()`
- **FR6**: Enable both user-friendly and developer-friendly error display
- **FR7**: Support error serialization for logging and monitoring

### Non-Functional Requirements

- **NFR1**: Error context overhead < 100 bytes per context entry
- **NFR2**: Context chaining has zero runtime cost when no error occurs
- **NFR3**: Error messages remain clear and actionable for users
- **NFR4**: Debug output includes full context chain
- **NFR5**: Error types remain serializable (for API responses)
- **NFR6**: Backward compatible with existing error handling code

## Acceptance Criteria

- [ ] `ProdigyError` enum updated to store context chain
- [ ] Manual `with_context()` pattern matching removed
- [ ] All Effect boundaries have `.context()` calls
- [ ] Error display shows concise message for users
- [ ] Debug display shows full context chain for developers
- [ ] Error chain inspection API available: `error.chain()`, `error.root_cause()`
- [ ] CLI error output shows full operation context
- [ ] Logging includes structured error context
- [ ] API error responses include context (optional detail level)
- [ ] Error serialization supports JSON and Display formats
- [ ] Documentation explains context chaining patterns
- [ ] Migration guide for adding context to existing code
- [ ] All error construction sites reviewed for context addition
- [ ] Integration tests verify error context preservation

## Technical Details

### Implementation Approach

**Phase 1: Error Type Enhancement** (2 days)

Enhance `ProdigyError` to store context chain:

```rust
use std::sync::Arc;

/// Error context entry
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub message: String,
    pub location: Option<&'static str>,  // file:line from caller
}

/// Enhanced error with context chain
#[derive(Debug, Clone)]
pub struct ProdigyError {
    kind: ErrorKind,
    context: Vec<ErrorContext>,
    source: Option<Arc<ProdigyError>>,  // For error chains
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    WorkflowNotFound(WorkflowId),
    ValidationFailed(String),
    ConfigError(String),
    ExecutionFailed(String),
    StorageError(String),
    GitError(String),
    // ... existing variants
}

impl ProdigyError {
    /// Create error from kind
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            context: Vec::new(),
            source: None,
        }
    }

    /// Add context to error (fluent API)
    pub fn context(mut self, message: impl Into<String>) -> Self {
        self.context.push(ErrorContext {
            message: message.into(),
            location: None,  // TODO: Add with macro
        });
        self
    }

    /// Add context with source location
    #[track_caller]
    pub fn context_at(mut self, message: impl Into<String>) -> Self {
        let location = std::panic::Location::caller();
        self.context.push(ErrorContext {
            message: message.into(),
            location: Some(location.file()),
        });
        self
    }

    /// Chain with another error as source
    pub fn source(mut self, source: ProdigyError) -> Self {
        self.source = Some(Arc::new(source));
        self
    }

    /// Get context chain
    pub fn chain(&self) -> &[ErrorContext] {
        &self.context
    }

    /// Get root error kind
    pub fn root_cause(&self) -> &ErrorKind {
        let mut current = self;
        while let Some(ref source) = current.source {
            current = source;
        }
        &current.kind
    }

    /// Get error kind
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}
```

**Phase 2: Display Implementation** (1 day)

Implement user-friendly and developer-friendly display:

```rust
use std::fmt;

impl fmt::Display for ProdigyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // User-friendly: show kind and first context
        write!(f, "{}", self.kind)?;

        if let Some(first_context) = self.context.first() {
            write!(f, ": {}", first_context.message)?;
        }

        Ok(())
    }
}

impl fmt::Debug for ProdigyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Developer-friendly: show full context chain
        writeln!(f, "Error: {:?}", self.kind)?;

        if !self.context.is_empty() {
            writeln!(f, "Context chain:")?;
            for (i, ctx) in self.context.iter().enumerate() {
                write!(f, "  {}: {}", i, ctx.message)?;
                if let Some(loc) = ctx.location {
                    write!(f, " (at {})", loc)?;
                }
                writeln!(f)?;
            }
        }

        if let Some(ref source) = self.source {
            writeln!(f, "Caused by:")?;
            write!(f, "  {:?}", source)?;
        }

        Ok(())
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::WorkflowNotFound(id) => write!(f, "Workflow not found: {}", id),
            ErrorKind::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            ErrorKind::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ErrorKind::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            ErrorKind::StorageError(msg) => write!(f, "Storage error: {}", msg),
            ErrorKind::GitError(msg) => write!(f, "Git error: {}", msg),
        }
    }
}
```

**Phase 3: Effect Integration** (2 days)

Add context at all Effect boundaries:

```rust
// Before (src/config/loader.rs)
pub fn load_from_path(path: PathBuf) -> Effect<Config, ConfigError, AppEnv> {
    IO::query(move |env| env.fs.read_to_string(&path))
        .and_then(|content| parse_config(&content))
}

// After
pub fn load_from_path(path: PathBuf) -> Effect<Config, ProdigyError, AppEnv> {
    IO::query(move |env| env.fs.read_to_string(&path))
        .map_err(|e| ProdigyError::new(ErrorKind::ConfigError(e.to_string())))
        .context(format!("Reading config file: {}", path.display()))
        .and_then(|content| {
            parse_config(&content)
                .map_err(|e| ProdigyError::new(ErrorKind::ConfigError(e.to_string())))
                .context("Parsing YAML configuration")
        })
        .context(format!("Loading configuration from {}", path.display()))
}
```

**Phase 4: CLI Error Display** (1 day)

Update CLI to show rich error context:

```rust
// src/cli/error_display.rs
pub fn display_error(error: &ProdigyError) {
    eprintln!("{}", "Error:".red().bold());
    eprintln!("  {}", error.kind());

    if !error.chain().is_empty() {
        eprintln!();
        eprintln!("{}", "Context:".yellow());
        for ctx in error.chain() {
            eprintln!("  → {}", ctx.message);
        }
    }

    if let Some(source) = &error.source {
        eprintln!();
        eprintln!("{}", "Caused by:".yellow());
        eprintln!("  {}", source);
    }

    #[cfg(debug_assertions)]
    {
        eprintln!();
        eprintln!("{}", "Debug info:".cyan());
        eprintln!("{:?}", error);
    }
}
```

Example output:
```
Error:
  Workflow not found: workflow-123

Context:
  → Loading workflow state
  → Executing workflow command
  → Processing CLI request

Caused by:
  Storage error: Database connection failed
```

**Phase 5: Error Serialization** (1 day)

Support structured error serialization:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct SerializableError {
    pub kind: String,
    pub message: String,
    pub context: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Box<SerializableError>>,
}

impl From<&ProdigyError> for SerializableError {
    fn from(error: &ProdigyError) -> Self {
        Self {
            kind: format!("{:?}", error.kind()),
            message: error.kind().to_string(),
            context: error.chain().iter().map(|c| c.message.clone()).collect(),
            source: error.source.as_ref().map(|s| Box::new(s.as_ref().into())),
        }
    }
}

impl ProdigyError {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(SerializableError::from(self))
            .unwrap_or_else(|_| serde_json::json!({ "error": "Serialization failed" }))
    }
}
```

### Architecture Changes

**Error Module Structure**:
```
src/
└── error/
    ├── mod.rs              # ProdigyError, ErrorKind
    ├── context.rs          # ErrorContext, chain management
    ├── display.rs          # Display and Debug impls
    ├── serialization.rs    # JSON serialization
    └── cli_display.rs      # CLI-specific formatting
```

**Error Flow**:
```
Operation
   ↓ (error occurs)
Error Kind Created
   ↓
Context Added (.context())
   ↓
More Context Added (.context())
   ↓
Propagated Up
   ↓
Displayed to User (with full context)
   ↓
Logged (structured)
```

### Data Structures

**Context Storage**:
```rust
/// Error with context chain
pub struct ProdigyError {
    kind: ErrorKind,
    context: Vec<ErrorContext>,  // Stored in creation order
    source: Option<Arc<ProdigyError>>,
}

/// Single context entry
pub struct ErrorContext {
    pub message: String,
    pub location: Option<&'static str>,
    pub metadata: HashMap<String, String>,  // Optional structured data
}
```

**Context Builder**:
```rust
impl ProdigyError {
    /// Builder pattern for rich context
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Some(last) = self.context.last_mut() {
            last.metadata.insert(key.into(), value.into());
        }
        self
    }
}

// Usage
ProdigyError::new(ErrorKind::WorkflowNotFound(id))
    .context("Loading workflow")
    .with_metadata("workflow_id", id.to_string())
    .with_metadata("user_id", user.id.to_string())
```

### APIs and Interfaces

**Error Construction**:
```rust
// Create error
let error = ProdigyError::new(ErrorKind::WorkflowNotFound(id));

// Add context
let error = error.context("Loading workflow state");

// Chain errors
let error = error.source(original_error);

// With location tracking
let error = error.context_at("Validating input");
```

**Error Inspection**:
```rust
// Get context chain
for ctx in error.chain() {
    println!("Context: {}", ctx.message);
}

// Get root cause
let root = error.root_cause();

// Get error kind
match error.kind() {
    ErrorKind::WorkflowNotFound(id) => {
        // Handle specific error
    }
    _ => {}
}
```

**Effect Integration**:
```rust
use stillwater::Effect;

// Effect with context
fn process_workflow(id: WorkflowId) -> Effect<Workflow, ProdigyError, AppEnv> {
    fetch_workflow(id)
        .context(format!("Fetching workflow {}", id))
        .and_then(|wf| validate_workflow(&wf))
        .context("Validating workflow")
        .and_then(|wf| execute_workflow(wf))
        .context("Executing workflow")
}
```

## Dependencies

- **Prerequisites**:
  - Spec 163 (Stillwater Validation Migration)
  - Spec 164 (Stillwater Effect Composition)
- **Affected Components**:
  - `src/error/mod.rs` - complete redesign
  - `src/cli/error_display.rs` - new error formatting
  - All Effect-based operations - add `.context()`
  - All Result-returning functions - migrate to new error type
- **External Dependencies**: `stillwater = "0.1"` (already added)

## Testing Strategy

### Unit Tests

**Context Chain Tests**:
```rust
#[test]
fn test_error_context_chain() {
    let error = ProdigyError::new(ErrorKind::WorkflowNotFound(WorkflowId::new()))
        .context("Loading workflow")
        .context("Processing request");

    let chain = error.chain();
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].message, "Loading workflow");
    assert_eq!(chain[1].message, "Processing request");
}

#[test]
fn test_error_source_chain() {
    let source = ProdigyError::new(ErrorKind::StorageError("DB error".to_string()));
    let error = ProdigyError::new(ErrorKind::WorkflowNotFound(WorkflowId::new()))
        .source(source);

    assert_eq!(error.root_cause(), &ErrorKind::StorageError("DB error".to_string()));
}

#[test]
fn test_error_display() {
    let error = ProdigyError::new(ErrorKind::ValidationFailed("Invalid input".to_string()))
        .context("Validating workflow");

    let display = format!("{}", error);
    assert!(display.contains("Validation failed"));
    assert!(display.contains("Validating workflow"));
}
```

**Serialization Tests**:
```rust
#[test]
fn test_error_serialization() {
    let error = ProdigyError::new(ErrorKind::ConfigError("Bad YAML".to_string()))
        .context("Loading config")
        .context("Starting application");

    let json = error.to_json();
    assert_eq!(json["kind"], "ConfigError");
    assert_eq!(json["context"].as_array().unwrap().len(), 2);
}
```

### Integration Tests

- Test error context preservation through Effect pipelines
- Test CLI error display formatting
- Test error logging with structured context
- Test API error responses include context

### Performance Tests

**Benchmark Error Creation**:
```rust
#[bench]
fn bench_error_with_context(b: &mut Bencher) {
    b.iter(|| {
        ProdigyError::new(ErrorKind::ValidationFailed("test".to_string()))
            .context("step 1")
            .context("step 2")
            .context("step 3")
    });
}
```

Expected: < 1μs per error with 3 context entries.

### User Acceptance

- Error messages are clear and actionable
- Full operation context visible in errors
- Debug mode shows detailed error chains
- Error logs contain structured context for analysis

## Documentation Requirements

### Code Documentation

- Document `ProdigyError` API with examples
- Explain context chaining patterns
- Provide cookbook of error handling patterns
- Document error serialization formats

### User Documentation

Update error handling documentation:
- Explain error message format
- Show example error outputs
- Document debug mode error display

### Architecture Updates

Add to ARCHITECTURE.md:

```markdown
### Error Handling

Prodigy uses context-enriched errors with full operation chain preservation:

```rust
// Error with context chain
fetch_workflow(id)
    .context(format!("Fetching workflow {}", id))
    .and_then(|wf| validate_workflow(&wf))
    .context("Validating workflow")
```

**Error Structure**:
- **Kind**: Base error type (WorkflowNotFound, ValidationFailed, etc.)
- **Context Chain**: Operation context from innermost to outermost
- **Source**: Optional causal error chain

**Display Modes**:
- **User Mode**: Concise error with first context
- **Debug Mode**: Full context chain and source errors

See `src/error/` for error types and `src/cli/error_display.rs` for formatting.
```

## Implementation Notes

### Context Strategy

**Add context at every Effect boundary**:
```rust
// File operations
IO::query(|env| env.fs.read_to_string(&path))
    .context(format!("Reading file: {}", path.display()))

// Database operations
IO::query(|env| env.db.fetch_workflow(id))
    .context(format!("Fetching workflow {}", id))

// Git operations
IO::execute(|env| env.git.commit(msg))
    .context("Creating git commit")

// High-level operations
process_workflow(id)
    .context(format!("Processing workflow {}", id))
```

**Context levels**:
1. **Low-level**: Specific I/O operation
2. **Mid-level**: Business operation
3. **High-level**: User action

### Error Conversion

Convert existing errors gradually:

```rust
// Old code
return Err(ConfigError::InvalidFormat(msg));

// New code
return Err(ProdigyError::new(ErrorKind::ConfigError(msg))
    .context("Parsing configuration"));
```

Provide `From` implementations for smooth migration:
```rust
impl From<ConfigError> for ProdigyError {
    fn from(e: ConfigError) -> Self {
        ProdigyError::new(ErrorKind::ConfigError(e.to_string()))
    }
}
```

### Common Patterns

**Effect with context**:
```rust
fn operation() -> Effect<T, ProdigyError, AppEnv> {
    // operation
        .context("High-level operation")
}
```

**Error chain**:
```rust
fn wrapper() -> Result<T, ProdigyError> {
    operation()
        .run(&env)
        .map_err(|e| e.context("Wrapper operation"))
}
```

**Conditional context**:
```rust
fn conditional(debug: bool) -> Effect<T, ProdigyError, AppEnv> {
    operation()
        .map_err(move |e| {
            if debug {
                e.with_metadata("debug", "true")
            } else {
                e
            }
        })
}
```

### Gotchas

- Context chain stored in creation order (innermost first)
- Display shows most recent context first (reverse order)
- Source chain can grow deep - consider depth limits
- Error cloning clones entire context chain (use Arc for source)
- Location tracking requires `#[track_caller]` on functions

## Migration and Compatibility

### Breaking Changes

Minimal - error type signature changes:
```rust
// Old
fn operation() -> Result<T, ConfigError>

// New
fn operation() -> Result<T, ProdigyError>
```

### Compatibility Layer

Provide error conversion for gradual migration:

```rust
// Convert old errors to new
impl From<OldError> for ProdigyError {
    fn from(e: OldError) -> Self {
        ProdigyError::new(ErrorKind::from(e))
    }
}

// Preserve old error types temporarily
#[deprecated(note = "Use ProdigyError instead")]
pub type ConfigError = ProdigyError;
```

### Migration Path

1. **Phase 1**: Add new error type alongside old
2. **Phase 2**: Migrate core modules to new error type
3. **Phase 3**: Convert remaining modules
4. **Phase 4**: Deprecate old error types
5. **Phase 5**: Remove deprecated types (one release later)

### Future Work

- Error metrics and tracking (count by kind, context analysis)
- Error recovery strategies (automatic retry with context)
- Error reporting service integration (send errors to monitoring)
- Machine-readable error codes (for programmatic handling)
- Error localization (i18n for error messages)
