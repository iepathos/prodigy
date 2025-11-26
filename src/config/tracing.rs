// Allow large error types from premortem's API design
#![allow(clippy::result_large_err)]

//! Configuration value tracing for debugging.
//!
//! This module provides types and functions for tracing where configuration
//! values come from and their override history. It wraps premortem's `TracedConfig`
//! with additional utilities for CLI output and issue detection.
//!
//! # Example
//!
//! ```ignore
//! use prodigy::config::tracing::{trace_config, TraceOutput};
//!
//! let traced = trace_config()?;
//!
//! // Query a specific path
//! if let Some(trace) = traced.trace("log_level") {
//!     println!("log_level: {}", trace.explain("log_level"));
//! }
//!
//! // Get all traces
//! for (path, trace) in traced.all_traces() {
//!     println!("{}", trace.explain(&path));
//! }
//!
//! // Get JSON output
//! let json = traced.to_json("log_level")?;
//! ```

use premortem::trace::{TracedConfig, ValueTrace};
use serde::{Deserialize, Serialize};

use super::builder::load_prodigy_config_traced_with;
use super::prodigy_config::ProdigyConfig;
use premortem::prelude::*;

/// Load configuration with tracing enabled.
///
/// Returns a traced configuration that can be queried for value origins.
pub fn trace_config() -> Result<TracedProdigyConfig, ConfigErrors> {
    let traced = load_prodigy_config_traced_with(&RealEnv)?;
    Ok(TracedProdigyConfig::new(traced))
}

/// Load configuration with tracing using a custom environment.
///
/// For testing with MockEnv.
pub fn trace_config_with<E: ConfigEnv>(env: &E) -> Result<TracedProdigyConfig, ConfigErrors> {
    let traced = load_prodigy_config_traced_with(env)?;
    Ok(TracedProdigyConfig::new(traced))
}

/// Wrapper around TracedConfig with additional utilities.
pub struct TracedProdigyConfig {
    inner: TracedConfig<ProdigyConfig>,
}

impl TracedProdigyConfig {
    /// Create a new traced config wrapper.
    pub fn new(inner: TracedConfig<ProdigyConfig>) -> Self {
        Self { inner }
    }

    /// Get the trace for a specific path.
    pub fn trace(&self, path: &str) -> Option<ValueTraceInfo> {
        self.inner.trace(path).map(ValueTraceInfo::from_premortem)
    }

    /// Check if a path was overridden.
    pub fn was_overridden(&self, path: &str) -> bool {
        self.inner.was_overridden(path)
    }

    /// Get all traced paths with their traces.
    pub fn all_traces(&self) -> Vec<(String, ValueTraceInfo)> {
        self.inner
            .traces()
            .map(|(path, trace)| (path.to_string(), ValueTraceInfo::from_premortem(trace)))
            .collect()
    }

    /// Get paths that were overridden by higher-priority sources.
    pub fn overridden_paths(&self) -> Vec<String> {
        self.inner
            .overridden_paths()
            .map(|s| s.to_string())
            .collect()
    }

    /// Get all traced paths.
    pub fn paths(&self) -> Vec<String> {
        self.inner.paths().map(|s| s.to_string()).collect()
    }

    /// Get the underlying configuration.
    pub fn config(&self) -> &ProdigyConfig {
        self.inner.value()
    }

    /// Consume and return the configuration.
    pub fn into_config(self) -> ProdigyConfig {
        self.inner.into_inner()
    }

    /// Generate JSON output for a specific path.
    pub fn to_json(&self, path: &str) -> Option<TraceJsonOutput> {
        self.trace(path).map(|trace| TraceJsonOutput {
            path: path.to_string(),
            final_value: trace.final_value.clone(),
            final_source: trace.final_source.clone(),
            history: trace.history.clone(),
        })
    }

    /// Generate JSON output for all overridden paths.
    pub fn overrides_to_json(&self) -> Vec<TraceJsonOutput> {
        self.overridden_paths()
            .into_iter()
            .filter_map(|path| self.to_json(&path))
            .collect()
    }

    /// Generate JSON output for all paths.
    pub fn all_to_json(&self) -> Vec<TraceJsonOutput> {
        self.paths()
            .into_iter()
            .filter_map(|path| self.to_json(&path))
            .collect()
    }
}

impl std::ops::Deref for TracedProdigyConfig {
    type Target = ProdigyConfig;

    fn deref(&self) -> &Self::Target {
        self.inner.value()
    }
}

/// Information about a traced value's source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSourceInfo {
    /// Type of source
    #[serde(rename = "type")]
    pub source_type: SourceType,

    /// Source identifier (file path, env var name, etc.)
    pub source: String,

    /// Line number for file sources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,

    /// Column number for file sources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl ValueSourceInfo {
    /// Create a new source info from premortem's SourceLocation.
    fn from_premortem(source: &premortem::error::SourceLocation) -> Self {
        let source_str = source.source.as_str();

        let source_type = if source_str == "defaults" {
            SourceType::Default
        } else if source_str.starts_with("env:") || source_str.starts_with("$") {
            SourceType::Environment
        } else {
            SourceType::File
        };

        Self {
            source_type,
            source: source_str.to_string(),
            line: source.line,
            column: source.column,
        }
    }

    /// Format the source for display.
    pub fn display(&self) -> String {
        match self.source_type {
            SourceType::Default => "default".to_string(),
            SourceType::Environment => {
                if self.source.starts_with("env:") {
                    format!("${}", &self.source[4..])
                } else {
                    format!("${}", self.source)
                }
            }
            SourceType::File => match self.line {
                Some(line) => format!("{}:{}", self.source, line),
                None => self.source.clone(),
            },
        }
    }
}

/// Type of configuration source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    /// Hardcoded default value
    Default,
    /// From configuration file
    File,
    /// From environment variable
    Environment,
}

/// Entry in the value history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The value at this point in history
    pub value: serde_json::Value,

    /// Source of this value
    pub source: ValueSourceInfo,

    /// Whether this value was overridden by a later source
    pub overridden: bool,
}

/// Information about a value's trace.
#[derive(Debug, Clone)]
pub struct ValueTraceInfo {
    /// The final resolved value
    pub final_value: serde_json::Value,

    /// Source of the final value
    pub final_source: ValueSourceInfo,

    /// All values this key had, in order of application
    pub history: Vec<HistoryEntry>,
}

impl ValueTraceInfo {
    /// Create from premortem's ValueTrace.
    fn from_premortem(trace: &ValueTrace) -> Self {
        let history: Vec<HistoryEntry> = trace
            .history
            .iter()
            .map(|tv| HistoryEntry {
                value: premortem_value_to_json(&tv.value),
                source: ValueSourceInfo::from_premortem(&tv.source),
                overridden: !tv.is_final,
            })
            .collect();

        Self {
            final_value: premortem_value_to_json(&trace.final_value.value),
            final_source: ValueSourceInfo::from_premortem(&trace.final_value.source),
            history,
        }
    }

    /// Check if the value was overridden.
    pub fn was_overridden(&self) -> bool {
        self.history.len() > 1
    }

    /// Get the number of sources that provided this value.
    pub fn source_count(&self) -> usize {
        self.history.len()
    }

    /// Generate a human-readable explanation of the trace.
    pub fn explain(&self, path: &str) -> String {
        let mut lines = Vec::new();

        // Format the final value
        let value_str = format_json_value(&self.final_value);
        lines.push(format!("{}: {}", path, value_str));

        // Format the history
        for (i, entry) in self.history.iter().enumerate() {
            let is_last = i == self.history.len() - 1;
            let prefix = if is_last {
                "  └──"
            } else {
                "  ├──"
            };

            let value_str = format_json_value(&entry.value);
            let marker = if entry.overridden {
                " (overridden)"
            } else {
                " ← final value"
            };

            lines.push(format!(
                "{} {}: {}{}",
                prefix,
                entry.source.display(),
                value_str,
                marker
            ));
        }

        lines.join("\n")
    }
}

/// JSON output format for trace information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceJsonOutput {
    /// Configuration path
    pub path: String,

    /// Final resolved value
    pub final_value: serde_json::Value,

    /// Source of final value
    pub final_source: ValueSourceInfo,

    /// Complete history of values
    pub history: Vec<HistoryEntry>,
}

/// Convert premortem Value to serde_json Value.
fn premortem_value_to_json(value: &premortem::value::Value) -> serde_json::Value {
    match value {
        premortem::value::Value::Null => serde_json::Value::Null,
        premortem::value::Value::Bool(b) => serde_json::Value::Bool(*b),
        premortem::value::Value::Integer(i) => serde_json::json!(*i),
        premortem::value::Value::Float(f) => serde_json::json!(*f),
        premortem::value::Value::String(s) => serde_json::Value::String(s.clone()),
        premortem::value::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(premortem_value_to_json).collect())
        }
        premortem::value::Value::Table(table) => {
            let map: serde_json::Map<String, serde_json::Value> = table
                .iter()
                .map(|(k, v)| (k.clone(), premortem_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Format a JSON value for display.
fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| format!("{:?}", value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_config_defaults() {
        let env = MockEnv::new();
        let traced = trace_config_with(&env).unwrap();

        // Should have traces for default values
        assert!(!traced.paths().is_empty());

        // Check log_level has a trace
        let trace = traced.trace("log_level");
        assert!(trace.is_some());

        let trace = trace.unwrap();
        assert_eq!(trace.final_value, serde_json::json!("info"));
        assert!(!trace.was_overridden());
    }

    #[test]
    fn test_trace_config_with_override() {
        use super::super::prodigy_config::global_config_path;

        let global_path = global_config_path();
        let env = MockEnv::new()
            .with_file(
                global_path.to_string_lossy().to_string(),
                "log_level: debug",
            )
            .with_env("PRODIGY__LOG_LEVEL", "warn");

        let traced = trace_config_with(&env).unwrap();

        let trace = traced.trace("log_level");
        assert!(trace.is_some());

        let trace = trace.unwrap();
        // Final value should be from env (highest priority)
        assert_eq!(trace.final_value, serde_json::json!("warn"));
        assert!(trace.was_overridden());
        assert!(trace.source_count() >= 2);
    }

    #[test]
    fn test_overridden_paths() {
        use super::super::prodigy_config::global_config_path;

        let global_path = global_config_path();
        let env = MockEnv::new()
            .with_file(
                global_path.to_string_lossy().to_string(),
                "log_level: debug\nmax_concurrent_specs: 8",
            )
            .with_env("PRODIGY__LOG_LEVEL", "warn");

        let traced = trace_config_with(&env).unwrap();
        let overridden = traced.overridden_paths();

        // log_level should be overridden (default -> file -> env)
        assert!(overridden.contains(&"log_level".to_string()));
    }

    #[test]
    fn test_explain_output() {
        use super::super::prodigy_config::global_config_path;

        let global_path = global_config_path();
        let env = MockEnv::new().with_file(
            global_path.to_string_lossy().to_string(),
            "log_level: debug",
        );

        let traced = trace_config_with(&env).unwrap();
        let trace = traced.trace("log_level").unwrap();

        let explanation = trace.explain("log_level");

        assert!(explanation.contains("log_level:"));
        assert!(explanation.contains("\"debug\""));
        assert!(explanation.contains("final value") || explanation.contains("overridden"));
    }

    #[test]
    fn test_json_output() {
        let env = MockEnv::new();
        let traced = trace_config_with(&env).unwrap();

        let json = traced.to_json("log_level");
        assert!(json.is_some());

        let json = json.unwrap();
        assert_eq!(json.path, "log_level");
        assert_eq!(json.final_value, serde_json::json!("info"));
        assert!(!json.history.is_empty());
    }

    #[test]
    fn test_source_type_detection() {
        // Test default detection
        let source = premortem::error::SourceLocation::new("defaults");
        let info = ValueSourceInfo::from_premortem(&source);
        assert_eq!(info.source_type, SourceType::Default);

        // Test env detection
        let source = premortem::error::SourceLocation::new("env:PRODIGY_LOG_LEVEL");
        let info = ValueSourceInfo::from_premortem(&source);
        assert_eq!(info.source_type, SourceType::Environment);

        // Test file detection
        let source = premortem::error::SourceLocation::new("config.yml")
            .with_line(10)
            .with_column(5);
        let info = ValueSourceInfo::from_premortem(&source);
        assert_eq!(info.source_type, SourceType::File);
        assert_eq!(info.line, Some(10));
        assert_eq!(info.column, Some(5));
    }

    #[test]
    fn test_source_display() {
        // Default
        let info = ValueSourceInfo {
            source_type: SourceType::Default,
            source: "defaults".to_string(),
            line: None,
            column: None,
        };
        assert_eq!(info.display(), "default");

        // Environment
        let info = ValueSourceInfo {
            source_type: SourceType::Environment,
            source: "env:LOG_LEVEL".to_string(),
            line: None,
            column: None,
        };
        assert_eq!(info.display(), "$LOG_LEVEL");

        // File with line
        let info = ValueSourceInfo {
            source_type: SourceType::File,
            source: "~/.prodigy/config.yml".to_string(),
            line: Some(10),
            column: None,
        };
        assert_eq!(info.display(), "~/.prodigy/config.yml:10");

        // File without line
        let info = ValueSourceInfo {
            source_type: SourceType::File,
            source: "config.yml".to_string(),
            line: None,
            column: None,
        };
        assert_eq!(info.display(), "config.yml");
    }

    #[test]
    fn test_all_to_json() {
        let env = MockEnv::new();
        let traced = trace_config_with(&env).unwrap();

        let all_json = traced.all_to_json();
        assert!(!all_json.is_empty());

        // Should have entry for log_level
        assert!(all_json.iter().any(|j| j.path == "log_level"));
    }
}
