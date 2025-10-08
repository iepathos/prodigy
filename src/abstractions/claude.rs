//! Claude CLI abstraction layer
//!
//! Provides trait-based abstraction for Claude CLI commands to enable
//! testing without actual Claude CLI installation.

use crate::abstractions::exit_status::ExitStatusExt;
use crate::subprocess::{
    ClaudeRunner as SubprocessClaudeRunner, ProcessCommand, ProcessCommandBuilder,
    SubprocessManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Classification of command execution errors
#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandErrorType {
    /// Transient error that may succeed on retry (rate limits, timeouts, network issues)
    TransientError,
    /// Command not found error (Claude CLI not installed)
    CommandNotFound,
    /// Permanent error that won't succeed on retry
    PermanentError,
}

/// Classification of command output results
#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputClassification {
    /// Command succeeded
    Success,
    /// Transient failure that may succeed on retry
    TransientFailure(String),
    /// Permanent failure that won't succeed on retry
    PermanentFailure,
}

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
/// use prodigy::abstractions::ClaudeClient;
/// use std::collections::HashMap;
///
/// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
/// let env_vars = HashMap::new();
///
/// let output = client.execute_command(
///     "claude",
///     &["code", "--command", "/prodigy-code-review"],
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
/// # use prodigy::abstractions::ClaudeClient;
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
    /// # use prodigy::abstractions::ClaudeClient;
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
    /// # use prodigy::abstractions::ClaudeClient;
    /// # async fn example(client: &dyn ClaudeClient) -> anyhow::Result<()> {
    /// match client.check_availability().await {
    ///     Ok(()) => println!("Claude CLI is ready!"),
    ///     Err(e) => eprintln!("Claude CLI unavailable: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn check_availability(&self) -> Result<()>;

    /// Execute the /prodigy-code-review command for automated code analysis
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
    /// # use prodigy::abstractions::ClaudeClient;
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

    /// Execute the /prodigy-implement-spec command to implement improvements
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
    /// # use prodigy::abstractions::ClaudeClient;
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

    /// Execute the /prodigy-lint command for code style and quality checks
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
    /// # use prodigy::abstractions::ClaudeClient;
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
/// use prodigy::abstractions::{ClaudeClient, RealClaudeClient};
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
    /// use prodigy::abstractions::RealClaudeClient;
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

    /// Calculate exponential backoff delay for retry attempts
    ///
    /// Returns a Duration with exponential backoff: 2^min(attempt, 3) seconds.
    /// This caps the delay at 8 seconds (2^3) to avoid excessive wait times.
    fn calculate_retry_delay(attempt: u32) -> std::time::Duration {
        std::time::Duration::from_secs(2u64.pow(attempt.min(3)))
    }

    /// Classify a command error based on error type and stderr content
    ///
    /// This function determines whether an error is transient (should retry),
    /// a command-not-found error (fatal), or a permanent error (should not retry).
    fn classify_command_error(
        error: &crate::subprocess::ProcessError,
        stderr: &str,
    ) -> CommandErrorType {
        use crate::subprocess::ProcessError;

        match error {
            ProcessError::CommandNotFound(_) => CommandErrorType::CommandNotFound,
            _ => {
                if Self::is_transient_error(stderr) {
                    CommandErrorType::TransientError
                } else {
                    CommandErrorType::PermanentError
                }
            }
        }
    }

    /// Determine if an error should be retried
    fn should_retry_error(error_type: &CommandErrorType, attempt: u32, max_retries: u32) -> bool {
        match error_type {
            CommandErrorType::TransientError => attempt < max_retries,
            CommandErrorType::CommandNotFound | CommandErrorType::PermanentError => false,
        }
    }

    /// Build a Claude command with arguments and environment variables
    ///
    /// This is a pure function that constructs a ProcessCommand for Claude CLI
    /// execution. It takes command arguments and optional environment variables,
    /// returning a fully configured ProcessCommand.
    fn build_claude_command(
        args: &[&str],
        env_vars: Option<&HashMap<String, String>>,
    ) -> ProcessCommand {
        let mut builder = ProcessCommandBuilder::new("claude");
        for arg in args {
            builder = builder.arg(arg);
        }

        if let Some(vars) = env_vars {
            for (key, value) in vars {
                builder = builder.env(key, value);
            }
        }

        builder.build()
    }

    /// Classify a command output result
    ///
    /// Determines whether the output represents a success, transient failure,
    /// or permanent failure based on the exit status and stderr content.
    fn classify_output_result(
        output: &crate::subprocess::runner::ProcessOutput,
    ) -> OutputClassification {
        if output.status.success() {
            OutputClassification::Success
        } else if Self::is_transient_error(&output.stderr) {
            OutputClassification::TransientFailure(output.stderr.clone())
        } else {
            OutputClassification::PermanentFailure
        }
    }

    /// Determine if a retry should continue based on output classification
    ///
    /// Returns true if the classification indicates a transient failure and
    /// we haven't exhausted our retry attempts.
    fn should_continue_retry(
        classification: &OutputClassification,
        attempt: u32,
        max_retries: u32,
    ) -> bool {
        matches!(classification, OutputClassification::TransientFailure(_))
            && attempt < max_retries
    }

    /// Handle process output result and determine next action
    ///
    /// Returns Ok(Some(output)) if we should return the result,
    /// Ok(None) if we should retry, or Err if there's a fatal error.
    fn handle_process_output(
        output: crate::subprocess::runner::ProcessOutput,
        attempt: u32,
        max_retries: u32,
        verbose: bool,
    ) -> (Option<std::process::Output>, Option<String>) {
        let classification = Self::classify_output_result(&output);

        match classification {
            OutputClassification::Success => (Some(Self::convert_to_std_output(output)), None),
            OutputClassification::TransientFailure(ref stderr) => {
                if Self::should_continue_retry(&classification, attempt, max_retries) {
                    if verbose {
                        eprintln!(
                            "⚠️  Transient error detected: {}",
                            stderr.lines().next().unwrap_or("Unknown error")
                        );
                    }
                    (None, Some(stderr.clone()))
                } else {
                    // Exhausted retries - return with failure
                    (Some(Self::convert_to_std_output(output)), None)
                }
            }
            OutputClassification::PermanentFailure => {
                (Some(Self::convert_to_std_output(output)), None)
            }
        }
    }

    /// Handle process execution error and determine next action
    ///
    /// Returns Ok(None) if we should retry, or Err if it's a fatal error.
    fn handle_process_error(
        error: crate::subprocess::ProcessError,
        attempt: u32,
        max_retries: u32,
        verbose: bool,
    ) -> Result<(Option<std::process::Output>, Option<String>)> {
        let error_type = Self::classify_command_error(&error, "");

        if error_type == CommandErrorType::CommandNotFound {
            return Err(anyhow::anyhow!("Claude CLI not found: {}", error));
        }

        if Self::should_retry_error(&error_type, attempt, max_retries) {
            if verbose {
                eprintln!("⚠️  IO error: {error}");
            }
            Ok((None, Some(error.to_string())))
        } else {
            Err(anyhow::anyhow!("Failed to execute command: {}", error))
        }
    }

    /// Convert ProcessOutput to std::process::Output
    ///
    /// Converts the subprocess abstraction's output format to the standard library's
    /// process output format for compatibility with existing code.
    fn convert_to_std_output(
        output: crate::subprocess::runner::ProcessOutput,
    ) -> std::process::Output {
        let exit_code = output.status.code().unwrap_or(1);
        std::process::Output {
            status: std::process::ExitStatus::from_raw(exit_code),
            stdout: output.stdout.into_bytes(),
            stderr: output.stderr.into_bytes(),
        }
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
        use tokio::time::sleep;

        let mut attempt = 0;
        let mut last_error = None;

        while attempt <= max_retries {
            if attempt > 0 {
                let delay = Self::calculate_retry_delay(attempt);
                if verbose {
                    println!(
                        "⏳ Retrying {command} after {delay:?} (attempt {attempt}/{max_retries})"
                    );
                }
                sleep(delay).await;
            }

            let cmd = Self::build_claude_command(args, env_vars.as_ref());
            let result = self.subprocess.runner().run(cmd).await;

            match result {
                Ok(output) => {
                    let (maybe_output, maybe_error) =
                        Self::handle_process_output(output, attempt, max_retries, verbose);

                    if let Some(output) = maybe_output {
                        return Ok(output);
                    }

                    last_error = maybe_error;
                }
                Err(error) => {
                    let (maybe_output, maybe_error) =
                        Self::handle_process_error(error, attempt, max_retries, verbose)?;

                    if let Some(output) = maybe_output {
                        return Ok(output);
                    }

                    last_error = maybe_error;
                }
            }

            attempt += 1;
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
        println!("🤖 Running /prodigy-code-review...");

        let mut env_vars = HashMap::new();
        if std::env::var("PRODIGY_AUTOMATION").unwrap_or_default() == "true" {
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
        }

        let args = vec![
            "--dangerously-skip-permissions",
            "--print",
            "/prodigy-code-review",
        ];

        let output = self
            .execute_command("/prodigy-code-review", &args, Some(env_vars), 2, verbose)
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
        println!("🔧 Running /prodigy-implement-spec {spec_id}...");

        let mut env_vars = HashMap::new();
        if std::env::var("PRODIGY_AUTOMATION").unwrap_or_default() == "true" {
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
        }

        let args = vec![
            "--dangerously-skip-permissions",
            "--print",
            "/prodigy-implement-spec",
            spec_id,
        ];

        let output = self
            .execute_command(
                &format!("/prodigy-implement-spec {spec_id}"),
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
        println!("🧹 Running /prodigy-lint...");

        let mut env_vars = HashMap::new();
        if std::env::var("PRODIGY_AUTOMATION").unwrap_or_default() == "true" {
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
        }

        let args = vec!["--dangerously-skip-permissions", "--print", "/prodigy-lint"];

        let output = self
            .execute_command("/prodigy-lint", &args, Some(env_vars), 2, verbose)
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
            "/prodigy-code-review",
        ];

        let output = self
            .execute_command("/prodigy-code-review", &args, None, 2, verbose)
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
            "/prodigy-implement-spec",
            spec_id,
        ];

        let output = self
            .execute_command(
                &format!("/prodigy-implement-spec {spec_id}"),
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

        let args = vec!["--dangerously-skip-permissions", "--print", "/prodigy-lint"];

        let output = self
            .execute_command("/prodigy-lint", &args, None, 2, verbose)
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
        assert_eq!(commands[0].0, "/prodigy-code-review");
        assert_eq!(commands[1].0, "/prodigy-implement-spec test-spec-123");
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

    #[test]
    fn test_calculate_retry_delay() {
        use std::time::Duration;

        // Test exponential backoff
        assert_eq!(
            RealClaudeClient::calculate_retry_delay(0),
            Duration::from_secs(1)
        ); // 2^0 = 1
        assert_eq!(
            RealClaudeClient::calculate_retry_delay(1),
            Duration::from_secs(2)
        ); // 2^1 = 2
        assert_eq!(
            RealClaudeClient::calculate_retry_delay(2),
            Duration::from_secs(4)
        ); // 2^2 = 4
        assert_eq!(
            RealClaudeClient::calculate_retry_delay(3),
            Duration::from_secs(8)
        ); // 2^3 = 8

        // Test capping at 2^3 for attempts > 3
        assert_eq!(
            RealClaudeClient::calculate_retry_delay(4),
            Duration::from_secs(8)
        );
        assert_eq!(
            RealClaudeClient::calculate_retry_delay(10),
            Duration::from_secs(8)
        );
    }

    #[test]
    fn test_classify_command_error() {
        use crate::subprocess::ProcessError;

        // Test CommandNotFound detection
        let error = ProcessError::CommandNotFound("claude".to_string());
        assert_eq!(
            RealClaudeClient::classify_command_error(&error, ""),
            CommandErrorType::CommandNotFound
        );

        // Test transient error detection with Io error
        let io_error = std::io::Error::other("IO error");
        let error = ProcessError::Io(io_error);
        assert_eq!(
            RealClaudeClient::classify_command_error(&error, "rate limit exceeded"),
            CommandErrorType::TransientError
        );

        let io_error = std::io::Error::other("IO error");
        let error = ProcessError::Io(io_error);
        assert_eq!(
            RealClaudeClient::classify_command_error(&error, "Connection timeout"),
            CommandErrorType::TransientError
        );

        let io_error = std::io::Error::other("IO error");
        let error = ProcessError::Io(io_error);
        assert_eq!(
            RealClaudeClient::classify_command_error(&error, "HTTP 503 Service Unavailable"),
            CommandErrorType::TransientError
        );

        // Test permanent error detection
        let io_error = std::io::Error::other("IO error");
        let error = ProcessError::Io(io_error);
        assert_eq!(
            RealClaudeClient::classify_command_error(&error, "Syntax error"),
            CommandErrorType::PermanentError
        );

        let io_error = std::io::Error::other("IO error");
        let error = ProcessError::Io(io_error);
        assert_eq!(
            RealClaudeClient::classify_command_error(&error, "Unknown command"),
            CommandErrorType::PermanentError
        );
    }

    #[test]
    fn test_should_retry_error() {
        // Test TransientError with retries available
        assert!(RealClaudeClient::should_retry_error(
            &CommandErrorType::TransientError,
            0,
            3
        ));
        assert!(RealClaudeClient::should_retry_error(
            &CommandErrorType::TransientError,
            2,
            3
        ));

        // Test TransientError with retries exhausted
        assert!(!RealClaudeClient::should_retry_error(
            &CommandErrorType::TransientError,
            3,
            3
        ));
        assert!(!RealClaudeClient::should_retry_error(
            &CommandErrorType::TransientError,
            5,
            3
        ));

        // Test CommandNotFound never retries
        assert!(!RealClaudeClient::should_retry_error(
            &CommandErrorType::CommandNotFound,
            0,
            3
        ));
        assert!(!RealClaudeClient::should_retry_error(
            &CommandErrorType::CommandNotFound,
            1,
            3
        ));

        // Test PermanentError never retries
        assert!(!RealClaudeClient::should_retry_error(
            &CommandErrorType::PermanentError,
            0,
            3
        ));
        assert!(!RealClaudeClient::should_retry_error(
            &CommandErrorType::PermanentError,
            2,
            3
        ));
    }

    #[test]
    fn test_build_claude_command() {
        use std::collections::HashMap;

        // Test basic command building
        let args = vec!["--help"];
        let cmd = RealClaudeClient::build_claude_command(&args, None);
        assert_eq!(cmd.program, "claude");
        assert_eq!(cmd.args, vec!["--help"]);
        assert!(cmd.env.is_empty());

        // Test command with multiple arguments
        let args = vec!["--dangerously-skip-permissions", "--print", "/test"];
        let cmd = RealClaudeClient::build_claude_command(&args, None);
        assert_eq!(cmd.program, "claude");
        assert_eq!(
            cmd.args,
            vec!["--dangerously-skip-permissions", "--print", "/test"]
        );

        // Test command with environment variables
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
        env_vars.insert("VERBOSE".to_string(), "1".to_string());

        let args = vec!["/lint"];
        let cmd = RealClaudeClient::build_claude_command(&args, Some(&env_vars));
        assert_eq!(cmd.program, "claude");
        assert_eq!(cmd.args, vec!["/lint"]);
        assert_eq!(cmd.env.len(), 2);
        assert_eq!(
            cmd.env.get("PRODIGY_AUTOMATION"),
            Some(&"true".to_string())
        );
        assert_eq!(cmd.env.get("VERBOSE"), Some(&"1".to_string()));

        // Test command with empty args and no environment variables
        let args: Vec<&str> = vec![];
        let cmd = RealClaudeClient::build_claude_command(&args, None);
        assert_eq!(cmd.program, "claude");
        assert!(cmd.args.is_empty());
        assert!(cmd.env.is_empty());
    }

    #[test]
    fn test_classify_output_result() {
        use crate::subprocess::runner::{ExitStatus, ProcessOutput};
        use std::time::Duration;

        // Test successful output
        let output = ProcessOutput {
            status: ExitStatus::Success,
            stdout: "success".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(1),
        };
        assert_eq!(
            RealClaudeClient::classify_output_result(&output),
            OutputClassification::Success
        );

        // Test transient failure
        let output = ProcessOutput {
            status: ExitStatus::Error(1),
            stdout: String::new(),
            stderr: "rate limit exceeded".to_string(),
            duration: Duration::from_secs(1),
        };
        assert_eq!(
            RealClaudeClient::classify_output_result(&output),
            OutputClassification::TransientFailure("rate limit exceeded".to_string())
        );

        // Test permanent failure
        let output = ProcessOutput {
            status: ExitStatus::Error(1),
            stdout: String::new(),
            stderr: "invalid argument".to_string(),
            duration: Duration::from_secs(1),
        };
        assert_eq!(
            RealClaudeClient::classify_output_result(&output),
            OutputClassification::PermanentFailure
        );
    }

    #[test]
    fn test_should_continue_retry() {
        // Test transient failure with retries available
        let classification = OutputClassification::TransientFailure("error".to_string());
        assert!(RealClaudeClient::should_continue_retry(
            &classification,
            0,
            3
        ));
        assert!(RealClaudeClient::should_continue_retry(
            &classification,
            2,
            3
        ));

        // Test transient failure with retries exhausted
        assert!(!RealClaudeClient::should_continue_retry(
            &classification,
            3,
            3
        ));
        assert!(!RealClaudeClient::should_continue_retry(
            &classification,
            5,
            3
        ));

        // Test permanent failure never retries
        let classification = OutputClassification::PermanentFailure;
        assert!(!RealClaudeClient::should_continue_retry(
            &classification,
            0,
            3
        ));
        assert!(!RealClaudeClient::should_continue_retry(
            &classification,
            2,
            3
        ));

        // Test success never retries
        let classification = OutputClassification::Success;
        assert!(!RealClaudeClient::should_continue_retry(
            &classification,
            0,
            3
        ));
        assert!(!RealClaudeClient::should_continue_retry(
            &classification,
            2,
            3
        ));
    }

    #[test]
    fn test_convert_to_std_output() {
        use crate::subprocess::runner::{ExitStatus, ProcessOutput};
        use std::time::Duration;

        // Test successful output conversion
        let process_output = ProcessOutput {
            status: ExitStatus::Success,
            stdout: "test output".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(1),
        };

        let std_output = RealClaudeClient::convert_to_std_output(process_output);
        assert!(std_output.status.success());
        assert_eq!(String::from_utf8_lossy(&std_output.stdout), "test output");
        assert!(std_output.stderr.is_empty());

        // Test output with exit code
        let process_output = ProcessOutput {
            status: ExitStatus::Error(42),
            stdout: String::new(),
            stderr: "error message".to_string(),
            duration: Duration::from_secs(1),
        };

        let std_output = RealClaudeClient::convert_to_std_output(process_output);
        assert!(!std_output.status.success());
        assert!(std_output.stdout.is_empty());
        assert_eq!(String::from_utf8_lossy(&std_output.stderr), "error message");

        // Test stdout/stderr byte conversion
        let process_output = ProcessOutput {
            status: ExitStatus::Success,
            stdout: "stdout content".to_string(),
            stderr: "stderr content".to_string(),
            duration: Duration::from_secs(1),
        };

        let std_output = RealClaudeClient::convert_to_std_output(process_output);
        assert_eq!(
            String::from_utf8_lossy(&std_output.stdout),
            "stdout content"
        );
        assert_eq!(
            String::from_utf8_lossy(&std_output.stderr),
            "stderr content"
        );
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
