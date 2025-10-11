//! Command runner implementation

use super::{CommandExecutor, ExecutionContext, ExecutionResult};
use crate::abstractions::exit_status::ExitStatusExt;
use crate::subprocess::runner::ProcessOutput;
use crate::subprocess::streaming::StreamingOutput;
use crate::subprocess::{ProcessCommand, ProcessCommandBuilder, SubprocessManager};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;

/// Build a ProcessCommand from execution context
///
/// This is a pure function that constructs a command with all configuration
/// from the execution context (working directory, env vars, timeout, stdin).
fn build_command_from_context(
    cmd: &str,
    args: &[String],
    context: &ExecutionContext,
) -> ProcessCommand {
    let mut builder = ProcessCommandBuilder::new(cmd)
        .args(args)
        .current_dir(&context.working_directory);

    // Set environment variables
    for (key, value) in &context.env_vars {
        builder = builder.env(key, value);
    }

    // Set timeout if specified
    if let Some(timeout) = context.timeout_seconds {
        builder = builder.timeout(std::time::Duration::from_secs(timeout));
    }

    // Set stdin if specified
    if let Some(stdin) = &context.stdin {
        builder = builder.stdin(stdin.clone());
    }

    builder.build()
}

/// Transform streaming output to ExecutionResult
///
/// This is a pure function that converts StreamingOutput (with stdout/stderr as `Vec<String>`)
/// into ExecutionResult (with stdout/stderr as joined strings).
fn streaming_output_to_result(output: StreamingOutput) -> ExecutionResult {
    ExecutionResult {
        success: output.status.success(),
        stdout: output.stdout.join("\n"),
        stderr: output.stderr.join("\n"),
        exit_code: output.status.code(),
        metadata: HashMap::new(),
    }
}

/// Transform batch output to ExecutionResult
///
/// This is a pure function that converts ProcessOutput (with stdout/stderr as String)
/// into ExecutionResult with the same string format.
fn batch_output_to_result(output: ProcessOutput) -> ExecutionResult {
    ExecutionResult {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.status.code(),
        metadata: HashMap::new(),
    }
}

/// Determine if streaming mode should be used
///
/// This is a pure function that checks the execution context to determine
/// whether to use streaming or batch mode for command execution.
fn should_use_streaming(context: &ExecutionContext) -> bool {
    context
        .streaming_config
        .as_ref()
        .map(|config| config.enabled)
        .unwrap_or(false)
}

/// Trait for running system commands
#[async_trait]
pub trait CommandRunner: Send + Sync {
    /// Run a command and return output
    async fn run_command(&self, cmd: &str, args: &[String]) -> Result<std::process::Output>;

    /// Run a command with full control
    async fn run_with_context(
        &self,
        cmd: &str,
        args: &[String],
        context: &ExecutionContext,
    ) -> Result<ExecutionResult>;

    /// Run a command with streaming output processing
    /// Default implementation falls back to buffered execution
    async fn run_with_streaming(
        &self,
        cmd: &str,
        args: &[String],
        context: &ExecutionContext,
        _output_handler: Box<dyn crate::subprocess::streaming::StreamProcessor>,
    ) -> Result<ExecutionResult> {
        // Default implementation falls back to buffered execution
        self.run_with_context(cmd, args, context).await
    }
}

/// Real implementation of command runner
pub struct RealCommandRunner {
    subprocess: SubprocessManager,
}

impl RealCommandRunner {
    /// Create a new command runner
    pub fn new() -> Self {
        Self {
            subprocess: SubprocessManager::production(),
        }
    }

    /// Create a new instance with custom subprocess manager (for testing)
    #[cfg(test)]
    pub fn with_subprocess(subprocess: SubprocessManager) -> Self {
        Self { subprocess }
    }

    /// Create stream processors from configuration
    fn create_processors(
        &self,
        config: &crate::subprocess::streaming::StreamingConfig,
    ) -> Result<Vec<Box<dyn crate::subprocess::streaming::StreamProcessor>>> {
        use crate::subprocess::streaming::{
            JsonLineProcessor, PatternMatchProcessor, ProcessorConfig, StreamProcessor,
        };

        let mut processors: Vec<Box<dyn StreamProcessor>> = Vec::new();

        for processor_config in &config.processors {
            match processor_config {
                ProcessorConfig::JsonLines { emit_events } => {
                    let (sender, _receiver) = tokio::sync::mpsc::channel(100);
                    processors.push(Box::new(JsonLineProcessor::new(sender, *emit_events)));
                }
                ProcessorConfig::PatternMatcher { patterns } => {
                    let (sender, _receiver) = tokio::sync::mpsc::channel(100);
                    processors.push(Box::new(PatternMatchProcessor::new(
                        patterns.clone(),
                        sender,
                    )));
                }
                ProcessorConfig::EventEmitter { .. } => {
                    // TODO: Implement event emitter when event system is ready
                    tracing::debug!("EventEmitter processor not yet implemented");
                }
                ProcessorConfig::Custom { id } => {
                    tracing::debug!("Custom processor '{}' not available in this context", id);
                }
            }
        }

        // Apply backpressure management if configured
        if let Some(max_lines) = config.buffer_config.max_lines {
            use crate::subprocess::streaming::BufferedStreamProcessor;

            processors = processors
                .into_iter()
                .map(|processor| {
                    Box::new(BufferedStreamProcessor::new(
                        processor,
                        max_lines,
                        config.buffer_config.overflow_strategy.clone(),
                        config.buffer_config.block_timeout,
                    )) as Box<dyn StreamProcessor>
                })
                .collect();
        }

        Ok(processors)
    }
}

impl Default for RealCommandRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandRunner for RealCommandRunner {
    async fn run_command(&self, cmd: &str, args: &[String]) -> Result<std::process::Output> {
        let command = ProcessCommandBuilder::new(cmd).args(args).build();

        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context(format!("Failed to execute command: {cmd}"))?;

        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(output.status.code().unwrap_or(1)),
            stdout: output.stdout.into_bytes(),
            stderr: output.stderr.into_bytes(),
        })
    }

    async fn run_with_context(
        &self,
        cmd: &str,
        args: &[String],
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let command = build_command_from_context(cmd, args, context);

        // Check if streaming is enabled
        if should_use_streaming(context) {
            if let Some(streaming_config) = &context.streaming_config {
                // Use streaming runner with the subprocess manager's runner
                let processors = self.create_processors(streaming_config)?;

                // Create streaming runner using TokioProcessRunner directly for now
                let streaming_runner = crate::subprocess::streaming::StreamingCommandRunner::new(
                    Box::new(crate::subprocess::runner::TokioProcessRunner),
                );

                let output = streaming_runner
                    .run_streaming(command, processors)
                    .await
                    .context(format!("Failed to execute command with streaming: {cmd}"))?;

                return Ok(streaming_output_to_result(output));
            }
        }

        // Fall back to batch mode
        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context(format!("Failed to execute command: {cmd}"))?;

        Ok(batch_output_to_result(output))
    }

    async fn run_with_streaming(
        &self,
        cmd: &str,
        args: &[String],
        context: &ExecutionContext,
        output_handler: Box<dyn crate::subprocess::streaming::StreamProcessor>,
    ) -> Result<ExecutionResult> {
        let command = build_command_from_context(cmd, args, context);

        // Create streaming runner
        let streaming_runner = crate::subprocess::streaming::StreamingCommandRunner::new(Box::new(
            crate::subprocess::runner::TokioProcessRunner,
        ));

        // Run with streaming, passing the single output handler
        let output = streaming_runner
            .run_streaming(command, vec![output_handler])
            .await
            .context(format!("Failed to execute command with streaming: {cmd}"))?;

        Ok(streaming_output_to_result(output))
    }
}

#[async_trait]
impl CommandExecutor for RealCommandRunner {
    async fn execute(
        &self,
        command: &str,
        args: &[String],
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        self.run_with_context(command, args, &context).await
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_build_command_basic() {
        let context = ExecutionContext::default();
        let command = build_command_from_context("echo", &["hello".to_string()], &context);

        assert_eq!(command.program, "echo");
        assert_eq!(command.args, vec!["hello"]);
    }

    #[test]
    fn test_build_command_with_env_vars() {
        let mut context = ExecutionContext::default();
        context
            .env_vars
            .insert("TEST_VAR".to_string(), "test_value".to_string());
        context
            .env_vars
            .insert("ANOTHER_VAR".to_string(), "another_value".to_string());

        let command = build_command_from_context("test", &[], &context);

        assert_eq!(command.env.get("TEST_VAR"), Some(&"test_value".to_string()));
        assert_eq!(
            command.env.get("ANOTHER_VAR"),
            Some(&"another_value".to_string())
        );
    }

    #[test]
    fn test_build_command_with_timeout() {
        let context = ExecutionContext {
            timeout_seconds: Some(60),
            ..Default::default()
        };

        let command = build_command_from_context("sleep", &["10".to_string()], &context);

        assert_eq!(command.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_build_command_with_stdin() {
        let context = ExecutionContext {
            stdin: Some("input data".to_string()),
            ..Default::default()
        };

        let command = build_command_from_context("cat", &[], &context);

        assert_eq!(command.stdin, Some("input data".to_string()));
    }

    #[test]
    fn test_streaming_output_to_result_success() {
        use crate::subprocess::streaming::StreamingOutput;
        use std::time::Duration;

        let output = StreamingOutput {
            status: std::process::ExitStatus::from_raw(0),
            stdout: vec!["line1".to_string(), "line2".to_string()],
            stderr: vec!["error1".to_string()],
            duration: Duration::from_secs(1),
        };

        let result = streaming_output_to_result(output);

        assert!(result.success);
        assert_eq!(result.stdout, "line1\nline2");
        assert_eq!(result.stderr, "error1");
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_streaming_output_to_result_failure() {
        use crate::subprocess::streaming::StreamingOutput;
        use std::time::Duration;

        let output = StreamingOutput {
            status: std::process::ExitStatus::from_raw(256), // Exit code 1 is encoded as 256 on Unix
            stdout: vec![],
            stderr: vec!["error message".to_string()],
            duration: Duration::from_secs(1),
        };

        let result = streaming_output_to_result(output);

        assert!(!result.success);
        assert_eq!(result.stdout, "");
        assert_eq!(result.stderr, "error message");
        assert_eq!(result.exit_code, Some(1));
    }

    #[test]
    fn test_batch_output_to_result_success() {
        use crate::subprocess::runner::{ExitStatus, ProcessOutput};
        use std::time::Duration;

        let output = ProcessOutput {
            status: ExitStatus::Success,
            stdout: "output text".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(1),
        };

        let result = batch_output_to_result(output);

        assert!(result.success);
        assert_eq!(result.stdout, "output text");
        assert_eq!(result.stderr, "");
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_batch_output_to_result_failure() {
        use crate::subprocess::runner::{ExitStatus, ProcessOutput};
        use std::time::Duration;

        let output = ProcessOutput {
            status: ExitStatus::Error(42),
            stdout: String::new(),
            stderr: "error output".to_string(),
            duration: Duration::from_secs(1),
        };

        let result = batch_output_to_result(output);

        assert!(!result.success);
        assert_eq!(result.stdout, "");
        assert_eq!(result.stderr, "error output");
        assert_eq!(result.exit_code, Some(42));
    }

    #[test]
    fn test_should_use_streaming_no_config() {
        let context = ExecutionContext::default();
        assert!(!should_use_streaming(&context));
    }

    #[test]
    fn test_should_use_streaming_disabled() {
        use crate::subprocess::streaming::{BufferConfig, StreamingConfig, StreamingMode};

        let context = ExecutionContext {
            streaming_config: Some(StreamingConfig {
                enabled: false,
                mode: StreamingMode::Streaming,
                processors: vec![],
                buffer_config: BufferConfig::default(),
            }),
            ..Default::default()
        };

        assert!(!should_use_streaming(&context));
    }

    #[test]
    fn test_should_use_streaming_enabled() {
        use crate::subprocess::streaming::{BufferConfig, StreamingConfig, StreamingMode};

        let context = ExecutionContext {
            streaming_config: Some(StreamingConfig {
                enabled: true,
                mode: StreamingMode::Streaming,
                processors: vec![],
                buffer_config: BufferConfig::default(),
            }),
            ..Default::default()
        };

        assert!(should_use_streaming(&context));
    }

    #[test]
    fn test_build_command_with_all_options() {
        let mut context = ExecutionContext::default();
        context
            .env_vars
            .insert("VAR1".to_string(), "value1".to_string());
        context.timeout_seconds = Some(30);
        context.stdin = Some("test input".to_string());

        let command = build_command_from_context(
            "sh",
            &["-c".to_string(), "echo $VAR1".to_string()],
            &context,
        );

        assert_eq!(command.program, "sh");
        assert_eq!(command.args, vec!["-c", "echo $VAR1"]);
        assert_eq!(command.env.get("VAR1"), Some(&"value1".to_string()));
        assert_eq!(command.timeout, Some(Duration::from_secs(30)));
        assert_eq!(command.stdin, Some("test input".to_string()));
    }

    #[tokio::test]
    async fn test_real_command_runner() {
        let runner = RealCommandRunner::new();

        // Test simple echo command
        let result = runner
            .run_command("echo", &["hello".to_string()])
            .await
            .unwrap();
        assert!(result.status.success());
        assert!(String::from_utf8_lossy(&result.stdout).contains("hello"));
    }

    #[tokio::test]
    async fn test_command_with_context() {
        let runner = RealCommandRunner::new();
        let mut context = ExecutionContext::default();
        context
            .env_vars
            .insert("TEST_VAR".to_string(), "test_value".to_string());

        // Test with environment variable
        let result = runner
            .run_with_context(
                "sh",
                &["-c".to_string(), "echo $TEST_VAR".to_string()],
                &context,
            )
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("test_value"));
    }

    // Mock implementation for testing
    pub struct MockCommandRunner {
        responses: std::sync::Mutex<Vec<ExecutionResult>>,
    }

    impl Default for MockCommandRunner {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockCommandRunner {
        pub fn new() -> Self {
            Self {
                responses: std::sync::Mutex::new(Vec::new()),
            }
        }

        pub fn add_response(&self, result: ExecutionResult) {
            self.responses.lock().unwrap().push(result);
        }
    }

    #[async_trait]
    impl CommandRunner for MockCommandRunner {
        async fn run_command(&self, _cmd: &str, _args: &[String]) -> Result<std::process::Output> {
            let mut responses = self.responses.lock().unwrap();
            if let Some(result) = responses.pop() {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(if result.success { 0 } else { 1 }),
                    stdout: result.stdout.into_bytes(),
                    stderr: result.stderr.into_bytes(),
                })
            } else {
                anyhow::bail!("No mock response configured")
            }
        }

        async fn run_with_context(
            &self,
            _cmd: &str,
            _args: &[String],
            _context: &ExecutionContext,
        ) -> Result<ExecutionResult> {
            let mut responses = self.responses.lock().unwrap();
            responses
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
        }

        async fn run_with_streaming(
            &self,
            _cmd: &str,
            _args: &[String],
            _context: &ExecutionContext,
            output_handler: Box<dyn crate::subprocess::streaming::StreamProcessor>,
        ) -> Result<ExecutionResult> {
            // For testing, simulate streaming by sending lines to the processor
            let result = {
                let mut responses = self.responses.lock().unwrap();
                responses.pop()
            };

            if let Some(result) = result {
                // Simulate line-by-line processing
                for line in result.stdout.lines() {
                    let _ = output_handler
                        .process_line(line, crate::subprocess::streaming::StreamSource::Stdout)
                        .await;
                }
                for line in result.stderr.lines() {
                    let _ = output_handler
                        .process_line(line, crate::subprocess::streaming::StreamSource::Stderr)
                        .await;
                }
                let _ = output_handler.on_complete(result.exit_code).await;
                Ok(result)
            } else {
                anyhow::bail!("No mock response configured")
            }
        }
    }

    #[tokio::test]
    async fn test_mock_command_runner() {
        let mock = MockCommandRunner::new();
        mock.add_response(ExecutionResult {
            success: true,
            stdout: "mocked output".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        });

        let result = mock
            .run_with_context("test", &[], &ExecutionContext::default())
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout, "mocked output");
    }
}
