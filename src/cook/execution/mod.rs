//! Command execution and subprocess management
//!
//! Provides abstractions for running commands and Claude CLI integration.

pub mod bridge;
#[cfg(test)]
pub mod bridge_tests;
pub mod claude;
pub mod claude_log_detection;
#[cfg(test)]
pub mod claude_log_path_test;
pub mod claude_stream_handler;
#[cfg(test)]
pub mod claude_streaming_test;
pub mod command;
#[cfg(test)]
pub mod command_tests;
pub mod data_pipeline;
pub mod dlq;
pub mod dlq_reprocessor;
#[cfg(test)]
pub mod dlq_reprocessor_test;
#[cfg(test)]
pub mod dlq_test;
pub mod errors;
#[cfg(test)]
pub mod errors_tests;
pub mod events;
pub mod executor;
#[cfg(test)]
pub mod executor_tests;
pub mod expression;
pub mod foreach;
#[cfg(test)]
pub mod foreach_tests;
pub mod input_source;
pub mod interpolation;
pub mod mapreduce;
#[cfg(test)]
pub mod mapreduce_integration_tests;
pub mod mapreduce_resume;
#[cfg(test)]
pub mod mapreduce_setup_test;
#[cfg(test)]
pub mod mapreduce_tests;
pub mod output;
#[cfg(test)]
pub mod output_tests;
pub mod process;
// pub mod process_kill_tests; // Already included via process_tests
#[cfg(test)]
pub mod process_tests;
pub mod progress;
pub mod progress_dashboard;
pub mod progress_display;
#[cfg(test)]
pub mod progress_display_tests;
#[cfg(test)]
pub mod progress_tests;
pub mod progress_tracker;
pub mod resume_lock;
#[cfg(test)]
pub mod resume_lock_tests;
pub mod runner;
pub mod setup_executor;
#[cfg(test)]
pub mod shell_failure_tests;
#[cfg(test)]
pub mod spec_121_integration_test;
pub mod state;
pub mod state_pure;
#[cfg(test)]
pub mod state_tests;
pub mod variable_capture;
#[cfg(test)]
pub mod variable_capture_test;
pub mod variables;
#[cfg(test)]
pub mod variables_test;

pub use bridge::{create_legacy_executor, LegacyExecutorBridge};
pub use claude::{ClaudeExecutor, ClaudeExecutorImpl};
pub use command::{CommandRequest, CommandSpec, CommandType, ExecutionConfig, OutputFormat};
pub use executor::{CommandExecutor as UnifiedExecutor, UnifiedCommandExecutor};
pub use mapreduce::{
    AgentResult, AgentStatus, MapPhase, MapReduceConfig, MapReduceExecutor, ReducePhase,
    ResumeOptions, ResumeResult, SetupPhase,
};
pub use resume_lock::{is_process_running, ResumeLock, ResumeLockData, ResumeLockManager};
pub use runner::{CommandRunner, RealCommandRunner};

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// Execution context for commands
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Environment variables to set
    pub env_vars: HashMap<String, String>,
    /// Working directory
    pub working_directory: std::path::PathBuf,
    /// Whether to capture output
    pub capture_output: bool,
    /// Timeout in seconds
    pub timeout_seconds: Option<u64>,
    /// Optional stdin input
    pub stdin: Option<String>,
    /// Whether to capture streaming output (for Claude observability)
    pub capture_streaming: bool,
    /// Streaming configuration for real-time output processing
    pub streaming_config: Option<crate::subprocess::streaming::StreamingConfig>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            env_vars: HashMap::new(),
            working_directory: std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()),
            capture_output: true,
            timeout_seconds: None,
            stdin: None,
            capture_streaming: false,
            streaming_config: None,
        }
    }
}

/// Result of command execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Exit status
    pub success: bool,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Metadata for additional execution information
    pub metadata: HashMap<String, String>,
}

impl ExecutionResult {
    /// Add JSON log location to metadata
    pub fn with_json_log_location(mut self, location: std::path::PathBuf) -> Self {
        self.metadata.insert(
            "claude_json_log".to_string(),
            location.to_string_lossy().to_string(),
        );
        self
    }

    /// Get JSON log location from metadata
    pub fn json_log_location(&self) -> Option<&str> {
        self.metadata.get("claude_json_log").map(String::as_str)
    }
}

/// Trait for executing commands
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command with context
    async fn execute(
        &self,
        command: &str,
        args: &[String],
        context: ExecutionContext,
    ) -> Result<ExecutionResult>;

    /// Execute a command and return output
    async fn execute_simple(&self, command: &str, args: &[String]) -> Result<String> {
        let result = self
            .execute(command, args, ExecutionContext::default())
            .await?;
        if result.success {
            Ok(result.stdout)
        } else {
            anyhow::bail!("Command failed: {}", result.stderr)
        }
    }
}
