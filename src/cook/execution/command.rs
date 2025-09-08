//! Unified command specification and types

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Unified command specification for all command types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandSpec {
    /// Claude AI agent command
    Claude {
        command: String,
        context: Option<String>,
        tools: Option<Vec<String>>,
        output_format: Option<OutputFormat>,
    },
    /// Shell command execution
    Shell {
        command: String,
        shell: Option<String>,
        working_dir: Option<PathBuf>,
        env: Option<HashMap<String, String>>,
    },
    /// Test command with validation
    Test {
        command: String,
        expected_exit_code: Option<i32>,
        validation_script: Option<String>,
        retry_config: Option<RetryConfig>,
    },
    /// Handler command for workflow actions
    Handler {
        action: HandlerAction,
        context: HandlerContext,
        condition: Option<String>,
    },
}

/// Command request with full configuration
#[derive(Debug, Clone)]
pub struct CommandRequest {
    pub spec: CommandSpec,
    pub execution_config: ExecutionConfig,
    pub context: ExecutionContext,
    pub metadata: CommandMetadata,
}

/// Execution configuration for commands
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

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout: None,
            capture_output: CaptureOutputMode::Both,
            working_dir: None,
            env: HashMap::new(),
            retry_config: None,
            resource_limits: None,
            validation: None,
        }
    }
}

/// Output capture mode for commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureOutputMode {
    None,
    Stdout,
    Stderr,
    Both,
    Structured, // For commands that output structured data
}

/// Command metadata for tracking and observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub command_id: String,
    pub step_id: String,
    pub workflow_id: String,
    pub iteration: usize,
    pub created_at: DateTime<Utc>,
    pub tags: HashMap<String, String>,
}

impl CommandMetadata {
    pub fn new(command_type: &str) -> Self {
        Self {
            command_id: uuid::Uuid::new_v4().to_string(),
            step_id: String::new(),
            workflow_id: String::new(),
            iteration: 0,
            created_at: Utc::now(),
            tags: HashMap::from([("type".to_string(), command_type.to_string())]),
        }
    }
}

/// Retry configuration for commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            exponential_base: 2.0,
        }
    }
}

/// Resource limits for command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_bytes: Option<u64>,
    pub max_cpu_percent: Option<f32>,
    pub max_disk_io_bytes: Option<u64>,
    pub max_network_bytes: Option<u64>,
    pub max_file_descriptors: Option<u32>,
}

/// Validation configuration for command output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub script: Option<String>,
    pub expected_pattern: Option<String>,
    pub forbidden_patterns: Option<Vec<String>>,
    pub json_schema: Option<serde_json::Value>,
}

/// Handler action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandlerAction {
    OnSuccess { command: String },
    OnFailure { command: String },
    Cleanup { command: String },
    Rollback { command: String },
}

/// Handler execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerContext {
    pub previous_result: Option<String>,
    pub error_message: Option<String>,
    pub workflow_state: HashMap<String, serde_json::Value>,
}

/// Output format specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    Json,
    Yaml,
    PlainText,
    Structured,
}

/// Command type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandType {
    Claude,
    Shell,
    Test,
    Handler,
}

/// Execution context with runtime state
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub working_dir: PathBuf,
    pub env_vars: HashMap<String, String>,
    pub variables: HashMap<String, String>,
    pub capture_output: bool,
    pub timeout: Option<Duration>,
    pub stdin: Option<String>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            env_vars: HashMap::new(),
            variables: HashMap::new(),
            capture_output: true,
            timeout: None,
            stdin: None,
        }
    }
}

impl ExecutionContext {
    /// Substitute variables in a string
    pub fn substitute_variables(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (key, value) in &self.variables {
            result = result.replace(&format!("${{{}}}", key), value);
            result = result.replace(&format!("${}", key), value);
        }
        result
    }
}

/// Executable command ready for process spawning
#[derive(Debug, Clone)]
pub struct ExecutableCommand {
    pub program: String,
    pub args: Vec<String>,
    pub command_type: CommandType,
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub expected_exit_code: Option<i32>,
    pub resource_requirements: ResourceRequirements,
    pub cleanup_requirements: CleanupRequirements,
}

impl ExecutableCommand {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            command_type: CommandType::Shell,
            working_dir: None,
            env: HashMap::new(),
            expected_exit_code: Some(0),
            resource_requirements: ResourceRequirements::default(),
            cleanup_requirements: CleanupRequirements::default(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(|s| s.into()));
        self
    }

    pub fn with_type(mut self, command_type: CommandType) -> Self {
        self.command_type = command_type;
        self
    }

    pub fn with_working_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.working_dir = dir;
        self
    }

    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    pub fn with_expected_exit_code(mut self, code: Option<i32>) -> Self {
        self.expected_exit_code = code;
        self
    }

    pub fn from_string(cmd: &str) -> Result<Self> {
        let parts = shell_words::split(cmd)?;
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }
        Ok(Self::new(&parts[0]).args(&parts[1..]))
    }

    pub fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }

    pub fn resource_requirements(&self) -> &ResourceRequirements {
        &self.resource_requirements
    }

    pub fn cleanup_requirements(&self) -> &CleanupRequirements {
        &self.cleanup_requirements
    }
}

/// Resource requirements for command execution
#[derive(Debug, Clone, Default)]
pub struct ResourceRequirements {
    pub estimated_memory_mb: Option<u64>,
    pub estimated_cpu_cores: Option<f32>,
    pub estimated_duration: Option<Duration>,
}

/// Cleanup requirements for process termination
#[derive(Debug, Clone)]
pub struct CleanupRequirements {
    pub kill_timeout: Duration,
    pub cleanup_children: bool,
    pub preserve_output: bool,
}

impl Default for CleanupRequirements {
    fn default() -> Self {
        Self {
            kill_timeout: Duration::from_secs(5),
            cleanup_children: true,
            preserve_output: false,
        }
    }
}

impl CommandSpec {
    /// Convert command spec to executable command
    pub fn to_executable_command(&self, context: &ExecutionContext) -> Result<ExecutableCommand> {
        match self {
            CommandSpec::Claude { command, .. } => {
                let substituted_command = context.substitute_variables(command);
                Ok(ExecutableCommand::new("claude")
                    .arg("--print")
                    .arg("--dangerously-skip-permissions")
                    .arg(&substituted_command)
                    .with_type(CommandType::Claude))
            }
            CommandSpec::Shell {
                command,
                shell,
                working_dir,
                env,
            } => {
                let substituted_command = context.substitute_variables(command);
                let shell_cmd = shell.as_deref().unwrap_or("sh");

                let mut exec = ExecutableCommand::new(shell_cmd)
                    .arg("-c")
                    .arg(&substituted_command)
                    .with_working_dir(working_dir.clone())
                    .with_type(CommandType::Shell);

                if let Some(env) = env {
                    exec = exec.with_env(env.clone());
                }

                Ok(exec)
            }
            CommandSpec::Test {
                command,
                expected_exit_code,
                ..
            } => {
                let substituted_command = context.substitute_variables(command);
                ExecutableCommand::from_string(&substituted_command)?
                    .with_expected_exit_code(*expected_exit_code)
                    .with_type(CommandType::Test)
                    .into()
            }
            CommandSpec::Handler { action, .. } => {
                self.action_to_executable_command(action, context)
            }
        }
    }

    fn action_to_executable_command(
        &self,
        action: &HandlerAction,
        context: &ExecutionContext,
    ) -> Result<ExecutableCommand> {
        let command = match action {
            HandlerAction::OnSuccess { command }
            | HandlerAction::OnFailure { command }
            | HandlerAction::Cleanup { command }
            | HandlerAction::Rollback { command } => command,
        };

        let substituted_command = context.substitute_variables(command);
        ExecutableCommand::from_string(&substituted_command)?
            .with_type(CommandType::Handler)
            .into()
    }
}

impl From<ExecutableCommand> for Result<ExecutableCommand> {
    fn from(cmd: ExecutableCommand) -> Self {
        Ok(cmd)
    }
}
