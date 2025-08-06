//! Mock Claude client implementation for testing

use crate::abstractions::claude::ClaudeClient;
use crate::abstractions::exit_status::ExitStatusExt;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Builder for creating configured mock Claude clients
pub struct MockClaudeClientBuilder {
    responses: HashMap<String, Result<String>>,
    availability: bool,
    error_on_call: Option<usize>,
    call_count: Arc<Mutex<usize>>,
}

impl MockClaudeClientBuilder {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            availability: true,
            error_on_call: None,
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn with_response(mut self, command: &str, response: Result<String>) -> Self {
        self.responses.insert(command.to_string(), response);
        self
    }

    pub fn with_success(mut self, command: &str, response: &str) -> Self {
        self.responses
            .insert(command.to_string(), Ok(response.to_string()));
        self
    }

    pub fn with_error(mut self, command: &str, error: &str) -> Self {
        self.responses.insert(
            command.to_string(),
            Err(anyhow::anyhow!(error.to_string())),
        );
        self
    }

    pub fn unavailable(mut self) -> Self {
        self.availability = false;
        self
    }

    pub fn fail_after(mut self, calls: usize) -> Self {
        self.error_on_call = Some(calls);
        self
    }

    pub fn build(self) -> MockClaudeClient {
        MockClaudeClient {
            responses: Arc::new(Mutex::new(self.responses)),
            availability: self.availability,
            error_on_call: self.error_on_call,
            call_count: self.call_count,
            default_response: Ok("Mock response".to_string()),
        }
    }
}

/// Mock implementation of ClaudeClient for testing
pub struct MockClaudeClient {
    responses: Arc<Mutex<HashMap<String, Result<String>>>>,
    availability: bool,
    error_on_call: Option<usize>,
    call_count: Arc<Mutex<usize>>,
    default_response: Result<String>,
}

impl MockClaudeClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            availability: true,
            error_on_call: None,
            call_count: Arc::new(Mutex::new(0)),
            default_response: Ok("Mock response".to_string()),
        }
    }

    pub fn builder() -> MockClaudeClientBuilder {
        MockClaudeClientBuilder::new()
    }

    pub fn get_call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }

    pub fn reset_call_count(&self) {
        *self.call_count.lock().unwrap() = 0;
    }
}

#[async_trait]
impl ClaudeClient for MockClaudeClient {
    async fn execute_command(
        &self,
        command: &str,
        args: &[&str],
        _env_vars: Option<HashMap<String, String>>,
        _max_retries: u32,
        _verbose: bool,
    ) -> Result<std::process::Output> {
        if !self.availability {
            return Err(anyhow::anyhow!("Claude CLI not available"));
        }

        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        let current_call = *count;
        drop(count);

        if let Some(fail_after) = self.error_on_call {
            if current_call > fail_after {
                return Err(anyhow::anyhow!("Simulated failure after {} calls", fail_after));
            }
        }

        // Create a key from command and args
        let key = format!("{} {}", command, args.join(" "));
        
        let responses = self.responses.lock().unwrap();
        let response_result = responses
            .get(&key)
            .or_else(|| responses.get(command));
        
        let response = match response_result {
            Some(Ok(s)) => s.clone(),
            Some(Err(e)) => return Err(anyhow::anyhow!(e.to_string())),
            None => match &self.default_response {
                Ok(s) => s.clone(),
                Err(e) => return Err(anyhow::anyhow!(e.to_string())),
            },
        };

        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: response.into_bytes(),
            stderr: Vec::new(),
        })
    }

    async fn check_availability(&self) -> Result<()> {
        if self.availability {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Claude CLI not available"))
        }
    }

    async fn code_review(&self, _verbose: bool) -> Result<bool> {
        if !self.availability {
            return Err(anyhow::anyhow!("Claude CLI not available"));
        }

        // Increment call count
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        let current_call = *count;
        drop(count);

        // Check if we should fail after a certain number of calls
        if let Some(fail_after) = self.error_on_call {
            if current_call > fail_after {
                return Err(anyhow::anyhow!("Simulated failure after {} calls", fail_after));
            }
        }

        let responses = self.responses.lock().unwrap();
        if let Some(response) = responses.get("/mmm-code-review") {
            match response {
                Ok(msg) => Ok(!msg.contains("No issues")),
                Err(e) => Err(anyhow::anyhow!(e.to_string())),
            }
        } else {
            Ok(true) // Default to having improvements
        }
    }

    async fn implement_spec(&self, spec_id: &str, _verbose: bool) -> Result<bool> {
        if !self.availability {
            return Err(anyhow::anyhow!("Claude CLI not available"));
        }

        let responses = self.responses.lock().unwrap();
        let key = format!("/mmm-implement-spec {}", spec_id);
        
        if let Some(response) = responses.get(&key).or_else(|| responses.get("/mmm-implement-spec")) {
            match response {
                Ok(_) => Ok(true),
                Err(e) => Err(anyhow::anyhow!(e.to_string())),
            }
        } else {
            Ok(true) // Default to successful implementation
        }
    }

    async fn lint(&self, _verbose: bool) -> Result<bool> {
        if !self.availability {
            return Err(anyhow::anyhow!("Claude CLI not available"));
        }

        // Increment call count
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        drop(count);

        let responses = self.responses.lock().unwrap();
        if let Some(response) = responses.get("/mmm-lint") {
            match response {
                Ok(_) => Ok(true),
                Err(e) => Err(anyhow::anyhow!(e.to_string())),
            }
        } else {
            Ok(true) // Default to successful linting
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_claude_builder() {
        let mock = MockClaudeClientBuilder::new()
            .with_success("/mmm-code-review", "No issues found")
            .with_error("/mmm-implement-spec", "Spec not found")
            .build();

        assert!(mock.check_availability().await.is_ok());

        let has_improvements = mock.code_review(false).await.unwrap();
        assert!(!has_improvements); // "No issues" means no improvements

        let error = mock
            .implement_spec("test-spec", false)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Spec not found"));
    }

    #[tokio::test]
    async fn test_mock_claude_unavailable() {
        let mock = MockClaudeClientBuilder::new().unavailable().build();

        assert!(mock.check_availability().await.is_err());

        let error = mock.code_review(false).await.unwrap_err();
        assert!(error.to_string().contains("not available"));
    }

    #[tokio::test]
    async fn test_mock_claude_fail_after() {
        let mock = MockClaudeClientBuilder::new()
            .with_success("/mmm-code-review", "Found issues")
            .fail_after(2)
            .build();

        // First two calls succeed
        assert!(mock.code_review(false).await.is_ok());
        assert!(mock.code_review(false).await.is_ok());

        // Third call fails
        let error = mock.code_review(false).await.unwrap_err();
        assert!(error.to_string().contains("Simulated failure"));
    }

    #[tokio::test]
    async fn test_call_counting() {
        let mock = MockClaudeClientBuilder::new()
            .with_success("/mmm-lint", "Success")
            .build();

        assert_eq!(mock.get_call_count(), 0);

        mock.lint(false).await.unwrap();
        assert_eq!(mock.get_call_count(), 1);

        mock.lint(false).await.unwrap();
        assert_eq!(mock.get_call_count(), 2);

        mock.reset_call_count();
        assert_eq!(mock.get_call_count(), 0);
    }
}