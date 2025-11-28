//! Configuration diagnostics and issue detection.
//!
//! This module provides utilities for detecting potential configuration issues
//! such as empty environment variables, typos in config keys, and other common
//! problems that can cause confusion.
//!
//! # Example
//!
//! ```
//! use prodigy::config::diagnostics::detect_issues;
//! use prodigy::config::tracing::trace_config_with;
//! use premortem::MockEnv;
//!
//! let env = MockEnv::new();
//! let traced = trace_config_with(&env).expect("trace failed");
//! let issues = detect_issues(&traced);
//!
//! for issue in &issues {
//!     println!("Warning: {}", issue.message);
//! }
//! ```

use super::tracing::{SourceType, TracedProdigyConfig};
use serde::{Deserialize, Serialize};

/// Configuration issue severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    /// Informational - may be intentional
    Info,
    /// Warning - likely unintentional
    Warning,
    /// Error - definitely wrong
    Error,
}

/// A detected configuration issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigIssue {
    /// Issue type
    #[serde(rename = "type")]
    pub issue_type: IssueType,

    /// Configuration path affected
    pub path: String,

    /// Severity of the issue
    pub severity: IssueSeverity,

    /// Human-readable message
    pub message: String,

    /// Suggested fix (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl ConfigIssue {
    /// Get a formatted message for display.
    pub fn display(&self) -> String {
        let severity_icon = match self.severity {
            IssueSeverity::Info => "ℹ",
            IssueSeverity::Warning => "⚠",
            IssueSeverity::Error => "✗",
        };

        let mut output = format!("{} {}", severity_icon, self.message);

        if let Some(ref suggestion) = self.suggestion {
            output.push_str(&format!("\n  Suggestion: {}", suggestion));
        }

        output
    }
}

/// Types of configuration issues.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueType {
    /// Environment variable is empty string
    EmptyEnvVar,

    /// Multiple sources override the same path
    MultipleOverrides,

    /// Environment variable overrides file config
    EnvOverridesFile,

    /// Value is set to default (might be unintentional)
    DefaultValue,

    /// Relative path might resolve differently
    RelativePathAmbiguity,
}

/// Detect potential issues in the configuration.
pub fn detect_issues(traced: &TracedProdigyConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();

    for (path, trace) in traced.all_traces() {
        // Check for empty string values from env vars
        if let SourceType::Environment = trace.final_source.source_type {
            if trace.final_value == serde_json::Value::String(String::new()) {
                issues.push(ConfigIssue {
                    issue_type: IssueType::EmptyEnvVar,
                    path: path.clone(),
                    severity: IssueSeverity::Warning,
                    message: format!(
                        "{} is set but empty from environment variable {}",
                        path,
                        trace.final_source.display()
                    ),
                    suggestion: Some(format!(
                        "Unset the variable or provide a value: unset {}",
                        trace
                            .final_source
                            .source
                            .strip_prefix("env:")
                            .unwrap_or(&trace.final_source.source)
                    )),
                });
            }
        }

        // Check for multiple overrides (potential confusion)
        if trace.source_count() > 2 {
            let sources: Vec<String> = trace.history.iter().map(|h| h.source.display()).collect();

            issues.push(ConfigIssue {
                issue_type: IssueType::MultipleOverrides,
                path: path.clone(),
                severity: IssueSeverity::Info,
                message: format!(
                    "\"{}\" was set in {} places: {}",
                    path,
                    trace.source_count(),
                    sources.join(" → ")
                ),
                suggestion: Some("Review if all overrides are intentional".to_string()),
            });
        }

        // Check for env overriding file config (might be unintentional)
        if trace.was_overridden() {
            let has_file_source = trace
                .history
                .iter()
                .any(|h| h.source.source_type == SourceType::File && h.overridden);
            let final_is_env = trace.final_source.source_type == SourceType::Environment;

            if has_file_source && final_is_env {
                issues.push(ConfigIssue {
                    issue_type: IssueType::EnvOverridesFile,
                    path: path.clone(),
                    severity: IssueSeverity::Info,
                    message: format!(
                        "\"{}\" is set in config file but overridden by {}",
                        path,
                        trace.final_source.display()
                    ),
                    suggestion: None,
                });
            }
        }
    }

    issues
}

/// Format issues for terminal output.
pub fn format_issues(issues: &[ConfigIssue]) -> String {
    if issues.is_empty() {
        return "No configuration issues detected.".to_string();
    }

    let mut output = String::from("Configuration issues detected:\n\n");

    for issue in issues {
        output.push_str(&issue.display());
        output.push_str("\n\n");
    }

    output
}

/// Format issues as JSON.
pub fn format_issues_json(issues: &[ConfigIssue]) -> String {
    serde_json::to_string_pretty(issues).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::prodigy_config::global_config_path;
    use crate::config::tracing::trace_config_with;
    use premortem::prelude::*;

    #[test]
    fn test_detect_empty_env_var() {
        // Use default_editor which is optional and can be empty
        let global_path = global_config_path();
        let env = MockEnv::new()
            .with_file(
                global_path.to_string_lossy().to_string(),
                "default_editor: vim",
            )
            .with_env("PRODIGY__DEFAULT_EDITOR", "");

        let traced = trace_config_with(&env).unwrap();
        let issues = detect_issues(&traced);

        let empty_env_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.issue_type == IssueType::EmptyEnvVar)
            .collect();

        assert!(
            !empty_env_issues.is_empty(),
            "Should detect empty env var issue"
        );
    }

    #[test]
    fn test_detect_multiple_overrides() {
        let global_path = global_config_path();
        let project_path = crate::config::prodigy_config::project_config_path();

        let env = MockEnv::new()
            .with_file(global_path.to_string_lossy().to_string(), "log_level: info")
            .with_file(
                project_path.to_string_lossy().to_string(),
                "log_level: debug",
            )
            .with_env("PRODIGY__LOG_LEVEL", "warn");

        let traced = trace_config_with(&env).unwrap();
        let issues = detect_issues(&traced);

        let multi_override_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.issue_type == IssueType::MultipleOverrides)
            .collect();

        assert!(
            !multi_override_issues.is_empty(),
            "Should detect multiple override issue"
        );
    }

    #[test]
    fn test_issue_display() {
        let issue = ConfigIssue {
            issue_type: IssueType::EmptyEnvVar,
            path: "log_level".to_string(),
            severity: IssueSeverity::Warning,
            message: "log_level is set but empty from $PRODIGY_LOG_LEVEL".to_string(),
            suggestion: Some("Unset the variable or provide a value".to_string()),
        };

        let display = issue.display();
        assert!(display.contains("⚠"));
        assert!(display.contains("log_level"));
        assert!(display.contains("Suggestion:"));
    }

    #[test]
    fn test_no_issues_for_clean_config() {
        let env = MockEnv::new();
        let traced = trace_config_with(&env).unwrap();
        let issues = detect_issues(&traced);

        // With defaults only, there shouldn't be any warning-level issues
        let warnings: Vec<_> = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning || i.severity == IssueSeverity::Error)
            .collect();

        assert!(
            warnings.is_empty(),
            "Clean config should have no warnings/errors"
        );
    }

    #[test]
    fn test_format_issues_empty() {
        let output = format_issues(&[]);
        assert!(output.contains("No configuration issues detected"));
    }

    #[test]
    fn test_format_issues_json() {
        let issues = vec![ConfigIssue {
            issue_type: IssueType::EmptyEnvVar,
            path: "test".to_string(),
            severity: IssueSeverity::Warning,
            message: "test message".to_string(),
            suggestion: None,
        }];

        let json = format_issues_json(&issues);
        assert!(json.contains("\"type\": \"empty_env_var\""));
        assert!(json.contains("\"path\": \"test\""));
    }
}
