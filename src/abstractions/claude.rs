//! Claude CLI abstraction layer
//!
//! Provides trait-based abstraction for Claude CLI commands to enable
//! testing without actual Claude CLI installation.

use crate::abstractions::exit_status::ExitStatusExt;
use crate::subprocess::{
    ClaudeRunner as SubprocessClaudeRunner, ProcessCommandBuilder, SubprocessManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Trait for Claude CLI operations providing testable abstraction
///
/// This trait abstracts all interactions with the Claude CLI, enabling
/// comprehensive testing without requiring an actual Claude CLI installation.
/// It provides both low-level command execution and high-level workflow operations.
///
/// # Design Goals
///
/// - **Testability**: Enable mocking for comprehensive test coverage
/// - **Retry Logic**: Built-in retry mechanism for transient failures
/// - **Environment Control**: Flexible environment variable handling
/// - **Command Abstraction**: High-level methods for common MMM workflows
///
/// # Examples
///
/// ## Basic Command Execution
///
/// ```rust
/// use mmm::abstractions::ClaudeClient;
/// use std::collections::HashMap;
///
/// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
/// let mut env_vars = HashMap::new();
/// env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());
///
/// let output = client.execute_command(
///     "claude",
///     &["code", "--command", "/mmm-code-review"],
///     Some(env_vars),
///     3, // max retries
///     true // verbose
/// ).await?;
///
/// println!("Exit status: {}", output.status.success());
/// # Ok(())
/// # }
/// ```
///
/// ## High-Level Workflow Operations
///
/// ```rust
/// # use mmm::abstractions::ClaudeClient;
/// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
/// // Check if Claude CLI is available
/// client.check_availability().await?;
///
/// // Execute code review workflow
/// let success = client.code_review(true).await?;
/// if success {
///     // Implement improvements based on review
///     client.implement_spec("iteration-123", false).await?;
///     
///     // Run final linting
///     client.lint(false).await?;
/// }
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait ClaudeClient: Send + Sync {
    /// Execute a Claude command with automatic retry logic
    ///
    /// Executes a Claude CLI command with the specified arguments and environment
    /// variables. Automatically retries on transient failures (network issues,
    /// rate limits, timeouts) up to the specified maximum retry count.
    ///
    /// # Arguments
    ///
    /// * `command` - The base command to execute (usually "claude")
    /// * `args` - Command-line arguments to pass
    /// * `env_vars` - Optional environment variables to set
    /// * `max_retries` - Maximum number of retry attempts for transient failures
    /// * `verbose` - Whether to enable verbose output logging
    ///
    /// # Returns
    ///
    /// Returns the process output including stdout, stderr, and exit status.
    /// On success, the exit status will be 0. On failure, returns an error
    /// describing the issue.
    ///
    /// # Errors
    ///
    /// - Command not found (Claude CLI not installed)
    /// - Non-transient command failures (syntax errors, etc.)
    /// - Transient failures that exceed the retry limit
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::abstractions::ClaudeClient;
    /// # use std::collections::HashMap;
    /// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
    /// let output = client.execute_command(
    ///     "claude",
    ///     &["code", "--help"],
    ///     None,
    ///     3,
    ///     false
    /// ).await?;
    ///
    /// println!("Output: {}", String::from_utf8_lossy(&output.stdout));
    /// # Ok(())
    /// # }
    /// ```
    async fn execute_command(
        &self,
        command: &str,
        args: &[&str],
        env_vars: Option<HashMap<String, String>>,
        max_retries: u32,
        verbose: bool,
    ) -> Result<std::process::Output>;

    /// Check if Claude CLI is available and properly configured
    ///
    /// Verifies that the Claude CLI is installed, accessible, and ready to use.
    /// This should be called before attempting any other operations to ensure
    /// the environment is properly set up.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if Claude CLI is available and working, or an error
    /// describing the availability issue.
    ///
    /// # Errors
    ///
    /// - Claude CLI not found in PATH
    /// - Claude CLI not properly authenticated
    /// - Network connectivity issues
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::abstractions::ClaudeClient;
    /// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
    /// match client.check_availability().await {
    ///     Ok(()) => println!("Claude CLI is ready!"),
    ///     Err(e) => eprintln!("Claude CLI unavailable: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn check_availability(&self) -> Result<()>;

    /// Execute the /mmm-code-review command for automated code analysis
    ///
    /// Runs the MMM code review workflow which analyzes code quality,
    /// identifies issues, and suggests improvements. This is typically
    /// the first step in an improvement iteration.
    ///
    /// # Arguments
    ///
    /// * `verbose` - Enable detailed logging and output
    ///
    /// # Returns
    ///
    /// Returns `true` if the code review completed successfully and found
    /// actionable improvements, `false` if no improvements were identified.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::abstractions::ClaudeClient;
    /// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
    /// let has_improvements = client.code_review(true).await?;
    /// if has_improvements {
    ///     println!("Code review found improvements to implement");
    /// } else {
    ///     println!("Code is in good shape, no issues found");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn code_review(&self, verbose: bool) -> Result<bool>;

    /// Execute the /mmm-implement-spec command to implement improvements
    ///
    /// Runs the MMM implementation workflow which takes a specification ID
    /// (typically generated by code review) and implements the suggested
    /// improvements automatically.
    ///
    /// # Arguments
    ///
    /// * `spec_id` - Unique identifier for the improvement specification
    /// * `verbose` - Enable detailed logging and output
    ///
    /// # Returns
    ///
    /// Returns `true` if the implementation was successful, `false` if
    /// implementation failed or was not possible.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::abstractions::ClaudeClient;
    /// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
    /// let spec_id = "iteration-1234-performance";
    /// let success = client.implement_spec(spec_id, false).await?;
    /// if success {
    ///     println!("Successfully implemented improvements from {}", spec_id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn implement_spec(&self, spec_id: &str, verbose: bool) -> Result<bool>;

    /// Execute the /mmm-lint command for code style and quality checks
    ///
    /// Runs automated linting and code formatting to ensure code quality
    /// and consistency. This is typically used as a final cleanup step
    /// after implementing improvements.
    ///
    /// # Arguments
    ///
    /// * `verbose` - Enable detailed logging and output
    ///
    /// # Returns
    ///
    /// Returns `true` if linting completed successfully and any issues
    /// were fixed, `false` if linting failed or found unfixable issues.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use mmm::abstractions::ClaudeClient;
    /// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
    /// let clean = client.lint(true).await?;
    /// if clean {
    ///     println!("Code passes all linting checks");
    /// } else {
    ///     println!("Linting found issues that need manual attention");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn lint(&self, verbose: bool) -> Result<bool>;
}

/// Production implementation of `ClaudeClient` using actual Claude CLI
///
/// This implementation executes real Claude CLI commands through the subprocess
/// system. It provides automatic retry logic for transient failures and
/// comprehensive error handling for robust operation in production environments.
///
/// # Features
///
/// - **Automatic Retries**: Detects and retries transient failures
/// - **Environment Isolation**: Properly manages environment variables
/// - **Error Classification**: Distinguishes between transient and permanent failures
/// - **Subprocess Management**: Uses the shared subprocess abstraction layer
///
/// # Examples
///
/// ```rust
/// use mmm::abstractions::{ClaudeClient, RealClaudeClient};
///
/// # async fn example() -> anyhow::Result<()> {
/// let client = RealClaudeClient::new();
///
/// // Check availability before use
/// client.check_availability().await?;
///
/// // Execute code review
/// let success = client.code_review(false).await?;
/// # Ok(())
/// # }
/// ```
pub struct RealClaudeClient {
    subprocess: SubprocessManager,
}

impl RealClaudeClient {
    /// Create a new `RealClaudeClient` instance with production subprocess manager
    ///
    /// Creates a new client configured for production use with the default
    /// subprocess manager. This is the standard way to create a client for
    /// actual Claude CLI operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mmm::abstractions::RealClaudeClient;
    ///
    /// let client = RealClaudeClient::new();
    /// // Ready to use for Claude CLI operations
    /// ```
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

    async fn code_review(&self, verbose: bool) -> Result<bool> {
        println!("🤖 Running /mmm-code-review...");

        let mut env_vars = HashMap::new();
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

    async fn code_review(&self, verbose: bool) -> Result<bool> {
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
        let result = mock.code_review(false).await.unwrap();
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
        let result = mock.code_review(false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_claude_error_response() {
        let mock = MockClaudeClient::new();

        // Add error response
        mock.add_error_response("rate limit exceeded", 1).await;

        // Test error handling
        let result = mock.code_review(false).await.unwrap();
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

    #[tokio::test]
    async fn test_add_error_response() {
        let mock = MockClaudeClient::new();
        mock.add_error_response("test error", 1).await;
        
        let result = mock.execute_command("/test", &[], None, 1, false).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Check that status is not success (exit code is non-zero)
        assert!(!output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stderr), "test error");
    }
    
    #[tokio::test]
    async fn test_add_success_response() {
        let mock = MockClaudeClient::new();
        mock.add_success_response("success output").await;
        
        let result = mock.execute_command("/test", &[], None, 1, false).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "success output");
    }
}
