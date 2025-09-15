use super::command::{Command, CommandMetadata};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Definition of a command including its arguments, options, and metadata
///
/// This struct captures the complete specification of a command that can be
/// executed within MMM workflows, including validation rules and defaults.
#[derive(Debug, Clone)]
pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    pub required_args: Vec<ArgumentDef>,
    pub optional_args: Vec<ArgumentDef>,
    pub options: Vec<OptionDef>,
    pub defaults: CommandMetadata,
}

/// Definition of a command argument
///
/// Represents a positional argument for a command, including its type
/// and validation requirements.
#[derive(Debug, Clone)]
pub struct ArgumentDef {
    pub name: String,
    pub description: String,
    pub arg_type: ArgumentType,
}

/// Definition of a command option (flag)
///
/// Represents an optional flag or parameter for a command, including
/// its type, validation rules, and default value.
#[derive(Debug, Clone)]
pub struct OptionDef {
    pub name: String,
    pub description: String,
    pub option_type: ArgumentType,
    pub default: Option<serde_json::Value>,
}

/// Type specification for command arguments and options
///
/// Used for validation and type checking of command parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgumentType {
    String,
    Integer,
    Boolean,
    Path,
    Enum(Vec<String>),
}

/// Registry of known commands and their specifications
///
/// Central repository for all available MMM commands, providing
/// validation, defaults, and metadata for command execution.
pub struct CommandRegistry {
    commands: HashMap<String, CommandDefinition>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
        };
        registry.register_built_in_commands();
        registry
    }

    /// Create command metadata with default values
    fn create_metadata(
        retries: u32,
        timeout: u64,
        continue_on_error: bool,
        commit_required: bool,
    ) -> CommandMetadata {
        CommandMetadata {
            retries: Some(retries),
            timeout: Some(timeout),
            continue_on_error: Some(continue_on_error),
            env: HashMap::new(),
            commit_required,
            analysis: None,
        }
    }

    /// Create a focus option for analysis commands
    fn create_focus_option(description: &str) -> OptionDef {
        OptionDef {
            name: "focus".to_string(),
            description: description.to_string(),
            option_type: ArgumentType::String,
            default: None,
        }
    }

    /// Create a max count option with default value
    fn create_max_option(name: &str, description: &str, default: i32) -> OptionDef {
        OptionDef {
            name: name.to_string(),
            description: description.to_string(),
            option_type: ArgumentType::Integer,
            default: Some(serde_json::json!(default)),
        }
    }

    fn register_built_in_commands(&mut self) {
        // Define all built-in commands in a declarative way
        let commands = vec![
            // prodigy-code-review
            CommandDefinition {
                name: "prodigy-code-review".to_string(),
                description: "Analyze code and generate improvement specs".to_string(),
                required_args: vec![],
                optional_args: vec![],
                options: vec![
                    Self::create_focus_option(
                        "Focus area for analysis (e.g., security, performance)",
                    ),
                    Self::create_max_option("max-issues", "Maximum number of issues to report", 10),
                ],
                defaults: Self::create_metadata(2, 300, false, true),
            },
            // prodigy-implement-spec
            CommandDefinition {
                name: "prodigy-implement-spec".to_string(),
                description: "Implement a specification file".to_string(),
                required_args: vec![ArgumentDef {
                    name: "spec-id".to_string(),
                    description: "Specification identifier".to_string(),
                    arg_type: ArgumentType::String,
                }],
                optional_args: vec![],
                options: vec![],
                defaults: Self::create_metadata(1, 600, false, true),
            },
            // prodigy-lint
            CommandDefinition {
                name: "prodigy-lint".to_string(),
                description: "Run linting and formatting tools".to_string(),
                required_args: vec![],
                optional_args: vec![],
                options: vec![OptionDef {
                    name: "fix".to_string(),
                    description: "Automatically fix issues".to_string(),
                    option_type: ArgumentType::Boolean,
                    default: Some(serde_json::json!(true)),
                }],
                defaults: Self::create_metadata(1, 180, true, false),
            },
            // prodigy-product-enhance
            CommandDefinition {
                name: "prodigy-product-enhance".to_string(),
                description: "Analyze code from product management perspective".to_string(),
                required_args: vec![],
                optional_args: vec![],
                options: vec![
                    Self::create_focus_option(
                        "Focus area for analysis (e.g., onboarding, api, cli-ux)",
                    ),
                    Self::create_max_option(
                        "max-enhancements",
                        "Maximum number of enhancements to propose",
                        10,
                    ),
                ],
                defaults: Self::create_metadata(2, 300, false, true),
            },
            // prodigy-cleanup-tech-debt
            CommandDefinition {
                name: "prodigy-cleanup-tech-debt".to_string(),
                description: "Analyze technical debt and generate cleanup specifications".to_string(),
                required_args: vec![],
                optional_args: vec![],
                options: vec![
                    Self::create_focus_option(
                        "Focus area for debt cleanup (e.g., performance, security, maintainability)",
                    ),
                    OptionDef {
                        name: "scope".to_string(),
                        description: "Scope for analysis (e.g., src/agents, src/mcp, tests, all)"
                            .to_string(),
                        option_type: ArgumentType::String,
                        default: Some(serde_json::json!("all")),
                    },
                ],
                defaults: Self::create_metadata(2, 300, false, true),
            },
        ];

        // Register all commands
        for command in commands {
            self.register(command);
        }
    }

    pub fn register(&mut self, definition: CommandDefinition) {
        self.commands.insert(definition.name.clone(), definition);
    }

    pub fn get(&self, name: &str) -> Option<&CommandDefinition> {
        self.commands.get(name)
    }

    pub fn validate_command(&self, command: &Command) -> Result<()> {
        let definition = self
            .commands
            .get(&command.name)
            .ok_or_else(|| anyhow!("Unknown command: {}", command.name))?;

        // Delegate all validation to pure functions
        self.validate_arguments(command, definition)?;
        self.validate_options(command, definition)?;

        Ok(())
    }

    // Function for argument validation - orchestrates all argument checks
    fn validate_arguments(&self, command: &Command, definition: &CommandDefinition) -> Result<()> {
        Self::validate_argument_counts(command, definition)?;
        Self::validate_required_arguments(command, definition)?;
        Self::validate_optional_arguments(command, definition)?;
        self.validate_argument_types(command, definition)?;
        Ok(())
    }

    /// Validate that the argument count is within expected bounds
    fn validate_argument_counts(command: &Command, definition: &CommandDefinition) -> Result<()> {
        let required_count = definition.required_args.len();
        let max_count = required_count + definition.optional_args.len();
        let provided_count = command.args.len();

        match () {
            _ if provided_count < required_count => Err(anyhow!(
                "Command '{}' requires {} arguments, but {} provided",
                command.name,
                required_count,
                provided_count
            )),
            _ if provided_count > max_count => Err(anyhow!(
                "Command '{}' expects at most {} arguments, but {} provided",
                command.name,
                max_count,
                provided_count
            )),
            _ => Ok(()),
        }
    }

    // Pure function for required arguments validation
    fn validate_required_arguments(
        command: &Command,
        definition: &CommandDefinition,
    ) -> Result<()> {
        if command.args.len() < definition.required_args.len() {
            return Err(anyhow!(
                "Command '{}' requires {} arguments, but {} provided",
                command.name,
                definition.required_args.len(),
                command.args.len()
            ));
        }
        Ok(())
    }

    // Pure function for optional arguments validation
    fn validate_optional_arguments(
        command: &Command,
        definition: &CommandDefinition,
    ) -> Result<()> {
        let total_expected_args = definition.required_args.len() + definition.optional_args.len();
        if command.args.len() > total_expected_args {
            return Err(anyhow!(
                "Command '{}' expects at most {} arguments, but {} provided",
                command.name,
                total_expected_args,
                command.args.len()
            ));
        }
        Ok(())
    }

    // Function for options validation with extracted logic
    fn validate_options(&self, command: &Command, definition: &CommandDefinition) -> Result<()> {
        for (opt_name, opt_value) in &command.options {
            match definition.options.iter().find(|o| &o.name == opt_name) {
                Some(opt_def) => self.validate_option_value(opt_value, &opt_def.option_type)?,
                None => eprintln!(
                    "Warning: Unknown option '{}' for command '{}'",
                    opt_name, command.name
                ),
            }
        }
        Ok(())
    }

    // Function for argument type validation - now handles the looping logic
    fn validate_argument_types(
        &self,
        command: &Command,
        definition: &CommandDefinition,
    ) -> Result<()> {
        for (i, arg_def) in definition.required_args.iter().enumerate() {
            if i >= command.args.len() {
                break;
            }

            // Skip validation for variables - they will be resolved at runtime
            if command.args[i].is_variable() {
                continue;
            }

            let arg_str = match &command.args[i] {
                crate::config::CommandArg::Literal(s) => s,
                crate::config::CommandArg::Variable(_) => continue,
            };

            self.validate_argument_type(arg_str, &arg_def.arg_type)?;
        }
        Ok(())
    }

    fn validate_argument_type(&self, value: &str, arg_type: &ArgumentType) -> Result<()> {
        match arg_type {
            ArgumentType::String => Ok(()),
            ArgumentType::Integer => {
                value
                    .parse::<i64>()
                    .map_err(|_| anyhow!("Expected integer value, got: {}", value))?;
                Ok(())
            }
            ArgumentType::Boolean => {
                value
                    .parse::<bool>()
                    .map_err(|_| anyhow!("Expected boolean value, got: {}", value))?;
                Ok(())
            }
            ArgumentType::Path => {
                // Just check it's not empty for now
                if value.is_empty() {
                    return Err(anyhow!("Path cannot be empty"));
                }
                Ok(())
            }
            ArgumentType::Enum(values) => {
                if !values.contains(&value.to_string()) {
                    return Err(anyhow!(
                        "Invalid value '{}'. Expected one of: {}",
                        value,
                        values.join(", ")
                    ));
                }
                Ok(())
            }
        }
    }

    fn validate_option_value(
        &self,
        value: &serde_json::Value,
        option_type: &ArgumentType,
    ) -> Result<()> {
        match option_type {
            ArgumentType::String => {
                if !value.is_string() {
                    return Err(anyhow!("Expected string value"));
                }
                Ok(())
            }
            ArgumentType::Integer => {
                if !value.is_number() {
                    return Err(anyhow!("Expected integer value"));
                }
                Ok(())
            }
            ArgumentType::Boolean => {
                if !value.is_boolean() {
                    return Err(anyhow!("Expected boolean value"));
                }
                Ok(())
            }
            ArgumentType::Path => {
                if !value.is_string() {
                    return Err(anyhow!("Expected path string"));
                }
                Ok(())
            }
            ArgumentType::Enum(values) => {
                if let Some(s) = value.as_str() {
                    if !values.contains(&s.to_string()) {
                        return Err(anyhow!(
                            "Invalid value '{}'. Expected one of: {}",
                            s,
                            values.join(", ")
                        ));
                    }
                    Ok(())
                } else {
                    Err(anyhow!("Expected string value for enum"))
                }
            }
        }
    }

    /// Apply metadata defaults to a command from definition
    fn apply_metadata_defaults(metadata: &mut CommandMetadata, defaults: &CommandMetadata) {
        if metadata.retries.is_none() {
            metadata.retries = defaults.retries;
        }
        if metadata.timeout.is_none() {
            metadata.timeout = defaults.timeout;
        }
        if metadata.continue_on_error.is_none() {
            metadata.continue_on_error = defaults.continue_on_error;
        }
        // Apply commit_required default from registry
        // Always apply the registry default - if the test needs different behavior,
        // it should use a different approach (like environment variables)
        metadata.commit_required = defaults.commit_required;
    }

    /// Apply option defaults to a command from definition
    fn apply_option_defaults(
        options: &mut HashMap<String, serde_json::Value>,
        option_definitions: &[OptionDef],
    ) {
        for opt_def in option_definitions {
            if !options.contains_key(&opt_def.name) {
                if let Some(default_value) = &opt_def.default {
                    options.insert(opt_def.name.clone(), default_value.clone());
                }
            }
        }
    }

    /// Apply defaults from command definition to a command
    pub fn apply_defaults(&self, command: &mut Command) {
        if let Some(definition) = self.commands.get(&command.name) {
            Self::apply_metadata_defaults(&mut command.metadata, &definition.defaults);
            Self::apply_option_defaults(&mut command.options, &definition.options);
        }
    }

    /// List all registered command names
    pub fn list_commands(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }
}

/// Global command registry
pub static COMMAND_REGISTRY: Lazy<CommandRegistry> = Lazy::new(CommandRegistry::new);

/// Validate a command against the global registry
pub fn validate_command(command: &Command) -> Result<()> {
    COMMAND_REGISTRY.validate_command(command)
}

/// Apply defaults to a command from the global registry
pub fn apply_command_defaults(command: &mut Command) {
    COMMAND_REGISTRY.apply_defaults(command);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_metadata() {
        let metadata = CommandRegistry::create_metadata(2, 300, false, true);
        assert_eq!(metadata.retries, Some(2));
        assert_eq!(metadata.timeout, Some(300));
        assert_eq!(metadata.continue_on_error, Some(false));
        assert!(metadata.commit_required);
        assert!(metadata.env.is_empty());
        assert!(metadata.analysis.is_none());
    }

    #[test]
    fn test_create_focus_option() {
        let option = CommandRegistry::create_focus_option("Test focus description");
        assert_eq!(option.name, "focus");
        assert_eq!(option.description, "Test focus description");
        assert_eq!(option.option_type, ArgumentType::String);
        assert!(option.default.is_none());
    }

    #[test]
    fn test_create_max_option() {
        let option = CommandRegistry::create_max_option("max-test", "Test max description", 25);
        assert_eq!(option.name, "max-test");
        assert_eq!(option.description, "Test max description");
        assert_eq!(option.option_type, ArgumentType::Integer);
        assert_eq!(option.default, Some(serde_json::json!(25)));
    }

    #[test]
    fn test_registry_initialization() {
        let registry = CommandRegistry::new();
        assert!(registry.get("prodigy-code-review").is_some());
        assert!(registry.get("prodigy-implement-spec").is_some());
        assert!(registry.get("prodigy-lint").is_some());
        assert!(registry.get("prodigy-product-enhance").is_some());
        assert!(registry.get("prodigy-cleanup-tech-debt").is_some());
        assert!(registry.get("unknown-command").is_none());
    }

    #[test]
    fn test_validate_required_arguments_success() {
        let mut cmd = Command::new("test-cmd");
        cmd.args = vec![
            crate::config::CommandArg::Literal("arg1".to_string()),
            crate::config::CommandArg::Literal("arg2".to_string()),
        ];

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![
                ArgumentDef {
                    name: "arg1".to_string(),
                    description: "First arg".to_string(),
                    arg_type: ArgumentType::String,
                },
                ArgumentDef {
                    name: "arg2".to_string(),
                    description: "Second arg".to_string(),
                    arg_type: ArgumentType::String,
                },
            ],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata::default(),
        };

        assert!(CommandRegistry::validate_required_arguments(&cmd, &definition).is_ok());
    }

    #[test]
    fn test_validate_required_arguments_failure() {
        let cmd = Command::new("test-cmd");

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![ArgumentDef {
                name: "arg1".to_string(),
                description: "First arg".to_string(),
                arg_type: ArgumentType::String,
            }],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata::default(),
        };

        let result = CommandRegistry::validate_required_arguments(&cmd, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires 1 arguments"));
    }

    #[test]
    fn test_validate_optional_arguments_within_limit() {
        let mut cmd = Command::new("test-cmd");
        cmd.args = vec![
            crate::config::CommandArg::Literal("arg1".to_string()),
            crate::config::CommandArg::Literal("opt1".to_string()),
        ];

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![ArgumentDef {
                name: "arg1".to_string(),
                description: "Required arg".to_string(),
                arg_type: ArgumentType::String,
            }],
            optional_args: vec![ArgumentDef {
                name: "opt1".to_string(),
                description: "Optional arg".to_string(),
                arg_type: ArgumentType::String,
            }],
            options: vec![],
            defaults: CommandMetadata::default(),
        };

        assert!(CommandRegistry::validate_optional_arguments(&cmd, &definition).is_ok());
    }

    #[test]
    fn test_validate_optional_arguments_exceeds_limit() {
        let mut cmd = Command::new("test-cmd");
        cmd.args = vec![
            crate::config::CommandArg::Literal("arg1".to_string()),
            crate::config::CommandArg::Literal("arg2".to_string()),
            crate::config::CommandArg::Literal("arg3".to_string()),
        ];

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![ArgumentDef {
                name: "arg1".to_string(),
                description: "Required arg".to_string(),
                arg_type: ArgumentType::String,
            }],
            optional_args: vec![ArgumentDef {
                name: "opt1".to_string(),
                description: "Optional arg".to_string(),
                arg_type: ArgumentType::String,
            }],
            options: vec![],
            defaults: CommandMetadata::default(),
        };

        let result = CommandRegistry::validate_optional_arguments(&cmd, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("expects at most 2 arguments"));
    }

    #[test]
    fn test_validate_argument_types_with_variables() {
        let registry = CommandRegistry::new();
        let mut cmd = Command::new("test-cmd");
        cmd.args = vec![
            crate::config::CommandArg::Variable("var1".to_string()),
            crate::config::CommandArg::Literal("123".to_string()),
        ];

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![
                ArgumentDef {
                    name: "arg1".to_string(),
                    description: "String arg".to_string(),
                    arg_type: ArgumentType::String,
                },
                ArgumentDef {
                    name: "arg2".to_string(),
                    description: "Integer arg".to_string(),
                    arg_type: ArgumentType::Integer,
                },
            ],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata::default(),
        };

        // Variables should be skipped for validation
        assert!(registry.validate_argument_types(&cmd, &definition).is_ok());
    }

    #[test]
    fn test_validate_argument_types_integer_validation() {
        let registry = CommandRegistry::new();
        let mut cmd = Command::new("test-cmd");
        cmd.args = vec![crate::config::CommandArg::Literal(
            "not-a-number".to_string(),
        )];

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![ArgumentDef {
                name: "arg1".to_string(),
                description: "Integer arg".to_string(),
                arg_type: ArgumentType::Integer,
            }],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata::default(),
        };

        let result = registry.validate_argument_types(&cmd, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected integer value"));
    }

    #[test]
    fn test_validate_options_with_unknown_option() {
        let registry = CommandRegistry::new();
        let mut cmd = Command::new("test-cmd");
        cmd.options
            .insert("unknown".to_string(), serde_json::json!("value"));

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata::default(),
        };

        // Unknown options should generate warning but not fail
        assert!(registry.validate_options(&cmd, &definition).is_ok());
    }

    #[test]
    fn test_validate_options_type_mismatch() {
        let registry = CommandRegistry::new();
        let mut cmd = Command::new("test-cmd");
        cmd.options
            .insert("count".to_string(), serde_json::json!("not-a-number"));

        let definition = CommandDefinition {
            name: "test-cmd".to_string(),
            description: "Test".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![OptionDef {
                name: "count".to_string(),
                description: "Count option".to_string(),
                option_type: ArgumentType::Integer,
                default: None,
            }],
            defaults: CommandMetadata::default(),
        };

        let result = registry.validate_options(&cmd, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected integer value"));
    }

    #[test]
    fn test_validate_valid_command() {
        let registry = CommandRegistry::new();

        let cmd = Command::new("prodigy-code-review");
        assert!(registry.validate_command(&cmd).is_ok());

        let mut cmd = Command::new("prodigy-implement-spec");
        cmd.args.push(crate::config::CommandArg::Literal(
            "iteration-123".to_string(),
        ));
        assert!(registry.validate_command(&cmd).is_ok());
    }

    #[test]
    fn test_validate_missing_required_args() {
        let registry = CommandRegistry::new();

        let cmd = Command::new("prodigy-implement-spec");
        assert!(registry.validate_command(&cmd).is_err());
    }

    #[test]
    fn test_validate_unknown_command() {
        let registry = CommandRegistry::new();

        let cmd = Command::new("unknown-command");
        assert!(registry.validate_command(&cmd).is_err());
    }

    #[test]
    fn test_apply_defaults() {
        let registry = CommandRegistry::new();

        let mut cmd = Command::new("prodigy-code-review");
        registry.apply_defaults(&mut cmd);

        assert_eq!(cmd.metadata.retries, Some(2));
        assert_eq!(cmd.metadata.timeout, Some(300));
        assert_eq!(cmd.metadata.continue_on_error, Some(false));
    }

    #[test]
    fn test_validate_option_types() {
        let registry = CommandRegistry::new();

        let mut cmd = Command::new("prodigy-code-review");
        cmd.options
            .insert("focus".to_string(), serde_json::json!("security"));
        cmd.options
            .insert("max-issues".to_string(), serde_json::json!(5));

        assert!(registry.validate_command(&cmd).is_ok());

        // Invalid option type
        cmd.options
            .insert("max-issues".to_string(), serde_json::json!("not-a-number"));
        assert!(registry.validate_command(&cmd).is_err());
    }

    #[test]
    fn test_validate_command_valid() {
        // Test validating a valid command
        let command = Command {
            name: "prodigy-code-review".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };

        let result = validate_command(&command);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_command_invalid_name() {
        // Test error for invalid command name
        let command = Command {
            name: "".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };

        let result = validate_command(&command);
        assert!(result.is_err());
        // Empty name is treated as unknown command
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

    #[test]
    fn test_validate_command_empty_command() {
        // Test error for unknown command
        let command = Command {
            name: "test-command".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };

        let result = validate_command(&command);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

    #[test]
    fn test_validate_argument_counts_exact_required() {
        let command = Command {
            name: "test".to_string(),
            args: vec![crate::config::CommandArg::Literal("arg1".to_string())],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };

        let definition = CommandDefinition {
            name: "test".to_string(),
            description: "Test command".to_string(),
            required_args: vec![ArgumentDef {
                name: "arg1".to_string(),
                arg_type: ArgumentType::String,
                description: "test arg".to_string(),
            }],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata {
                retries: None,
                timeout: None,
                continue_on_error: None,
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        assert!(CommandRegistry::validate_argument_counts(&command, &definition).is_ok());
    }

    #[test]
    fn test_validate_argument_counts_too_few() {
        let command = Command {
            name: "test".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };

        let definition = CommandDefinition {
            name: "test".to_string(),
            description: "Test command".to_string(),
            required_args: vec![ArgumentDef {
                name: "arg1".to_string(),
                arg_type: ArgumentType::String,
                description: "test arg".to_string(),
            }],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata {
                retries: None,
                timeout: None,
                continue_on_error: None,
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        let result = CommandRegistry::validate_argument_counts(&command, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires 1 arguments, but 0 provided"));
    }

    #[test]
    fn test_validate_argument_counts_too_many() {
        let command = Command {
            name: "test".to_string(),
            args: vec![
                crate::config::CommandArg::Literal("arg1".to_string()),
                crate::config::CommandArg::Literal("arg2".to_string()),
                crate::config::CommandArg::Literal("arg3".to_string()),
            ],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };

        let definition = CommandDefinition {
            name: "test".to_string(),
            description: "Test command".to_string(),
            required_args: vec![ArgumentDef {
                name: "arg1".to_string(),
                arg_type: ArgumentType::String,
                description: "test arg".to_string(),
            }],
            optional_args: vec![ArgumentDef {
                name: "opt1".to_string(),
                arg_type: ArgumentType::String,
                description: "optional arg".to_string(),
            }],
            options: vec![],
            defaults: CommandMetadata {
                retries: None,
                timeout: None,
                continue_on_error: None,
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        let result = CommandRegistry::validate_argument_counts(&command, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("expects at most 2 arguments, but 3 provided"));
    }

    #[test]
    fn test_validate_required_arguments_with_variables() {
        let _registry = CommandRegistry::new();
        let command = Command {
            name: "test".to_string(),
            args: vec![
                crate::config::CommandArg::Variable("var1".to_string()),
                crate::config::CommandArg::Literal("literal".to_string()),
            ],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };

        let definition = CommandDefinition {
            name: "test".to_string(),
            description: "Test command".to_string(),
            required_args: vec![
                ArgumentDef {
                    name: "arg1".to_string(),
                    arg_type: ArgumentType::Integer,
                    description: "test arg".to_string(),
                },
                ArgumentDef {
                    name: "arg2".to_string(),
                    arg_type: ArgumentType::String,
                    description: "test arg 2".to_string(),
                },
            ],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata {
                retries: None,
                timeout: None,
                continue_on_error: None,
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        // Should succeed because we have the right number of arguments
        // Variables are validated in validate_argument_types, not validate_required_arguments
        assert!(CommandRegistry::validate_required_arguments(&command, &definition).is_ok());
    }

    #[test]
    fn test_validate_command_options_valid() {
        let registry = CommandRegistry::new();
        let mut command = Command {
            name: "test".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };
        command
            .options
            .insert("test-option".to_string(), serde_json::json!("value"));

        let definition = CommandDefinition {
            name: "test".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![OptionDef {
                name: "test-option".to_string(),
                option_type: ArgumentType::String,
                description: "test option".to_string(),
                default: None,
            }],
            defaults: CommandMetadata {
                retries: None,
                timeout: None,
                continue_on_error: None,
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        assert!(registry.validate_options(&command, &definition).is_ok());
    }

    #[test]
    fn test_validate_command_options_invalid_type() {
        let registry = CommandRegistry::new();
        let mut command = Command {
            name: "test".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: CommandMetadata::default(),
            id: None,
            outputs: None,
            analysis: None,
        };
        command
            .options
            .insert("test-option".to_string(), serde_json::json!("not-a-number"));

        let definition = CommandDefinition {
            name: "test".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![OptionDef {
                name: "test-option".to_string(),
                option_type: ArgumentType::Integer,
                description: "test option".to_string(),
                default: None,
            }],
            defaults: CommandMetadata {
                retries: None,
                timeout: None,
                continue_on_error: None,
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        let result = registry.validate_options(&command, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected integer value"));
    }
}
