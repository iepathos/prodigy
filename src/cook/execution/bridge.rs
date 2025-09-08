//! Backward compatibility bridge for existing executor interfaces

use super::command::*;
use super::executor::{CommandExecutor as UnifiedExecutor, CommandResult, UnifiedCommandExecutor};
use super::{ClaudeExecutor, CommandExecutor, CommandRunner, ExecutionContext, ExecutionResult};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Legacy executor bridge for backward compatibility
pub struct LegacyExecutorBridge {
    unified_executor: Arc<UnifiedCommandExecutor>,
}

impl LegacyExecutorBridge {
    pub fn new(unified_executor: Arc<UnifiedCommandExecutor>) -> Self {
        Self { unified_executor }
    }

    /// Convert legacy execution context to unified context
    pub fn to_unified_context(context: &ExecutionContext) -> super::command::ExecutionContext {
        super::command::ExecutionContext {
            working_dir: context.working_directory.clone(),
            env_vars: context.env_vars.clone(),
            variables: HashMap::new(),
            capture_output: context.capture_output,
            timeout: context.timeout_seconds.map(std::time::Duration::from_secs),
            stdin: context.stdin.clone(),
        }
    }

    /// Convert unified result to legacy result
    pub fn from_unified_result(result: CommandResult) -> ExecutionResult {
        ExecutionResult {
            success: result.is_success(),
            stdout: result.get_output_text().unwrap_or_default().to_string(),
            stderr: result.get_error_text().unwrap_or_default().to_string(),
            exit_code: result.exit_code,
        }
    }
}

#[async_trait]
impl ClaudeExecutor for LegacyExecutorBridge {
    async fn execute_claude_command(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        let request = CommandRequest {
            spec: CommandSpec::Claude {
                command: command.to_string(),
                context: None,
                tools: None,
                output_format: None,
            },
            execution_config: ExecutionConfig {
                timeout: None,
                capture_output: CaptureOutputMode::Both,
                working_dir: Some(project_path.to_path_buf()),
                env: env_vars.clone(),
                retry_config: None,
                resource_limits: None,
                validation: None,
            },
            context: super::command::ExecutionContext {
                working_dir: project_path.to_path_buf(),
                env_vars,
                variables: HashMap::new(),
                capture_output: true,
                timeout: None,
                stdin: Some("".to_string()), // Claude requires some stdin
            },
            metadata: CommandMetadata::new("claude_legacy"),
        };

        let result = self.unified_executor.execute(request).await?;
        Ok(Self::from_unified_result(result))
    }

    async fn check_claude_cli(&self) -> Result<bool> {
        // Check if Claude CLI is available
        let request = CommandRequest {
            spec: CommandSpec::Shell {
                command: "claude --version".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            execution_config: ExecutionConfig {
                timeout: Some(std::time::Duration::from_secs(5)),
                capture_output: CaptureOutputMode::Both,
                working_dir: None,
                env: HashMap::new(),
                retry_config: None,
                resource_limits: None,
                validation: None,
            },
            context: super::command::ExecutionContext::default(),
            metadata: CommandMetadata::new("claude_check"),
        };

        match self.unified_executor.execute(request).await {
            Ok(result) => Ok(result.is_success()),
            Err(_) => Ok(false),
        }
    }

    async fn get_claude_version(&self) -> Result<String> {
        let request = CommandRequest {
            spec: CommandSpec::Shell {
                command: "claude --version".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            execution_config: ExecutionConfig {
                timeout: Some(std::time::Duration::from_secs(5)),
                capture_output: CaptureOutputMode::Stdout,
                working_dir: None,
                env: HashMap::new(),
                retry_config: None,
                resource_limits: None,
                validation: None,
            },
            context: super::command::ExecutionContext::default(),
            metadata: CommandMetadata::new("claude_version"),
        };

        let result = self.unified_executor.execute(request).await?;
        if result.is_success() {
            Ok(result
                .get_output_text()
                .unwrap_or_default()
                .trim()
                .to_string())
        } else {
            anyhow::bail!("Failed to get Claude version")
        }
    }
}

#[async_trait]
impl CommandExecutor for LegacyExecutorBridge {
    async fn execute(
        &self,
        command: &str,
        args: &[String],
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // Determine command type
        let spec = if command == "claude" {
            CommandSpec::Claude {
                command: args.first().cloned().unwrap_or_default(),
                context: None,
                tools: None,
                output_format: None,
            }
        } else {
            // Build full command string
            let full_command = if args.is_empty() {
                command.to_string()
            } else {
                format!("{} {}", command, shell_words::join(args))
            };

            CommandSpec::Shell {
                command: full_command,
                shell: None,
                working_dir: Some(context.working_directory.clone()),
                env: Some(context.env_vars.clone()),
            }
        };

        let request = CommandRequest {
            spec,
            execution_config: ExecutionConfig {
                timeout: context.timeout_seconds.map(std::time::Duration::from_secs),
                capture_output: if context.capture_output {
                    CaptureOutputMode::Both
                } else {
                    CaptureOutputMode::None
                },
                working_dir: Some(context.working_directory.clone()),
                env: context.env_vars.clone(),
                retry_config: None,
                resource_limits: None,
                validation: None,
            },
            context: Self::to_unified_context(&context),
            metadata: CommandMetadata::new("legacy"),
        };

        let result = self.unified_executor.execute(request).await?;
        Ok(Self::from_unified_result(result))
    }
}

/// Create a legacy-compatible executor from a command runner
pub fn create_legacy_executor<R: CommandRunner + 'static>(_runner: R) -> impl ClaudeExecutor {
    // Create unified executor components
    let resource_monitor = Arc::new(super::executor::ResourceMonitor);
    let security_context = Arc::new(super::process::SecurityContext);
    let process_manager = Arc::new(super::process::ProcessManager::with_monitors(
        resource_monitor.clone(),
        security_context,
    ));
    let output_processor = Arc::new(super::output::OutputProcessor::new());
    let observability = Arc::new(NoOpObservability);

    let unified_executor = Arc::new(UnifiedCommandExecutor::new(
        process_manager,
        output_processor,
        observability,
        resource_monitor,
    ));

    LegacyExecutorBridge::new(unified_executor)
}

/// No-op observability for backward compatibility
pub struct NoOpObservability;

#[async_trait]
impl super::executor::ObservabilityCollector for NoOpObservability {
    async fn record_command_start(&self, _context: &super::executor::ExecutionContextInternal) {
        // No-op
    }

    async fn record_command_complete(&self, _result: &Result<CommandResult>) {
        // No-op
    }
}
