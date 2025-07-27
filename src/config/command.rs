use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// The command name (e.g., "mmm-code-review")
    pub name: String,

    /// Positional arguments for the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Named options/flags for the command
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,

    /// Command-specific metadata
    #[serde(default)]
    pub metadata: CommandMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandMetadata {
    /// Number of retry attempts (overrides global setting)
    pub retries: Option<u32>,

    /// Timeout in seconds
    pub timeout: Option<u64>,

    /// Continue workflow on command failure
    pub continue_on_error: Option<bool>,

    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowCommand {
    /// Legacy string format
    Simple(String),
    /// New structured format
    Structured(Command),
}

impl WorkflowCommand {
    pub fn to_command(&self) -> Command {
        match self {
            WorkflowCommand::Simple(s) => Command::from_string(s),
            WorkflowCommand::Structured(c) => c.clone(),
        }
    }
}

impl Command {
    /// Create a new command with default metadata
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            args: Vec::new(),
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
        }
    }

    /// Parse a command from a simple string format
    pub fn from_string(s: &str) -> Self {
        // Remove leading slash if present
        let s = s.strip_prefix('/').unwrap_or(s);

        // For now, just create a simple command with the name
        // More sophisticated parsing can be added later
        Self::new(s)
    }

    /// Add a positional argument
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add an option
    pub fn with_option(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.options.insert(key.into(), value);
        self
    }

    /// Set retries
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.metadata.retries = Some(retries);
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.metadata.timeout = Some(timeout);
        self
    }

    /// Set continue on error
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.metadata.continue_on_error = Some(continue_on_error);
        self
    }

    /// Add environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.env.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_creation() {
        let cmd = Command::new("mmm-code-review")
            .with_arg("test")
            .with_option("focus", serde_json::json!("security"))
            .with_retries(3)
            .with_timeout(300);

        assert_eq!(cmd.name, "mmm-code-review");
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(cmd.args[0], "test");
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );
        assert_eq!(cmd.metadata.retries, Some(3));
        assert_eq!(cmd.metadata.timeout, Some(300));
    }

    #[test]
    fn test_command_from_string() {
        let cmd1 = Command::from_string("mmm-code-review");
        assert_eq!(cmd1.name, "mmm-code-review");
        assert!(cmd1.args.is_empty());

        let cmd2 = Command::from_string("/mmm-lint");
        assert_eq!(cmd2.name, "mmm-lint");
    }

    #[test]
    fn test_workflow_command_conversion() {
        let simple = WorkflowCommand::Simple("mmm-code-review".to_string());
        let cmd = simple.to_command();
        assert_eq!(cmd.name, "mmm-code-review");

        let structured = WorkflowCommand::Structured(Command::new("mmm-lint"));
        let cmd = structured.to_command();
        assert_eq!(cmd.name, "mmm-lint");
    }

    #[test]
    fn test_command_serialization() {
        let cmd =
            Command::new("mmm-code-review").with_option("focus", serde_json::json!("performance"));

        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, cmd.name);
        assert_eq!(deserialized.options, cmd.options);
    }
}
