---
number: 106
title: Unified Error Handling System
category: foundation
priority: high
status: draft
dependencies: [103]
created: 2025-01-21
---

# Specification 106: Unified Error Handling System

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [103 - Fix Critical Unwrap Calls]

## Context

The Prodigy codebase lacks a unified error handling strategy. Different modules use different error types, some use strings, some use anyhow, and others define custom errors. This inconsistency makes it difficult to handle errors properly, provide good user experience, and debug issues. The lack of structured errors also makes it impossible to handle specific error cases programmatically.

## Objective

Establish a unified error handling system with structured error types, consistent patterns, and clear guidelines that improve debuggability, user experience, and code maintainability.

## Requirements

### Functional Requirements
- Define unified error type hierarchy for the entire application
- Support structured errors with error codes and context
- Provide consistent error formatting and display
- Enable programmatic error handling based on error types
- Support error context chaining for debugging
- Implement error recovery strategies where appropriate

### Non-Functional Requirements
- Zero overhead for the happy path
- Rich debugging information in error messages
- Machine-readable error codes for automation
- Consistent error messages across the application
- Support for internationalization (future)

## Acceptance Criteria

- [ ] Single error type hierarchy used throughout codebase
- [ ] All public APIs return structured Result types
- [ ] Error messages include actionable information for users
- [ ] Error codes documented and consistent
- [ ] Error handling guidelines documented and enforced
- [ ] Automated error code generation from enum variants
- [ ] Test coverage for all error paths

## Technical Details

### Implementation Approach

1. **Phase 1: Define Error Hierarchy**
   ```rust
   use thiserror::Error;

   #[derive(Error, Debug)]
   pub enum ProdigyError {
       #[error("[E{code:04}] Configuration error: {message}")]
       Config {
           code: u16,
           message: String,
           #[source]
           source: Option<Box<dyn std::error::Error + Send + Sync>>,
       },

       #[error("[E{code:04}] Session error: {message}")]
       Session {
           code: u16,
           message: String,
           session_id: Option<String>,
           #[source]
           source: Option<Box<dyn std::error::Error + Send + Sync>>,
       },

       #[error("[E{code:04}] Storage error: {message}")]
       Storage {
           code: u16,
           message: String,
           path: Option<PathBuf>,
           #[source]
           source: Option<Box<dyn std::error::Error + Send + Sync>>,
       },

       #[error("[E{code:04}] Execution error: {message}")]
       Execution {
           code: u16,
           message: String,
           command: Option<String>,
           #[source]
           source: Option<Box<dyn std::error::Error + Send + Sync>>,
       },

       #[error("[E{code:04}] Git operation failed: {message}")]
       Git {
           code: u16,
           message: String,
           operation: String,
           #[source]
           source: Option<Box<dyn std::error::Error + Send + Sync>>,
       },

       #[error("[E{code:04}] {message}")]
       Other {
           code: u16,
           message: String,
           #[source]
           source: Option<Box<dyn std::error::Error + Send + Sync>>,
       },
   }
   ```

2. **Phase 2: Create Error Builders**
   ```rust
   impl ProdigyError {
       pub fn config(message: impl Into<String>) -> Self {
           Self::Config {
               code: 1001,
               message: message.into(),
               source: None,
           }
       }

       pub fn with_source(mut self, source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
           match &mut self {
               Self::Config { source: src, .. } |
               Self::Session { source: src, .. } |
               Self::Storage { source: src, .. } => {
                   *src = Some(source.into());
               }
               _ => {}
           }
           self
       }

       pub fn with_context(self, context: impl Display) -> Self {
           // Add context to message
           self
       }
   }
   ```

3. **Phase 3: Error Code Registry**
   ```rust
   pub mod error_codes {
       // Configuration errors (1000-1999)
       pub const CONFIG_NOT_FOUND: u16 = 1001;
       pub const CONFIG_INVALID_YAML: u16 = 1002;
       pub const CONFIG_MISSING_REQUIRED: u16 = 1003;

       // Session errors (2000-2999)
       pub const SESSION_NOT_FOUND: u16 = 2001;
       pub const SESSION_ALREADY_EXISTS: u16 = 2002;
       pub const SESSION_CORRUPTED: u16 = 2003;
       pub const SESSION_LOCKED: u16 = 2004;

       // Storage errors (3000-3999)
       pub const STORAGE_IO_ERROR: u16 = 3001;
       pub const STORAGE_PERMISSION_DENIED: u16 = 3002;
       pub const STORAGE_DISK_FULL: u16 = 3003;

       // Execution errors (4000-4999)
       pub const EXEC_COMMAND_NOT_FOUND: u16 = 4001;
       pub const EXEC_TIMEOUT: u16 = 4002;
       pub const EXEC_SUBPROCESS_FAILED: u16 = 4003;

       // Git errors (5000-5999)
       pub const GIT_REPO_NOT_FOUND: u16 = 5001;
       pub const GIT_DIRTY_WORKTREE: u16 = 5002;
       pub const GIT_MERGE_CONFLICT: u16 = 5003;
   }
   ```

4. **Phase 4: Result Type Alias**
   ```rust
   pub type Result<T> = std::result::Result<T, ProdigyError>;

   // For library boundaries
   pub type LibResult<T> = std::result::Result<T, ProdigyError>;

   // For application boundaries (with anyhow for flexibility)
   pub type AppResult<T> = anyhow::Result<T>;
   ```

5. **Phase 5: Error Display and Formatting**
   ```rust
   impl ProdigyError {
       pub fn exit_code(&self) -> i32 {
           match self {
               Self::Config { .. } => 2,
               Self::Session { .. } => 3,
               Self::Storage { .. } => 4,
               Self::Execution { .. } => 5,
               Self::Git { .. } => 6,
               Self::Other { .. } => 1,
           }
       }

       pub fn user_message(&self) -> String {
           // User-friendly message without technical details
           match self {
               Self::Config { message, .. } => format!("Configuration problem: {}", message),
               Self::Session { message, session_id, .. } => {
                   if let Some(id) = session_id {
                       format!("Session {} error: {}", id, message)
                   } else {
                       format!("Session error: {}", message)
                   }
               }
               _ => self.to_string(),
           }
       }

       pub fn developer_message(&self) -> String {
           // Full error chain with sources
           format!("{:#}", self)
       }
   }
   ```

### Error Handling Patterns

```rust
// For recoverable errors
fn try_operation() -> Result<Data> {
    match risky_operation() {
        Ok(data) => Ok(data),
        Err(e) => {
            log::warn!("Operation failed, attempting recovery: {}", e);
            recovery_operation()
                .map_err(|e| ProdigyError::execution("Recovery failed").with_source(e))
        }
    }
}

// For adding context
fn load_config(path: &Path) -> Result<Config> {
    std::fs::read_to_string(path)
        .map_err(|e| {
            ProdigyError::config(format!("Cannot read config file: {}", path.display()))
                .with_source(e)
        })
        .and_then(|content| {
            serde_yaml::from_str(&content)
                .map_err(|e| {
                    ProdigyError::config("Invalid YAML syntax")
                        .with_source(e)
                })
        })
}

// For CLI error handling
fn main() -> AppResult<()> {
    if let Err(e) = run_application() {
        match e.downcast_ref::<ProdigyError>() {
            Some(prodigy_err) => {
                eprintln!("{}", prodigy_err.user_message());
                if log::log_enabled!(log::Level::Debug) {
                    eprintln!("\nDebug information:\n{}", prodigy_err.developer_message());
                }
                std::process::exit(prodigy_err.exit_code());
            }
            None => {
                eprintln!("Unexpected error: {:#}", e);
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
```

## Dependencies

- **Prerequisites**:
  - Spec 103 (Fix unwrap calls first to identify error patterns)
- **Affected Components**: All modules
- **External Dependencies**:
  - Keep: anyhow (for application layer)
  - Add: thiserror (for library errors)

## Testing Strategy

- **Unit Tests**: Test error creation and chaining
- **Error Path Tests**: Test all error conditions
- **Display Tests**: Verify error message formatting
- **Recovery Tests**: Test error recovery strategies
- **Integration Tests**: Test error propagation

## Documentation Requirements

- **Error Code Reference**: Document all error codes and meanings
- **Error Handling Guide**: Best practices for error handling
- **Troubleshooting Guide**: Common errors and solutions
- **API Documentation**: Document possible errors for each function

## Implementation Notes

- Start with most critical modules first
- Keep anyhow for application boundaries
- Use thiserror for library code
- Consider error budgets for different operations
- Add telemetry for error tracking (future)

## Migration and Compatibility

Gradual migration strategy:
```rust
// Phase 1: Wrap existing errors
impl From<std::io::Error> for ProdigyError {
    fn from(err: std::io::Error) -> Self {
        ProdigyError::storage("IO operation failed").with_source(err)
    }
}

// Phase 2: Replace string errors
// Before:
return Err("Invalid configuration".to_string());
// After:
return Err(ProdigyError::config("Invalid configuration"));

// Phase 3: Add error codes and context
return Err(ProdigyError::Config {
    code: error_codes::CONFIG_INVALID_YAML,
    message: "Invalid YAML syntax".to_string(),
    source: Some(Box::new(yaml_err)),
});
```