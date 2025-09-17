//! Command runner implementation

use super::{CommandExecutor, ExecutionContext, ExecutionResult};
use crate::abstractions::exit_status::ExitStatusExt;
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{Context, Result};
use async_trait::async_trait;

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

        let command = builder.build();

        // Check if streaming is enabled
        if let Some(streaming_config) = &context.streaming_config {
            if streaming_config.enabled {
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

                return Ok(ExecutionResult {
                    success: output.status.success(),
                    stdout: output.stdout.join("\n"),
                    stderr: output.stderr.join("\n"),
                    exit_code: output.status.code(),
                });
            }
        }

        // Fall back to batch mode
        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context(format!("Failed to execute command: {cmd}"))?;

        Ok(ExecutionResult {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code(),
        })
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
    }

    #[tokio::test]
    async fn test_mock_command_runner() {
        let mock = MockCommandRunner::new();
        mock.add_response(ExecutionResult {
            success: true,
            stdout: "mocked output".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let result = mock
            .run_with_context("test", &[], &ExecutionContext::default())
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout, "mocked output");
    }
}
