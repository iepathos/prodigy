use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a command argument that can be a literal value or a variable
#[derive(Debug, Clone, PartialEq)]
pub enum CommandArg {
    /// A literal string value
    Literal(String),
    /// A variable reference (e.g., "$FILE", "$ARG")
    Variable(String),
}

// Custom serialization for CommandArg
impl Serialize for CommandArg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CommandArg::Literal(s) => serializer.serialize_str(s),
            CommandArg::Variable(var) => serializer.serialize_str(&format!("${{{var}}}")),
        }
    }
}

// Custom deserialization for CommandArg
impl<'de> Deserialize<'de> for CommandArg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(CommandArg::parse(&s))
    }
}

impl CommandArg {
    /// Check if this is a variable reference
    pub fn is_variable(&self) -> bool {
        matches!(self, CommandArg::Variable(_))
    }

    /// Resolve the argument value given a context
    pub fn resolve(&self, variables: &HashMap<String, String>) -> String {
        match self {
            CommandArg::Literal(s) => s.clone(),
            CommandArg::Variable(var) => variables.get(var).cloned().unwrap_or_else(|| {
                // Return the variable reference if not found
                format!("${var}")
            }),
        }
    }

    /// Parse from a string, detecting variables by $ prefix
    pub fn parse(s: &str) -> Self {
        // Handle ${VAR} format
        if s.starts_with("${") && s.ends_with('}') {
            CommandArg::Variable(s[2..s.len() - 1].to_string())
        } else if let Some(var) = s.strip_prefix('$') {
            // Handle $VAR format
            CommandArg::Variable(var.to_string())
        } else {
            CommandArg::Literal(s.to_string())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// The command name (e.g., "mmm-code-review")
    pub name: String,

    /// Positional arguments for the command
    #[serde(default)]
    pub args: Vec<CommandArg>,

    /// Named options/flags for the command
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,

    /// Command-specific metadata
    #[serde(default)]
    pub metadata: CommandMetadata,

    /// Unique identifier for this command in the workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Outputs this command produces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<HashMap<String, OutputDeclaration>>,

    /// Inputs this command expects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, InputReference>>,
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
pub struct OutputDeclaration {
    /// File pattern for git commit extraction (since we only extract from git commits)
    pub file_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputReference {
    /// Reference to output: "${command_id.output_name}"
    pub from: String,

    /// How to pass the input to the command
    pub pass_as: InputMethod,

    /// Fallback value if reference not found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMethod {
    /// Pass as positional argument
    Argument { position: usize },

    /// Set as environment variable
    Environment { name: String },

    /// Pass via stdin
    Stdin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowCommand {
    /// Legacy string format
    Simple(String),
    /// Full structured format
    Structured(Box<Command>),
    /// Simple object format with focus
    SimpleObject(SimpleCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleCommand {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
}

impl WorkflowCommand {
    pub fn to_command(&self) -> Command {
        match self {
            WorkflowCommand::Simple(s) => Command::from_string(s),
            WorkflowCommand::SimpleObject(simple) => {
                let mut cmd = Command::new(&simple.name);
                if let Some(focus) = &simple.focus {
                    cmd.options
                        .insert("focus".to_string(), serde_json::json!(focus));
                }
                cmd
            }
            WorkflowCommand::Structured(c) => *c.clone(),
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
            id: None,
            outputs: None,
            inputs: None,
        }
    }

    /// Parse a command from a simple string format
    pub fn from_string(s: &str) -> Self {
        // Use the command parser for proper argument handling
        match crate::config::command_parser::parse_command_string(s) {
            Ok(cmd) => cmd,
            Err(_) => {
                // Fallback to simple name-only command for backward compatibility
                let s = s.strip_prefix('/').unwrap_or(s);
                Self::new(s)
            }
        }
    }

    /// Add a positional argument
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        let arg_str = arg.into();
        self.args.push(CommandArg::parse(&arg_str));
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
        assert_eq!(cmd.args[0], CommandArg::Literal("test".to_string()));
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

        // Test parsing commands with arguments
        let cmd3 = Command::from_string("mmm-implement-spec iteration-123");
        assert_eq!(cmd3.name, "mmm-implement-spec");
        assert_eq!(cmd3.args.len(), 1);
        assert_eq!(
            cmd3.args[0],
            CommandArg::Literal("iteration-123".to_string())
        );

        // Test parsing commands with options
        let cmd4 = Command::from_string("mmm-code-review --focus security");
        assert_eq!(cmd4.name, "mmm-code-review");
        assert_eq!(
            cmd4.options.get("focus"),
            Some(&serde_json::json!("security"))
        );
    }

    #[test]
    fn test_workflow_command_conversion() {
        let simple = WorkflowCommand::Simple("mmm-code-review".to_string());
        let cmd = simple.to_command();
        assert_eq!(cmd.name, "mmm-code-review");

        let simple_obj = WorkflowCommand::SimpleObject(SimpleCommand {
            name: "mmm-code-review".to_string(),
            focus: Some("security".to_string()),
        });
        let cmd = simple_obj.to_command();
        assert_eq!(cmd.name, "mmm-code-review");
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );

        let structured = WorkflowCommand::Structured(Box::new(Command::new("mmm-lint")));
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
