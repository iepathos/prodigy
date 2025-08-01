use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use super::builder::ProcessCommandBuilder;
use super::error::ProcessError;
use super::runner::ProcessRunner;

#[async_trait]
pub trait ClaudeRunner: Send + Sync {
    async fn check_availability(&self) -> Result<bool, ProcessError>;
    async fn run_command(
        &self,
        cmd: &str,
        args: &[String],
        env_vars: &HashMap<String, String>,
    ) -> Result<String, ProcessError>;
}

pub struct ClaudeRunnerImpl {
    runner: Arc<dyn ProcessRunner>,
}

impl ClaudeRunnerImpl {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl ClaudeRunner for ClaudeRunnerImpl {
    async fn check_availability(&self) -> Result<bool, ProcessError> {
        let result = self
            .runner
            .run(
                ProcessCommandBuilder::new("claude")
                    .args(&["--version"])
                    .build(),
            )
            .await;

        match result {
            Ok(output) => Ok(output.status.success()),
            Err(ProcessError::CommandNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn run_command(
        &self,
        cmd: &str,
        args: &[String],
        env_vars: &HashMap<String, String>,
    ) -> Result<String, ProcessError> {
        let mut builder = ProcessCommandBuilder::new("claude").arg(cmd);

        for arg in args {
            builder = builder.arg(arg);
        }

        for (key, value) in env_vars {
            builder = builder.env(key, value);
        }

        let output = self.runner.run(builder.build()).await?;

        if !output.status.success() {
            return Err(ProcessError::ExitCode(output.status.code().unwrap_or(1)));
        }

        Ok(output.stdout)
    }
}
