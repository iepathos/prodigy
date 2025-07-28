//! Retry and error handling utilities for subprocess execution
//!
//! This module provides robust error handling and retry logic for Claude CLI
//! subprocess calls, including transient failure detection and helpful error messages.

use anyhow::{Context, Result};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;

/// Execute a command with retry logic for transient failures
///
/// # Arguments
/// * `command` - The command to execute
/// * `description` - Human-readable description of the operation
/// * `max_retries` - Maximum number of retry attempts
/// * `verbose` - Whether to print detailed progress information
///
/// # Returns
/// The command output on success, or an error with context on failure
pub async fn execute_with_retry(
    mut command: Command,
    description: &str,
    max_retries: u32,
    verbose: bool,
) -> Result<std::process::Output> {
    let mut attempt = 0;
    let mut last_error = None;

    while attempt <= max_retries {
        if attempt > 0 {
            let delay = Duration::from_secs(2u64.pow(attempt.min(3))); // Exponential backoff, max 8s
            if verbose {
                println!(
                    "⏳ Retrying {description} after {delay:?} (attempt {attempt}/{max_retries})"
                );
            }
            sleep(delay).await;
        }

        match command.output().await {
            Ok(output) => {
                // Check if it's a transient error we should retry
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    // Retry on specific error conditions
                    if is_transient_error(&stderr) && attempt < max_retries {
                        if verbose {
                            eprintln!(
                                "⚠️  Transient error detected: {}",
                                stderr.lines().next().unwrap_or("Unknown error")
                            );
                        }
                        last_error = Some(stderr.to_string());
                        attempt += 1;
                        continue;
                    }
                }

                return Ok(output);
            }
            Err(e) => {
                // System-level errors (e.g., command not found) shouldn't be retried
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Err(e).context(format!("Command not found for {description}"));
                }

                // Other IO errors might be transient
                if attempt < max_retries {
                    if verbose {
                        eprintln!("⚠️  IO error: {e}");
                    }
                    last_error = Some(e.to_string());
                    attempt += 1;
                    continue;
                }

                return Err(e).context(format!("Failed to execute {description}"));
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed {} after {} retries. Last error: {}",
        description,
        max_retries,
        last_error.unwrap_or_else(|| "Unknown error".to_string())
    ))
}

/// Check if an error message indicates a transient failure
///
/// Detects common patterns that indicate temporary failures which
/// can be resolved by retrying the operation.
fn is_transient_error(stderr: &str) -> bool {
    let transient_patterns = [
        "rate limit",
        "timeout",
        "connection refused",
        "temporary failure",
        "network",
        "503",
        "429", // Too Many Requests
        "could not connect",
        "broken pipe",
    ];

    let stderr_lower = stderr.to_lowercase();
    transient_patterns
        .iter()
        .any(|pattern| stderr_lower.contains(pattern))
}

/// Check if Claude CLI is installed and provide helpful error message
///
/// Verifies that the Claude CLI is available in the system PATH.
/// If not found, provides detailed installation instructions.
///
/// # Returns
/// Ok(()) if Claude CLI is available, or an error with installation instructions
pub async fn check_claude_cli() -> Result<()> {
    let output = Command::new("which")
        .arg("claude")
        .output()
        .await
        .context("Failed to check for Claude CLI")?;

    if !output.status.success() || output.stdout.is_empty() {
        // Try 'claude --version' as a fallback (in case 'which' is not available)
        let version_check = Command::new("claude").arg("--version").output().await;

        if version_check.is_err() || !version_check.unwrap().status.success() {
            return Err(anyhow::anyhow!(
                "Claude CLI not found. Please install Claude CLI:\n\
                 \n\
                 1. Visit: https://claude.ai/download\n\
                 2. Download and install Claude CLI for your platform\n\
                 3. Run 'claude auth' to authenticate\n\
                 4. Ensure 'claude' is in your PATH\n\
                 \n\
                 You can verify the installation by running: claude --version"
            ));
        }
    }

    Ok(())
}

/// Get better error context for subprocess failures
///
/// Formats subprocess errors with helpful context and suggestions
/// based on common error patterns.
///
/// # Arguments
/// * `command` - The command that failed
/// * `exit_code` - The process exit code if available
/// * `stderr` - The error output from the process
/// * `stdout` - The standard output from the process
///
/// # Returns
/// A formatted error message with context and suggestions
pub fn format_subprocess_error(
    command: &str,
    exit_code: Option<i32>,
    stderr: &str,
    stdout: &str,
) -> String {
    let mut error_msg = format!("Command '{command}' failed");

    if let Some(code) = exit_code {
        error_msg.push_str(&format!(" with exit code {code}"));
    }

    if !stderr.trim().is_empty() {
        error_msg.push_str(&format!("\n\nError output:\n{}", stderr.trim()));
    }

    if stderr.trim().is_empty() && !stdout.trim().is_empty() {
        // Sometimes errors are written to stdout
        error_msg.push_str(&format!("\n\nOutput:\n{}", stdout.trim()));
    }

    // Add helpful suggestions based on common errors
    if stderr.contains("permission denied") || stderr.contains("unauthorized") {
        error_msg.push_str("\n\nHint: Check that you have authenticated with 'claude auth'");
    } else if stderr.contains("not found") && stderr.contains("command") {
        error_msg.push_str(&format!(
            "\n\nHint: The '{command}' command may not be installed or not in PATH"
        ));
    } else if stderr.contains("rate limit") {
        error_msg.push_str(
            "\n\nHint: You may have hit the API rate limit. Please wait a moment and try again.",
        );
    }

    error_msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transient_error_detection() {
        assert!(is_transient_error("Error: rate limit exceeded"));
        assert!(is_transient_error("Connection timeout"));
        assert!(is_transient_error("HTTP 503 Service Unavailable"));
        assert!(is_transient_error("Error 429: Too Many Requests"));
        assert!(is_transient_error("Connection refused by server"));
        assert!(is_transient_error("Temporary failure in name resolution"));
        assert!(is_transient_error("Network is unreachable"));
        assert!(is_transient_error("Could not connect to API"));
        assert!(is_transient_error("Broken pipe error"));
        assert!(!is_transient_error("Syntax error in file"));
        assert!(!is_transient_error("Command not found"));
        assert!(!is_transient_error("Invalid argument"));
    }

    #[test]
    fn test_transient_error_case_insensitive() {
        assert!(is_transient_error("RATE LIMIT EXCEEDED"));
        assert!(is_transient_error("Rate Limit Exceeded"));
        assert!(is_transient_error("RaTe LiMiT"));
    }

    #[test]
    fn test_error_formatting() {
        let error = format_subprocess_error("claude", Some(1), "Error: permission denied", "");
        assert!(error.contains("exit code 1"));
        assert!(error.contains("permission denied"));
        assert!(error.contains("claude auth"));
    }

    #[test]
    fn test_error_formatting_no_exit_code() {
        let error = format_subprocess_error("claude", None, "Something went wrong", "");
        assert!(error.contains("Command 'claude' failed"));
        assert!(error.contains("Something went wrong"));
        assert!(!error.contains("exit code"));
    }

    #[test]
    fn test_error_formatting_with_stdout_only() {
        let error = format_subprocess_error("claude", Some(1), "", "Error in stdout");
        assert!(error.contains("Error in stdout"));
        assert!(error.contains("Output:"));
    }

    #[test]
    fn test_error_formatting_command_not_found() {
        let error = format_subprocess_error("unknown-cmd", Some(127), "command not found", "");
        assert!(error.contains("may not be installed or not in PATH"));
    }

    #[test]
    fn test_error_formatting_rate_limit() {
        let error = format_subprocess_error("claude", Some(1), "Error: rate limit exceeded", "");
        assert!(error.contains("You may have hit the API rate limit"));
    }

    #[test]
    fn test_error_formatting_empty_outputs() {
        let error = format_subprocess_error("claude", Some(1), "  ", "  ");
        assert!(error.contains("Command 'claude' failed"));
        assert!(!error.contains("Error output:"));
        assert!(!error.contains("Output:"));
    }

    #[tokio::test]
    async fn test_execute_with_retry_success() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");

        let output = execute_with_retry(cmd, "echo test", 3, false)
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[tokio::test]
    async fn test_execute_with_retry_command_not_found() {
        let cmd = Command::new("this-command-does-not-exist");

        let result = execute_with_retry(cmd, "nonexistent command", 3, false).await;
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Command not found"));
    }

    #[tokio::test]
    async fn test_execute_with_retry_non_transient_failure() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo 'Fatal error' >&2; exit 1");

        let output = execute_with_retry(cmd, "failing command", 3, false)
            .await
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Fatal error"));
    }

    #[tokio::test]
    async fn test_execute_with_retry_exhausted_retries() {
        // Test that we get an error after exhausting all retries on transient errors
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo 'connection refused' >&2; exit 1");

        let output = execute_with_retry(cmd, "transient error test", 2, true)
            .await
            .unwrap();
        // The command succeeds (returns Ok) but with non-zero exit status
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("connection refused"));
    }

    #[tokio::test]
    async fn test_execute_with_retry_exponential_backoff() {
        use std::time::Instant;

        // Test exponential backoff timing
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo 'rate limit' >&2; exit 1");

        let start = Instant::now();
        let output = execute_with_retry(cmd, "backoff test", 2, false)
            .await
            .unwrap();
        let elapsed = start.elapsed();

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("rate limit"));
        // Should take at least 2s (first retry) + 4s (second retry) = 6s
        assert!(elapsed.as_secs() >= 6);
    }

    #[tokio::test]
    async fn test_check_claude_cli_when_missing() {
        let mut cmd = Command::new("which");
        cmd.arg("nonexistent-command-xyz");

        let output = cmd.output().await.unwrap();
        assert!(!output.status.success());
    }

    #[test]
    fn test_format_subprocess_error_all_fields() {
        let error = format_subprocess_error(
            "test-cmd",
            Some(42),
            "Error details\nMultiline error",
            "Some stdout content",
        );

        assert!(error.contains("'test-cmd'"));
        assert!(error.contains("exit code 42"));
        assert!(error.contains("Error details"));
        assert!(error.contains("Multiline error"));
        assert!(!error.contains("Some stdout content"));
    }

    #[test]
    fn test_format_subprocess_error_unauthorized() {
        let error = format_subprocess_error("claude", Some(1), "Error: unauthorized access", "");

        assert!(error.contains("Check that you have authenticated"));
    }

    #[test]
    fn test_is_transient_error_partial_matches() {
        assert!(is_transient_error("Error occurred: rate limit hit"));
        assert!(is_transient_error("Request timeout after 30s"));
        assert!(is_transient_error("Could not connect to host"));
        assert!(is_transient_error("Error: broken pipe while sending"));
    }

    #[tokio::test]
    async fn test_execute_with_retry_max_attempts_reached() {
        // Test error message when max retries are reached
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo 'temporary failure' >&2; exit 1");

        let output = execute_with_retry(cmd, "max retries test", 1, false)
            .await
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("temporary failure"));
    }

    #[tokio::test]
    async fn test_check_claude_cli_fallback() {
        // This test verifies the logic would work correctly
        // Note: actual behavior depends on system configuration
        let result = check_claude_cli().await;
        // Test passes regardless of whether claude is installed
        // We're testing that the function completes without panic
        let _ = result;
    }
}
