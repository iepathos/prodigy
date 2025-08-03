use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default function for serde to return true
fn default_true() -> bool {
    true
}

/// Default cache duration in seconds (5 minutes)
fn default_cache_duration() -> u64 {
    300
}

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
    #[must_use]
    pub fn is_variable(&self) -> bool {
        matches!(self, CommandArg::Variable(_))
    }

    /// Resolve the argument value given a context
    #[must_use]
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
    #[must_use]
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

/// Structured command representation for workflow execution
///
/// Represents a fully-specified command with its arguments, options,
/// inputs, outputs, and metadata. This is the primary command format
/// for complex workflows with data flow between commands.
#[derive(Debug, Clone, Serialize)]
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

    /// Analysis requirements for this command (convenience field)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisConfig>,
}

/// Configuration for per-step analysis requirements
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisConfig {
    /// Force fresh analysis even if cached
    #[serde(default)]
    pub force_refresh: bool,

    /// Maximum age of cached analysis in seconds
    #[serde(default = "default_cache_duration")]
    pub max_cache_age: u64,
}

/// Metadata for command execution control
///
/// Contains optional parameters that control how a command is executed,
/// including retry behavior, timeouts, and error handling strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Whether this command is required to create commits (defaults to true)
    #[serde(default = "default_true")]
    pub commit_required: bool,

    /// Analysis requirements for this command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisConfig>,
}

impl Default for CommandMetadata {
    fn default() -> Self {
        Self {
            retries: None,
            timeout: None,
            continue_on_error: None,
            env: HashMap::new(),
            commit_required: true,
            analysis: None,
        }
    }
}

/// Declaration of a command output
///
/// Specifies how to extract and name outputs from command execution
/// for use by subsequent commands in the workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDeclaration {
    /// File pattern for git commit extraction (since we only extract from git commits)
    pub file_pattern: String,
}

/// Reference to an output from a previous command
///
/// Specifies which command's output to use and how to pass it
/// to the current command (via argument, environment, or stdin).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputReference {
    /// Reference to output: `"${command_id.output_name}"`
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
    /// Simple object format
    SimpleObject(SimpleCommand),
}

/// Simple command representation for basic workflows
///
/// Represents a command as a simple object with optional properties,
/// used for backward compatibility and simple workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleCommand {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisConfig>,
}

impl WorkflowCommand {
    #[must_use]
    pub fn to_command(&self) -> Command {
        match self {
            WorkflowCommand::Simple(s) => Command::from_string(s),
            WorkflowCommand::SimpleObject(simple) => {
                let mut cmd = Command::new(&simple.name);
                if let Some(commit_required) = simple.commit_required {
                    cmd.metadata.commit_required = commit_required;
                }
                if let Some(args) = &simple.args {
                    for arg in args {
                        cmd.args.push(CommandArg::parse(arg));
                    }
                }
                if let Some(analysis) = simple.analysis.clone() {
                    cmd.analysis = Some(analysis.clone());
                    cmd.metadata.analysis = Some(analysis);
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
            analysis: None,
        }
    }

    /// Parse a command from a simple string format
    #[must_use]
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
    #[must_use]
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.metadata.retries = Some(retries);
        self
    }

    /// Set timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.metadata.timeout = Some(timeout);
        self
    }

    /// Set continue on error
    #[must_use]
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

// Custom deserialization for Command to handle top-level commit_required
impl<'de> Deserialize<'de> for Command {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct CommandHelper {
            name: String,
            #[serde(default)]
            args: Vec<CommandArg>,
            #[serde(default)]
            options: HashMap<String, serde_json::Value>,
            #[serde(default)]
            metadata: CommandMetadata,
            id: Option<String>,
            outputs: Option<HashMap<String, OutputDeclaration>>,
            inputs: Option<HashMap<String, InputReference>>,
            // Allow commit_required at top level for convenience
            commit_required: Option<bool>,
            // Allow analysis at top level for convenience
            analysis: Option<AnalysisConfig>,
        }

        let helper = CommandHelper::deserialize(deserializer)?;

        let mut metadata = helper.metadata;
        // If commit_required is specified at top level, use it
        if let Some(commit_required) = helper.commit_required {
            metadata.commit_required = commit_required;
        }

        // Handle analysis configuration - prefer top-level over metadata.analysis
        let analysis = helper.analysis.or(metadata.analysis.clone());
        if analysis.is_some() {
            metadata.analysis = analysis.clone();
        }

        Ok(Command {
            name: helper.name,
            args: helper.args,
            options: helper.options,
            metadata,
            id: helper.id,
            outputs: helper.outputs,
            inputs: helper.inputs,
            analysis,
        })
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
            commit_required: None,
            args: None,
            analysis: None,
        });
        let cmd = simple_obj.to_command();
        assert_eq!(cmd.name, "mmm-code-review");

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

    #[test]
    fn test_commit_required_field() {
        // Test default value
        let cmd = Command::new("mmm-implement-spec");
        assert!(cmd.metadata.commit_required);

        // Test SimpleCommand with commit_required set to false
        let simple_obj = WorkflowCommand::SimpleObject(SimpleCommand {
            name: "mmm-lint".to_string(),
            commit_required: Some(false),
            args: None,
            analysis: None,
        });
        let cmd = simple_obj.to_command();
        assert_eq!(cmd.name, "mmm-lint");
        assert!(!cmd.metadata.commit_required);

        // Test SimpleCommand with commit_required set to true
        let simple_obj = WorkflowCommand::SimpleObject(SimpleCommand {
            name: "mmm-fix".to_string(),
            commit_required: Some(true),
            args: None,
            analysis: None,
        });
        let cmd = simple_obj.to_command();
        assert_eq!(cmd.name, "mmm-fix");
        assert!(cmd.metadata.commit_required);

        // Test SimpleCommand with commit_required not set (should default to true)
        let simple_obj = WorkflowCommand::SimpleObject(SimpleCommand {
            name: "mmm-refactor".to_string(),
            commit_required: None,
            args: None,
            analysis: None,
        });
        let cmd = simple_obj.to_command();
        assert_eq!(cmd.name, "mmm-refactor");
        assert!(cmd.metadata.commit_required);
    }

    #[test]
    fn test_commit_required_serialization() {
        // Test serialization and deserialization of SimpleCommand with commit_required
        let simple_cmd = SimpleCommand {
            name: "mmm-lint".to_string(),
            commit_required: Some(false),
            args: None,
            analysis: None,
        };

        let json = serde_json::to_string(&simple_cmd).unwrap();
        assert!(json.contains("\"commit_required\":false"));

        let deserialized: SimpleCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "mmm-lint");
        assert_eq!(deserialized.commit_required, Some(false));

        // Test that commit_required is omitted when None
        let simple_cmd_none = SimpleCommand {
            name: "mmm-test".to_string(),
            commit_required: None,
            args: None,
            analysis: None,
        };
        let json_none = serde_json::to_string(&simple_cmd_none).unwrap();
        assert!(!json_none.contains("commit_required"));
    }

    #[test]
    fn test_analysis_config_defaults() {
        let analysis_config = AnalysisConfig {
            force_refresh: false,
            max_cache_age: 300,
        };

        assert!(!analysis_config.force_refresh);
        assert_eq!(analysis_config.max_cache_age, 300);
    }

    #[test]
    fn test_analysis_config_serialization() {
        let analysis_config = AnalysisConfig {
            force_refresh: true,
            max_cache_age: 600,
        };

        let json = serde_json::to_string(&analysis_config).unwrap();
        let deserialized: AnalysisConfig = serde_json::from_str(&json).unwrap();

        assert!(deserialized.force_refresh);
        assert_eq!(deserialized.max_cache_age, 600);
    }

    #[test]
    fn test_command_with_analysis_config() {
        let mut cmd = Command::new("mmm-code-review");
        cmd.metadata.analysis = Some(AnalysisConfig {
            force_refresh: false,
            max_cache_age: 300,
        });

        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"analysis\""));
        assert!(json.contains("\"max_cache_age\":300"));

        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert!(deserialized.metadata.analysis.is_some());
        let analysis = deserialized.metadata.analysis.unwrap();
        assert_eq!(analysis.max_cache_age, 300);
    }

    #[test]
    fn test_default_cache_duration() {
        assert_eq!(default_cache_duration(), 300);
    }

    #[test]
    fn test_analysis_config_with_defaults() {
        // Test that deserializing with minimal fields works
        let json = r#"{
            "force_refresh": true
        }"#;
        let deserialized: AnalysisConfig = serde_json::from_str(json).unwrap();
        assert!(deserialized.force_refresh);
        assert_eq!(deserialized.max_cache_age, 300); // Should use default
    }
}
