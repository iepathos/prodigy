//! Unified command executor implementation

use super::command::*;
use super::output::{OutputProcessor, ProcessOutput, ProcessedOutput};
use super::process::{ProcessManager, UnifiedProcess};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Unified command executor that handles all command types
pub struct UnifiedCommandExecutor {
    process_manager: Arc<ProcessManager>,
    _output_processor: Arc<OutputProcessor>,
    observability: Arc<dyn ObservabilityCollector>,
    resource_monitor: Arc<ResourceMonitor>,
}

impl UnifiedCommandExecutor {
    pub fn new(
        process_manager: Arc<ProcessManager>,
        _output_processor: Arc<OutputProcessor>,
        observability: Arc<dyn ObservabilityCollector>,
        resource_monitor: Arc<ResourceMonitor>,
    ) -> Self {
        Self {
            process_manager,
            _output_processor,
            observability,
            resource_monitor,
        }
    }

    /// Validate a command request before execution
    async fn validate_request(&self, request: &CommandRequest) -> Result<()> {
        // Validate command spec
        match &request.spec {
            CommandSpec::Claude { command, .. } => {
                if command.is_empty() {
                    anyhow::bail!("Claude command cannot be empty");
                }
            }
            CommandSpec::Shell { command, .. } => {
                if command.is_empty() {
                    anyhow::bail!("Shell command cannot be empty");
                }
            }
            CommandSpec::Test { command, .. } => {
                if command.is_empty() {
                    anyhow::bail!("Test command cannot be empty");
                }
            }
            CommandSpec::Handler { .. } => {
                // Handler validation is done elsewhere
            }
        }

        // Validate resource limits if specified
        if let Some(limits) = &request.execution_config.resource_limits {
            self.resource_monitor.validate_limits(limits).await?;
        }

        Ok(())
    }

    /// Execute command with full context
    async fn execute_with_context(
        &self,
        request: CommandRequest,
        mut exec_context: ExecutionContextInternal,
    ) -> Result<CommandResult> {
        let start_time = Instant::now();

        // Convert command spec to executable command
        let executable = request
            .spec
            .to_executable_command(&request.context)
            .with_context(|| "Failed to create executable command")?;

        // Apply execution configuration
        let configured_executable =
            self.apply_execution_config(executable, &request.execution_config)?;

        // Spawn process with unified process management
        let mut process = self
            .process_manager
            .spawn(configured_executable, &exec_context)
            .await
            .with_context(|| "Failed to spawn process")?;

        // Setup timeout handling
        let timeout_future = self.setup_timeout(&request.execution_config.timeout);

        // Wait for completion or timeout
        let execution_result = tokio::select! {
            result = process.wait() => {
                let exit_status = result?;
                let output = self.collect_output(&mut process, &request.execution_config.capture_output).await?;
                self.create_result_from_process(exit_status, output, start_time.elapsed())
            }
            _ = timeout_future => {
                process.kill().await?;
                Err(anyhow::anyhow!("Command execution timed out"))
            }
        };

        // Post-execution processing
        let final_result = self
            .post_process_result(execution_result?, &request)
            .await?;

        // Update execution context
        exec_context.update_from_result(&final_result);

        Ok(final_result)
    }

    /// Apply execution configuration to executable command
    fn apply_execution_config(
        &self,
        mut executable: ExecutableCommand,
        config: &ExecutionConfig,
    ) -> Result<ExecutableCommand> {
        // Apply working directory
        if let Some(ref dir) = config.working_dir {
            executable.working_dir = Some(dir.clone());
        }

        // Apply environment variables
        for (key, value) in &config.env {
            executable.env.insert(key.clone(), value.clone());
        }

        // Apply resource limits
        if let Some(ref limits) = config.resource_limits {
            executable.resource_requirements = ResourceRequirements {
                estimated_memory_mb: limits.max_memory_bytes.map(|b| b / 1_048_576),
                estimated_cpu_cores: limits.max_cpu_percent.map(|p| p / 100.0),
                estimated_duration: None,
            };
        }

        Ok(executable)
    }

    /// Setup timeout future
    async fn setup_timeout(&self, timeout: &Option<Duration>) {
        if let Some(duration) = timeout {
            tokio::time::sleep(*duration).await;
        } else {
            // Sleep forever if no timeout
            std::future::pending::<()>().await;
        }
    }

    /// Collect output after process completion
    async fn collect_output(
        &self,
        process: &mut UnifiedProcess,
        capture_mode: &CaptureOutputMode,
    ) -> Result<ProcessOutput> {
        match capture_mode {
            CaptureOutputMode::None => Ok(ProcessOutput::empty()),
            CaptureOutputMode::Stdout => {
                let stdout = self.read_stream(process.stdout()).await?;
                Ok(ProcessOutput::new().with_stdout(stdout))
            }
            CaptureOutputMode::Stderr => {
                let stderr = self.read_stream(process.stderr()).await?;
                Ok(ProcessOutput::new().with_stderr(stderr))
            }
            CaptureOutputMode::Both => {
                let stdout = self.read_stream(process.stdout()).await?;
                let stderr = self.read_stream(process.stderr()).await?;
                Ok(ProcessOutput::new().with_stdout(stdout).with_stderr(stderr))
            }
            CaptureOutputMode::Structured => self.process_structured_output(process).await,
        }
    }

    /// Read output stream
    async fn read_stream(
        &self,
        stream: &mut Option<impl tokio::io::AsyncRead + Unpin>,
    ) -> Result<String> {
        use tokio::io::AsyncReadExt;

        if let Some(ref mut stream) = stream {
            let mut buffer = String::new();
            stream.read_to_string(&mut buffer).await?;
            Ok(buffer)
        } else {
            Ok(String::new())
        }
    }

    /// Process structured output
    async fn process_structured_output(
        &self,
        process: &mut UnifiedProcess,
    ) -> Result<ProcessOutput> {
        let stdout = self.read_stream(process.stdout()).await?;

        // Try to parse as JSON
        let structured_data = serde_json::from_str(&stdout).ok();

        Ok(ProcessOutput::new()
            .with_stdout(stdout)
            .with_structured_data(structured_data))
    }

    /// Create result from process execution
    fn create_result_from_process(
        &self,
        exit_status: std::process::ExitStatus,
        output: ProcessOutput,
        duration: Duration,
    ) -> Result<CommandResult> {
        let exit_code = exit_status.code();
        let success = exit_status.success();

        let status = if success {
            CommandStatus::Success
        } else if let Some(code) = exit_code {
            CommandStatus::Failed {
                reason: FailureReason::NonZeroExit(code),
                retryable: true,
            }
        } else {
            CommandStatus::Failed {
                reason: FailureReason::ProcessError("Process terminated by signal".to_string()),
                retryable: false,
            }
        };

        Ok(CommandResult {
            command_id: Uuid::new_v4().to_string(),
            command_spec: CommandSpec::Shell {
                command: String::new(),
                shell: None,
                working_dir: None,
                env: None,
            },
            status,
            output: ProcessedOutput {
                content: output,
                format: OutputFormat::PlainText,
                processing_duration: Duration::from_secs(0),
                warnings: Vec::new(),
            },
            execution_time: duration,
            resource_usage: ResourceUsage::default(),
            exit_code,
            error: None,
            validation_result: None,
            metadata: ExecutionMetadata::new(),
        })
    }

    /// Post-process command result
    async fn post_process_result(
        &self,
        mut result: CommandResult,
        request: &CommandRequest,
    ) -> Result<CommandResult> {
        // Apply command type specific post-processing
        match &request.spec {
            CommandSpec::Claude { .. } => {
                result = self.post_process_claude_result(result, request).await?;
            }
            CommandSpec::Shell { .. } => {
                result = self.post_process_shell_result(result, request).await?;
            }
            CommandSpec::Test {
                expected_exit_code, ..
            } => {
                result = self
                    .post_process_test_result(result, *expected_exit_code)
                    .await?;
            }
            CommandSpec::Handler { .. } => {
                result = self.post_process_handler_result(result, request).await?;
            }
        }

        // Apply validation if configured
        if let Some(validation_config) = &request.execution_config.validation {
            result = self.apply_validation(result, validation_config).await?;
        }

        Ok(result)
    }

    async fn post_process_claude_result(
        &self,
        mut result: CommandResult,
        _request: &CommandRequest,
    ) -> Result<CommandResult> {
        // Claude-specific post-processing
        result.command_spec = _request.spec.clone();
        Ok(result)
    }

    async fn post_process_shell_result(
        &self,
        mut result: CommandResult,
        _request: &CommandRequest,
    ) -> Result<CommandResult> {
        // Shell-specific post-processing
        result.command_spec = _request.spec.clone();
        Ok(result)
    }

    async fn post_process_test_result(
        &self,
        mut result: CommandResult,
        expected_exit_code: Option<i32>,
    ) -> Result<CommandResult> {
        // Test-specific post-processing
        if let Some(expected) = expected_exit_code {
            if result.exit_code != Some(expected) {
                result.status = CommandStatus::Failed {
                    reason: FailureReason::NonZeroExit(result.exit_code.unwrap_or(-1)),
                    retryable: false,
                };
            }
        }
        Ok(result)
    }

    async fn post_process_handler_result(
        &self,
        mut result: CommandResult,
        _request: &CommandRequest,
    ) -> Result<CommandResult> {
        // Handler-specific post-processing
        result.command_spec = _request.spec.clone();
        Ok(result)
    }

    async fn apply_validation(
        &self,
        mut result: CommandResult,
        validation: &ValidationConfig,
    ) -> Result<CommandResult> {
        let mut issues = Vec::new();

        // Check expected pattern
        if let Some(ref pattern) = validation.expected_pattern {
            let re = regex::Regex::new(pattern)?;
            if let Some(ref stdout) = result.output.content.stdout {
                if !re.is_match(stdout) {
                    issues.push(ValidationIssue {
                        level: ValidationLevel::Error,
                        message: format!("Output does not match expected pattern: {}", pattern),
                    });
                }
            }
        }

        // Check forbidden patterns
        if let Some(ref patterns) = validation.forbidden_patterns {
            for pattern in patterns {
                let re = regex::Regex::new(pattern)?;
                if let Some(ref stdout) = result.output.content.stdout {
                    if re.is_match(stdout) {
                        issues.push(ValidationIssue {
                            level: ValidationLevel::Error,
                            message: format!("Output contains forbidden pattern: {}", pattern),
                        });
                    }
                }
            }
        }

        if !issues.is_empty() {
            result.validation_result = Some(ValidationResult {
                passed: false,
                issues: issues.clone(),
            });
            result.status = CommandStatus::Failed {
                reason: FailureReason::ValidationFailed(issues),
                retryable: false,
            };
        } else {
            result.validation_result = Some(ValidationResult {
                passed: true,
                issues: Vec::new(),
            });
        }

        Ok(result)
    }
}

/// Command executor trait
#[async_trait]
impl CommandExecutor for UnifiedCommandExecutor {
    async fn execute(&self, request: CommandRequest) -> Result<CommandResult> {
        let execution_id = Uuid::new_v4();

        // Pre-execution validation
        self.validate_request(&request).await?;

        // Create execution context
        let exec_context = ExecutionContextBuilder::new()
            .with_id(execution_id)
            .with_request(&request)
            .with_resource_limits(&request.execution_config.resource_limits)
            .build()?;

        // Record execution start
        self.observability.record_command_start(&exec_context).await;

        // Execute command with unified pipeline
        let result = self.execute_with_context(request, exec_context).await;

        // Record execution completion
        self.observability.record_command_complete(&result).await;

        result
    }

    async fn validate(&self, request: &CommandRequest) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Basic validation
        if let Err(e) = self.validate_request(request).await {
            issues.push(ValidationIssue {
                level: ValidationLevel::Error,
                message: e.to_string(),
            });
        }

        Ok(issues)
    }

    fn supports(&self, _command_type: &CommandType) -> bool {
        // Unified executor supports all command types
        true
    }

    fn capabilities(&self) -> ExecutorCapabilities {
        ExecutorCapabilities {
            supported_command_types: vec![
                CommandType::Claude,
                CommandType::Shell,
                CommandType::Test,
                CommandType::Handler,
            ],
            max_concurrent_executions: Some(10),
            supported_output_formats: vec![
                OutputFormat::Json,
                OutputFormat::Yaml,
                OutputFormat::PlainText,
                OutputFormat::Structured,
            ],
            timeout_support: true,
            resource_limiting_support: true,
            security_context_support: true,
        }
    }

    async fn estimate_resources(&self, request: &CommandRequest) -> Result<ResourceEstimate> {
        let estimate = match &request.spec {
            CommandSpec::Claude { .. } => ResourceEstimate {
                estimated_duration: None, // Claude commands are unpredictable
                estimated_memory_mb: Some(512),
                estimated_cpu_percent: Some(10.0),
                estimated_disk_io_mb: Some(100),
                confidence: 0.5,
            },
            CommandSpec::Shell { command, .. } => {
                // Estimate based on command type
                if command.starts_with("git") {
                    ResourceEstimate {
                        estimated_duration: Some(Duration::from_secs(5)),
                        estimated_memory_mb: Some(128),
                        estimated_cpu_percent: Some(20.0),
                        estimated_disk_io_mb: Some(50),
                        confidence: 0.8,
                    }
                } else {
                    ResourceEstimate {
                        estimated_duration: Some(Duration::from_secs(10)),
                        estimated_memory_mb: Some(256),
                        estimated_cpu_percent: Some(50.0),
                        estimated_disk_io_mb: Some(100),
                        confidence: 0.3,
                    }
                }
            }
            CommandSpec::Test { .. } => ResourceEstimate {
                estimated_duration: Some(Duration::from_secs(30)),
                estimated_memory_mb: Some(512),
                estimated_cpu_percent: Some(80.0),
                estimated_disk_io_mb: Some(200),
                confidence: 0.7,
            },
            CommandSpec::Handler { .. } => ResourceEstimate {
                estimated_duration: Some(Duration::from_secs(2)),
                estimated_memory_mb: Some(64),
                estimated_cpu_percent: Some(10.0),
                estimated_disk_io_mb: Some(10),
                confidence: 0.9,
            },
        };

        Ok(estimate)
    }
}

/// Command result structure
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command_id: String,
    pub command_spec: CommandSpec,
    pub status: CommandStatus,
    pub output: ProcessedOutput,
    pub execution_time: Duration,
    pub resource_usage: ResourceUsage,
    pub exit_code: Option<i32>,
    pub error: Option<CommandError>,
    pub validation_result: Option<ValidationResult>,
    pub metadata: ExecutionMetadata,
}

impl CommandResult {
    pub fn is_success(&self) -> bool {
        matches!(self.status, CommandStatus::Success)
    }

    pub fn is_retryable(&self) -> bool {
        match &self.status {
            CommandStatus::Failed { retryable, .. } => *retryable,
            CommandStatus::TimedOut => true,
            CommandStatus::ResourceLimitExceeded => true,
            _ => false,
        }
    }

    pub fn get_output_text(&self) -> Option<&str> {
        self.output.content.stdout.as_deref()
    }

    pub fn get_error_text(&self) -> Option<&str> {
        self.output
            .content
            .stderr
            .as_deref()
            .or(self.output.content.error_summary.as_deref())
    }

    pub fn get_structured_output<T>(&self) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        if let Some(data) = &self.output.content.structured_data {
            Ok(Some(serde_json::from_value(data.clone())?))
        } else {
            Ok(None)
        }
    }
}

/// Command execution status
#[derive(Debug, Clone)]
pub enum CommandStatus {
    Success,
    Failed {
        reason: FailureReason,
        retryable: bool,
    },
    TimedOut,
    Cancelled,
    ResourceLimitExceeded,
}

/// Failure reason enumeration
#[derive(Debug, Clone)]
pub enum FailureReason {
    NonZeroExit(i32),
    ProcessError(String),
    ValidationFailed(Vec<ValidationIssue>),
    SecurityViolation(String),
    ResourceExhaustion(String),
    InternalError(String),
}

/// Resource usage tracking
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub cpu_time: Duration,
    pub wall_clock_time: Duration,
    pub peak_memory_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub network_bytes: Option<u64>,
}

/// Execution metadata
#[derive(Debug, Clone)]
pub struct ExecutionMetadata {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub hostname: String,
    pub process_id: Option<u32>,
    pub parent_process_id: Option<u32>,
    pub working_directory: PathBuf,
    pub environment_hash: String,
    pub git_commit: Option<String>,
    pub observability_trace_id: Option<String>,
}

impl Default for ExecutionMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionMetadata {
    pub fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            started_at: now,
            completed_at: now,
            hostname: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            process_id: Some(std::process::id()),
            parent_process_id: None,
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            environment_hash: String::new(),
            git_commit: None,
            observability_trace_id: None,
        }
    }
}

/// Command error type
#[derive(Debug, Clone)]
pub struct CommandError {
    pub message: String,
    pub details: Option<String>,
}

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub passed: bool,
    pub issues: Vec<ValidationIssue>,
}

/// Validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub message: String,
}

/// Validation level
#[derive(Debug, Clone)]
pub enum ValidationLevel {
    Error,
    Warning,
    Info,
}

/// Executor capabilities
#[derive(Debug, Clone)]
pub struct ExecutorCapabilities {
    pub supported_command_types: Vec<CommandType>,
    pub max_concurrent_executions: Option<usize>,
    pub supported_output_formats: Vec<OutputFormat>,
    pub timeout_support: bool,
    pub resource_limiting_support: bool,
    pub security_context_support: bool,
}

/// Resource estimate for command execution
#[derive(Debug, Clone)]
pub struct ResourceEstimate {
    pub estimated_duration: Option<Duration>,
    pub estimated_memory_mb: Option<u64>,
    pub estimated_cpu_percent: Option<f32>,
    pub estimated_disk_io_mb: Option<u64>,
    pub confidence: f32,
}

/// Command executor trait
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command with the unified pipeline
    async fn execute(&self, request: CommandRequest) -> Result<CommandResult>;

    /// Validate a command request without executing it
    async fn validate(&self, request: &CommandRequest) -> Result<Vec<ValidationIssue>>;

    /// Check if this executor supports the given command type
    fn supports(&self, command_type: &CommandType) -> bool;

    /// Get executor capabilities and limitations
    fn capabilities(&self) -> ExecutorCapabilities;

    /// Estimate resource requirements for a command
    async fn estimate_resources(&self, request: &CommandRequest) -> Result<ResourceEstimate>;
}

/// Observability collector trait
#[async_trait]
pub trait ObservabilityCollector: Send + Sync {
    async fn record_command_start(&self, context: &ExecutionContextInternal);
    async fn record_command_complete(&self, result: &Result<CommandResult>);
}

/// Resource monitor
pub struct ResourceMonitor;

impl ResourceMonitor {
    pub async fn validate_limits(&self, _limits: &ResourceLimits) -> Result<()> {
        // TODO: Implement resource limit validation
        Ok(())
    }

    pub async fn check_resources(&self, _requirements: &ResourceRequirements) -> Result<()> {
        // TODO: Implement resource availability check
        Ok(())
    }
}

/// Internal execution context
pub struct ExecutionContextInternal {
    pub id: Uuid,
    pub request: CommandRequest,
    pub resource_limits: Option<ResourceLimits>,
}

impl ExecutionContextInternal {
    pub fn update_from_result(&mut self, _result: &CommandResult) {
        // Update context from result
    }
}

/// Execution context builder
pub struct ExecutionContextBuilder {
    id: Option<Uuid>,
    request: Option<CommandRequest>,
    resource_limits: Option<ResourceLimits>,
}

impl Default for ExecutionContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionContextBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            request: None,
            resource_limits: None,
        }
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_request(mut self, request: &CommandRequest) -> Self {
        self.request = Some(request.clone());
        self
    }

    pub fn with_resource_limits(mut self, limits: &Option<ResourceLimits>) -> Self {
        self.resource_limits = limits.clone();
        self
    }

    pub fn build(self) -> Result<ExecutionContextInternal> {
        Ok(ExecutionContextInternal {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            request: self
                .request
                .ok_or_else(|| anyhow::anyhow!("Request is required"))?,
            resource_limits: self.resource_limits,
        })
    }
}
