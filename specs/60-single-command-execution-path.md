---
number: 60
title: Single Command Execution Path
category: architecture
priority: high
status: draft
dependencies: [58, 59]
created: 2025-09-03
---

# Specification 60: Single Command Execution Path

**Category**: architecture
**Priority**: high
**Status**: draft
**Dependencies**: [58 - Unified Execution Model, 59 - Input Abstraction]

## Context

The current architecture has evolved to handle different command types through separate execution paths, leading to code duplication, inconsistent feature support, and maintenance complexity. As identified in the Architecture Assessment, there are multiple execution pathways for different command types:

1. **Claude Commands** (`ClaudeExecutor`) - AI agent command execution with specific output handling
2. **Shell Commands** (`CommandExecutor`) - System command execution with different error handling
3. **Test Commands** - Validation commands with exit code interpretation
4. **Handler Commands** - Failure recovery and success actions with different contexts

Each path has its own implementation for:
- Command execution and process management
- Output capturing and processing
- Error handling and retry logic
- Timeout management and resource cleanup
- Git integration and commit verification
- Progress reporting and observability
- Environment variable handling and context management

This fragmentation has created several critical issues:
- **Inconsistent Behavior**: Same features work differently across command types
- **Code Duplication**: Similar logic implemented multiple times with subtle differences
- **Testing Complexity**: Each path requires separate test scenarios and mocks
- **Feature Drift**: New features added to one path but not others
- **Debugging Difficulty**: Different logging and error reporting across execution types

The validation bug that exposed this problem is symptomatic of a deeper architectural issue: there is no single, consistent way that commands are executed in the system.

## Objective

Create a unified command execution pipeline that consolidates all command types into a single, consistent execution path. This pipeline should handle Claude commands, shell commands, test commands, and handlers through the same interface while maintaining their specific behaviors and ensuring all workflow features (validation, timeouts, handlers, commit verification) work consistently across all command types.

## Requirements

### Functional Requirements

#### Unified Command Interface
- Single execution interface for all command types (Claude, shell, test, handler)
- Consistent command specification format across all types
- Unified parameter passing and environment variable handling
- Common timeout and resource management for all commands
- Standardized output capturing and processing

#### Command Type Support
- **Claude Commands**: AI agent interactions with proper context and tool usage
- **Shell Commands**: System command execution with proper shell handling
- **Test Commands**: Validation with exit code interpretation and retry logic
- **Handler Commands**: Failure recovery and success actions with workflow context

#### Feature Consistency
- Validation configuration works across all command types
- Timeout handling applies uniformly to all executions
- Output capturing follows same patterns for all command types
- Error handling and retry logic consistent across commands
- Git integration and commit requirements work for all types
- Progress reporting and observability integrated uniformly

#### Process Management
- Unified process spawning and lifecycle management
- Consistent resource cleanup and disposal
- Signal handling and graceful termination
- Process isolation and security considerations
- Resource limits enforcement across all command types

### Non-Functional Requirements

#### Performance
- Zero performance regression for existing command types
- Efficient resource utilization and cleanup
- Minimal overhead for command execution setup
- Optimal process reuse where applicable
- Fast process startup and termination

#### Reliability
- Robust error handling and recovery for all command types
- Consistent timeout behavior and cleanup
- Proper resource management and leak prevention
- Graceful degradation under resource constraints
- Comprehensive logging and observability

#### Maintainability
- Single source of truth for command execution logic
- Clear separation between command types and execution mechanics
- Easily testable components with minimal coupling
- Comprehensive error messages and debugging information
- Consistent logging format across all execution paths

## Acceptance Criteria

- [ ] Single CommandExecutor interface handles all command types
- [ ] Claude, shell, test, and handler commands execute through same pipeline
- [ ] All workflow features work consistently across command types
- [ ] Process management is unified with proper resource cleanup
- [ ] Error handling provides consistent behavior and messages
- [ ] Timeout and resource limits apply uniformly to all commands
- [ ] Output capturing follows same format for all command types
- [ ] Git integration works identically across command types
- [ ] Progress reporting integrates seamlessly with observability
- [ ] Backward compatibility maintained for all existing command formats
- [ ] Performance benchmarks show no regression for any command type
- [ ] Comprehensive test coverage for unified execution pipeline

## Technical Details

### Implementation Approach

#### Phase 1: Unified Command Specification

```rust
// src/cook/execution/command.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandSpec {
    Claude {
        command: String,
        context: Option<String>,
        tools: Option<Vec<String>>,
        output_format: Option<OutputFormat>,
    },
    Shell {
        command: String,
        shell: Option<String>,
        working_dir: Option<PathBuf>,
        env: Option<HashMap<String, String>>,
    },
    Test {
        command: String,
        expected_exit_code: Option<i32>,
        validation_script: Option<String>,
        retry_config: Option<RetryConfig>,
    },
    Handler {
        action: HandlerAction,
        context: HandlerContext,
        condition: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub spec: CommandSpec,
    pub execution_config: ExecutionConfig,
    pub context: ExecutionContext,
    pub metadata: CommandMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub timeout: Option<Duration>,
    pub capture_output: CaptureOutputMode,
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub retry_config: Option<RetryConfig>,
    pub resource_limits: Option<ResourceLimits>,
    pub validation: Option<ValidationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureOutputMode {
    None,
    Stdout,
    Stderr,
    Both,
    Structured, // For commands that output structured data
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub command_id: String,
    pub step_id: String,
    pub workflow_id: String,
    pub iteration: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tags: HashMap<String, String>,
}

impl CommandSpec {
    pub fn to_executable_command(&self, context: &ExecutionContext) -> Result<ExecutableCommand> {
        match self {
            CommandSpec::Claude { command, .. } => {
                let substituted_command = context.substitute_variables(command);
                ExecutableCommand::new("claude")
                    .arg("--print")
                    .arg(&substituted_command)
                    .with_type(CommandType::Claude)
            }
            CommandSpec::Shell { command, shell, working_dir, env } => {
                let substituted_command = context.substitute_variables(command);
                let shell_cmd = shell.as_deref().unwrap_or("sh");
                
                ExecutableCommand::new(shell_cmd)
                    .arg("-c")
                    .arg(&substituted_command)
                    .with_working_dir(working_dir.clone())
                    .with_env(env.clone().unwrap_or_default())
                    .with_type(CommandType::Shell)
            }
            CommandSpec::Test { command, expected_exit_code, .. } => {
                let substituted_command = context.substitute_variables(command);
                ExecutableCommand::from_string(&substituted_command)
                    .with_expected_exit_code(*expected_exit_code)
                    .with_type(CommandType::Test)
            }
            CommandSpec::Handler { action, .. } => {
                self.action_to_executable_command(action, context)
            }
        }
    }
}
```

#### Phase 2: Unified Command Executor

```rust
// src/cook/execution/executor.rs
pub struct UnifiedCommandExecutor {
    process_manager: Arc<ProcessManager>,
    output_processor: Arc<OutputProcessor>,
    observability: Arc<dyn ObservabilityCollector>,
    resource_monitor: Arc<ResourceMonitor>,
}

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
    
    async fn execute_with_context(
        &self,
        request: CommandRequest,
        mut exec_context: ExecutionContext,
    ) -> Result<CommandResult> {
        let start_time = Instant::now();
        
        // Convert command spec to executable command
        let executable = request.spec.to_executable_command(&exec_context.context)?;
        
        // Apply execution configuration
        let configured_executable = self.apply_execution_config(executable, &request.execution_config)?;
        
        // Spawn process with unified process management
        let mut process = self.process_manager
            .spawn(configured_executable, &exec_context)
            .await?;
        
        // Setup timeout handling
        let timeout_future = self.setup_timeout(&request.execution_config.timeout);
        
        // Setup output processing
        let output_future = self.process_output(&mut process, &request.execution_config.capture_output);
        
        // Wait for completion or timeout
        let execution_result = tokio::select! {
            result = process.wait() => {
                let output = output_future.await?;
                self.create_result_from_process(result?, output, start_time.elapsed())
            }
            _ = timeout_future => {
                process.kill().await?;
                Err(anyhow::anyhow!("Command execution timed out"))
            }
        };
        
        // Post-execution processing
        let final_result = self.post_process_result(execution_result, &request).await?;
        
        // Update execution context
        exec_context.update_from_result(&final_result);
        
        Ok(final_result)
    }
    
    async fn process_output(
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
                let (stdout, stderr) = tokio::join!(
                    self.read_stream(process.stdout()),
                    self.read_stream(process.stderr())
                );
                Ok(ProcessOutput::new()
                    .with_stdout(stdout?)
                    .with_stderr(stderr?))
            }
            CaptureOutputMode::Structured => {
                self.process_structured_output(process).await
            }
        }
    }
    
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
            CommandSpec::Test { expected_exit_code, .. } => {
                result = self.post_process_test_result(result, *expected_exit_code).await?;
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
}
```

#### Phase 3: Process Management Layer

```rust
// src/cook/execution/process.rs
pub struct ProcessManager {
    resource_monitor: Arc<ResourceMonitor>,
    security_context: Arc<SecurityContext>,
    cleanup_registry: Arc<Mutex<HashMap<ProcessId, CleanupHandler>>>,
}

impl ProcessManager {
    pub async fn spawn(
        &self,
        executable: ExecutableCommand,
        context: &ExecutionContext,
    ) -> Result<UnifiedProcess> {
        // Security validation
        self.security_context.validate_command(&executable).await?;
        
        // Resource availability check
        self.resource_monitor.check_resources(&executable.resource_requirements()).await?;
        
        // Create process with unified configuration
        let mut command = self.create_system_command(&executable, context)?;
        
        // Apply security restrictions
        command = self.apply_security_context(command, context)?;
        
        // Apply resource limits
        command = self.apply_resource_limits(command, &executable.resource_requirements())?;
        
        // Spawn process
        let child = command.spawn()
            .with_context(|| format!("Failed to spawn command: {}", executable.display()))?;
        
        let process = UnifiedProcess::new(child, executable.command_type());
        
        // Register for cleanup
        let cleanup_handler = CleanupHandler::new(process.id(), executable.cleanup_requirements());
        self.cleanup_registry.lock().await.insert(process.id(), cleanup_handler);
        
        Ok(process)
    }
    
    fn create_system_command(
        &self,
        executable: &ExecutableCommand,
        context: &ExecutionContext,
    ) -> Result<tokio::process::Command> {
        let mut command = tokio::process::Command::new(&executable.program);
        
        // Add arguments
        command.args(&executable.args);
        
        // Set working directory
        if let Some(working_dir) = &executable.working_dir {
            command.current_dir(working_dir);
        } else if let Some(context_dir) = &context.working_dir {
            command.current_dir(context_dir);
        }
        
        // Set environment variables
        for (key, value) in &executable.env {
            command.env(key, value);
        }
        
        // Configure stdio based on command type
        match executable.command_type() {
            CommandType::Claude => {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.stdin(Stdio::null());
            }
            CommandType::Shell => {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.stdin(Stdio::inherit());
            }
            CommandType::Test => {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.stdin(Stdio::null());
            }
            CommandType::Handler => {
                command.stdout(Stdio::inherit());
                command.stderr(Stdio::inherit());
                command.stdin(Stdio::null());
            }
        }
        
        Ok(command)
    }
    
    async fn cleanup_process(&self, process_id: ProcessId) -> Result<()> {
        if let Some(cleanup_handler) = self.cleanup_registry.lock().await.remove(&process_id) {
            cleanup_handler.cleanup().await?;
        }
        Ok(())
    }
}

pub struct UnifiedProcess {
    child: tokio::process::Child,
    command_type: CommandType,
    started_at: Instant,
    resource_usage: ResourceUsage,
}

impl UnifiedProcess {
    pub fn new(child: tokio::process::Child, command_type: CommandType) -> Self {
        Self {
            child,
            command_type,
            started_at: Instant::now(),
            resource_usage: ResourceUsage::default(),
        }
    }
    
    pub async fn wait(&mut self) -> Result<ExitStatus> {
        let exit_status = self.child.wait().await?;
        self.resource_usage.duration = self.started_at.elapsed();
        Ok(exit_status)
    }
    
    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
    
    pub fn stdout(&mut self) -> &mut Option<tokio::process::ChildStdout> {
        &mut self.child.stdout
    }
    
    pub fn stderr(&mut self) -> &mut Option<tokio::process::ChildStderr> {
        &mut self.child.stderr
    }
    
    pub fn id(&self) -> ProcessId {
        ProcessId(self.child.id().unwrap_or(0))
    }
}
```

#### Phase 4: Output Processing Layer

```rust
// src/cook/execution/output.rs
pub struct OutputProcessor {
    formatters: HashMap<CommandType, Box<dyn OutputFormatter>>,
    parsers: HashMap<OutputFormat, Box<dyn OutputParser>>,
}

impl OutputProcessor {
    pub fn new() -> Self {
        let mut formatters: HashMap<CommandType, Box<dyn OutputFormatter>> = HashMap::new();
        formatters.insert(CommandType::Claude, Box::new(ClaudeOutputFormatter));
        formatters.insert(CommandType::Shell, Box::new(ShellOutputFormatter));
        formatters.insert(CommandType::Test, Box::new(TestOutputFormatter));
        formatters.insert(CommandType::Handler, Box::new(HandlerOutputFormatter));
        
        let mut parsers: HashMap<OutputFormat, Box<dyn OutputParser>> = HashMap::new();
        parsers.insert(OutputFormat::Json, Box::new(JsonOutputParser));
        parsers.insert(OutputFormat::Yaml, Box::new(YamlOutputParser));
        parsers.insert(OutputFormat::PlainText, Box::new(PlainTextOutputParser));
        
        Self { formatters, parsers }
    }
    
    pub async fn process_output(
        &self,
        raw_output: ProcessOutput,
        command_type: CommandType,
        output_format: Option<OutputFormat>,
    ) -> Result<ProcessedOutput> {
        // Apply command-type specific formatting
        let formatted_output = if let Some(formatter) = self.formatters.get(&command_type) {
            formatter.format(&raw_output).await?
        } else {
            raw_output.clone()
        };
        
        // Apply format-specific parsing if requested
        let parsed_output = if let Some(format) = output_format {
            if let Some(parser) = self.parsers.get(&format) {
                parser.parse(&formatted_output).await?
            } else {
                formatted_output
            }
        } else {
            formatted_output
        };
        
        Ok(ProcessedOutput::new(parsed_output))
    }
}

#[async_trait]
pub trait OutputFormatter: Send + Sync {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput>;
}

pub struct ClaudeOutputFormatter;

#[async_trait]
impl OutputFormatter for ClaudeOutputFormatter {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        let mut formatted = output.clone();
        
        // Claude-specific output processing
        if let Some(stdout) = &output.stdout {
            // Remove Claude CLI formatting artifacts
            let cleaned = self.remove_claude_artifacts(stdout);
            // Extract structured data if present
            let structured = self.extract_structured_data(&cleaned)?;
            formatted.stdout = Some(cleaned);
            formatted.structured_data = structured;
        }
        
        Ok(formatted)
    }
}

pub struct ShellOutputFormatter;

#[async_trait]
impl OutputFormatter for ShellOutputFormatter {
    async fn format(&self, output: &ProcessOutput) -> Result<ProcessOutput> {
        let mut formatted = output.clone();
        
        // Shell-specific output processing
        if let Some(stderr) = &output.stderr {
            // Parse common shell error patterns
            formatted.error_summary = self.extract_error_summary(stderr);
        }
        
        Ok(formatted)
    }
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub structured_data: Option<serde_json::Value>,
    pub error_summary: Option<String>,
    pub metadata: OutputMetadata,
}

#[derive(Debug, Clone)]
pub struct ProcessedOutput {
    pub content: ProcessOutput,
    pub format: OutputFormat,
    pub processing_duration: Duration,
    pub warnings: Vec<String>,
}
```

### Architecture Changes

#### Unified Execution Pipeline
```
┌─────────────────────────────────────────────────────┐
│              Workflow Step Request                   │
│  ┌─────────────────────────────────────────────┐    │
│  │  WorkflowStep → CommandRequest               │    │
│  │                                             │    │
│  │  • Extract CommandSpec                      │    │
│  │  • Apply ExecutionConfig                    │    │
│  │  • Create CommandMetadata                   │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│            Unified Command Executor                  │
│  ┌─────────────────────────────────────────────┐    │
│  │          Pre-Execution Pipeline              │    │
│  │                                             │    │
│  │  • Request validation                       │    │
│  │  • Security context creation               │    │
│  │  • Resource availability check             │    │
│  │  • Execution context setup                 │    │
│  └─────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────┐    │
│  │           Command Processing                 │    │
│  │                                             │    │
│  │  • CommandSpec → ExecutableCommand          │    │
│  │  • Variable substitution                    │    │
│  │  • Configuration application                │    │
│  │  • Environment preparation                  │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│               Process Management                     │
│  ┌─────────────────────────────────────────────┐    │
│  │            Process Spawning                  │    │
│  │                                             │    │
│  │  • Security validation                      │    │
│  │  • Resource limit application               │    │
│  │  • Process creation                         │    │
│  │  • Cleanup registration                     │    │
│  └─────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────┐    │
│  │          Execution Monitoring                │    │
│  │                                             │    │
│  │  • Timeout management                       │    │
│  │  • Output streaming                         │    │
│  │  • Resource monitoring                      │    │
│  │  • Signal handling                          │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│              Output Processing                       │
│  ┌─────────────────────────────────────────────┐    │
│  │         Output Capture                       │    │
│  │                                             │    │
│  │  • Stream reading                           │    │
│  │  • Content buffering                        │    │
│  │  • Format detection                         │    │
│  │  • Error extraction                         │    │
│  └─────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────┐    │
│  │      Command-Type Specific Processing       │    │
│  │                                             │    │
│  │  • Claude: Artifact removal, JSON parsing  │    │
│  │  • Shell: Error pattern recognition        │    │
│  │  • Test: Exit code interpretation          │    │
│  │  • Handler: Action result validation       │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│            Post-Execution Processing                 │
│  ┌─────────────────────────────────────────────┐    │
│  │         Result Validation                    │    │
│  │                                             │    │
│  │  • Exit code verification                   │    │
│  │  • Output validation                        │    │
│  │  • Error classification                     │    │
│  │  • Success criteria checking                │    │
│  └─────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────┐    │
│  │          Integration Points                  │    │
│  │                                             │    │
│  │  • Git commit verification                  │    │
│  │  • Progress reporting update                │    │
│  │  • Observability event emission             │    │
│  │  • Resource cleanup                         │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### Data Structures

#### Command Result System
```rust
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

#[derive(Debug, Clone)]
pub enum CommandStatus {
    Success,
    Failed { reason: FailureReason, retryable: bool },
    TimedOut,
    Cancelled,
    ResourceLimitExceeded,
}

#[derive(Debug, Clone)]
pub enum FailureReason {
    NonZeroExit(i32),
    ProcessError(String),
    ValidationFailed(Vec<ValidationIssue>),
    SecurityViolation(String),
    ResourceExhaustion(String),
    InternalError(String),
}

#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu_time: Duration,
    pub wall_clock_time: Duration,
    pub peak_memory_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub network_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExecutionMetadata {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub hostname: String,
    pub process_id: Option<u32>,
    pub parent_process_id: Option<u32>,
    pub working_directory: PathBuf,
    pub environment_hash: String, // Hash of environment variables for debugging
    pub git_commit: Option<String>,
    pub observability_trace_id: Option<String>,
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
        self.output.content.stderr.as_deref()
            .or_else(|| self.output.content.error_summary.as_deref())
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
```

### APIs and Interfaces

#### Command Executor Trait
```rust
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

#[derive(Debug, Clone)]
pub struct ExecutorCapabilities {
    pub supported_command_types: Vec<CommandType>,
    pub max_concurrent_executions: Option<usize>,
    pub supported_output_formats: Vec<OutputFormat>,
    pub timeout_support: bool,
    pub resource_limiting_support: bool,
    pub security_context_support: bool,
}

#[derive(Debug, Clone)]
pub struct ResourceEstimate {
    pub estimated_duration: Option<Duration>,
    pub estimated_memory_mb: Option<u64>,
    pub estimated_cpu_percent: Option<f32>,
    pub estimated_disk_io_mb: Option<u64>,
    pub confidence: f32, // 0.0 to 1.0
}
```

#### Backward Compatibility Bridge
```rust
// Maintains compatibility with existing executor interfaces
pub struct LegacyExecutorBridge {
    unified_executor: Arc<UnifiedCommandExecutor>,
}

#[async_trait]
impl crate::cook::execution::ClaudeExecutor for LegacyExecutorBridge {
    async fn execute_claude_command(
        &self,
        command: &str,
        context: &crate::cook::ExecutionContext,
    ) -> Result<crate::cook::ExecutionResult> {
        let request = CommandRequest {
            spec: CommandSpec::Claude {
                command: command.to_string(),
                context: None,
                tools: None,
                output_format: None,
            },
            execution_config: ExecutionConfig::from_legacy_context(context),
            context: ExecutionContext::from_legacy_context(context),
            metadata: CommandMetadata::new("claude_legacy"),
        };
        
        let result = self.unified_executor.execute(request).await?;
        Ok(crate::cook::ExecutionResult::from_unified_result(result))
    }
}

#[async_trait]
impl crate::cook::execution::CommandExecutor for LegacyExecutorBridge {
    async fn execute_shell_command(
        &self,
        command: &str,
        context: &crate::cook::ExecutionContext,
    ) -> Result<crate::cook::ExecutionResult> {
        let request = CommandRequest {
            spec: CommandSpec::Shell {
                command: command.to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            execution_config: ExecutionConfig::from_legacy_context(context),
            context: ExecutionContext::from_legacy_context(context),
            metadata: CommandMetadata::new("shell_legacy"),
        };
        
        let result = self.unified_executor.execute(request).await?;
        Ok(crate::cook::ExecutionResult::from_unified_result(result))
    }
}
```

## Dependencies

- **Prerequisites**: 
  - Specification 58: Unified Execution Model (provides execution context and workflow integration)
  - Specification 59: Input Abstraction (provides consistent variable substitution)
- **Affected Components**:
  - `src/cook/execution/`: Complete refactoring of command execution
  - `src/cook/orchestrator.rs`: Integration with unified command execution
  - `src/cook/workflow/`: Use unified execution for all step types
  - All existing command executors (ClaudeExecutor, CommandExecutor, etc.)
- **External Dependencies**:
  - `tokio`: Async runtime for process management
  - `uuid`: Command execution tracking
  - `serde`: Configuration and result serialization
  - `chrono`: Timestamp handling

## Testing Strategy

### Unit Tests
- Command specification creation and validation
- Process management lifecycle (spawn, monitor, cleanup)
- Output processing for all command types
- Error handling and recovery mechanisms
- Resource limit enforcement and monitoring

### Integration Tests
- End-to-end execution for all command types
- Backward compatibility with existing executors
- Resource cleanup under various failure scenarios
- Timeout handling and process termination
- Security context application and validation

### Performance Tests
- Command execution overhead measurement
- Process spawning and cleanup performance
- Memory usage analysis under concurrent execution
- Resource monitoring accuracy and overhead
- Comparative benchmarks against existing executors

### System Tests
- Long-running command execution stability
- Resource exhaustion handling
- Signal handling and graceful termination
- Security boundary enforcement
- Error recovery and retry logic

## Documentation Requirements

### Code Documentation
- Unified command execution architecture
- Process management and security model
- Output processing and formatting system
- Resource monitoring and limiting mechanisms
- Error handling and recovery strategies

### User Documentation
- Migration guide from existing command execution
- Command specification format reference
- Resource configuration and tuning guide
- Troubleshooting common execution issues
- Best practices for command design

### Architecture Documentation
- Update ARCHITECTURE.md with unified execution model
- Document security model and process isolation
- Add performance characteristics and resource usage
- Include error handling and recovery flows

## Implementation Notes

### Security Considerations
- Process isolation and sandboxing where available
- Command validation to prevent injection attacks
- Resource limits to prevent resource exhaustion
- Working directory and file system access controls
- Environment variable sanitization

### Performance Optimization
- Process pool reuse for similar command types
- Lazy initialization of expensive resources
- Efficient output streaming and buffering
- Smart resource monitoring with minimal overhead
- Process cleanup optimization

### Error Handling Strategy
- Consistent error classification across command types
- Detailed error context for debugging
- Retry logic with exponential backoff
- Graceful degradation on resource constraints
- Comprehensive logging for troubleshooting

### Monitoring and Observability
- Command execution metrics collection
- Resource usage tracking and reporting
- Performance bottleneck identification
- Error pattern analysis and alerting
- Integration with existing observability systems

## Migration and Compatibility

### Breaking Changes
- None for standard workflow configurations
- Internal command executor APIs will change
- Some advanced configuration options may have different names
- Error message formats will be more consistent but different

### Migration Timeline
1. **Week 1-2**: Implement core unified command executor
2. **Week 3**: Add all command type support (Claude, shell, test, handler)
3. **Week 4**: Implement process management and security features
4. **Week 5**: Add output processing and result validation
5. **Week 6**: Create backward compatibility bridges
6. **Week 7**: Integration testing and performance validation
7. **Week 8**: Documentation and migration guides

### Rollback Plan
- Feature flag to disable unified execution
- Ability to fall back to legacy executors per command type
- Monitoring to detect execution failures or performance issues
- Quick rollback mechanism with minimal workflow disruption