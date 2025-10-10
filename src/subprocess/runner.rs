use async_trait::async_trait;
use futures::stream::Stream;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use super::error::ProcessError;

#[derive(Debug, Clone)]
pub struct ProcessCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub timeout: Option<Duration>,
    pub stdin: Option<String>,
    pub suppress_stderr: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitStatus {
    Success,
    Error(i32),
    Timeout,
    Signal(i32),
}

impl ExitStatus {
    pub fn success(&self) -> bool {
        matches!(self, ExitStatus::Success)
    }

    pub fn code(&self) -> Option<i32> {
        match self {
            ExitStatus::Success => Some(0),
            ExitStatus::Error(code) => Some(*code),
            _ => None,
        }
    }
}

/// ExitStatusHelper for creating exit statuses
pub struct ExitStatusHelper;

impl ExitStatusHelper {
    /// Create a success exit status
    pub fn success() -> ExitStatus {
        ExitStatus::Success
    }

    /// Create a failure exit status with code
    pub fn failure(code: i32) -> ExitStatus {
        ExitStatus::Error(code)
    }
}

pub type ProcessStreamItem = Result<String, ProcessError>;
pub type ProcessStreamFut = Pin<Box<dyn Stream<Item = ProcessStreamItem> + Send>>;

pub struct ProcessStream {
    pub stdout: ProcessStreamFut,
    pub stderr: ProcessStreamFut,
    pub status: Pin<Box<dyn futures::Future<Output = Result<ExitStatus, ProcessError>> + Send>>,
}

#[async_trait]
pub trait ProcessRunner: Send + Sync {
    async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput, ProcessError>;
    async fn run_streaming(&self, command: ProcessCommand) -> Result<ProcessStream, ProcessError>;
}

pub struct TokioProcessRunner;

impl TokioProcessRunner {
    /// Normalize a line by removing trailing newlines
    fn normalize_line(mut line: String) -> String {
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        line
    }

    /// Create a line stream from a buffered reader
    fn create_line_stream<R>(reader: tokio::io::BufReader<R>) -> ProcessStreamFut
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        use tokio::io::AsyncBufReadExt;

        Box::pin(futures::stream::unfold(reader, |mut reader| async move {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => None, // EOF
                Ok(_) => {
                    let normalized = Self::normalize_line(line);
                    Some((Ok(normalized), reader))
                }
                Err(e) => Some((
                    Err(ProcessError::IoError {
                        command: String::new(),
                        source: e,
                    }),
                    reader,
                )),
            }
        })) as ProcessStreamFut
    }

    /// Convert a std ExitStatus to our ExitStatus enum
    fn convert_exit_status(status: std::process::ExitStatus) -> ExitStatus {
        if status.success() {
            ExitStatus::Success
        } else {
            ExitStatus::Error(status.code().unwrap_or(-1))
        }
    }

    /// Create a status future with optional timeout
    fn create_status_future(
        mut child: tokio::process::Child,
        timeout: Option<Duration>,
        program: String,
        args: Vec<String>,
    ) -> Pin<Box<dyn futures::Future<Output = Result<ExitStatus, ProcessError>> + Send>> {
        Box::pin(async move {
            let status = if let Some(timeout_duration) = timeout {
                match tokio::time::timeout(timeout_duration, child.wait()).await {
                    Ok(Ok(status)) => Self::convert_exit_status(status),
                    Ok(Err(e)) => {
                        return Err(ProcessError::IoError {
                            command: format!("{} {}", program, args.join(" ")),
                            source: e,
                        })
                    }
                    Err(_) => ExitStatus::Timeout,
                }
            } else {
                match child.wait().await {
                    Ok(status) => Self::convert_exit_status(status),
                    Err(e) => {
                        return Err(ProcessError::IoError {
                            command: format!("{} {}", program, args.join(" ")),
                            source: e,
                        })
                    }
                }
            };

            Ok(status)
        })
    }

    /// Log command execution details
    fn log_command_start(command: &ProcessCommand) {
        tracing::debug!(
            "Executing subprocess: {} {}",
            command.program,
            command.args.join(" ")
        );

        if !command.env.is_empty() {
            tracing::warn!(
                "Environment variables count: {}, total size: {} bytes",
                command.env.len(),
                command
                    .env
                    .iter()
                    .map(|(k, v)| k.len() + v.len() + 2)
                    .sum::<usize>()
            );
            tracing::trace!("Environment variables: {:?}", command.env);
        }

        // Log argument sizes to help debug E2BIG errors
        let args_size: usize = command.args.iter().map(|s| s.len()).sum();
        if args_size > 10000 {
            tracing::warn!(
                "Large arguments detected: {} args, {} total bytes",
                command.args.len(),
                args_size
            );
            for (i, arg) in command.args.iter().enumerate() {
                if arg.len() > 1000 {
                    tracing::warn!("  arg[{}]: {} bytes", i, arg.len());
                }
            }
        }

        if let Some(ref dir) = command.working_dir {
            tracing::trace!("Working directory: {:?}", dir);
        }

        if let Some(ref stdin) = command.stdin {
            tracing::trace!("Stdin provided: {} bytes", stdin.len());
        }
    }

    /// Configure the command with environment and working directory
    fn configure_command(command: &ProcessCommand) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&command.program);

        // Set up process group for proper signal handling on Unix
        #[cfg(unix)]
        {
            // Create new process group to ensure all child processes are terminated together
            cmd.process_group(0);
        }

        cmd.args(&command.args);

        // Clear inherited environment to prevent "Argument list too long" errors
        // when parent process has accumulated many environment variables from MapReduce
        cmd.env_clear();

        // Preserve essential system environment variables that commands depend on
        Self::preserve_essential_env(&mut cmd);

        // Add explicitly specified environment variables (these take precedence)
        for (key, value) in &command.env {
            cmd.env(key, value);
        }

        if let Some(dir) = &command.working_dir {
            cmd.current_dir(dir);
        }

        Self::configure_stdio(&mut cmd, command);
        cmd
    }

    /// Preserve essential system environment variables
    fn preserve_essential_env(cmd: &mut tokio::process::Command) {
        // Essential variables that most commands need to function
        let essential_vars = [
            "PATH", "HOME", "USER", "SHELL", "LANG", "LC_ALL", "LC_CTYPE", "TMPDIR", "TERM",
        ];

        for var in &essential_vars {
            if let Ok(value) = std::env::var(var) {
                cmd.env(var, value);
            }
        }
    }

    /// Configure stdio pipes for the process
    fn configure_stdio(cmd: &mut tokio::process::Command, command: &ProcessCommand) {
        if command.stdin.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        }

        cmd.stdout(std::process::Stdio::piped());

        if command.suppress_stderr {
            cmd.stderr(std::process::Stdio::null());
        } else {
            cmd.stderr(std::process::Stdio::piped());
        }
    }

    /// Write stdin data to the child process
    async fn write_stdin(
        child: &mut tokio::process::Child,
        stdin_data: &str,
    ) -> Result<(), ProcessError> {
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(stdin_data.as_bytes())
                .await
                .map_err(ProcessError::Io)?;
            stdin.shutdown().await.map_err(ProcessError::Io)?;
        }
        Ok(())
    }

    /// Wait for process with optional timeout
    async fn wait_with_timeout(
        child: tokio::process::Child,
        timeout: Option<std::time::Duration>,
    ) -> Result<std::process::Output, ProcessError> {
        match timeout {
            Some(duration) => {
                match tokio::time::timeout(duration, child.wait_with_output()).await {
                    Ok(result) => result.map_err(ProcessError::Io),
                    Err(_) => Err(ProcessError::Timeout(duration)),
                }
            }
            None => child.wait_with_output().await.map_err(ProcessError::Io),
        }
    }

    /// Convert process exit status to our ExitStatus enum
    fn parse_exit_status(status: std::process::ExitStatus) -> ExitStatus {
        if status.success() {
            ExitStatus::Success
        } else if let Some(code) = status.code() {
            ExitStatus::Error(code)
        } else {
            Self::parse_signal_status(status)
        }
    }

    /// Parse signal status on Unix systems
    #[cfg(unix)]
    fn parse_signal_status(status: std::process::ExitStatus) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            ExitStatus::Signal(signal)
        } else {
            ExitStatus::Error(1)
        }
    }

    #[cfg(not(unix))]
    fn parse_signal_status(_status: std::process::ExitStatus) -> ExitStatus {
        ExitStatus::Error(1)
    }

    /// Build ProcessOutput from command output
    fn build_output(
        output: std::process::Output,
        command: &ProcessCommand,
        status: ExitStatus,
        duration: std::time::Duration,
    ) -> ProcessOutput {
        ProcessOutput {
            status,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: if command.suppress_stderr {
                String::new()
            } else {
                String::from_utf8_lossy(&output.stderr).to_string()
            },
            duration,
        }
    }

    /// Log the process execution result
    fn log_result(result: &ProcessOutput, command: &ProcessCommand) {
        let command_str = format!("{} {}", command.program, command.args.join(" "));

        match &result.status {
            ExitStatus::Success => {
                tracing::debug!(
                    "Subprocess completed successfully in {:?}: {}",
                    result.duration,
                    command_str
                );
                tracing::trace!("Stdout length: {} bytes", result.stdout.len());
                tracing::trace!("Stderr length: {} bytes", result.stderr.len());
            }
            ExitStatus::Error(code) => {
                tracing::debug!(
                    "Subprocess failed with exit code {} in {:?}: {}",
                    code,
                    result.duration,
                    command_str
                );
                if !result.stderr.is_empty() {
                    tracing::trace!("Stderr: {}", result.stderr);
                }
            }
            ExitStatus::Signal(signal) => {
                tracing::warn!(
                    "Subprocess terminated by signal {} in {:?}: {}",
                    signal,
                    result.duration,
                    command_str
                );
            }
            ExitStatus::Timeout => {
                tracing::warn!(
                    "Subprocess timed out after {:?}: {}",
                    result.duration,
                    command_str
                );
            }
        }
    }

    /// Map spawn error to ProcessError
    fn map_spawn_error(error: std::io::Error, program: &str) -> ProcessError {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProcessError::CommandNotFound(program.to_string())
        } else {
            ProcessError::Io(error)
        }
    }
}

#[async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput, ProcessError> {
        let start = std::time::Instant::now();

        // Log command details
        Self::log_command_start(&command);

        // Configure and spawn the process
        let mut cmd = Self::configure_command(&command);
        let mut child = cmd
            .spawn()
            .map_err(|e| Self::map_spawn_error(e, &command.program))?;

        // Write stdin if provided
        if let Some(stdin_data) = &command.stdin {
            Self::write_stdin(&mut child, stdin_data).await?;
        }

        // Wait for process completion with optional timeout
        let output = Self::wait_with_timeout(child, command.timeout).await?;

        let duration = start.elapsed();
        let status = Self::parse_exit_status(output.status);
        let result = Self::build_output(output, &command, status.clone(), duration);

        // Log the result
        Self::log_result(&result, &command);

        Ok(result)
    }

    async fn run_streaming(&self, command: ProcessCommand) -> Result<ProcessStream, ProcessError> {
        use tokio::io::BufReader;

        // Log command execution
        Self::log_command_start(&command);

        // Configure and spawn the process
        let mut cmd = Self::configure_command(&command);
        let mut child = cmd.spawn().map_err(|e| ProcessError::SpawnFailed {
            command: format!("{} {}", command.program, command.args.join(" ")),
            source: e.into(),
        })?;

        // Write stdin if provided
        if let Some(stdin_data) = &command.stdin {
            Self::write_stdin(&mut child, stdin_data).await?;
        }

        // Take ownership of output streams
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProcessError::InternalError {
                message: "Failed to capture stdout".to_string(),
            })?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ProcessError::InternalError {
                message: "Failed to capture stderr".to_string(),
            })?;

        // Create stdout stream
        let stdout_stream = Self::create_line_stream(BufReader::new(stdout));

        // Create stderr stream
        let stderr_stream = Self::create_line_stream(BufReader::new(stderr));

        // Create status future
        let status_fut = Self::create_status_future(
            child,
            command.timeout,
            command.program.clone(),
            command.args.clone(),
        );

        Ok(ProcessStream {
            stdout: stdout_stream,
            stderr: stderr_stream,
            status: status_fut,
        })
    }
}
