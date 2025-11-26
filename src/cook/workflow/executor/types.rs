//! Core type definitions for workflow execution
//!
//! This module contains the fundamental types used throughout the workflow executor,
//! including command types, capture configurations, and execution results.

use crate::commands::AttributeValue;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Capture output configuration - either a boolean or a variable name
///
/// This type supports flexible output capture configurations:
/// - `Disabled`: Don't capture output (default)
/// - `Default`: Capture to command-type-specific variable names
/// - `Variable(name)`: Capture to a custom variable name
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CaptureOutput {
    /// Don't capture output
    #[default]
    Disabled,
    /// Capture to default variable names (claude.output, shell.output, etc.)
    Default,
    /// Capture to a custom variable name
    Variable(String),
}

impl Serialize for CaptureOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CaptureOutput::Disabled => serializer.serialize_bool(false),
            CaptureOutput::Default => serializer.serialize_bool(true),
            CaptureOutput::Variable(s) => serializer.serialize_str(s),
        }
    }
}

impl CaptureOutput {
    /// Check if output should be captured
    pub fn is_enabled(&self) -> bool {
        !matches!(self, CaptureOutput::Disabled)
    }

    /// Get the variable name to use for captured output
    ///
    /// Returns `None` if capture is disabled, otherwise returns the appropriate
    /// variable name based on the command type and capture configuration.
    pub fn get_variable_name(&self, command_type: &CommandType) -> Option<String> {
        match self {
            CaptureOutput::Disabled => None,
            CaptureOutput::Default => {
                // Use command-type specific default names
                Some(match command_type {
                    CommandType::Claude(_) | CommandType::Legacy(_) => "claude.output".to_string(),
                    CommandType::Shell(_) => "shell.output".to_string(),
                    CommandType::Handler { .. } => "handler.output".to_string(),
                    CommandType::Test(_) => "test.output".to_string(),
                    CommandType::Foreach(_) => "foreach.output".to_string(),
                    CommandType::WriteFile(_) => "write_file.output".to_string(),
                })
            }
            CaptureOutput::Variable(name) => Some(name.clone()),
        }
    }
}

/// Custom deserializer for CaptureOutput that accepts bool or string
///
/// Supports YAML/JSON configurations like:
/// ```yaml
/// capture_output: true              # -> CaptureOutput::Default
/// capture_output: false             # -> CaptureOutput::Disabled
/// capture_output: "my_var"          # -> CaptureOutput::Variable("my_var")
/// ```
pub fn deserialize_capture_output<'de, D>(deserializer: D) -> Result<CaptureOutput, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum CaptureOutputHelper {
        Bool(bool),
        String(String),
    }

    match CaptureOutputHelper::deserialize(deserializer)? {
        CaptureOutputHelper::Bool(false) => Ok(CaptureOutput::Disabled),
        CaptureOutputHelper::Bool(true) => Ok(CaptureOutput::Default),
        CaptureOutputHelper::String(s) => Ok(CaptureOutput::Variable(s)),
    }
}

/// Command type for workflow steps
///
/// Represents the different types of commands that can be executed in a workflow.
/// Each variant contains the configuration specific to that command type.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandType {
    /// Claude CLI command with args
    Claude(String),
    /// Shell command to execute
    Shell(String),
    /// Test command with retry logic
    Test(crate::config::command::TestCommand),
    /// Foreach command for parallel iteration
    Foreach(crate::config::command::ForeachConfig),
    /// Write file command with formatting and validation
    WriteFile(crate::config::command::WriteFileConfig),
    /// Legacy name-based approach (deprecated)
    Legacy(String),
    /// Modular command handler with dynamic attributes
    Handler {
        handler_name: String,
        attributes: HashMap<String, AttributeValue>,
    },
}

/// Result of executing a step
///
/// Contains the outcome of a step execution, including success status,
/// exit code, output streams, and optional debugging information.
#[derive(Debug, Clone, Default)]
pub struct StepResult {
    /// Whether the step executed successfully
    pub success: bool,
    /// Exit code from the command (if applicable)
    pub exit_code: Option<i32>,
    /// Standard output from the command
    pub stdout: String,
    /// Standard error output from the command
    pub stderr: String,
    /// Optional path to Claude JSON log file for debugging
    ///
    /// When a Claude command is executed, this field contains the path to the
    /// detailed JSON log file that can be used for troubleshooting and analysis.
    pub json_log_location: Option<String>,
}

/// Variable resolution tracking for verbose output
///
/// Tracks how variables are resolved during interpolation, useful for
/// debugging and understanding variable substitution in workflow commands.
#[derive(Debug, Clone)]
pub struct VariableResolution {
    /// The variable name that was resolved
    pub name: String,
    /// The raw expression before interpolation (e.g., "${foo.bar}")
    pub raw_expression: String,
    /// The final resolved value after interpolation
    pub resolved_value: String,
}
