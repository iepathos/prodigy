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
            tracing::debug!(
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
    fn configure_command(
        command: &ProcessCommand,
    ) -> Result<tokio::process::Command, ProcessError> {
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
        // This MUST succeed - if PATH is missing, fail loudly rather than spawn with no PATH
        Self::preserve_essential_env(&mut cmd, &command.program)?;

        // Add explicitly specified environment variables (these take precedence)
        for (key, value) in &command.env {
            cmd.env(key, value);
        }

        if let Some(dir) = &command.working_dir {
            cmd.current_dir(dir);
        }

        Self::configure_stdio(&mut cmd, command);
        Ok(cmd)
    }

    /// Preserve essential system environment variables
    /// Returns an error if critical environment variables (like PATH) are missing
    fn preserve_essential_env(
        cmd: &mut tokio::process::Command,
        program: &str,
    ) -> Result<(), ProcessError> {
        // PATH is absolutely critical - without it, no commands can be found
        let critical_vars = ["PATH"];

        // Other important but not critical variables
        let optional_vars = ["HOME", "USER", "SHELL", "TMPDIR", "TERM"];

        let mut preserved = Vec::new();
        let mut optional_failed = Vec::new();

        // Critical variables MUST exist - fail loudly if missing
        for var in &critical_vars {
            match std::env::var(var) {
                Ok(value) => {
                    cmd.env(var, &value);
                    preserved.push(var.to_string());
                    tracing::debug!(
                        "Preserved critical env var {} for command '{}': {}",
                        var,
                        program,
                        value
                    );
                }
                Err(e) => {
                    // Log current process environment state for debugging
                    tracing::error!(
                        "❌ CRITICAL: Required environment variable {} is not available for command '{}': {:?}",
                        var,
                        program,
                        e
                    );

                    // Log all available env vars to help diagnose the issue
                    let available_vars: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
                    tracing::error!(
                        "Available environment variables in parent process ({}): {}",
                        available_vars.len(),
                        available_vars.join(", ")
                    );

                    return Err(ProcessError::InternalError {
                        message: format!(
                            "Critical environment variable {} is not available (required for '{}' command). \
                             This indicates environment corruption after long-running execution. \
                             Error: {:?}",
                            var, program, e
                        ),
                    });
                }
            }
        }

        // Optional variables can fail silently but we log warnings
        for var in &optional_vars {
            match std::env::var(var) {
                Ok(value) => {
                    cmd.env(var, value);
                    preserved.push(var.to_string());
                }
                Err(_) => {
                    optional_failed.push(var.to_string());
                }
            }
        }

        // Handle locale variables with intelligent fallbacks
        Self::preserve_locale_env(cmd, &mut preserved);

        if !optional_failed.is_empty() {
            tracing::debug!(
                "Optional env vars not available for '{}': {}",
                program,
                optional_failed.join(", ")
            );
        }

        tracing::trace!(
            "Preserved {} env vars for '{}': {}",
            preserved.len(),
            program,
            preserved.join(", ")
        );

        Ok(())
    }

    /// Preserve locale environment variables with intelligent fallbacks
    fn preserve_locale_env(cmd: &mut tokio::process::Command, preserved: &mut Vec<String>) {
        // Try to get locale in order of preference: LC_ALL > LC_CTYPE > LANG > default
        let locale = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_else(|_| "en_US.UTF-8".to_string());

        // Set LANG if not already set (provides baseline locale)
        if std::env::var("LANG").is_ok() {
            if let Ok(value) = std::env::var("LANG") {
                cmd.env("LANG", value);
                preserved.push("LANG".to_string());
            }
        } else {
            cmd.env("LANG", &locale);
            preserved.push("LANG (default)".to_string());
            tracing::trace!("Using default LANG: {}", locale);
        }

        // Set LC_ALL if present, otherwise let it inherit from LANG
        if let Ok(value) = std::env::var("LC_ALL") {
            cmd.env("LC_ALL", value);
            preserved.push("LC_ALL".to_string());
        }

        // Set LC_CTYPE if present, otherwise let it inherit from LANG
        if let Ok(value) = std::env::var("LC_CTYPE") {
            cmd.env("LC_CTYPE", value);
            preserved.push("LC_CTYPE".to_string());
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

    /// Spawn and configure a process with optional stdin
    async fn spawn_configured_process(
        command: &ProcessCommand,
    ) -> Result<tokio::process::Child, ProcessError> {
        // Configure the process - this may fail if critical env vars like PATH are missing
        let mut cmd = Self::configure_command(command)?;

        // Log command execution for debugging
        if command.program == "claude" {
            tracing::debug!(
                "Spawning claude command: {} (working_dir: {:?})",
                command.args.join(" "),
                command.working_dir
            );
        }

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            // Enhanced error logging
            tracing::error!(
                "Failed to spawn '{}': {:?} (kind: {:?})",
                command.program,
                e,
                e.kind()
            );

            // If NotFound, log current PATH from parent for debugging
            if e.kind() == std::io::ErrorKind::NotFound {
                if let Ok(path) = std::env::var("PATH") {
                    tracing::error!(
                        "Command '{}' not found. Parent process PATH: {}",
                        command.program,
                        path
                    );
                } else {
                    tracing::error!(
                        "❌ CRITICAL: Command '{}' not found AND parent process has no PATH! \
                         This indicates severe environment corruption.",
                        command.program
                    );
                }
            }

            ProcessError::SpawnFailed {
                command: format!("{} {}", command.program, command.args.join(" ")),
                source: e.into(),
            }
        })?;

        // Write stdin if provided
        if let Some(stdin_data) = &command.stdin {
            Self::write_stdin(&mut child, stdin_data).await?;
        }

        Ok(child)
    }

    /// Extract a stream from a child process, converting None to error
    fn extract_stream<T>(stream: Option<T>, stream_name: &str) -> Result<T, ProcessError> {
        stream.ok_or_else(|| ProcessError::InternalError {
            message: format!("Failed to capture {}", stream_name),
        })
    }

    /// Extract and create output streams from a child process
    fn create_output_streams(
        child: &mut tokio::process::Child,
    ) -> Result<(ProcessStreamFut, ProcessStreamFut), ProcessError> {
        use tokio::io::BufReader;

        // Take ownership of output streams with simplified error handling
        let stdout = Self::extract_stream(child.stdout.take(), "stdout")?;
        let stderr = Self::extract_stream(child.stderr.take(), "stderr")?;

        // Create stdout and stderr streams
        let stdout_stream = Self::create_line_stream(BufReader::new(stdout));
        let stderr_stream = Self::create_line_stream(BufReader::new(stderr));

        Ok((stdout_stream, stderr_stream))
    }
}

#[async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput, ProcessError> {
        let start = std::time::Instant::now();

        // Log command details
        Self::log_command_start(&command);

        // Configure and spawn the process
        let mut cmd = Self::configure_command(&command)?;
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
        // Log command execution
        Self::log_command_start(&command);

        // Spawn and configure process with stdin
        let mut child = Self::spawn_configured_process(&command).await?;

        // Extract and create output streams
        let (stdout_stream, stderr_stream) = Self::create_output_streams(&mut child)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    /// Test helper to create a basic ProcessCommand for testing
    fn test_command() -> ProcessCommand {
        ProcessCommand {
            program: "echo".to_string(),
            args: vec!["test".to_string()],
            env: HashMap::new(),
            working_dir: None,
            timeout: None,
            stdin: None,
            suppress_stderr: false,
        }
    }

    #[test]
    fn test_extract_stream_with_some() {
        let value = Some(42);
        let result = TokioProcessRunner::extract_stream(value, "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_extract_stream_with_none() {
        let value: Option<i32> = None;
        let result = TokioProcessRunner::extract_stream(value, "test_stream");
        assert!(result.is_err());
        match result.unwrap_err() {
            ProcessError::InternalError { message } => {
                assert_eq!(message, "Failed to capture test_stream");
            }
            _ => panic!("Expected InternalError"),
        }
    }

    #[tokio::test]
    async fn test_spawn_configured_process() {
        let mut command = test_command();
        command.program = "sh".to_string();
        command.args = vec!["-c".to_string(), "echo hello".to_string()];

        let child = TokioProcessRunner::spawn_configured_process(&command).await;
        assert!(child.is_ok());

        let mut child = child.unwrap();
        let status = child.wait().await;
        assert!(status.is_ok());
        assert!(status.unwrap().success());
    }

    #[tokio::test]
    async fn test_spawn_configured_process_with_stdin() {
        let mut command = test_command();
        command.program = "sh".to_string();
        command.args = vec!["-c".to_string(), "cat".to_string()];
        command.stdin = Some("test input".to_string());

        let child = TokioProcessRunner::spawn_configured_process(&command).await;
        assert!(child.is_ok());

        let child = child.unwrap();
        let output = child.wait_with_output().await;
        assert!(output.is_ok());

        let output = output.unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "test input");
    }

    #[tokio::test]
    async fn test_spawn_configured_process_nonexistent() {
        let mut command = test_command();
        command.program = "nonexistent_command_12345".to_string();

        let result = TokioProcessRunner::spawn_configured_process(&command).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ProcessError::SpawnFailed { command, .. } => {
                assert!(command.contains("nonexistent_command_12345"));
            }
            _ => panic!("Expected SpawnFailed error"),
        }
    }

    #[tokio::test]
    async fn test_create_output_streams() {
        use tokio::process::Command;

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo test");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().unwrap();

        let result = TokioProcessRunner::create_output_streams(&mut child);
        assert!(result.is_ok());

        let (_stdout_stream, _stderr_stream) = result.unwrap();
        // Streams were successfully created

        // Clean up
        let _ = child.wait().await;
    }

    #[test]
    fn test_normalize_line() {
        assert_eq!(
            TokioProcessRunner::normalize_line("test\n".to_string()),
            "test"
        );
        assert_eq!(
            TokioProcessRunner::normalize_line("test\r\n".to_string()),
            "test"
        );
        assert_eq!(
            TokioProcessRunner::normalize_line("test".to_string()),
            "test"
        );
        assert_eq!(TokioProcessRunner::normalize_line("".to_string()), "");
        assert_eq!(
            TokioProcessRunner::normalize_line("test\nmulti".to_string()),
            "test\nmulti"
        );
    }

    #[test]
    fn test_convert_exit_status() {
        use std::os::unix::process::ExitStatusExt;

        // Test success
        let status = std::process::ExitStatus::from_raw(0);
        assert_eq!(
            TokioProcessRunner::convert_exit_status(status),
            super::ExitStatus::Success
        );

        // Test error code
        let status = std::process::ExitStatus::from_raw(256); // Exit code 1
        match TokioProcessRunner::convert_exit_status(status) {
            super::ExitStatus::Error(code) => assert_eq!(code, 1),
            _ => panic!("Expected Error status"),
        }
    }
}
