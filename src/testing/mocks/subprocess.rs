//! Mock subprocess execution for testing

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::cook::execution::{CommandExecutor, ExecutionContext, ExecutionResult};

/// Builder for creating configured mock subprocess managers
pub struct MockSubprocessManagerBuilder {
    responses: HashMap<String, (String, String, i32)>, // (stdout, stderr, exit_code)
    default_response: (String, String, i32),
}

impl Default for MockSubprocessManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSubprocessManagerBuilder {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            default_response: (String::new(), String::new(), 0),
        }
    }

    pub fn with_command_response(
        mut self,
        command: &str,
        stdout: &str,
        stderr: &str,
        exit_code: i32,
    ) -> Self {
        self.responses.insert(
            command.to_string(),
            (stdout.to_string(), stderr.to_string(), exit_code),
        );
        self
    }

    pub fn with_success(mut self, command: &str, stdout: &str) -> Self {
        self.responses
            .insert(command.to_string(), (stdout.to_string(), String::new(), 0));
        self
    }

    pub fn with_error(mut self, command: &str, stderr: &str, exit_code: i32) -> Self {
        self.responses.insert(
            command.to_string(),
            (String::new(), stderr.to_string(), exit_code),
        );
        self
    }

    pub fn with_default_response(mut self, stdout: &str, stderr: &str, exit_code: i32) -> Self {
        self.default_response = (stdout.to_string(), stderr.to_string(), exit_code);
        self
    }

    pub fn build(self) -> MockSubprocessManager {
        MockSubprocessManager {
            responses: Arc::new(Mutex::new(self.responses)),
            default_response: self.default_response,
            call_history: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Mock subprocess manager for testing
pub struct MockSubprocessManager {
    #[allow(clippy::type_complexity)]
    responses: Arc<Mutex<HashMap<String, (String, String, i32)>>>,
    default_response: (String, String, i32),
    #[allow(clippy::type_complexity)]
    call_history: Arc<Mutex<Vec<(String, Vec<String>)>>>, // (command, args)
}

impl Default for MockSubprocessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSubprocessManager {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            default_response: (String::new(), String::new(), 0),
            call_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn builder() -> MockSubprocessManagerBuilder {
        MockSubprocessManagerBuilder::new()
    }

    pub async fn run_command(&self, command: &str, args: &[&str]) -> Result<(String, String, i32)> {
        // Record the call
        let mut history = self.call_history.lock().unwrap();
        history.push((
            command.to_string(),
            args.iter().map(|s| s.to_string()).collect(),
        ));
        drop(history);

        // Look up response
        let responses = self.responses.lock().unwrap();
        let response = responses
            .get(command)
            .cloned()
            .unwrap_or_else(|| self.default_response.clone());

        if response.2 != 0 {
            Err(anyhow::anyhow!(
                "Command failed with exit code {}: {}",
                response.2,
                response.1
            ))
        } else {
            Ok(response)
        }
    }

    pub fn get_call_history(&self) -> Vec<(String, Vec<String>)> {
        self.call_history.lock().unwrap().clone()
    }

    pub fn was_called_with(&self, command: &str, args: &[&str]) -> bool {
        let history = self.call_history.lock().unwrap();
        history.iter().any(|(cmd, cmd_args)| {
            cmd == command
                && cmd_args.len() == args.len()
                && cmd_args.iter().zip(args.iter()).all(|(a, b)| a == *b)
        })
    }

    pub fn reset_history(&self) {
        self.call_history.lock().unwrap().clear();
    }
}

/// Mock command executor for testing
pub struct CommandExecutorMock {
    responses: Arc<Mutex<HashMap<String, ExecutionResult>>>,
}

impl CommandExecutorMock {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_response(&mut self, command: &str, result: ExecutionResult) {
        let mut responses = self.responses.lock().unwrap();
        responses.insert(command.to_string(), result);
    }
}

impl Default for CommandExecutorMock {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandExecutor for CommandExecutorMock {
    async fn execute(
        &self,
        command: &str,
        _args: &[String],
        _context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        let responses = self.responses.lock().unwrap();
        if let Some(result) = responses.get(command) {
            Ok(ExecutionResult {
                success: result.success,
                stdout: result.stdout.clone(),
                stderr: result.stderr.clone(),
                exit_code: result.exit_code,
                metadata: HashMap::new(),
            })
        } else {
            Ok(ExecutionResult {
                success: true,
                stdout: "default output".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_subprocess_builder() {
        let mock = MockSubprocessManagerBuilder::new()
            .with_success("git", "status output")
            .with_error("cargo", "compilation failed", 1)
            .build();

        let (stdout, _, code) = mock.run_command("git", &["status"]).await.unwrap();
        assert_eq!(stdout, "status output");
        assert_eq!(code, 0);

        let error = mock.run_command("cargo", &["build"]).await.unwrap_err();
        assert!(error.to_string().contains("compilation failed"));
    }

    #[tokio::test]
    async fn test_mock_subprocess_history() {
        let mock = MockSubprocessManager::new();

        mock.run_command("git", &["status"]).await.ok();
        mock.run_command("cargo", &["test"]).await.ok();

        let history = mock.get_call_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, "git");
        assert_eq!(history[1].0, "cargo");

        assert!(mock.was_called_with("git", &["status"]));
        assert!(mock.was_called_with("cargo", &["test"]));
        assert!(!mock.was_called_with("npm", &["install"]));
    }

    #[tokio::test]
    async fn test_mock_subprocess_default_response() {
        let mock = MockSubprocessManagerBuilder::new()
            .with_default_response("default output", "", 0)
            .build();

        let (stdout, _, _) = mock.run_command("unknown", &[]).await.unwrap();
        assert_eq!(stdout, "default output");
    }
}
