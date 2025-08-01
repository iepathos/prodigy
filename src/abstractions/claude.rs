//! Claude CLI abstraction layer
//!
//! Provides trait-based abstraction for Claude CLI commands to enable
//! testing without actual Claude CLI installation.

use crate::abstractions::exit_status::ExitStatusExt;
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager, ClaudeRunner as SubprocessClaudeRunner};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Trait for Claude CLI operations
#[async_trait]
pub trait ClaudeClient: Send + Sync {
    /// Execute a Claude command with retry logic
    async fn execute_command(
        &self,
        command: &str,
        args: &[&str],
        env_vars: Option<HashMap<String, String>>,
        max_retries: u32,
        verbose: bool,
    ) -> Result<std::process::Output>;

    /// Check if Claude CLI is available
    async fn check_availability(&self) -> Result<()>;

    /// Execute /mmm-code-review command
    async fn code_review(&self, verbose: bool, focus: Option<&str>) -> Result<bool>;

    /// Execute /mmm-implement-spec command
    async fn implement_spec(&self, spec_id: &str, verbose: bool) -> Result<bool>;

    /// Execute /mmm-lint command
    async fn lint(&self, verbose: bool) -> Result<bool>;
}

/// Real implementation of `ClaudeClient`
pub struct RealClaudeClient {
    subprocess: SubprocessManager,
}

impl RealClaudeClient {
    /// Create a new `RealClaudeClient` instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            subprocess: SubprocessManager::production(),
        }
    }

    /// Create a new instance with custom subprocess manager (for testing)
    #[cfg(test)]
    pub fn with_subprocess(subprocess: SubprocessManager) -> Self {
        Self { subprocess }
    }

    /// Check if an error message indicates a transient failure
    fn is_transient_error(stderr: &str) -> bool {
        let transient_patterns = [
            "rate limit",
            "timeout",
            "connection refused",
            "temporary failure",
            "network",
            "503",
            "429",
            "could not connect",
            "broken pipe",
        ];

        let stderr_lower = stderr.to_lowercase();
        transient_patterns
            .iter()
            .any(|pattern| stderr_lower.contains(pattern))
    }
}

impl Default for RealClaudeClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ClaudeClient for RealClaudeClient {
    async fn execute_command(
        &self,
        command: &str,
        args: &[&str],
        env_vars: Option<HashMap<String, String>>,
        max_retries: u32,
        verbose: bool,
    ) -> Result<std::process::Output> {
        use std::time::Duration;
        use tokio::time::sleep;

        let mut attempt = 0;
        let mut last_error = None;

        while attempt <= max_retries {
            if attempt > 0 {
                let delay = Duration::from_secs(2u64.pow(attempt.min(3)));
                if verbose {
                    println!(
                        "⏳ Retrying {command} after {delay:?} (attempt {attempt}/{max_retries})"
                    );
                }
                sleep(delay).await;
            }

            let mut builder = ProcessCommandBuilder::new("claude");
            for arg in args {
                builder = builder.arg(arg);
            }

            // Set environment variables if provided
            if let Some(ref vars) = env_vars {
                for (key, value) in vars {
                    builder = builder.env(key, value);
                }
            }

            match self.subprocess.runner().run(builder.build()).await {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = &output.stderr;

                        // Retry on transient errors
                        if Self::is_transient_error(stderr) && attempt < max_retries {
                            if verbose {
                                eprintln!(
                                    "⚠️  Transient error detected: {}",
                                    stderr.lines().next().unwrap_or("Unknown error")
                                );
                            }
                            last_error = Some(stderr.to_string());
                            attempt += 1;
                            continue;
                        }
                    }

                    // Convert to std::process::Output
                    return Ok(std::process::Output {
                        status: std::process::ExitStatus::from_raw(
                            output.status.code().unwrap_or(1),
                        ),
                        stdout: output.stdout.into_bytes(),
                        stderr: output.stderr.into_bytes(),
                    });
                }
                Err(e) => {
                    if matches!(&e, crate::subprocess::ProcessError::CommandNotFound(_)) {
                        return Err(anyhow::anyhow!("Claude CLI not found: {}", e));
                    }

                    if attempt < max_retries {
                        if verbose {
                            eprintln!("⚠️  IO error: {e}");
                        }
                        last_error = Some(e.to_string());
                        attempt += 1;
                        continue;
                    }

                    return Err(anyhow::anyhow!("Failed to execute {}: {}", command, e));
                }
            }
        }

        Err(anyhow::anyhow!(
            "Failed {} after {} retries. Last error: {}",
            command,
            max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        ))
    }

    async fn check_availability(&self) -> Result<()> {
        let output = self
            .subprocess
            .runner()
            .run(ProcessCommandBuilder::new("which").arg("claude").build())
            .await?;

        if !output.status.success() || output.stdout.is_empty() {
            // Try 'claude --version' as a fallback
            let version_check = self.subprocess.claude().check_availability().await?;

            if !version_check {
                return Err(anyhow::anyhow!(
                    "Claude CLI not found. Please install Claude CLI:\n\
                     \n\
                     1. Visit: https://claude.ai/download\n\
                     2. Download and install Claude CLI for your platform\n\
                     3. Run 'claude auth' to authenticate\n\
                     4. Ensure 'claude' is in your PATH\n\
                     \n\
                     You can verify the installation by running: claude --version"
                ));
            }
        }

        Ok(())
    }

    async fn code_review(&self, verbose: bool, focus: Option<&str>) -> Result<bool> {
        println!("🤖 Running /mmm-code-review...");

        let mut env_vars = HashMap::new();
        if let Some(f) = focus {
            env_vars.insert("MMM_FOCUS".to_string(), f.to_string());
        }
        if std::env::var("MMM_AUTOMATION").unwrap_or_default() == "true" {
            env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());
        }

        let args = vec![
            "--dangerously-skip-permissions",
            "--print",
            "/mmm-code-review",
        ];

        let output = self
            .execute_command("/mmm-code-review", &args, Some(env_vars), 2, verbose)
            .await?;

        if verbose {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                println!("Claude output:\n{stdout}");
            }
        }

        Ok(output.status.success())
    }

    async fn implement_spec(&self, spec_id: &str, verbose: bool) -> Result<bool> {
        println!("🔧 Running /mmm-implement-spec {}...", spec_id);

        let mut env_vars = HashMap::new();
        if std::env::var("MMM_AUTOMATION").unwrap_or_default() == "true" {
            env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());
        }

        let args = vec![
            "--dangerously-skip-permissions",
            "--print",
            "/mmm-implement-spec",
            spec_id,
        ];

        let output = self
            .execute_command(
                &format!("/mmm-implement-spec {spec_id}"),
                &args,
                Some(env_vars),
                2,
                verbose,
            )
            .await?;

        if verbose {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                println!("Claude output:\n{stdout}");
            }
        }

        Ok(output.status.success())
    }

    async fn lint(&self, verbose: bool) -> Result<bool> {
        println!("🧹 Running /mmm-lint...");

        let mut env_vars = HashMap::new();
        if std::env::var("MMM_AUTOMATION").unwrap_or_default() == "true" {
            env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());
        }

        let args = vec!["--dangerously-skip-permissions", "--print", "/mmm-lint"];

        let output = self
            .execute_command("/mmm-lint", &args, Some(env_vars), 2, verbose)
            .await?;

        if verbose {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                println!("Claude output:\n{stdout}");
            }
        }

        Ok(output.status.success())
    }
}

/// Type alias for called commands tracking
type CalledCommands = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// Mock implementation of `ClaudeClient` for testing
pub struct MockClaudeClient {
    /// Predefined responses for `execute_command`
    pub command_responses: Arc<Mutex<Vec<Result<std::process::Output>>>>,
    /// Whether Claude CLI is available
    pub is_available: bool,
    /// Track called commands for verification
    pub called_commands: CalledCommands,
}


impl MockClaudeClient {
    /// Create a new `MockClaudeClient` instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            command_responses: Arc::new(Mutex::new(Vec::new())),
            is_available: true,
            called_commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a response for the next command
    pub async fn add_response(&self, response: Result<std::process::Output>) {
        self.command_responses.lock().await.push(response);
    }

    /// Add a successful response
    pub async fn add_success_response(&self, stdout: &str) {
        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        };
        self.add_response(Ok(output)).await;
    }

    /// Add an error response
    pub async fn add_error_response(&self, stderr: &str, exit_code: i32) {
        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(exit_code),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        };
        self.add_response(Ok(output)).await;
    }

    /// Get the list of called commands
    pub async fn get_called_commands(&self) -> Vec<(String, Vec<String>)> {
        self.called_commands.lock().await.clone()
    }
}

impl Default for MockClaudeClient {
    fn default() -> Self {
        Self::new()
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
        // Track the called command
        let args_vec: Vec<String> = args.iter().map(|&s| s.to_string()).collect();
        self.called_commands
            .lock()
            .await
            .push((command.to_string(), args_vec));

        // Return the next predefined response
        let mut responses = self.command_responses.lock().await;
        if responses.is_empty() {
            return Err(anyhow::anyhow!("No mock response configured"));
        }
        responses.remove(0)
    }

    async fn check_availability(&self) -> Result<()> {
        if self.is_available {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Claude CLI not available (mock)"))
        }
    }

    async fn code_review(&self, verbose: bool, _focus: Option<&str>) -> Result<bool> {
        if !self.is_available {
            return Err(anyhow::anyhow!("Claude CLI not available"));
        }

        let args = vec![
            "--dangerously-skip-permissions",
            "--print",
            "/mmm-code-review",
        ];

        let output = self
            .execute_command("/mmm-code-review", &args, None, 2, verbose)
            .await?;

        Ok(output.status.success())
    }

    async fn implement_spec(&self, spec_id: &str, verbose: bool) -> Result<bool> {
        if !self.is_available {
            return Err(anyhow::anyhow!("Claude CLI not available"));
        }

        let args = vec![
            "--dangerously-skip-permissions",
            "--print",
            "/mmm-implement-spec",
            spec_id,
        ];

        let output = self
            .execute_command(
                &format!("/mmm-implement-spec {spec_id}"),
                &args,
                None,
                2,
                verbose,
            )
            .await?;

        Ok(output.status.success())
    }

    async fn lint(&self, verbose: bool) -> Result<bool> {
        if !self.is_available {
            return Err(anyhow::anyhow!("Claude CLI not available"));
        }

        let args = vec!["--dangerously-skip-permissions", "--print", "/mmm-lint"];

        let output = self
            .execute_command("/mmm-lint", &args, None, 2, verbose)
            .await?;

        Ok(output.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_claude_client() {
        let mock = MockClaudeClient::new();

        // Add responses
        mock.add_success_response("Code review completed").await;
        mock.add_success_response("Implementation completed").await;

        // Test code_review
        let result = mock.code_review(false, Some("performance")).await.unwrap();
        assert!(result);

        // Test implement_spec
        let result = mock.implement_spec("test-spec-123", false).await.unwrap();
        assert!(result);

        // Verify called commands
        let commands = mock.get_called_commands().await;
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].0, "/mmm-code-review");
        assert_eq!(commands[1].0, "/mmm-implement-spec test-spec-123");
    }

    #[tokio::test]
    async fn test_mock_claude_unavailable() {
        let mut mock = MockClaudeClient::new();
        mock.is_available = false;

        // Test availability check
        let result = mock.check_availability().await;
        assert!(result.is_err());

        // Test commands fail when unavailable
        let result = mock.code_review(false, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_claude_error_response() {
        let mock = MockClaudeClient::new();

        // Add error response
        mock.add_error_response("rate limit exceeded", 1).await;

        // Test error handling
        let result = mock.code_review(false, None).await.unwrap();
        assert!(!result);
    }

    #[test]
    fn test_transient_error_detection() {
        assert!(RealClaudeClient::is_transient_error(
            "Error: rate limit exceeded"
        ));
        assert!(RealClaudeClient::is_transient_error("Connection timeout"));
        assert!(RealClaudeClient::is_transient_error(
            "HTTP 503 Service Unavailable"
        ));
        assert!(!RealClaudeClient::is_transient_error("Syntax error"));
    }
}
