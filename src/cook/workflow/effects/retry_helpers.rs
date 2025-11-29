//! Retry policy helpers for workflow execution
//!
//! This module provides retry policy configurations for Claude and shell commands
//! using Stillwater's built-in retry system with exponential backoff.

use std::time::Duration;
use stillwater::retry::RetryPolicy;

/// Create retry policy for Claude transient errors
///
/// Uses exponential backoff with:
/// - Base delay: 5 seconds
/// - Max retries: 5
/// - Jitter: 25% (requires "jitter" feature)
pub fn claude_retry_policy() -> RetryPolicy {
    RetryPolicy::exponential(Duration::from_secs(5)).with_max_retries(5)
}

/// Create retry policy for shell commands (fewer retries)
///
/// Uses exponential backoff with:
/// - Base delay: 2 seconds
/// - Max retries: 2
pub fn shell_retry_policy() -> RetryPolicy {
    RetryPolicy::exponential(Duration::from_secs(2)).with_max_retries(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_retry_policy_configuration() {
        // Policy is created successfully if this doesn't panic
        let _policy = claude_retry_policy();
    }

    #[test]
    fn test_shell_retry_policy_configuration() {
        // Policy is created successfully if this doesn't panic
        let _policy = shell_retry_policy();
    }
}
