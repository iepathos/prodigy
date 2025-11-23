//! # Prodigy Error System
//!
//! This module provides a comprehensive error handling system for Prodigy with support for
//! error context chaining, structured error codes, and user-friendly error messages.
//!
//! ## Overview
//!
//! The error system is built around [`ProdigyError`], a unified error type that supports:
//! - **Context Chaining**: Build rich error context through `.context()` calls
//! - **Error Codes**: Structured error codes for categorization (E1001, E2001, etc.)
//! - **User Messages**: End-user friendly error descriptions
//! - **Developer Messages**: Detailed diagnostic information with full context chain
//! - **Serialization**: Convert errors to JSON for API responses and logging
//!
//! ## Context Chaining Pattern
//!
//! The core pattern is to add context at **Effect boundaries** - points where your code
//! transitions between different layers of abstraction or performs I/O operations.
//!
//! ### Basic Usage
//!
//! ```rust
//! use prodigy::error::{ProdigyError, ErrorExt};
//!
//! fn read_config(path: &str) -> Result<Config, ProdigyError> {
//!     // Effect boundary: file I/O
//!     let content = std::fs::read_to_string(path)
//!         .map_err(ProdigyError::from)
//!         .context("Failed to read configuration file")?;
//!
//!     // Effect boundary: parsing
//!     let config: Config = serde_json::from_str(&content)
//!         .map_err(ProdigyError::from)
//!         .context("Failed to parse configuration JSON")?;
//!
//!     Ok(config)
//! }
//!
//! fn load_application_config() -> Result<Config, ProdigyError> {
//!     // Effect boundary: calling lower-level function
//!     read_config("config.json")
//!         .context("Failed to load application configuration")?
//! }
//! ```
//!
//! This creates a context chain like:
//! ```text
//! Failed to load application configuration
//!   └─ Failed to read configuration file
//!      └─ No such file or directory (os error 2)
//! ```
//!
//! ### Effect Boundaries
//!
//! Add `.context()` calls at these boundaries:
//!
//! 1. **I/O Operations**
//!    ```rust
//!    std::fs::write(path, data)
//!        .map_err(ProdigyError::from)
//!        .context(format!("Failed to write to {}", path))?;
//!    ```
//!
//! 2. **External Calls**
//!    ```rust
//!    subprocess.execute()
//!        .context("Failed to execute git command")?;
//!    ```
//!
//! 3. **Layer Transitions**
//!    ```rust
//!    storage.save_checkpoint(checkpoint)
//!        .context("Failed to persist workflow checkpoint")?;
//!    ```
//!
//! 4. **Error Propagation**
//!    ```rust
//!    validate_workflow(&workflow)
//!        .context(format!("Validation failed for workflow '{}'", workflow.name))?;
//!    ```
//!
//! ### Advanced Patterns
//!
//! **Dynamic Context with Closures**:
//! ```rust
//! work_items.iter()
//!     .map(|item| {
//!         process_item(item)
//!             .with_context(|| format!("Failed to process item {}", item.id))
//!     })
//!     .collect::<Result<Vec<_>, _>>()?;
//! ```
//!
//! **Context with Location Tracking**:
//! ```rust
//! use prodigy::error::helpers::common;
//!
//! fn critical_operation() -> Result<(), ProdigyError> {
//!     do_something()
//!         .map_err(|e| common::execution_error(
//!             "Critical operation failed",
//!             Some(e)
//!         ))
//!         .context_with_location("In critical_operation", file!(), line!())?;
//!     Ok(())
//! }
//! ```
//!
//! ## Error Construction
//!
//! Use the helper functions in [`helpers::common`] for creating errors:
//!
//! ```rust
//! use prodigy::error::helpers::common;
//!
//! // Configuration errors
//! return Err(common::config_error("Invalid timeout value", None));
//!
//! // Storage errors with path
//! return Err(common::storage_error_with_path(
//!     "Failed to read checkpoint",
//!     path,
//!     Some(io_error)
//! ));
//!
//! // Execution errors with command context
//! return Err(common::execution_error_with_command(
//!     "Command failed",
//!     "git commit",
//!     Some(1),
//!     None
//! ));
//! ```
//!
//! ## Displaying Errors
//!
//! Errors support multiple display formats:
//!
//! **User Message** (end-user friendly):
//! ```rust
//! println!("{}", error.user_message());
//! // Output: "Failed to load workflow configuration. Please check the file path and try again."
//! ```
//!
//! **Developer Message** (full diagnostic info):
//! ```rust
//! eprintln!("{}", error.developer_message());
//! // Output:
//! // Error: Failed to load application configuration
//! //   Context:
//! //     - Failed to read configuration file
//! //     - Failed to open config.json
//! //   Source: No such file or directory (os error 2)
//! ```
//!
//! ## Serialization
//!
//! Convert errors to JSON for APIs and logging:
//!
//! ```rust
//! use prodigy::error::SerializableError;
//!
//! let serializable = SerializableError::from(error);
//! let json = serde_json::to_string(&serializable)?;
//! ```
//!
//! ## Migration Guide
//!
//! To add context to existing error handling:
//!
//! **Before**:
//! ```rust
//! let data = read_file(path)?;
//! ```
//!
//! **After**:
//! ```rust
//! let data = read_file(path)
//!     .context(format!("Failed to read file at {}", path))?;
//! ```
//!
//! See the migration guide in `docs/specs/` for comprehensive examples.

use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

pub mod codes;
pub mod helpers;
pub mod serialization;

#[cfg(test)]
mod tests;

pub use codes::{describe_error_code, ErrorCode};
pub use helpers::{common, ErrorExt};
pub use serialization::SerializableError;

/// Error context entry
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub message: String,
    pub location: Option<&'static str>,
}

/// The unified error type for the entire Prodigy application
#[derive(Error, Debug)]
pub enum ProdigyError {
    #[error("[E{code:04}] Configuration error: {message}")]
    Config {
        code: u16,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },

    #[error("[E{code:04}] Session error: {message}")]
    Session {
        code: u16,
        message: String,
        session_id: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },

    #[error("[E{code:04}] Storage error: {message}")]
    Storage {
        code: u16,
        message: String,
        path: Option<PathBuf>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },

    #[error("[E{code:04}] Execution error: {message}")]
    Execution {
        code: u16,
        message: String,
        command: Option<String>,
        exit_code: Option<i32>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },

    #[error("[E{code:04}] Workflow error: {message}")]
    Workflow {
        code: u16,
        message: String,
        workflow_name: Option<String>,
        step: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },

    #[error("[E{code:04}] Git operation failed: {message}")]
    Git {
        code: u16,
        message: String,
        operation: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },

    #[error("[E{code:04}] Validation error: {message}")]
    Validation {
        code: u16,
        message: String,
        field: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },

    #[error("[E{code:04}] {message}")]
    Other {
        code: u16,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        context: Vec<ErrorContext>,
        error_source: Option<Arc<ProdigyError>>,
    },
}

impl ProdigyError {
    /// Create a configuration error with default code
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            code: ErrorCode::CONFIG_GENERIC,
            message: message.into(),
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a configuration error with specific code
    pub fn config_with_code(code: u16, message: impl Into<String>) -> Self {
        Self::Config {
            code,
            message: message.into(),
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a session error with default code
    pub fn session(message: impl Into<String>) -> Self {
        Self::Session {
            code: ErrorCode::SESSION_GENERIC,
            message: message.into(),
            session_id: None,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a session error with specific code and session ID
    pub fn session_with_code(
        code: u16,
        message: impl Into<String>,
        session_id: Option<String>,
    ) -> Self {
        Self::Session {
            code,
            message: message.into(),
            session_id,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a storage error with default code
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            code: ErrorCode::STORAGE_GENERIC,
            message: message.into(),
            path: None,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a storage error with specific code and path
    pub fn storage_with_code(code: u16, message: impl Into<String>, path: Option<PathBuf>) -> Self {
        Self::Storage {
            code,
            message: message.into(),
            path,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create an execution error with default code
    pub fn execution(message: impl Into<String>) -> Self {
        Self::Execution {
            code: ErrorCode::EXEC_GENERIC,
            message: message.into(),
            command: None,
            exit_code: None,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create an execution error with specific code
    pub fn execution_with_code(
        code: u16,
        message: impl Into<String>,
        command: Option<String>,
    ) -> Self {
        Self::Execution {
            code,
            message: message.into(),
            command,
            exit_code: None,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a workflow error with default code
    pub fn workflow(message: impl Into<String>) -> Self {
        Self::Workflow {
            code: ErrorCode::WORKFLOW_GENERIC,
            message: message.into(),
            workflow_name: None,
            step: None,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a workflow error with specific code
    pub fn workflow_with_code(
        code: u16,
        message: impl Into<String>,
        workflow_name: Option<String>,
    ) -> Self {
        Self::Workflow {
            code,
            message: message.into(),
            workflow_name,
            step: None,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a git error with specific code and operation
    pub fn git(code: u16, message: impl Into<String>, operation: impl Into<String>) -> Self {
        Self::Git {
            code,
            message: message.into(),
            operation: operation.into(),
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a validation error with default code
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            code: ErrorCode::VALIDATION_GENERIC,
            message: message.into(),
            field: None,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a validation error with specific code and field
    pub fn validation_with_code(
        code: u16,
        message: impl Into<String>,
        field: Option<String>,
    ) -> Self {
        Self::Validation {
            code,
            message: message.into(),
            field,
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Create a generic other error
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            code: ErrorCode::OTHER_GENERIC,
            message: message.into(),
            source: None,
            context: Vec::new(),
            error_source: None,
        }
    }

    /// Add a source error to this error
    pub fn with_source(
        mut self,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        match &mut self {
            Self::Config { source: src, .. }
            | Self::Session { source: src, .. }
            | Self::Storage { source: src, .. }
            | Self::Execution { source: src, .. }
            | Self::Workflow { source: src, .. }
            | Self::Git { source: src, .. }
            | Self::Validation { source: src, .. }
            | Self::Other { source: src, .. } => {
                *src = Some(source.into());
            }
        }
        self
    }

    /// Add context to the error message (legacy method, kept for compatibility)
    pub fn with_context(mut self, context: impl Display) -> Self {
        match &mut self {
            Self::Config { message, .. }
            | Self::Session { message, .. }
            | Self::Storage { message, .. }
            | Self::Execution { message, .. }
            | Self::Workflow { message, .. }
            | Self::Git { message, .. }
            | Self::Validation { message, .. }
            | Self::Other { message, .. } => {
                *message = format!("{}: {}", message, context);
            }
        }
        self
    }

    /// Add context to error (fluent API, preferred for new code)
    pub fn context(mut self, message: impl Into<String>) -> Self {
        let ctx = ErrorContext {
            message: message.into(),
            location: None,
        };
        match &mut self {
            Self::Config { context, .. }
            | Self::Session { context, .. }
            | Self::Storage { context, .. }
            | Self::Execution { context, .. }
            | Self::Workflow { context, .. }
            | Self::Git { context, .. }
            | Self::Validation { context, .. }
            | Self::Other { context, .. } => {
                context.push(ctx);
            }
        }
        self
    }

    /// Add context with source location tracking
    #[track_caller]
    pub fn context_at(mut self, message: impl Into<String>) -> Self {
        let location = std::panic::Location::caller();
        let ctx = ErrorContext {
            message: message.into(),
            location: Some(location.file()),
        };
        match &mut self {
            Self::Config { context, .. }
            | Self::Session { context, .. }
            | Self::Storage { context, .. }
            | Self::Execution { context, .. }
            | Self::Workflow { context, .. }
            | Self::Git { context, .. }
            | Self::Validation { context, .. }
            | Self::Other { context, .. } => {
                context.push(ctx);
            }
        }
        self
    }

    /// Chain with another ProdigyError as source
    pub fn with_error_source(mut self, source: ProdigyError) -> Self {
        match &mut self {
            Self::Config { error_source, .. }
            | Self::Session { error_source, .. }
            | Self::Storage { error_source, .. }
            | Self::Execution { error_source, .. }
            | Self::Workflow { error_source, .. }
            | Self::Git { error_source, .. }
            | Self::Validation { error_source, .. }
            | Self::Other { error_source, .. } => {
                *error_source = Some(Arc::new(source));
            }
        }
        self
    }

    /// Get context chain
    pub fn chain(&self) -> &[ErrorContext] {
        match self {
            Self::Config { context, .. }
            | Self::Session { context, .. }
            | Self::Storage { context, .. }
            | Self::Execution { context, .. }
            | Self::Workflow { context, .. }
            | Self::Git { context, .. }
            | Self::Validation { context, .. }
            | Self::Other { context, .. } => context,
        }
    }

    /// Get root error (follows error_source chain)
    pub fn root_cause(&self) -> &ProdigyError {
        let mut current = self;
        loop {
            match current {
                Self::Config { error_source, .. }
                | Self::Session { error_source, .. }
                | Self::Storage { error_source, .. }
                | Self::Execution { error_source, .. }
                | Self::Workflow { error_source, .. }
                | Self::Git { error_source, .. }
                | Self::Validation { error_source, .. }
                | Self::Other { error_source, .. } => {
                    if let Some(ref src) = error_source {
                        current = src;
                    } else {
                        return current;
                    }
                }
            }
        }
    }

    /// Get a reference to the error_source if present
    pub fn error_source(&self) -> Option<&ProdigyError> {
        match self {
            Self::Config { error_source, .. }
            | Self::Session { error_source, .. }
            | Self::Storage { error_source, .. }
            | Self::Execution { error_source, .. }
            | Self::Workflow { error_source, .. }
            | Self::Git { error_source, .. }
            | Self::Validation { error_source, .. }
            | Self::Other { error_source, .. } => error_source.as_deref(),
        }
    }

    /// Get the exit code for this error
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Config { .. } => 2,
            Self::Session { .. } => 3,
            Self::Storage { .. } => 4,
            Self::Execution { .. } => 5,
            Self::Workflow { .. } => 6,
            Self::Git { .. } => 7,
            Self::Validation { .. } => 8,
            Self::Other { .. } => 1,
        }
    }

    /// Get the error code
    pub fn code(&self) -> u16 {
        match self {
            Self::Config { code, .. }
            | Self::Session { code, .. }
            | Self::Storage { code, .. }
            | Self::Execution { code, .. }
            | Self::Workflow { code, .. }
            | Self::Git { code, .. }
            | Self::Validation { code, .. }
            | Self::Other { code, .. } => *code,
        }
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::Config { message, .. } => format!("Configuration problem: {}", message),
            Self::Session {
                message,
                session_id,
                ..
            } => {
                if let Some(id) = session_id {
                    format!("Session {} error: {}", id, message)
                } else {
                    format!("Session error: {}", message)
                }
            }
            Self::Storage { message, path, .. } => {
                if let Some(p) = path {
                    format!("Storage error at {}: {}", p.display(), message)
                } else {
                    format!("Storage error: {}", message)
                }
            }
            Self::Execution {
                message, command, ..
            } => {
                if let Some(cmd) = command {
                    format!("Command '{}' failed: {}", cmd, message)
                } else {
                    format!("Execution error: {}", message)
                }
            }
            Self::Workflow {
                message,
                workflow_name,
                step,
                ..
            } => {
                let mut msg = String::from("Workflow error");
                if let Some(name) = workflow_name {
                    msg.push_str(&format!(" in '{}'", name));
                }
                if let Some(s) = step {
                    msg.push_str(&format!(" at step '{}'", s));
                }
                format!("{}: {}", msg, message)
            }
            Self::Git {
                message, operation, ..
            } => {
                format!("Git {} failed: {}", operation, message)
            }
            Self::Validation { message, field, .. } => {
                if let Some(f) = field {
                    format!("Validation error for '{}': {}", f, message)
                } else {
                    format!("Validation error: {}", message)
                }
            }
            Self::Other { message, .. } => message.clone(),
        }
    }

    /// Get a developer-friendly error message with full chain
    pub fn developer_message(&self) -> String {
        let mut msg = format!("{:#}", self);

        // Add context chain if present
        let context_chain = self.chain();
        if !context_chain.is_empty() {
            msg.push_str("\n\nContext chain:");
            for (i, ctx) in context_chain.iter().enumerate() {
                msg.push_str(&format!("\n  {}: {}", i, ctx.message));
                if let Some(loc) = ctx.location {
                    msg.push_str(&format!(" (at {})", loc));
                }
            }
        }

        // Add error source chain if present
        if let Some(src) = self.error_source() {
            msg.push_str("\n\nCaused by:");
            msg.push_str(&format!("\n  {}", src.developer_message()));
        }

        msg
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Execution {
                exit_code: Some(code),
                ..
            } => {
                // Non-zero but not fatal exit codes
                *code != 0 && *code < 128
            }
            Self::Storage { code, .. } => {
                // Temporary storage issues are recoverable
                *code == ErrorCode::STORAGE_TEMPORARY || *code == ErrorCode::STORAGE_LOCK_BUSY
            }
            _ => false,
        }
    }

    /// Set the exit code for an execution error
    pub fn with_exit_code(mut self, exit_code: i32) -> Self {
        if let Self::Execution {
            exit_code: ref mut ec,
            ..
        } = self
        {
            *ec = Some(exit_code);
        }
        self
    }

    /// Set the workflow step for a workflow error
    pub fn with_step(mut self, step: impl Into<String>) -> Self {
        if let Self::Workflow {
            step: ref mut s, ..
        } = self
        {
            *s = Some(step.into());
        }
        self
    }
}

/// Type alias for Results using ProdigyError
pub type Result<T> = std::result::Result<T, ProdigyError>;

/// Type alias for library Results (same as Result for now)
pub type LibResult<T> = std::result::Result<T, ProdigyError>;

/// Type alias for application Results (using anyhow for flexibility)
pub type AppResult<T> = anyhow::Result<T>;

// Conversion from common error types

impl From<std::io::Error> for ProdigyError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;

        let (code, message) = match err.kind() {
            ErrorKind::NotFound => (ErrorCode::STORAGE_NOT_FOUND, "File or directory not found"),
            ErrorKind::PermissionDenied => {
                (ErrorCode::STORAGE_PERMISSION_DENIED, "Permission denied")
            }
            ErrorKind::AlreadyExists => (ErrorCode::STORAGE_ALREADY_EXISTS, "Already exists"),
            ErrorKind::InvalidInput => (ErrorCode::VALIDATION_INVALID_INPUT, "Invalid input"),
            ErrorKind::InvalidData => (ErrorCode::VALIDATION_INVALID_DATA, "Invalid data"),
            ErrorKind::TimedOut => (ErrorCode::EXEC_TIMEOUT, "Operation timed out"),
            ErrorKind::Interrupted => (ErrorCode::EXEC_INTERRUPTED, "Operation interrupted"),
            ErrorKind::WouldBlock => (
                ErrorCode::STORAGE_LOCK_BUSY,
                "Resource temporarily unavailable",
            ),
            _ => (ErrorCode::STORAGE_IO_ERROR, "IO operation failed"),
        };

        ProdigyError::storage_with_code(code, message, None).with_source(err)
    }
}

impl From<serde_yaml::Error> for ProdigyError {
    fn from(err: serde_yaml::Error) -> Self {
        ProdigyError::config_with_code(ErrorCode::CONFIG_INVALID_YAML, "Invalid YAML syntax")
            .with_source(err)
    }
}

impl From<serde_json::Error> for ProdigyError {
    fn from(err: serde_json::Error) -> Self {
        ProdigyError::config_with_code(ErrorCode::CONFIG_INVALID_JSON, "Invalid JSON syntax")
            .with_source(err)
    }
}

// Note: ProdigyError automatically converts to anyhow::Error because it implements std::error::Error
