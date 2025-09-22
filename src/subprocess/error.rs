use crate::error::{ErrorCode, ProdigyError};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Process timed out after {0:?}")]
    Timeout(Duration),

    #[error("Process exited with code {0}")]
    ExitCode(i32),

    #[error("Process terminated by signal {0}")]
    Signal(i32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Mock expectation not met: {0}")]
    MockExpectationNotMet(String),

    #[error("Failed to spawn process: {command}")]
    SpawnFailed {
        command: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Command failed: {command}")]
    CommandFailed {
        command: String,
        status: crate::subprocess::runner::ExitStatus,
        stderr: String,
    },

    #[error("IO error for command: {command}")]
    IoError {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Internal error: {message}")]
    InternalError { message: String },
}

/// Convert ProcessError to ProdigyError
impl From<ProcessError> for ProdigyError {
    fn from(err: ProcessError) -> Self {
        let (code, command, exit_code) = match &err {
            ProcessError::CommandNotFound(cmd) => {
                (ErrorCode::EXEC_COMMAND_NOT_FOUND, Some(cmd.clone()), None)
            }
            ProcessError::Timeout(_) => (ErrorCode::EXEC_TIMEOUT, None, None),
            ProcessError::ExitCode(code) => (ErrorCode::EXEC_SUBPROCESS_FAILED, None, Some(*code)),
            ProcessError::Signal(sig) => (ErrorCode::EXEC_SIGNAL_RECEIVED, None, Some(*sig)),
            ProcessError::Io(_) => (ErrorCode::EXEC_SPAWN_FAILED, None, None),
            ProcessError::Utf8(_) => (ErrorCode::EXEC_OUTPUT_ERROR, None, None),
            ProcessError::MockExpectationNotMet(_) => (ErrorCode::EXEC_GENERIC, None, None),
            ProcessError::SpawnFailed { command, .. } => {
                (ErrorCode::EXEC_SPAWN_FAILED, Some(command.clone()), None)
            }
            ProcessError::CommandFailed {
                command, status, ..
            } => {
                let exit_code = status.code();
                (
                    ErrorCode::EXEC_SUBPROCESS_FAILED,
                    Some(command.clone()),
                    exit_code,
                )
            }
            ProcessError::IoError { command, .. } => {
                (ErrorCode::EXEC_SPAWN_FAILED, Some(command.clone()), None)
            }
            ProcessError::InternalError { .. } => (ErrorCode::EXEC_GENERIC, None, None),
        };

        let mut error = ProdigyError::execution_with_code(code, err.to_string(), command);
        if let ProdigyError::Execution {
            exit_code: ex_code, ..
        } = &mut error
        {
            *ex_code = exit_code;
        }
        error.with_source(err)
    }
}
