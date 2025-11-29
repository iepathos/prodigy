//! Retry policy helpers for workflow execution
//!
//! This module provides retry policy configurations for Claude and shell commands
//! using Stillwater's built-in retry system with exponential backoff.

use crate::cook::execution::command::RetryConfig;
use std::time::Duration;
use stillwater::retry::RetryPolicy;
use tracing::warn;

/// Create default retry policy for Claude transient errors
///
/// Uses exponential backoff with:
/// - Base delay: 5 seconds
/// - Max retries: 5
/// - Jitter: 25%
/// - Max delay: 120 seconds
///
/// IMPORTANT: RetryPolicy requires at least one bound (max_retries OR max_delay).
/// Without at least one bound, the policy will panic at construction.
pub fn default_claude_retry_policy() -> RetryPolicy {
    RetryPolicy::exponential(Duration::from_secs(5))
        .with_max_retries(5)
        .with_jitter(0.25)
        .with_max_delay(Duration::from_secs(120))
}

/// Create retry policy for shell commands (fewer retries)
///
/// Uses exponential backoff with:
/// - Base delay: 2 seconds
/// - Max retries: 2
pub fn shell_retry_policy() -> RetryPolicy {
    RetryPolicy::exponential(Duration::from_secs(2)).with_max_retries(2)
}

/// Parse retry policy from configuration
///
/// Converts a RetryConfig into a Stillwater RetryPolicy, ensuring at least one
/// bound (max_retries or max_delay) is always set to satisfy RetryPolicy's
/// validation requirements.
///
/// # Arguments
///
/// * `config` - Optional retry configuration. If None, returns default Claude retry policy
///
/// # Returns
///
/// A configured RetryPolicy with bounds and optional jitter
pub fn parse_retry_policy(config: Option<&RetryConfig>) -> RetryPolicy {
    match config {
        Some(cfg) => {
            // Create base policy from strategy
            let base = match cfg.strategy.as_str() {
                "constant" => RetryPolicy::constant(cfg.initial_delay),
                "linear" => RetryPolicy::linear(cfg.initial_delay),
                "exponential" => RetryPolicy::exponential(cfg.initial_delay),
                _ => {
                    warn!(
                        "Unknown retry strategy '{}', using exponential",
                        cfg.strategy
                    );
                    RetryPolicy::exponential(cfg.initial_delay)
                }
            };

            // Always set max_retries to ensure at least one bound
            let mut policy = base.with_max_retries(cfg.max_attempts);

            // Add jitter if specified
            if let Some(jitter) = cfg.jitter {
                policy = policy.with_jitter(jitter);
            }

            // Add max_delay
            policy = policy.with_max_delay(cfg.max_delay);

            policy
        }
        None => default_claude_retry_policy(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_claude_retry_policy() {
        // Policy is created successfully if this doesn't panic
        let _policy = default_claude_retry_policy();
    }

    #[test]
    fn test_shell_retry_policy_configuration() {
        // Policy is created successfully if this doesn't panic
        let _policy = shell_retry_policy();
    }

    #[test]
    fn test_parse_retry_policy_with_config() {
        let config = RetryConfig {
            strategy: "exponential".to_string(),
            max_attempts: 3,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
            jitter: Some(0.1),
            exponential_base: None,
        };

        // Should not panic
        let _policy = parse_retry_policy(Some(&config));
    }

    #[test]
    fn test_parse_retry_policy_constant_strategy() {
        let config = RetryConfig {
            strategy: "constant".to_string(),
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            jitter: None,
            exponential_base: None,
        };

        let _policy = parse_retry_policy(Some(&config));
    }

    #[test]
    fn test_parse_retry_policy_linear_strategy() {
        let config = RetryConfig {
            strategy: "linear".to_string(),
            max_attempts: 4,
            initial_delay: Duration::from_secs(3),
            max_delay: Duration::from_secs(90),
            jitter: Some(0.2),
            exponential_base: None,
        };

        let _policy = parse_retry_policy(Some(&config));
    }

    #[test]
    fn test_parse_retry_policy_unknown_strategy_falls_back() {
        let config = RetryConfig {
            strategy: "unknown".to_string(),
            max_attempts: 3,
            initial_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(60),
            jitter: None,
            exponential_base: None,
        };

        // Should use exponential as fallback and not panic
        let _policy = parse_retry_policy(Some(&config));
    }

    #[test]
    fn test_parse_retry_policy_none_uses_default() {
        let _policy = parse_retry_policy(None);
        // Should be equivalent to default_claude_retry_policy()
        let _default = default_claude_retry_policy();
        // Both should be created successfully
    }
}
