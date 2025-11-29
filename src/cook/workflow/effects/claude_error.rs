//! Claude command error types with transient error classification
//!
//! This module provides error types for Claude command execution with automatic
//! classification of transient (retryable) versus permanent errors.
//!
//! # Error Classification
//!
//! - **Transient errors** (should retry): HTTP 5xx, overload, timeouts
//! - **Permanent errors** (fail fast): authentication, invalid commands
//!
//! # Example
//!
//! ```ignore
//! use prodigy::cook::workflow::effects::claude_error::ClaudeError;
//!
//! let error = ClaudeError::HttpError { status: 500, message: "Internal error".to_string() };
//! assert!(error.is_transient());
//! ```

use std::time::Duration;

/// Claude command error types with transient classification
#[derive(Debug, Clone, thiserror::Error)]
pub enum ClaudeError {
    /// HTTP error from Claude API
    #[error("HTTP {status}: {message}")]
    HttpError { status: u16, message: String },

    /// Service overloaded or rate limited
    #[error("Service overloaded: {message}")]
    Overloaded { message: String },

    /// Network timeout
    #[error("Network timeout after {duration:?}")]
    Timeout { duration: Duration },

    /// Authentication failed
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    /// Invalid command syntax
    #[error("Invalid command: {message}")]
    InvalidCommand { message: String },

    /// Process execution error
    #[error("Process failed: {message}")]
    ProcessError { message: String },
}

impl ClaudeError {
    /// Check if error is transient (should retry)
    ///
    /// Transient errors:
    /// - HTTP 5xx errors
    /// - Overload/rate limit
    /// - Timeouts
    /// - Process spawn errors (conservative retry)
    ///
    /// Permanent errors:
    /// - Authentication failures
    /// - Invalid commands
    pub fn is_transient(&self) -> bool {
        match self {
            ClaudeError::HttpError { status, .. } => *status >= 500 && *status < 600,
            ClaudeError::Overloaded { .. } => true,
            ClaudeError::Timeout { .. } => true,
            ClaudeError::AuthenticationFailed { .. } => false,
            ClaudeError::InvalidCommand { .. } => false,
            ClaudeError::ProcessError { .. } => true, // Conservative: retry process errors
        }
    }

    /// Parse error from Claude stderr output
    ///
    /// Attempts to classify errors based on stderr patterns.
    pub fn from_stderr(stderr: &str) -> Self {
        let stderr_lower = stderr.to_lowercase();

        // Check for HTTP status codes
        if stderr_lower.contains("500") || stderr_lower.contains("internal server error") {
            return ClaudeError::HttpError {
                status: 500,
                message: stderr.to_string(),
            };
        }

        if stderr_lower.contains("502") || stderr_lower.contains("bad gateway") {
            return ClaudeError::HttpError {
                status: 502,
                message: stderr.to_string(),
            };
        }

        if stderr_lower.contains("503") || stderr_lower.contains("service unavailable") {
            return ClaudeError::HttpError {
                status: 503,
                message: stderr.to_string(),
            };
        }

        // Check for overload/rate limiting
        if stderr_lower.contains("overload")
            || stderr_lower.contains("rate limit")
            || stderr_lower.contains("too many requests")
        {
            return ClaudeError::Overloaded {
                message: stderr.to_string(),
            };
        }

        // Check for authentication errors
        if stderr_lower.contains("authentication")
            || stderr_lower.contains("unauthorized")
            || stderr_lower.contains("invalid token")
            || stderr_lower.contains("api key")
        {
            return ClaudeError::AuthenticationFailed {
                message: stderr.to_string(),
            };
        }

        // Check for command errors
        if stderr_lower.contains("command not found")
            || stderr_lower.contains("invalid command")
            || stderr_lower.contains("syntax error")
        {
            return ClaudeError::InvalidCommand {
                message: stderr.to_string(),
            };
        }

        // Default to process error
        ClaudeError::ProcessError {
            message: stderr.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_transient_http_5xx() {
        let error = ClaudeError::HttpError {
            status: 500,
            message: "Internal error".to_string(),
        };
        assert!(error.is_transient());

        let error = ClaudeError::HttpError {
            status: 502,
            message: "Bad gateway".to_string(),
        };
        assert!(error.is_transient());

        let error = ClaudeError::HttpError {
            status: 503,
            message: "Service unavailable".to_string(),
        };
        assert!(error.is_transient());
    }

    #[test]
    fn test_is_transient_http_4xx() {
        let error = ClaudeError::HttpError {
            status: 400,
            message: "Bad request".to_string(),
        };
        assert!(!error.is_transient());

        let error = ClaudeError::HttpError {
            status: 404,
            message: "Not found".to_string(),
        };
        assert!(!error.is_transient());
    }

    #[test]
    fn test_is_transient_overload() {
        let error = ClaudeError::Overloaded {
            message: "Service overloaded".to_string(),
        };
        assert!(error.is_transient());
    }

    #[test]
    fn test_is_transient_timeout() {
        let error = ClaudeError::Timeout {
            duration: Duration::from_secs(60),
        };
        assert!(error.is_transient());
    }

    #[test]
    fn test_is_transient_auth() {
        let error = ClaudeError::AuthenticationFailed {
            message: "Invalid token".to_string(),
        };
        assert!(!error.is_transient());
    }

    #[test]
    fn test_is_transient_invalid_command() {
        let error = ClaudeError::InvalidCommand {
            message: "Syntax error".to_string(),
        };
        assert!(!error.is_transient());
    }

    #[test]
    fn test_is_transient_process_error() {
        let error = ClaudeError::ProcessError {
            message: "Failed to spawn".to_string(),
        };
        assert!(error.is_transient());
    }

    #[test]
    fn test_from_stderr_500() {
        let error = ClaudeError::from_stderr("Error: HTTP 500 Internal Server Error");
        assert!(matches!(error, ClaudeError::HttpError { status: 500, .. }));
        assert!(error.is_transient());
    }

    #[test]
    fn test_from_stderr_overload() {
        let error = ClaudeError::from_stderr("Service is currently overloaded");
        assert!(matches!(error, ClaudeError::Overloaded { .. }));
        assert!(error.is_transient());
    }

    #[test]
    fn test_from_stderr_rate_limit() {
        let error = ClaudeError::from_stderr("Rate limit exceeded");
        assert!(matches!(error, ClaudeError::Overloaded { .. }));
        assert!(error.is_transient());
    }

    #[test]
    fn test_from_stderr_auth() {
        let error = ClaudeError::from_stderr("Authentication failed: invalid API key");
        assert!(matches!(error, ClaudeError::AuthenticationFailed { .. }));
        assert!(!error.is_transient());
    }

    #[test]
    fn test_from_stderr_invalid_command() {
        let error = ClaudeError::from_stderr("Command not found: /invalid");
        assert!(matches!(error, ClaudeError::InvalidCommand { .. }));
        assert!(!error.is_transient());
    }

    #[test]
    fn test_from_stderr_generic() {
        let error = ClaudeError::from_stderr("Some other error");
        assert!(matches!(error, ClaudeError::ProcessError { .. }));
        assert!(error.is_transient());
    }

    #[test]
    fn test_error_display() {
        let error = ClaudeError::HttpError {
            status: 500,
            message: "Internal error".to_string(),
        };
        let display = error.to_string();
        assert!(display.contains("HTTP 500"));
        assert!(display.contains("Internal error"));
    }

    #[test]
    fn test_error_display_timeout() {
        let error = ClaudeError::Timeout {
            duration: Duration::from_secs(60),
        };
        let display = error.to_string();
        assert!(display.contains("timeout"));
        assert!(display.contains("60s"));
    }
}
