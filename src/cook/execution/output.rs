//! Output processing and formatting for unified command execution

use super::command::{CommandType, OutputFormat};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::Duration;

/// Output processor for command results
pub struct OutputProcessor {
    formatters: HashMap<CommandType, Box<dyn OutputFormatter>>,
    parsers: HashMap<OutputFormat, Box<dyn OutputParser>>,
}

impl OutputProcessor {
    pub fn new() -> Self {
        let mut formatters: HashMap<CommandType, Box<dyn OutputFormatter>> = HashMap::new();
        formatters.insert(CommandType::Claude, Box::new(ClaudeOutputFormatter));
        formatters.insert(CommandType::Shell, Box::new(ShellOutputFormatter));
        formatters.insert(CommandType::Test, Box::new(TestOutputFormatter));
        formatters.insert(CommandType::Handler, Box::new(HandlerOutputFormatter));

        let mut parsers: HashMap<OutputFormat, Box<dyn OutputParser>> = HashMap::new();
        parsers.insert(OutputFormat::Json, Box::new(JsonOutputParser));
        parsers.insert(OutputFormat::Yaml, Box::new(YamlOutputParser));
        parsers.insert(OutputFormat::PlainText, Box::new(PlainTextOutputParser));

        Self {
            formatters,
            parsers,
        }
    }

    /// Process raw output into formatted output
    pub async fn process_output(
        &self,
        raw_output: ProcessOutput,
        command_type: CommandType,
        output_format: Option<OutputFormat>,
    ) -> Result<ProcessedOutput> {
        // Apply command-type specific formatting
        let formatted_output = if let Some(formatter) = self.formatters.get(&command_type) {
            formatter.format(&raw_output).await?
        } else {
            raw_output.clone()
        };

        // Apply format-specific parsing if requested
        let parsed_output = if let Some(ref format) = output_format {
            if let Some(parser) = self.parsers.get(format) {
                parser.parse(&formatted_output).await?
            } else {
                formatted_output
            }
        } else {
            formatted_output
        };

        Ok(ProcessedOutput::new(parsed_output, output_format))
    }
}

impl Default for OutputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Raw process output
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub structured_data: Option<JsonValue>,
    pub error_summary: Option<String>,
    pub metadata: OutputMetadata,
}

impl ProcessOutput {
    pub fn empty() -> Self {
        Self {
            stdout: None,
            stderr: None,
            structured_data: None,
            error_summary: None,
            metadata: OutputMetadata::default(),
        }
    }

    pub fn new() -> Self {
        Self::empty()
    }

    pub fn with_stdout(mut self, stdout: String) -> Self {
        self.stdout = Some(stdout);
        self
    }

    pub fn with_stderr(mut self, stderr: String) -> Self {
        self.stderr = Some(stderr);
        self
    }

    pub fn with_structured_data(mut self, data: Option<JsonValue>) -> Self {
        self.structured_data = data;
        self
    }
}

/// Output metadata
#[derive(Debug, Clone, Default)]
pub struct OutputMetadata {
    pub bytes_stdout: usize,
    pub bytes_stderr: usize,
    pub lines_stdout: usize,
    pub lines_stderr: usize,
    pub truncated: bool,
}

/// Processed output with formatting applied
#[derive(Debug, Clone)]
pub struct ProcessedOutput {
    pub content: ProcessOutput,
    pub format: OutputFormat,
    pub processing_duration: Duration,
    pub warnings: Vec<String>,
}

impl ProcessedOutput {
    pub fn new(content: ProcessOutput, format: Option<OutputFormat>) -> Self {
        Self {
            content,
            format: format.unwrap_or(OutputFormat::PlainText),
            processing_duration: Duration::from_secs(0),
            warnings: Vec::new(),
        }
    }
}

/// Output formatter trait
#[async_trait]
pub trait OutputFormatter: Send + Sync {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput>;
}

/// Output parser trait
#[async_trait]
pub trait OutputParser: Send + Sync {
    async fn parse(&self, output: &ProcessOutput) -> Result<ProcessOutput>;
}

/// Claude output formatter
pub struct ClaudeOutputFormatter;

#[async_trait]
impl OutputFormatter for ClaudeOutputFormatter {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        let mut formatted = output.clone();

        // Claude-specific output processing
        if let Some(ref stdout) = output.stdout {
            // Remove Claude CLI formatting artifacts
            let cleaned = self.remove_claude_artifacts(stdout);

            // Extract structured data if present
            let structured = self.extract_structured_data(&cleaned)?;

            formatted.stdout = Some(cleaned);
            formatted.structured_data = structured;
        }

        // Process Claude error messages
        if let Some(ref stderr) = output.stderr {
            formatted.error_summary = self.extract_claude_error(stderr);
        }

        Ok(formatted)
    }
}

impl ClaudeOutputFormatter {
    fn remove_claude_artifacts(&self, stdout: &str) -> String {
        // Remove common Claude CLI formatting
        stdout
            .lines()
            .filter(|line| {
                // Filter out Claude metadata lines
                !line.starts_with("Claude:") && !line.starts_with("[Claude]")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn extract_structured_data(&self, output: &str) -> Result<Option<JsonValue>> {
        // Look for JSON blocks in the output
        if let Some(start) = output.find("```json") {
            if let Some(end) = output[start..].find("```\n").map(|i| start + i) {
                let json_str = &output[start + 7..end].trim();
                if let Ok(json) = serde_json::from_str(json_str) {
                    return Ok(Some(json));
                }
            }
        }

        // Try to parse the entire output as JSON
        if let Ok(json) = serde_json::from_str(output) {
            return Ok(Some(json));
        }

        Ok(None)
    }

    fn extract_claude_error(&self, stderr: &str) -> Option<String> {
        // Extract meaningful error from Claude stderr
        for line in stderr.lines() {
            if line.contains("Error:") || line.contains("Failed:") {
                return Some(line.to_string());
            }
        }
        None
    }
}

/// Shell output formatter
pub struct ShellOutputFormatter;

#[async_trait]
impl OutputFormatter for ShellOutputFormatter {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        let mut formatted = output.clone();

        // Shell-specific output processing
        if let Some(ref stderr) = output.stderr {
            // Parse common shell error patterns
            formatted.error_summary = self.extract_error_summary(stderr);
        }

        Ok(formatted)
    }
}

impl ShellOutputFormatter {
    fn extract_error_summary(&self, stderr: &str) -> Option<String> {
        // Extract meaningful errors from shell stderr
        let error_patterns = [
            "command not found",
            "No such file or directory",
            "Permission denied",
            "fatal:",
            "error:",
            "Error:",
        ];

        for line in stderr.lines() {
            for pattern in &error_patterns {
                if line.contains(pattern) {
                    return Some(line.to_string());
                }
            }
        }

        // If no specific error pattern, return first non-empty line
        stderr
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(String::from)
    }
}

/// Test output formatter
pub struct TestOutputFormatter;

#[async_trait]
impl OutputFormatter for TestOutputFormatter {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        let mut formatted = output.clone();

        // Test-specific output processing
        if let Some(ref stdout) = output.stdout {
            // Extract test results
            formatted.structured_data = self.extract_test_results(stdout);
        }

        Ok(formatted)
    }
}

impl TestOutputFormatter {
    fn extract_test_results(&self, stdout: &str) -> Option<JsonValue> {
        // Look for common test output patterns
        let mut results = serde_json::Map::new();

        // Check for test summary patterns
        for line in stdout.lines() {
            if line.contains("tests passed") || line.contains("test result:") {
                results.insert("summary".to_string(), JsonValue::String(line.to_string()));
            }
            if line.contains("PASSED") {
                results.insert(
                    "status".to_string(),
                    JsonValue::String("passed".to_string()),
                );
            }
            if line.contains("FAILED") {
                results.insert(
                    "status".to_string(),
                    JsonValue::String("failed".to_string()),
                );
            }
        }

        if !results.is_empty() {
            Some(JsonValue::Object(results))
        } else {
            None
        }
    }
}

/// Handler output formatter
pub struct HandlerOutputFormatter;

#[async_trait]
impl OutputFormatter for HandlerOutputFormatter {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        // Handler output is typically minimal, just pass through
        Ok(output.clone())
    }
}

/// JSON output parser
pub struct JsonOutputParser;

#[async_trait]
impl OutputParser for JsonOutputParser {
    async fn parse(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        let mut parsed = output.clone();

        if let Some(ref stdout) = output.stdout {
            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<JsonValue>(stdout) {
                parsed.structured_data = Some(json);
            }
        }

        Ok(parsed)
    }
}

/// YAML output parser
pub struct YamlOutputParser;

#[async_trait]
impl OutputParser for YamlOutputParser {
    async fn parse(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        let mut parsed = output.clone();

        if let Some(ref stdout) = output.stdout {
            // Try to parse as YAML
            if let Ok(yaml_value) = serde_yaml::from_str::<serde_yaml::Value>(stdout) {
                // Convert YAML to JSON for unified handling
                if let Ok(json_str) = serde_json::to_string(&yaml_value) {
                    if let Ok(json) = serde_json::from_str(&json_str) {
                        parsed.structured_data = Some(json);
                    }
                }
            }
        }

        Ok(parsed)
    }
}

/// Plain text output parser
pub struct PlainTextOutputParser;

#[async_trait]
impl OutputParser for PlainTextOutputParser {
    async fn parse(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        // Plain text doesn't need parsing, just pass through
        Ok(output.clone())
    }
}
