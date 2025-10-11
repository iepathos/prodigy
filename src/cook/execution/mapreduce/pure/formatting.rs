//! Pure functions for formatting output messages
//!
//! These functions format error messages, execution summaries,
//! and other user-facing output.

use regex::Regex;
use std::sync::LazyLock;
use std::time::Duration;

use crate::cook::workflow::StepResult;

// Static regex patterns for secret sanitization
static PASSWORD_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"password[=:]\s*\S+").expect("Invalid regex pattern"));
static TOKEN_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"token[=:]\s*\S+").expect("Invalid regex pattern"));
static API_KEY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"api[_-]?key[=:]\s*\S+").expect("Invalid regex pattern"));

/// Format setup error message
///
/// # Arguments
///
/// * `step_index` - Zero-based step index
/// * `result` - Step execution result
/// * `is_claude_command` - Whether this was a Claude command
///
/// # Returns
///
/// Formatted error message string
pub fn format_setup_error(
    step_index: usize,
    result: &StepResult,
    is_claude_command: bool,
) -> String {
    let mut msg = format!("Setup command {} failed", step_index + 1);

    if let Some(exit_code) = result.exit_code {
        msg.push_str(&format!(" with exit code {}", exit_code));
    }

    if !result.stderr.is_empty() {
        msg.push_str(&format!(
            "\nStderr: {}",
            truncate_output(&result.stderr, 500)
        ));
    }

    if is_claude_command {
        if let Some(log_path) = &result.json_log_location {
            msg.push_str(&format!("\n📝 Claude log: {}", log_path));
        }
    }

    msg
}

/// Format commit requirement error
///
/// # Arguments
///
/// * `step_name` - Name of the step
/// * `json_log_location` - Optional path to Claude JSON log
///
/// # Returns
///
/// Formatted error message
pub fn format_commit_requirement_error(step_name: &str, json_log_location: Option<&str>) -> String {
    let mut msg = format!(
        "Step '{}' has commit_required=true but no commits were created",
        step_name
    );

    if let Some(log_path) = json_log_location {
        msg.push_str(&format!("\n📝 Claude log: {}", log_path));
    }

    msg
}

/// Format agent execution summary
///
/// # Arguments
///
/// * `total` - Total number of agents
/// * `successful` - Number of successful agents
/// * `failed` - Number of failed agents
/// * `duration` - Total duration
///
/// # Returns
///
/// Formatted summary string
pub fn format_execution_summary(
    total: usize,
    successful: usize,
    failed: usize,
    duration: Duration,
) -> String {
    format!(
        "Executed {} agents ({} successful, {} failed) in {:?}",
        total, successful, failed, duration
    )
}

/// Format phase completion message
///
/// # Arguments
///
/// * `phase` - Phase name
/// * `duration` - Phase duration
///
/// # Returns
///
/// Formatted completion message
pub fn format_phase_completion(phase: &str, duration: Duration) -> String {
    format!("{} phase completed in {:?}", phase, duration)
}

/// Truncate output for display
///
/// # Arguments
///
/// * `output` - Output string to truncate
/// * `max_chars` - Maximum characters to show
///
/// # Returns
///
/// Truncated string with indicator if truncated
pub fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        output.to_string()
    } else {
        format!(
            "{}... ({} more chars)",
            &output[..max_chars],
            output.len() - max_chars
        )
    }
}

/// Sanitize output for logging (remove sensitive data)
///
/// # Arguments
///
/// * `output` - Output string to sanitize
///
/// # Returns
///
/// Sanitized string with secrets masked
pub fn sanitize_output(output: &str) -> String {
    let mut result = output.to_string();

    result = PASSWORD_PATTERN
        .replace_all(&result, "password=***")
        .to_string();
    result = TOKEN_PATTERN.replace_all(&result, "token=***").to_string();
    result = API_KEY_PATTERN
        .replace_all(&result, "api_key=***")
        .to_string();

    result
}

/// Format duration in human-readable format
///
/// # Arguments
///
/// * `duration` - Duration to format
///
/// # Returns
///
/// Human-readable duration string
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Format file size
///
/// # Arguments
///
/// * `bytes` - Size in bytes
///
/// # Returns
///
/// Human-readable size string
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

/// Format progress percentage
///
/// # Arguments
///
/// * `current` - Current progress value
/// * `total` - Total value
///
/// # Returns
///
/// Formatted percentage string
pub fn format_progress(current: usize, total: usize) -> String {
    if total == 0 {
        return "0%".to_string();
    }
    let percentage = (current as f64 / total as f64) * 100.0;
    format!("{:.1}%", percentage)
}

/// Build error context for debugging
///
/// # Arguments
///
/// * `agent_id` - Agent identifier
/// * `item` - Work item that failed
/// * `error` - Error message
///
/// # Returns
///
/// Formatted error context string
pub fn build_error_context(agent_id: &str, item: &serde_json::Value, error: &str) -> String {
    format!(
        "Agent {} failed processing item:\n  Item: {}\n  Error: {}",
        agent_id,
        serde_json::to_string(item).unwrap_or_else(|_| "unknown".to_string()),
        error
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_setup_error_basic() {
        let result = StepResult {
            success: false,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "error message".to_string(),
            json_log_location: None,
        };

        let msg = format_setup_error(0, &result, false);
        assert!(msg.contains("Setup command 1 failed"));
        assert!(msg.contains("exit code 1"));
        assert!(msg.contains("error message"));
    }

    #[test]
    fn test_format_setup_error_with_claude_log() {
        let result = StepResult {
            success: false,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            json_log_location: Some("/path/to/log.json".to_string()),
        };

        let msg = format_setup_error(0, &result, true);
        assert!(msg.contains("Claude log"));
        assert!(msg.contains("/path/to/log.json"));
    }

    #[test]
    fn test_format_commit_requirement_error() {
        let msg = format_commit_requirement_error("test-step", None);
        assert!(msg.contains("test-step"));
        assert!(msg.contains("commit_required=true"));
        assert!(msg.contains("no commits were created"));
    }

    #[test]
    fn test_format_execution_summary() {
        let duration = Duration::from_secs(120);
        let msg = format_execution_summary(10, 8, 2, duration);
        assert!(msg.contains("10 agents"));
        assert!(msg.contains("8 successful"));
        assert!(msg.contains("2 failed"));
    }

    #[test]
    fn test_format_phase_completion() {
        let duration = Duration::from_secs(60);
        let msg = format_phase_completion("Setup", duration);
        assert!(msg.contains("Setup phase completed"));
    }

    #[test]
    fn test_truncate_output_short() {
        let output = "short message";
        assert_eq!(truncate_output(output, 100), "short message");
    }

    #[test]
    fn test_truncate_output_long() {
        let output = "a".repeat(1000);
        let truncated = truncate_output(&output, 100);
        assert!(truncated.len() < output.len());
        assert!(truncated.contains("more chars"));
    }

    #[test]
    fn test_sanitize_output_password() {
        let output = "password=secret123 and password: mypass";
        let sanitized = sanitize_output(output);
        assert!(sanitized.contains("password=***"));
        assert!(!sanitized.contains("secret123"));
        assert!(!sanitized.contains("mypass"));
    }

    #[test]
    fn test_sanitize_output_token() {
        let output = "token=abc123 and token: xyz789";
        let sanitized = sanitize_output(output);
        assert!(sanitized.contains("token=***"));
        assert!(!sanitized.contains("abc123"));
        assert!(!sanitized.contains("xyz789"));
    }

    #[test]
    fn test_sanitize_output_api_key() {
        let output = "api_key=sk-123 and api-key: key456";
        let sanitized = sanitize_output(output);
        assert!(sanitized.contains("api_key=***"));
        assert!(!sanitized.contains("sk-123"));
        assert!(!sanitized.contains("key456"));
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(2048), "2.00 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(2 * 1024 * 1024), "2.00 MB");
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.00 GB");
    }

    #[test]
    fn test_format_progress_normal() {
        assert_eq!(format_progress(50, 100), "50.0%");
    }

    #[test]
    fn test_format_progress_zero_total() {
        assert_eq!(format_progress(0, 0), "0%");
    }

    #[test]
    fn test_format_progress_partial() {
        assert_eq!(format_progress(1, 3), "33.3%");
    }

    #[test]
    fn test_build_error_context() {
        let item = serde_json::json!({"id": 1, "name": "test"});
        let ctx = build_error_context("agent-1", &item, "failed to process");
        assert!(ctx.contains("agent-1"));
        assert!(ctx.contains("test"));
        assert!(ctx.contains("failed to process"));
    }
}
