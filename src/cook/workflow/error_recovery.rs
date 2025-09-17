//! Error recovery for workflow resume operations
//!
//! Provides robust error recovery mechanisms for resuming interrupted workflows
//! with error handlers and failure recovery strategies.

use super::checkpoint::{
    RetryState as CheckpointRetryState, WorkflowCheckpoint,
};
use super::executor::WorkflowContext;
use super::on_failure::{HandlerCommand, HandlerStrategy, OnFailureConfig};
use crate::cook::execution::CommandExecutor;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// State for error recovery during resume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecoveryState {
    /// Active error handlers from workflow
    pub active_handlers: Vec<ErrorHandler>,
    /// Error context for interpolation
    pub error_context: HashMap<String, Value>,
    /// History of handler executions
    pub handler_execution_history: Vec<HandlerExecution>,
    /// Retry state for current operation
    pub retry_state: Option<CheckpointRetryState>,
    /// Correlation ID for error tracking
    pub correlation_id: String,
    /// Recovery attempts made
    pub recovery_attempts: usize,
    /// Maximum recovery attempts allowed
    pub max_recovery_attempts: usize,
}

impl Default for ErrorRecoveryState {
    fn default() -> Self {
        Self {
            active_handlers: Vec::new(),
            error_context: HashMap::new(),
            handler_execution_history: Vec::new(),
            retry_state: None,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            recovery_attempts: 0,
            max_recovery_attempts: 3,
        }
    }
}

/// Error handler definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandler {
    /// Unique handler identifier
    pub id: String,
    /// Condition for handler execution
    pub condition: Option<String>,
    /// Commands to execute
    pub commands: Vec<HandlerCommand>,
    /// Retry configuration
    pub retry_config: Option<RetryConfig>,
    /// Handler timeout
    pub timeout: Option<Duration>,
    /// Handler scope
    pub scope: ErrorHandlerScope,
    /// Strategy for error recovery
    pub strategy: HandlerStrategy,
}

/// Retry configuration for error handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

/// Scope of error handler application
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorHandlerScope {
    Command,
    Step,
    Phase,
    Workflow,
}

/// Record of handler execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerExecution {
    pub handler_id: String,
    pub executed_at: DateTime<Utc>,
    pub success: bool,
    pub error: Option<String>,
    pub retry_attempt: usize,
    pub duration: Duration,
}

/// Recovery action to take
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    Retry {
        delay: Duration,
        max_attempts: usize,
    },
    Fallback {
        alternative_path: String,
    },
    PartialResume {
        from_step: usize,
    },
    RequestIntervention {
        message: String,
    },
    SafeAbort {
        cleanup_actions: Vec<HandlerCommand>,
    },
    Continue,
}

/// Resume error types
#[derive(Debug)]
pub enum ResumeError {
    CorruptedCheckpoint(String),
    MissingDependency(String),
    EnvironmentMismatch(String),
    HandlerExecutionFailed(String),
    RecoveryLimitExceeded,
    Other(anyhow::Error),
}

impl std::fmt::Display for ResumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResumeError::CorruptedCheckpoint(msg) => write!(f, "Corrupted checkpoint: {}", msg),
            ResumeError::MissingDependency(dep) => write!(f, "Missing dependency: {}", dep),
            ResumeError::EnvironmentMismatch(msg) => write!(f, "Environment mismatch: {}", msg),
            ResumeError::HandlerExecutionFailed(msg) => {
                write!(f, "Handler execution failed: {}", msg)
            }
            ResumeError::RecoveryLimitExceeded => write!(f, "Recovery limit exceeded"),
            ResumeError::Other(err) => write!(f, "Other error: {}", err),
        }
    }
}

impl std::error::Error for ResumeError {}

/// Error recovery manager for resume operations
pub struct ResumeErrorRecovery {
    /// Recovery state
    pub(crate) recovery_state: ErrorRecoveryState,
    /// Command executor for handlers
    command_executor: Option<Arc<dyn CommandExecutor>>,
}

impl Default for ResumeErrorRecovery {
    fn default() -> Self {
        Self::new()
    }
}

impl ResumeErrorRecovery {
    /// Create new error recovery manager
    pub fn new() -> Self {
        Self {
            recovery_state: ErrorRecoveryState::default(),
            command_executor: None,
        }
    }

    /// Set command executor for handler execution
    pub fn with_executor(mut self, executor: Arc<dyn CommandExecutor>) -> Self {
        self.command_executor = Some(executor);
        self
    }

    /// Restore error handlers from checkpoint
    pub async fn restore_error_handlers(
        &mut self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<Vec<ErrorHandler>> {
        info!("Restoring error handlers from checkpoint");

        // Extract handlers from checkpoint if they exist
        let handlers = if let Some(recovery_state) = self.extract_recovery_state(checkpoint)? {
            self.recovery_state = recovery_state;
            self.recovery_state.active_handlers.clone()
        } else {
            Vec::new()
        };

        // Validate handlers are still applicable
        self.validate_error_handlers(&handlers).await?;

        // Initialize error context from checkpoint
        self.restore_error_context(checkpoint).await?;

        info!("Restored {} error handlers", handlers.len());
        Ok(handlers)
    }

    /// Handle error during resume operation
    pub async fn handle_resume_error(
        &mut self,
        error: &ResumeError,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<RecoveryAction> {
        warn!("Handling resume error: {}", error);

        // Check recovery attempt limit
        if self.recovery_state.recovery_attempts >= self.recovery_state.max_recovery_attempts {
            error!("Recovery attempt limit exceeded");
            return Ok(RecoveryAction::SafeAbort {
                cleanup_actions: Vec::new(),
            });
        }

        self.recovery_state.recovery_attempts += 1;

        match error {
            ResumeError::CorruptedCheckpoint(msg) => {
                warn!("Attempting checkpoint repair: {}", msg);
                self.attempt_checkpoint_repair(checkpoint).await
            }
            ResumeError::MissingDependency(dep) => {
                warn!("Resolving missing dependency: {}", dep);
                self.resolve_missing_dependencies(dep).await
            }
            ResumeError::EnvironmentMismatch(msg) => {
                warn!("Adapting to environment changes: {}", msg);
                self.adapt_to_environment_changes(msg).await
            }
            ResumeError::HandlerExecutionFailed(msg) => {
                warn!("Handler execution failed: {}", msg);
                self.handle_handler_failure(msg).await
            }
            ResumeError::RecoveryLimitExceeded => {
                error!("Recovery limit already exceeded");
                Ok(RecoveryAction::SafeAbort {
                    cleanup_actions: Vec::new(),
                })
            }
            ResumeError::Other(e) => {
                warn!("Applying default recovery for: {}", e);
                self.default_error_recovery(e).await
            }
        }
    }

    /// Execute error handler with resume context
    pub async fn execute_error_handler_with_resume_context(
        &mut self,
        handler: &ErrorHandler,
        error_msg: &str,
        workflow_context: &mut WorkflowContext,
    ) -> Result<bool> {
        info!(
            "Executing error handler {} with strategy {:?}",
            handler.id, handler.strategy
        );

        // Add error context variables
        self.recovery_state.error_context.insert(
            "error.message".to_string(),
            Value::String(error_msg.to_string()),
        );
        self.recovery_state.error_context.insert(
            "error.correlation_id".to_string(),
            Value::String(self.recovery_state.correlation_id.clone()),
        );
        self.recovery_state.error_context.insert(
            "error.handler".to_string(),
            Value::String(handler.id.clone()),
        );

        // Execute handler commands
        let start_time = std::time::Instant::now();
        let mut success = true;

        for command in &handler.commands {
            match self
                .execute_handler_command(command, workflow_context)
                .await
            {
                Ok(_) => {
                    debug!("Handler command executed successfully");
                }
                Err(e) => {
                    warn!("Handler command failed: {}", e);
                    if !command.continue_on_error {
                        success = false;
                        break;
                    }
                }
            }
        }

        // Record execution
        let execution = HandlerExecution {
            handler_id: handler.id.clone(),
            executed_at: Utc::now(),
            success,
            error: if success {
                None
            } else {
                Some(error_msg.to_string())
            },
            retry_attempt: self.recovery_state.recovery_attempts,
            duration: start_time.elapsed(),
        };

        self.recovery_state
            .handler_execution_history
            .push(execution);

        Ok(success)
    }

    /// Extract recovery state from checkpoint
    fn extract_recovery_state(
        &self,
        _checkpoint: &WorkflowCheckpoint,
    ) -> Result<Option<ErrorRecoveryState>> {
        // Look for error recovery state in checkpoint metadata
        // This would be stored as part of the checkpoint extension
        // For now, return None if not found
        Ok(None)
    }

    /// Validate that error handlers are still applicable
    async fn validate_error_handlers(&self, handlers: &[ErrorHandler]) -> Result<()> {
        for handler in handlers {
            // Validate handler conditions if present
            if let Some(ref condition) = handler.condition {
                debug!("Validating handler condition: {}", condition);
                // TODO: Implement condition evaluation
            }

            // Validate handler commands are executable
            for command in &handler.commands {
                if command.shell.is_none() && command.claude.is_none() {
                    return Err(anyhow!(
                        "Invalid handler command: no shell or claude command specified"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Restore error context from checkpoint
    async fn restore_error_context(&mut self, checkpoint: &WorkflowCheckpoint) -> Result<()> {
        // Restore any error context from previous execution
        if let Some(retry_state) = checkpoint
            .completed_steps
            .last()
            .and_then(|step| step.retry_state.clone())
        {
            self.recovery_state.retry_state = Some(retry_state);
        }

        Ok(())
    }

    /// Attempt to repair corrupted checkpoint
    async fn attempt_checkpoint_repair(
        &self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<RecoveryAction> {
        info!("Attempting checkpoint repair");

        // Try to recover from partial state
        if checkpoint.completed_steps.is_empty() {
            warn!("No completed steps in checkpoint, starting from beginning");
            return Ok(RecoveryAction::PartialResume { from_step: 0 });
        }

        // Find last known good state
        let last_good_step = checkpoint
            .completed_steps
            .iter()
            .rposition(|step| step.success)
            .unwrap_or(0);

        info!("Resuming from last known good step: {}", last_good_step);
        Ok(RecoveryAction::PartialResume {
            from_step: last_good_step,
        })
    }

    /// Resolve missing dependencies
    async fn resolve_missing_dependencies(&self, dependency: &str) -> Result<RecoveryAction> {
        info!("Attempting to resolve missing dependency: {}", dependency);

        // Check if dependency is a command
        if dependency.starts_with("claude:") || dependency.starts_with("shell:") {
            return Ok(RecoveryAction::RequestIntervention {
                message: format!("Missing command dependency: {}. Please ensure the command is available and retry.", dependency),
            });
        }

        // For other dependencies, request user intervention
        Ok(RecoveryAction::RequestIntervention {
            message: format!(
                "Missing dependency: {}. Please install or configure the dependency and retry.",
                dependency
            ),
        })
    }

    /// Adapt to environment changes
    async fn adapt_to_environment_changes(&self, issue: &str) -> Result<RecoveryAction> {
        info!("Adapting to environment changes: {}", issue);

        // Try to continue with current environment
        warn!("Environment has changed since checkpoint. Attempting to continue with current environment.");
        Ok(RecoveryAction::Continue)
    }

    /// Handle handler execution failure
    async fn handle_handler_failure(&mut self, msg: &str) -> Result<RecoveryAction> {
        warn!("Handler execution failed: {}", msg);

        // Check if we should retry
        if let Some(ref mut retry_state) = self.recovery_state.retry_state {
            if retry_state.current_attempt < retry_state.max_attempts {
                retry_state.current_attempt += 1;
                let delay = Duration::from_secs(2_u64.pow(retry_state.current_attempt as u32));
                return Ok(RecoveryAction::Retry {
                    delay,
                    max_attempts: retry_state.max_attempts,
                });
            }
        }

        // Handler failed, request intervention
        Ok(RecoveryAction::RequestIntervention {
            message: format!(
                "Error handler failed: {}. Manual intervention required.",
                msg
            ),
        })
    }

    /// Default error recovery strategy
    async fn default_error_recovery(&self, error: &anyhow::Error) -> Result<RecoveryAction> {
        warn!("Applying default error recovery: {}", error);

        // Try a simple retry with backoff
        Ok(RecoveryAction::Retry {
            delay: Duration::from_secs(5),
            max_attempts: 3,
        })
    }

    /// Execute a handler command
    async fn execute_handler_command(
        &self,
        command: &HandlerCommand,
        _workflow_context: &mut WorkflowContext,
    ) -> Result<()> {
        if let Some(ref shell_cmd) = command.shell {
            info!("Executing shell handler: {}", shell_cmd);
            // TODO: Execute shell command through command executor
            // For now, just log
            debug!("Would execute shell command: {}", shell_cmd);
        }

        if let Some(ref claude_cmd) = command.claude {
            info!("Executing claude handler: {}", claude_cmd);
            // TODO: Execute claude command through command executor
            // For now, just log
            debug!("Would execute claude command: {}", claude_cmd);
        }

        Ok(())
    }
}

/// Convert OnFailureConfig to ErrorHandler
pub fn on_failure_to_error_handler(
    on_failure: &OnFailureConfig,
    step_index: usize,
) -> Option<ErrorHandler> {
    let commands = on_failure.handler_commands();
    if commands.is_empty() {
        return None;
    }

    let strategy = match on_failure {
        OnFailureConfig::Detailed(config) => config.strategy.clone(),
        _ => HandlerStrategy::Recovery,
    };

    Some(ErrorHandler {
        id: format!("step_{}_handler", step_index),
        condition: None,
        commands,
        retry_config: None,
        timeout: match on_failure {
            OnFailureConfig::Detailed(config) => config.timeout.map(Duration::from_secs),
            _ => None,
        },
        scope: ErrorHandlerScope::Step,
        strategy,
    })
}

/// Save error recovery state to checkpoint
pub fn save_recovery_state_to_checkpoint(
    checkpoint: &mut WorkflowCheckpoint,
    recovery_state: &ErrorRecoveryState,
) {
    // Store recovery state in checkpoint metadata
    // This would be implemented as an extension to the checkpoint structure
    // For now, we can store it in the variable_state as a JSON value
    checkpoint.variable_state.insert(
        "__error_recovery_state".to_string(),
        serde_json::to_value(recovery_state).unwrap_or(Value::Null),
    );
}

/// Load error recovery state from checkpoint
pub fn load_recovery_state_from_checkpoint(
    checkpoint: &WorkflowCheckpoint,
) -> Option<ErrorRecoveryState> {
    checkpoint
        .variable_state
        .get("__error_recovery_state")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}
