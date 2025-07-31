use super::command::Command;
use super::command_discovery::CommandDiscovery;
use super::command_validator::{CommandDefinition, CommandRegistry as StaticCommandRegistry};
use super::metadata_parser::MetadataParser;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Dynamic command registry that combines discovered and built-in commands
///
/// This registry automatically discovers commands from the filesystem and
/// combines them with built-in commands, providing a unified interface for
/// command validation and metadata application.
pub struct DynamicCommandRegistry {
    discovered_commands: HashMap<String, CommandDefinition>,
    fallback_registry: StaticCommandRegistry,
    discovery: CommandDiscovery,
    parser: MetadataParser,
}

impl DynamicCommandRegistry {
    /// Create a new dynamic command registry
    ///
    /// # Arguments
    /// * `commands_dir` - Optional path to commands directory (defaults to .claude/commands)
    pub async fn new(commands_dir: Option<PathBuf>) -> Result<Self> {
        let commands_dir = commands_dir.unwrap_or_else(|| PathBuf::from(".claude/commands"));
        let discovery = CommandDiscovery::new(commands_dir);
        let parser = MetadataParser::new();
        let fallback_registry = StaticCommandRegistry::new();

        let mut registry = Self {
            discovered_commands: HashMap::new(),
            fallback_registry,
            discovery,
            parser,
        };

        registry.refresh().await?;
        Ok(registry)
    }

    /// Refresh the command registry by rescanning the filesystem
    pub async fn refresh(&mut self) -> Result<()> {
        let command_files = self.discovery.scan_commands().await?;
        let mut new_commands = HashMap::new();

        for file in command_files {
            match self.parser.parse_command_file(&file) {
                Ok(definition) => {
                    new_commands.insert(definition.name.clone(), definition);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse command file {}: {}",
                        file.path.display(),
                        e
                    );
                    // Create minimal definition for unparseable commands
                    let minimal = self.parser.create_minimal_definition(&file);
                    new_commands.insert(minimal.name.clone(), minimal);
                }
            }
        }

        self.discovered_commands = new_commands;
        Ok(())
    }

    /// Get a command definition by name
    pub fn get(&self, name: &str) -> Option<&CommandDefinition> {
        self.discovered_commands
            .get(name)
            .or_else(|| self.fallback_registry.get(name))
    }

    /// Validate a command against its definition
    pub fn validate_command(&self, command: &Command) -> Result<()> {
        // Check discovered commands first
        if let Some(definition) = self.discovered_commands.get(&command.name) {
            return self.validate_against_definition(command, definition);
        }

        // Fall back to static registry
        if let Some(_definition) = self.fallback_registry.get(&command.name) {
            return self.fallback_registry.validate_command(command);
        }

        // Command not found
        Err(anyhow!("Unknown command: {}", command.name))
    }

    /// Apply default values from command definition
    pub fn apply_defaults(&self, command: &mut Command) {
        if let Some(definition) = self.discovered_commands.get(&command.name) {
            self.apply_definition_defaults(command, definition);
        } else {
            self.fallback_registry.apply_defaults(command);
        }
    }

    /// Validate command against a specific definition
    fn validate_against_definition(
        &self,
        command: &Command,
        definition: &CommandDefinition,
    ) -> Result<()> {
        if definition.required_args.is_empty() && definition.options.is_empty() {
            // Permissive validation for minimal definitions
            return Ok(());
        }

        // Use strict validation for detailed definitions
        self.validate_strict(command, definition)
    }

    /// Strict validation logic
    fn validate_strict(&self, command: &Command, definition: &CommandDefinition) -> Result<()> {
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

    /// Validate argument type
    fn validate_argument_type(
        &self,
        value: &str,
        arg_type: &super::command_validator::ArgumentType,
    ) -> Result<()> {
        use super::command_validator::ArgumentType;

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

    /// Validate option value
    fn validate_option_value(
        &self,
        value: &serde_json::Value,
        option_type: &super::command_validator::ArgumentType,
    ) -> Result<()> {
        use super::command_validator::ArgumentType;

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

    /// Apply default values from definition to command
    fn apply_definition_defaults(&self, command: &mut Command, definition: &CommandDefinition) {
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

    /// List all available commands
    pub fn list_commands(&self) -> Vec<String> {
        let mut commands: Vec<String> = self.discovered_commands.keys().cloned().collect();

        // Add fallback commands that aren't already discovered
        for cmd in self.fallback_registry.list_commands() {
            if !commands.contains(&cmd) {
                commands.push(cmd);
            }
        }

        commands.sort();
        commands
    }
}

impl Default for DynamicCommandRegistry {
    fn default() -> Self {
        // For tests and cases where async construction isn't possible
        Self {
            discovered_commands: HashMap::new(),
            fallback_registry: StaticCommandRegistry::new(),
            discovery: CommandDiscovery::new(PathBuf::from(".claude/commands")),
            parser: MetadataParser::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Command, CommandArg};
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_dynamic_registry_creation() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Should have built-in commands from fallback registry
        assert!(registry.get("mmm-code-review").is_some());
        assert!(registry.get("mmm-implement-spec").is_some());
    }

    #[tokio::test]
    async fn test_command_discovery_and_validation() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        // Create a test command file
        let command_content = r#"# /mmm-custom-test

Custom test command.

## Variables

TARGET: $ARGUMENTS (required - target to test)

## Options

- `--verbose`: Enable verbose output
"#;

        fs::write(commands_dir.join("mmm-custom-test.md"), command_content)
            .await
            .unwrap();

        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Should discover the custom command
        assert!(registry.get("mmm-custom-test").is_some());

        // Test validation
        let mut cmd = Command::new("mmm-custom-test");
        cmd.args.push(CommandArg::Literal("target.rs".to_string()));

        assert!(registry.validate_command(&cmd).is_ok());
    }

    #[tokio::test]
    async fn test_permissive_validation() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        // Create a minimal command file
        let command_content = r#"# /mmm-minimal

A minimal command without metadata.
"#;

        fs::write(commands_dir.join("mmm-minimal.md"), command_content)
            .await
            .unwrap();

        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Should allow any arguments/options for minimal commands
        let mut cmd = Command::new("mmm-minimal");
        cmd.args.push(CommandArg::Literal("arg1".to_string()));
        cmd.args.push(CommandArg::Literal("arg2".to_string()));
        cmd.options
            .insert("any-option".to_string(), serde_json::json!("value"));

        assert!(registry.validate_command(&cmd).is_ok());
    }

    #[tokio::test]
    async fn test_list_commands() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        // Create a custom command
        fs::write(
            commands_dir.join("mmm-custom.md"),
            "# /mmm-custom\n\nCustom command.",
        )
        .await
        .unwrap();

        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();
        let commands = registry.list_commands();

        // Should include both discovered and built-in commands
        assert!(commands.contains(&"mmm-custom".to_string()));
        assert!(commands.contains(&"mmm-code-review".to_string()));
        assert!(commands.contains(&"mmm-implement-spec".to_string()));
    }
}
