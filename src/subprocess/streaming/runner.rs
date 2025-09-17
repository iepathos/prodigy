//! Streaming command runner implementation

use super::processor::StreamProcessor;
use super::types::{StreamSource, StreamingOutput};
use crate::subprocess::{ProcessCommand, ProcessError, ProcessRunner};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

/// Command runner with streaming support
pub struct StreamingCommandRunner {
    inner: Box<dyn ProcessRunner>,
}

impl StreamingCommandRunner {
    /// Create a new streaming command runner
    pub fn new(inner: Box<dyn ProcessRunner>) -> Self {
        Self { inner }
    }

    /// Run a command with streaming output processing
    pub async fn run_streaming(
        &self,
        command: ProcessCommand,
        processors: Vec<Box<dyn StreamProcessor>>,
    ) -> Result<StreamingOutput> {
        let start = Instant::now();

        // Build the tokio command
        let mut cmd = Command::new(&command.program);
        cmd.args(&command.args);

        // Set environment variables
        for (key, value) in &command.env {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(dir) = &command.working_dir {
            cmd.current_dir(dir);
        }

        // Configure stdio for streaming
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        if command.suppress_stderr {
            cmd.stderr(Stdio::null());
        } else {
            cmd.stderr(Stdio::piped());
        }

        // Spawn the process
        let mut child = cmd.spawn().context("Failed to spawn process")?;

        // Handle stdin if provided
        if let Some(stdin_data) = &command.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin
                    .write_all(stdin_data.as_bytes())
                    .await
                    .context("Failed to write to stdin")?;
                stdin.flush().await.context("Failed to flush stdin")?;
            }
        }

        // Take ownership of output streams
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;

        // Create shared processor references
        let processors = std::sync::Arc::new(processors);

        // Process streams in parallel
        let stdout_processors = processors.clone();
        let stderr_processors = processors.clone();

        let stdout_handle = tokio::spawn(async move {
            process_stream(stdout, StreamSource::Stdout, &*stdout_processors).await
        });

        let stderr_handle = tokio::spawn(async move {
            process_stream(stderr, StreamSource::Stderr, &*stderr_processors).await
        });

        // Apply timeout if specified
        let status = if let Some(timeout_duration) = command.timeout {
            match tokio::time::timeout(timeout_duration, child.wait()).await {
                Ok(Ok(status)) => status,
                Ok(Err(e)) => {
                    // Process wait error
                    let error = anyhow::Error::new(e);
                    for processor in processors.iter() {
                        let _ = processor.on_error(&error).await;
                    }
                    return Err(error);
                }
                Err(_) => {
                    // Timeout occurred
                    child
                        .kill()
                        .await
                        .context("Failed to kill timed out process")?;
                    let timeout_err =
                        anyhow::anyhow!("Process timed out after {:?}", timeout_duration);
                    for processor in processors.iter() {
                        let _ = processor.on_error(&timeout_err).await;
                    }
                    return Err(timeout_err);
                }
            }
        } else {
            child.wait().await.context("Failed to wait for process")?
        };

        // Wait for stream processing to complete
        let (stdout_lines, stderr_lines) = tokio::try_join!(stdout_handle, stderr_handle)?;
        let stdout_lines = stdout_lines?;
        let stderr_lines = stderr_lines?;

        // Notify processors of completion
        let exit_code = status.code();
        for processor in processors.iter() {
            processor.on_complete(exit_code).await?;
        }

        Ok(StreamingOutput {
            status,
            stdout: stdout_lines,
            stderr: stderr_lines,
            duration: start.elapsed(),
        })
    }

    /// Run a command without streaming (fallback to batch mode)
    pub async fn run_batch(
        &self,
        command: ProcessCommand,
    ) -> Result<crate::subprocess::ProcessOutput> {
        self.inner
            .run(command)
            .await
            .map_err(|e| anyhow::anyhow!("Process execution failed: {}", e))
    }
}

/// Process a stream line by line
async fn process_stream(
    stream: impl AsyncRead + Unpin,
    source: StreamSource,
    processors: &[Box<dyn StreamProcessor>],
) -> Result<Vec<String>> {
    let reader = BufReader::new(stream);
    let mut lines_reader = reader.lines();
    let mut output = Vec::new();

    while let Ok(Some(line)) = lines_reader.next_line().await {
        // Store for final output
        output.push(line.clone());

        // Process through all handlers
        for processor in processors {
            if let Err(e) = processor.process_line(&line, source).await {
                tracing::warn!("Processor failed to handle line from {:?}: {}", source, e);
                // Continue with other processors even if one fails
            }
        }
    }

    Ok(output)
}

/// Streaming runner that implements ProcessRunner trait
pub struct StreamingProcessRunner;

impl StreamingProcessRunner {
    /// Create a new streaming process runner
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProcessRunner for StreamingProcessRunner {
    async fn run(
        &self,
        command: ProcessCommand,
    ) -> Result<crate::subprocess::ProcessOutput, ProcessError> {
        // Create a streaming runner with the default process runner
        let runner =
            StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

        // Run with streaming with empty processors for now
        let processors: Vec<Box<dyn StreamProcessor>> = vec![];
        let result = runner
            .run_streaming(command, processors)
            .await
            .map_err(|e| ProcessError::Io(std::io::Error::other(e.to_string())))?;

        // Convert to ProcessOutput
        Ok(crate::subprocess::ProcessOutput {
            status: if result.status.success() {
                crate::subprocess::runner::ExitStatus::Success
            } else {
                crate::subprocess::runner::ExitStatus::Error(result.status.code().unwrap_or(-1))
            },
            stdout: result.stdout.join("\n"),
            stderr: result.stderr.join("\n"),
            duration: result.duration,
        })
    }

    async fn run_streaming(
        &self,
        command: ProcessCommand,
    ) -> Result<crate::subprocess::ProcessStream, ProcessError> {
        // This is already a streaming runner, delegate to inner implementation
        // For now, we'll use the default implementation
        let runner = crate::subprocess::runner::TokioProcessRunner;
        runner.run_streaming(command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::subprocess::streaming::processor::LoggingProcessor;

    #[tokio::test]
    async fn test_streaming_echo() {
        let runner =
            StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

        let processors: Vec<Box<dyn StreamProcessor>> =
            vec![Box::new(LoggingProcessor::new("test"))];

        let command = ProcessCommand {
            program: "echo".to_string(),
            args: vec!["hello world".to_string()],
            env: Default::default(),
            working_dir: None,
            timeout: None,
            stdin: None,
            suppress_stderr: false,
        };

        let result = runner.run_streaming(command, processors).await.unwrap();
        assert!(result.status.success());
        assert!(!result.stdout.is_empty());
        assert_eq!(result.stdout[0], "hello world");
    }

    #[tokio::test]
    async fn test_streaming_with_timeout() {
        let runner =
            StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

        let processors: Vec<Box<dyn StreamProcessor>> = vec![];

        let command = ProcessCommand {
            program: "sleep".to_string(),
            args: vec!["10".to_string()],
            env: Default::default(),
            working_dir: None,
            timeout: Some(Duration::from_millis(100)),
            stdin: None,
            suppress_stderr: false,
        };

        let result = runner.run_streaming(command, processors).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
