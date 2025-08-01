use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::error::ProcessError;
use super::runner::{ExitStatus, ProcessCommand, ProcessOutput, ProcessRunner, ProcessStream};

#[derive(Clone)]
pub struct MockProcessRunner {
    expectations: Arc<Mutex<Vec<MockExpectation>>>,
    call_history: Arc<Mutex<Vec<ProcessCommand>>>,
}

struct MockExpectation {
    program: String,
    #[allow(clippy::type_complexity)]
    args_matcher: Option<Box<dyn Fn(&[String]) -> bool + Send + Sync>>,
    response: ProcessOutput,
    times_called: usize,
    expected_times: Option<usize>,
}

pub struct MockCommandConfig {
    runner: MockProcessRunner,
    expectation: MockExpectation,
}

impl MockProcessRunner {
    pub fn new() -> Self {
        Self {
            expectations: Arc::new(Mutex::new(Vec::new())),
            call_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn expect_command(&mut self, program: &str) -> MockCommandConfig {
        MockCommandConfig {
            runner: self.clone(),
            expectation: MockExpectation {
                program: program.to_string(),
                args_matcher: None,
                response: ProcessOutput {
                    status: ExitStatus::Success,
                    stdout: String::new(),
                    stderr: String::new(),
                    duration: Duration::from_millis(10),
                },
                times_called: 0,
                expected_times: None,
            },
        }
    }

    pub fn verify_called(&self, program: &str, times: usize) -> bool {
        let history = self.call_history.lock().unwrap();
        let count = history.iter().filter(|cmd| cmd.program == program).count();
        count == times
    }

    pub fn get_call_history(&self) -> Vec<ProcessCommand> {
        self.call_history.lock().unwrap().clone()
    }

    pub fn reset(&mut self) {
        self.expectations.lock().unwrap().clear();
        self.call_history.lock().unwrap().clear();
    }

    /// Add a response for a specific program (helper for tests)
    pub async fn add_response(&self, program: &str, response: Result<ProcessOutput, ProcessError>) {
        let mut expectations = self.expectations.lock().unwrap();
        let output = match response {
            Ok(output) => output,
            Err(_) => ProcessOutput {
                status: ExitStatus::Error(1),
                stdout: String::new(),
                stderr: "Mock error".to_string(),
                duration: Duration::from_millis(10),
            },
        };

        expectations.push(MockExpectation {
            program: program.to_string(),
            args_matcher: None,
            response: output,
            times_called: 0,
            expected_times: None,
        });
    }

    /// Get all calls made to this mock (helper for tests)
    pub async fn get_calls(&self) -> Vec<ProcessCommand> {
        self.get_call_history()
    }
}

#[async_trait]
impl ProcessRunner for MockProcessRunner {
    async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput, ProcessError> {
        self.call_history.lock().unwrap().push(command.clone());

        let mut expectations = self.expectations.lock().unwrap();

        for expectation in expectations.iter_mut() {
            if expectation.program != command.program {
                continue;
            }

            if let Some(ref args_matcher) = expectation.args_matcher {
                if !(args_matcher)(&command.args) {
                    continue;
                }
            }

            expectation.times_called += 1;

            if let Some(expected) = expectation.expected_times {
                if expectation.times_called > expected {
                    return Err(ProcessError::MockExpectationNotMet(format!(
                        "Command '{}' called {} times, expected {}",
                        command.program, expectation.times_called, expected
                    )));
                }
            }

            return Ok(expectation.response.clone());
        }

        Err(ProcessError::MockExpectationNotMet(format!(
            "No expectation found for command: {} {:?}",
            command.program, command.args
        )))
    }

    async fn run_streaming(&self, _command: ProcessCommand) -> Result<ProcessStream, ProcessError> {
        todo!("Streaming mock support will be implemented as needed")
    }
}

impl MockCommandConfig {
    pub fn with_args<F>(mut self, matcher: F) -> Self
    where
        F: Fn(&[String]) -> bool + Send + Sync + 'static,
    {
        self.expectation.args_matcher = Some(Box::new(matcher));
        self
    }

    pub fn returns_stdout(mut self, stdout: &str) -> Self {
        self.expectation.response.stdout = stdout.to_string();
        self
    }

    pub fn returns_stderr(mut self, stderr: &str) -> Self {
        self.expectation.response.stderr = stderr.to_string();
        self
    }

    pub fn returns_exit_code(mut self, code: i32) -> Self {
        self.expectation.response.status = if code == 0 {
            ExitStatus::Success
        } else {
            ExitStatus::Error(code)
        };
        self
    }

    pub fn returns_success(mut self) -> Self {
        self.expectation.response.status = ExitStatus::Success;
        self
    }

    pub fn times(mut self, n: usize) -> Self {
        self.expectation.expected_times = Some(n);
        self
    }

    pub fn finish(self) {
        self.runner
            .expectations
            .lock()
            .unwrap()
            .push(self.expectation);
    }
}

impl Default for MockProcessRunner {
    fn default() -> Self {
        Self::new()
    }
}
