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
}
