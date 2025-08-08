//! Adapter to make subprocess module compatible with commands module

use super::runner::TokioProcessRunner;
use super::{ProcessCommand, ProcessError, ProcessRunner};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

/// Errors that can occur during subprocess execution  
pub type SubprocessError = ProcessError;

/// Trait for executing subprocesses (adapter for commands module)
#[async_trait]
pub trait SubprocessExecutor: Send + Sync {
    /// Execute a command with optional parameters
    async fn execute(
        &self,
        command: &str,
        args: &[&str],
        working_dir: Option<&Path>,
        env: Option<HashMap<String, String>>,
        timeout: Option<Duration>,
    ) -> Result<Output, SubprocessError>;
}

/// Real subprocess executor that runs actual commands
pub struct RealSubprocessExecutor;

#[async_trait]
impl SubprocessExecutor for RealSubprocessExecutor {
    async fn execute(
        &self,
        command: &str,
        args: &[&str],
        working_dir: Option<&Path>,
        env: Option<HashMap<String, String>>,
        timeout: Option<Duration>,
    ) -> Result<Output, SubprocessError> {
        let runner = TokioProcessRunner;

        let cmd = ProcessCommand {
            program: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: env.unwrap_or_default(),
            working_dir: working_dir.map(|p| p.to_path_buf()),
            timeout,
            stdin: None,
            suppress_stderr: false,
        };

        let output = runner.run(cmd).await?;

        // Convert ProcessOutput to std::process::Output
        // Note: We need to create a mock Output since std::process::Output
        // doesn't have a public constructor
        Ok(create_output(
            output.status.code().unwrap_or(0),
            output.stdout.into_bytes(),
            output.stderr.into_bytes(),
        ))
    }
}

/// Helper to create a std::process::Output
fn create_output(exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8>) -> Output {
    Output {
        status: create_exit_status(exit_code),
        stdout,
        stderr,
    }
}

/// Helper to create an ExitStatus
#[cfg(unix)]
fn create_exit_status(code: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(windows)]
fn create_exit_status(code: i32) -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code as u32)
}

/// Mock subprocess executor for testing
#[cfg(test)]
pub struct MockSubprocessExecutor {
    responses: std::sync::Mutex<
        Vec<(
            String,
            Vec<String>,
            Option<std::path::PathBuf>,
            Option<Duration>,
            Output,
        )>,
    >,
}

#[cfg(test)]
impl MockSubprocessExecutor {
    pub fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn expect_execute(
        &mut self,
        command: &str,
        args: Vec<&str>,
        working_dir: Option<std::path::PathBuf>,
        _env: Option<HashMap<String, String>>,
        timeout: Option<Duration>,
        output: Output,
    ) {
        let mut responses = self.responses.lock().unwrap();
        responses.push((
            command.to_string(),
            args.iter().map(|s| s.to_string()).collect(),
            working_dir,
            timeout,
            output,
        ));
    }
}

#[cfg(test)]
#[async_trait]
impl SubprocessExecutor for MockSubprocessExecutor {
    async fn execute(
        &self,
        command: &str,
        args: &[&str],
        working_dir: Option<&Path>,
        _env: Option<HashMap<String, String>>,
        timeout: Option<Duration>,
    ) -> Result<Output, SubprocessError> {
        let mut responses = self.responses.lock().unwrap();

        for i in 0..responses.len() {
            let (ref cmd, ref expected_args, ref expected_dir, ref expected_timeout, _) =
                responses[i];

            let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let working_dir_path = working_dir.map(|p| p.to_path_buf());

            if cmd == command
                && expected_args == &args_vec
                && expected_dir == &working_dir_path
                && expected_timeout == &timeout
            {
                let (_, _, _, _, output) = responses.remove(i);
                return Ok(output);
            }
        }

        Err(SubprocessError::MockExpectationNotMet(format!(
            "Unexpected command: {} {:?}",
            command, args
        )))
    }
}
