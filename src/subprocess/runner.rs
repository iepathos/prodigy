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

#[async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput, ProcessError> {
        let start = std::time::Instant::now();

        let mut cmd = tokio::process::Command::new(&command.program);

        cmd.args(&command.args);

        for (key, value) in &command.env {
            cmd.env(key, value);
        }

        if let Some(dir) = &command.working_dir {
            cmd.current_dir(dir);
        }

        if command.stdin.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ProcessError::CommandNotFound(command.program.clone())
            } else {
                ProcessError::Io(e)
            }
        })?;

        if let Some(stdin_data) = &command.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin
                    .write_all(stdin_data.as_bytes())
                    .await
                    .map_err(ProcessError::Io)?;
                stdin.shutdown().await.map_err(ProcessError::Io)?;
            }
        }

        let output = if let Some(timeout_duration) = command.timeout {
            match tokio::time::timeout(timeout_duration, child.wait_with_output()).await {
                Ok(result) => result.map_err(ProcessError::Io)?,
                Err(_) => {
                    return Err(ProcessError::Timeout(timeout_duration));
                }
            }
        } else {
            child.wait_with_output().await.map_err(ProcessError::Io)?
        };

        let duration = start.elapsed();

        let status = if output.status.success() {
            ExitStatus::Success
        } else if let Some(code) = output.status.code() {
            ExitStatus::Error(code)
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(signal) = output.status.signal() {
                    ExitStatus::Signal(signal)
                } else {
                    ExitStatus::Error(1)
                }
            }
            #[cfg(not(unix))]
            {
                ExitStatus::Error(1)
            }
        };

        Ok(ProcessOutput {
            status,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration,
        })
    }

    async fn run_streaming(&self, _command: ProcessCommand) -> Result<ProcessStream, ProcessError> {
        todo!("Streaming support will be implemented as needed")
    }
}
