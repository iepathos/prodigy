//! Utility functions for worktree management operations
//!
//! This module contains pure utility functions for worktree operations including:
//! - Variable interpolation for merge workflows
//! - Message formatting for user output
//! - Helper functions for data transformation
//!
//! # Responsibilities
//!
//! - String interpolation and variable substitution
//! - User-facing message formatting
//! - Pure data transformations
//! - Helper logic without side effects
//!
//! # Design Principles
//!
//! Functions in this module follow functional programming principles:
//! - Pure functions with no side effects
//! - No I/O operations
//! - Stateless transformations
//! - Testable without dependencies

use std::collections::HashMap;

/// Interpolate variables in a string using ${variable} syntax
///
/// Replaces all occurrences of ${key} with the corresponding value from the
/// variables HashMap. Variables not found in the map are left unchanged.
///
/// # Arguments
///
/// * `input` - String containing ${variable} placeholders
/// * `variables` - HashMap of variable names to their values
///
/// # Returns
///
/// String with all ${variable} placeholders replaced with their values
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use prodigy::worktree::manager_utilities::interpolate_variables;
///
/// let mut vars = HashMap::new();
/// vars.insert("name".to_string(), "session-123".to_string());
/// vars.insert("branch".to_string(), "feature/test".to_string());
///
/// let input = "Merging ${name} from ${branch}";
/// let result = interpolate_variables(input, &vars);
/// assert_eq!(result, "Merging session-123 from feature/test");
/// ```
pub fn interpolate_variables(input: &str, variables: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in variables {
        let placeholder = format!("${{{}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}

/// Format a cleanup instruction message for a merged session
///
/// Creates a user-friendly message instructing how to clean up a merged
/// worktree session using the CLI command.
///
/// # Arguments
///
/// * `session_name` - Name of the session that has been merged
///
/// # Returns
///
/// Formatted cleanup instruction message
///
/// # Examples
///
/// ```
/// use prodigy::worktree::manager_utilities::format_cleanup_message;
///
/// let msg = format_cleanup_message("session-abc123");
/// assert!(msg.contains("session-abc123"));
/// assert!(msg.contains("prodigy worktree cleanup"));
/// ```
pub fn format_cleanup_message(session_name: &str) -> String {
    format!(
        "ℹ️  Session '{}' has been merged. You can clean it up with: prodigy worktree cleanup {}",
        session_name, session_name
    )
}

/// Truncate a string value for display purposes
///
/// Truncates long strings to a maximum length and appends "... (truncated)"
/// to indicate the value was shortened. Used for displaying variable values
/// in logs without overwhelming output.
///
/// # Arguments
///
/// * `value` - String value to potentially truncate
/// * `max_length` - Maximum length before truncation (default: 100)
///
/// # Returns
///
/// Either the original string if short enough, or truncated version
///
/// # Examples
///
/// ```
/// use prodigy::worktree::manager_utilities::truncate_for_display;
///
/// let short = "short value";
/// assert_eq!(truncate_for_display(short, 100), "short value");
///
/// let long = "a".repeat(150);
/// let result = truncate_for_display(&long, 100);
/// assert!(result.ends_with("... (truncated)"));
/// assert!(result.len() < 150);
/// ```
pub fn truncate_for_display(value: &str, max_length: usize) -> String {
    if value.len() > max_length {
        format!("{}... (truncated)", &value[..max_length])
    } else {
        value.to_string()
    }
}

/// Build a formatted list of variable key-value pairs for logging
///
/// Creates a formatted string representation of variables for debug logging,
/// with each variable on its own line and long values truncated.
///
/// # Arguments
///
/// * `variables` - HashMap of variables to format
/// * `indent` - Indentation string (e.g., "  " for 2 spaces)
///
/// # Returns
///
/// Multi-line string with formatted variables
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use prodigy::worktree::manager_utilities::format_variables_for_log;
///
/// let mut vars = HashMap::new();
/// vars.insert("key1".to_string(), "value1".to_string());
/// vars.insert("key2".to_string(), "value2".to_string());
///
/// let formatted = format_variables_for_log(&vars, "  ");
/// assert!(formatted.contains("key1 = value1"));
/// assert!(formatted.contains("key2 = value2"));
/// ```
pub fn format_variables_for_log(variables: &HashMap<String, String>, indent: &str) -> String {
    let mut lines = Vec::new();
    let mut sorted_vars: Vec<_> = variables.iter().collect();
    sorted_vars.sort_by_key(|(k, _)| *k);

    for (key, value) in sorted_vars {
        let display_value = truncate_for_display(value, 100);
        lines.push(format!("{}{} = {}", indent, key, display_value));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_variables() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "test-session".to_string());
        vars.insert("branch".to_string(), "main".to_string());

        let input = "Merge ${name} to ${branch}";
        let result = interpolate_variables(input, &vars);
        assert_eq!(result, "Merge test-session to main");
    }

    #[test]
    fn test_interpolate_variables_missing() {
        let vars = HashMap::new();
        let input = "Merge ${name} to ${branch}";
        let result = interpolate_variables(input, &vars);
        assert_eq!(result, "Merge ${name} to ${branch}");
    }

    #[test]
    fn test_format_cleanup_message() {
        let msg = format_cleanup_message("session-123");
        assert!(msg.contains("session-123"));
        assert!(msg.contains("prodigy worktree cleanup session-123"));
    }

    #[test]
    fn test_truncate_short_value() {
        let value = "short";
        assert_eq!(truncate_for_display(value, 100), "short");
    }

    #[test]
    fn test_truncate_long_value() {
        let value = "a".repeat(150);
        let result = truncate_for_display(&value, 100);
        assert!(result.ends_with("... (truncated)"));
        assert!(result.len() < 150);
    }

    #[test]
    fn test_format_variables_for_log() {
        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), "value1".to_string());
        vars.insert("key2".to_string(), "value2".to_string());

        let formatted = format_variables_for_log(&vars, "  ");
        assert!(formatted.contains("key1 = value1"));
        assert!(formatted.contains("key2 = value2"));
    }
}
