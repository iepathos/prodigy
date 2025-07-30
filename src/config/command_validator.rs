use super::command::{Command, CommandMetadata};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    pub required_args: Vec<ArgumentDef>,
    pub optional_args: Vec<ArgumentDef>,
    pub options: Vec<OptionDef>,
    pub defaults: CommandMetadata,
}

#[derive(Debug, Clone)]
pub struct ArgumentDef {
    pub name: String,
    pub description: String,
    pub arg_type: ArgumentType,
}

#[derive(Debug, Clone)]
pub struct OptionDef {
    pub name: String,
    pub description: String,
    pub option_type: ArgumentType,
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgumentType {
    String,
    Integer,
    Boolean,
    Path,
    Enum(Vec<String>),
}

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
        // Register mmm-code-review
        self.register(CommandDefinition {
            name: "mmm-code-review".to_string(),
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
            },
        });

        // Register mmm-implement-spec
        self.register(CommandDefinition {
            name: "mmm-implement-spec".to_string(),
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
            },
        });

        // Register mmm-lint
        self.register(CommandDefinition {
            name: "mmm-lint".to_string(),
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
            },
        });

        // Register mmm-product-enhance
        self.register(CommandDefinition {
            name: "mmm-product-enhance".to_string(),
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
            },
        });

        // Register mmm-cleanup-tech-debt
        self.register(CommandDefinition {
            name: "mmm-cleanup-tech-debt".to_string(),
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

        // Validate required arguments
        if command.args.len() < definition.required_args.len() {
            return Err(anyhow!(
                "Command '{}' requires {} arguments, but {} provided",
                command.name,
                definition.required_args.len(),
                command.args.len()
            ));
        }

        // Validate argument types
        for (i, arg_def) in definition.required_args.iter().enumerate() {
            if i < command.args.len() {
                // Skip validation for variables - they will be resolved at runtime
                if !command.args[i].is_variable() {
                    let arg_str = match &command.args[i] {
                        crate::config::CommandArg::Literal(s) => s,
                        crate::config::CommandArg::Variable(_) => continue,
                    };
                    self.validate_argument_type(arg_str, &arg_def.arg_type)?;
                }
            }
        }

        // Validate optional arguments
        let total_expected_args = definition.required_args.len() + definition.optional_args.len();
        if command.args.len() > total_expected_args {
            return Err(anyhow!(
                "Command '{}' expects at most {} arguments, but {} provided",
                command.name,
                total_expected_args,
                command.args.len()
            ));
        }

        // Validate options
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
        assert!(registry.get("mmm-code-review").is_some());
        assert!(registry.get("mmm-implement-spec").is_some());
        assert!(registry.get("mmm-lint").is_some());
        assert!(registry.get("mmm-product-enhance").is_some());
        assert!(registry.get("mmm-cleanup-tech-debt").is_some());
        assert!(registry.get("unknown-command").is_none());
    }

    #[test]
    fn test_validate_valid_command() {
        let registry = CommandRegistry::new();

        let cmd = Command::new("mmm-code-review");
        assert!(registry.validate_command(&cmd).is_ok());

        let mut cmd = Command::new("mmm-implement-spec");
        cmd.args.push(crate::config::CommandArg::Literal(
            "iteration-123".to_string(),
        ));
        assert!(registry.validate_command(&cmd).is_ok());
    }

    #[test]
    fn test_validate_missing_required_args() {
        let registry = CommandRegistry::new();

        let cmd = Command::new("mmm-implement-spec");
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

        let mut cmd = Command::new("mmm-code-review");
        registry.apply_defaults(&mut cmd);

        assert_eq!(cmd.metadata.retries, Some(2));
        assert_eq!(cmd.metadata.timeout, Some(300));
        assert_eq!(cmd.metadata.continue_on_error, Some(false));
    }

    #[test]
    fn test_validate_option_types() {
        let registry = CommandRegistry::new();

        let mut cmd = Command::new("mmm-code-review");
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
}
