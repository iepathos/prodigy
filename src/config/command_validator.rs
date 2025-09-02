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

    fn register_built_in_commands(&mut self) {
        // Register prodigy-code-review
        self.register(CommandDefinition {
            name: "prodigy-code-review".to_string(),
            description: "Analyze code and generate improvement specs".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![
                OptionDef {
                    name: "focus".to_string(),
                    description: "Focus area for analysis (e.g., security, performance)"
                        .to_string(),
                    option_type: ArgumentType::String,
                    default: None,
                },
                OptionDef {
                    name: "max-issues".to_string(),
                    description: "Maximum number of issues to report".to_string(),
                    option_type: ArgumentType::Integer,
                    default: Some(serde_json::json!(10)),
                },
            ],
            defaults: CommandMetadata {
                retries: Some(2),
                timeout: Some(300),
                continue_on_error: Some(false),
                env: HashMap::new(),
                commit_required: true,
                analysis: None,
            },
        });

        // Register prodigy-implement-spec
        self.register(CommandDefinition {
            name: "prodigy-implement-spec".to_string(),
            description: "Implement a specification file".to_string(),
            required_args: vec![ArgumentDef {
                name: "spec-id".to_string(),
                description: "Specification identifier".to_string(),
                arg_type: ArgumentType::String,
            }],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata {
                retries: Some(1),
                timeout: Some(600),
                continue_on_error: Some(false),
                env: HashMap::new(),
                commit_required: true,
                analysis: None,
            },
        });

        // Register prodigy-lint
        self.register(CommandDefinition {
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
            defaults: CommandMetadata {
                retries: Some(1),
                timeout: Some(180),
                continue_on_error: Some(true),
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        });

        // Register prodigy-product-enhance
        self.register(CommandDefinition {
            name: "prodigy-product-enhance".to_string(),
            description: "Analyze code from product management perspective".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![
                OptionDef {
                    name: "focus".to_string(),
                    description: "Focus area for analysis (e.g., onboarding, api, cli-ux)"
                        .to_string(),
                    option_type: ArgumentType::String,
                    default: None,
                },
                OptionDef {
                    name: "max-enhancements".to_string(),
                    description: "Maximum number of enhancements to propose".to_string(),
                    option_type: ArgumentType::Integer,
                    default: Some(serde_json::json!(10)),
                },
            ],
            defaults: CommandMetadata {
                retries: Some(2),
                timeout: Some(300),
                continue_on_error: Some(false),
                env: HashMap::new(),
                commit_required: true,
                analysis: None,
            },
        });

        // Register prodigy-cleanup-tech-debt
        self.register(CommandDefinition {
            name: "prodigy-cleanup-tech-debt".to_string(),
            description: "Analyze technical debt and generate cleanup specifications".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![
                OptionDef {
                    name: "focus".to_string(),
                    description:
                        "Focus area for debt cleanup (e.g., performance, security, maintainability)"
                            .to_string(),
                    option_type: ArgumentType::String,
                    default: None,
                },
                OptionDef {
                    name: "scope".to_string(),
                    description: "Scope for analysis (e.g., src/agents, src/mcp, tests, all)"
                        .to_string(),
                    option_type: ArgumentType::String,
                    default: Some(serde_json::json!("all")),
                },
            ],
            defaults: CommandMetadata {
                retries: Some(2),
                timeout: Some(300),
                continue_on_error: Some(false),
                env: HashMap::new(),
                commit_required: true,
                analysis: None,
            },
        });
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

        Self::validate_argument_counts(command, definition)?;
        self.validate_required_arguments(command, definition)?;
        self.validate_command_options(command, definition)?;

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

    /// Validate required argument types
    fn validate_required_arguments(
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

    /// Validate command options
    fn validate_command_options(
        &self,
        command: &Command,
        definition: &CommandDefinition,
    ) -> Result<()> {
        for (opt_name, opt_value) in &command.options {
            if let Some(opt_def) = definition.options.iter().find(|o| &o.name == opt_name) {
                self.validate_option_value(opt_value, &opt_def.option_type)?;
            } else {
                // Warning: unknown option (but don't fail)
                eprintln!(
                    "Warning: Unknown option '{}' for command '{}'",
                    opt_name, command.name
                );
            }
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

    /// Apply defaults from command definition to a command
    pub fn apply_defaults(&self, command: &mut Command) {
        if let Some(definition) = self.commands.get(&command.name) {
            // Apply default metadata values if not set
            if command.metadata.retries.is_none() {
                command.metadata.retries = definition.defaults.retries;
            }
            if command.metadata.timeout.is_none() {
                command.metadata.timeout = definition.defaults.timeout;
            }
            if command.metadata.continue_on_error.is_none() {
                command.metadata.continue_on_error = definition.defaults.continue_on_error;
            }
            // Apply commit_required default from registry
            // Always apply the registry default - if the test needs different behavior,
            // it should use a different approach (like environment variables)
            command.metadata.commit_required = definition.defaults.commit_required;

            // Apply default option values if not set
            for opt_def in &definition.options {
                if !command.options.contains_key(&opt_def.name) {
                    if let Some(default_value) = &opt_def.default {
                        command
                            .options
                            .insert(opt_def.name.clone(), default_value.clone());
                    }
                }
            }
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
        let registry = CommandRegistry::new();
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

        // Should succeed because variables are skipped during validation
        assert!(registry
            .validate_required_arguments(&command, &definition)
            .is_ok());
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

        assert!(registry
            .validate_command_options(&command, &definition)
            .is_ok());
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

        let result = registry.validate_command_options(&command, &definition);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected integer value"));
    }
}
