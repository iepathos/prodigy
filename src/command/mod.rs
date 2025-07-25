use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;

pub mod dispatcher;
pub mod history;
pub mod registry;

pub use dispatcher::CommandDispatcher;
pub use history::CommandHistory;
pub use registry::CommandRegistry;

#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> Vec<&str> {
        vec![]
    }

    async fn execute(&self, context: CommandContext) -> Result<CommandOutput>;

    fn validate_args(&self, _args: &[String]) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct CommandContext {
    pub args: Vec<String>,
    pub config: crate::config::Config,
    pub project_path: Option<std::path::PathBuf>,
    pub variables: HashMap<String, String>,
}

#[derive(Debug)]
pub struct CommandOutput {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl CommandOutput {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

pub struct BatchCommand {
    pub commands: Vec<String>,
    pub stop_on_error: bool,
}

impl BatchCommand {
    pub fn new(commands: Vec<String>) -> Self {
        Self {
            commands,
            stop_on_error: true,
        }
    }

    pub fn continue_on_error(mut self) -> Self {
        self.stop_on_error = false;
        self
    }
}
