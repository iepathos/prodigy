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
        Self::validate_json_value_type(value, option_type)
    }

    /// Pure function to validate JSON value against expected type
    fn validate_json_value_type(
        value: &serde_json::Value,
        expected_type: &super::command_validator::ArgumentType,
    ) -> Result<()> {
        use super::command_validator::ArgumentType;

        let type_name = Self::get_type_name(expected_type);
        let is_valid = Self::check_value_type(value, expected_type);

        match (is_valid, expected_type) {
            (true, _) => Ok(()),
            (false, ArgumentType::Enum(values)) if value.is_string() => Err(anyhow!(
                "Invalid value '{}'. Expected one of: {}",
                value.as_str().unwrap_or(""),
                values.join(", ")
            )),
            (false, _) => Err(anyhow!("Expected {} value", type_name)),
        }
    }

    /// Get human-readable type name
    fn get_type_name(arg_type: &super::command_validator::ArgumentType) -> &'static str {
        use super::command_validator::ArgumentType;

        match arg_type {
            ArgumentType::String => "string",
            ArgumentType::Integer => "integer",
            ArgumentType::Boolean => "boolean",
            ArgumentType::Path => "path string",
            ArgumentType::Enum(_) => "string value for enum",
        }
    }

    /// Check if JSON value matches expected type
    fn check_value_type(
        value: &serde_json::Value,
        expected_type: &super::command_validator::ArgumentType,
    ) -> bool {
        use super::command_validator::ArgumentType;

        match expected_type {
            ArgumentType::String | ArgumentType::Path => value.is_string(),
            ArgumentType::Integer => value.is_number(),
            ArgumentType::Boolean => value.is_boolean(),
            ArgumentType::Enum(values) => value
                .as_str()
                .map(|s| values.contains(&s.to_string()))
                .unwrap_or(false),
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
    use crate::config::command_validator::{ArgumentType, CommandDefinition, OptionDef};
    use crate::config::{command, Command, CommandArg};
    use std::collections::HashMap;
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
        assert!(registry.get("prodigy-code-review").is_some());
        assert!(registry.get("prodigy-implement-spec").is_some());
    }

    #[tokio::test]
    async fn test_command_discovery_and_validation() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        // Create a test command file
        let command_content = r#"# /prodigy-custom-test

Custom test command.

## Variables

TARGET: $ARGUMENTS (required - target to test)

## Options

- `--verbose`: Enable verbose output
"#;

        fs::write(commands_dir.join("prodigy-custom-test.md"), command_content)
            .await
            .unwrap();

        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Should discover the custom command
        assert!(registry.get("prodigy-custom-test").is_some());

        // Test validation
        let mut cmd = Command::new("prodigy-custom-test");
        cmd.args.push(CommandArg::Literal("target.rs".to_string()));

        assert!(registry.validate_command(&cmd).is_ok());
    }

    #[tokio::test]
    async fn test_permissive_validation() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        // Create a minimal command file
        let command_content = r#"# /prodigy-minimal

A minimal command without metadata.
"#;

        fs::write(commands_dir.join("prodigy-minimal.md"), command_content)
            .await
            .unwrap();

        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Should allow any arguments/options for minimal commands
        let mut cmd = Command::new("prodigy-minimal");
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
            commands_dir.join("prodigy-custom.md"),
            "# /prodigy-custom\n\nCustom command.",
        )
        .await
        .unwrap();

        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();
        let commands = registry.list_commands();

        // Should include both discovered and built-in commands
        assert!(commands.contains(&"prodigy-custom".to_string()));
        assert!(commands.contains(&"prodigy-code-review".to_string()));
        assert!(commands.contains(&"prodigy-implement-spec".to_string()));
    }

    #[test]
    fn test_validate_json_value_type_string() {
        use super::super::command_validator::ArgumentType;

        // Valid string
        let value = serde_json::json!("test string");
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::String);
        assert!(result.is_ok());

        // Invalid - number instead of string
        let value = serde_json::json!(123);
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::String);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected string value"));

        // Invalid - boolean instead of string
        let value = serde_json::json!(true);
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::String);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_json_value_type_integer() {
        use super::super::command_validator::ArgumentType;

        // Valid integer
        let value = serde_json::json!(42);
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Integer);
        assert!(result.is_ok());

        // Valid float (JSON numbers include floats)
        let value = serde_json::json!(3.5);
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Integer);
        assert!(result.is_ok());

        // Invalid - string instead of number
        let value = serde_json::json!("42");
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Integer);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected integer value"));
    }

    #[test]
    fn test_validate_json_value_type_boolean() {
        use super::super::command_validator::ArgumentType;

        // Valid true
        let value = serde_json::json!(true);
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Boolean);
        assert!(result.is_ok());

        // Valid false
        let value = serde_json::json!(false);
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Boolean);
        assert!(result.is_ok());

        // Invalid - string instead of boolean
        let value = serde_json::json!("true");
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Boolean);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected boolean value"));

        // Invalid - number instead of boolean
        let value = serde_json::json!(1);
        let result =
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Boolean);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_json_value_type_path() {
        use super::super::command_validator::ArgumentType;

        // Valid path string
        let value = serde_json::json!("/path/to/file.txt");
        let result = DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Path);
        assert!(result.is_ok());

        // Valid relative path
        let value = serde_json::json!("./relative/path");
        let result = DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Path);
        assert!(result.is_ok());

        // Invalid - number instead of path string
        let value = serde_json::json!(123);
        let result = DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected path string value"));

        // Invalid - object instead of path string
        let value = serde_json::json!({"path": "/some/path"});
        let result = DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_json_value_type_enum() {
        use super::super::command_validator::ArgumentType;

        let valid_values = vec![
            "option1".to_string(),
            "option2".to_string(),
            "option3".to_string(),
        ];

        // Valid enum value
        let value = serde_json::json!("option1");
        let result = DynamicCommandRegistry::validate_json_value_type(
            &value,
            &ArgumentType::Enum(valid_values.clone()),
        );
        assert!(result.is_ok());

        // Valid enum value - different option
        let value = serde_json::json!("option3");
        let result = DynamicCommandRegistry::validate_json_value_type(
            &value,
            &ArgumentType::Enum(valid_values.clone()),
        );
        assert!(result.is_ok());

        // Invalid - value not in enum
        let value = serde_json::json!("invalid_option");
        let result = DynamicCommandRegistry::validate_json_value_type(
            &value,
            &ArgumentType::Enum(valid_values.clone()),
        );
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Invalid value 'invalid_option'"));
        assert!(error_msg.contains("Expected one of: option1, option2, option3"));

        // Invalid - non-string value for enum
        let value = serde_json::json!(123);
        let result = DynamicCommandRegistry::validate_json_value_type(
            &value,
            &ArgumentType::Enum(valid_values.clone()),
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Expected string value for enum"));

        // Invalid - boolean value for enum
        let value = serde_json::json!(true);
        let result = DynamicCommandRegistry::validate_json_value_type(
            &value,
            &ArgumentType::Enum(valid_values.clone()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_get_type_name() {
        use super::super::command_validator::ArgumentType;

        assert_eq!(
            DynamicCommandRegistry::get_type_name(&ArgumentType::String),
            "string"
        );
        assert_eq!(
            DynamicCommandRegistry::get_type_name(&ArgumentType::Integer),
            "integer"
        );
        assert_eq!(
            DynamicCommandRegistry::get_type_name(&ArgumentType::Boolean),
            "boolean"
        );
        assert_eq!(
            DynamicCommandRegistry::get_type_name(&ArgumentType::Path),
            "path string"
        );
        assert_eq!(
            DynamicCommandRegistry::get_type_name(&ArgumentType::Enum(vec![])),
            "string value for enum"
        );
    }

    #[test]
    fn test_check_value_type() {
        use super::super::command_validator::ArgumentType;

        // String type checks
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!("test"),
            &ArgumentType::String
        ));
        assert!(!DynamicCommandRegistry::check_value_type(
            &serde_json::json!(123),
            &ArgumentType::String
        ));

        // Integer type checks
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!(42),
            &ArgumentType::Integer
        ));
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!(3.5),
            &ArgumentType::Integer
        ));
        assert!(!DynamicCommandRegistry::check_value_type(
            &serde_json::json!("42"),
            &ArgumentType::Integer
        ));

        // Boolean type checks
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!(true),
            &ArgumentType::Boolean
        ));
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!(false),
            &ArgumentType::Boolean
        ));
        assert!(!DynamicCommandRegistry::check_value_type(
            &serde_json::json!(1),
            &ArgumentType::Boolean
        ));

        // Path type checks (same as string)
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!("/path"),
            &ArgumentType::Path
        ));
        assert!(!DynamicCommandRegistry::check_value_type(
            &serde_json::json!(null),
            &ArgumentType::Path
        ));

        // Enum type checks
        let enum_values = vec!["a".to_string(), "b".to_string()];
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!("a"),
            &ArgumentType::Enum(enum_values.clone())
        ));
        assert!(DynamicCommandRegistry::check_value_type(
            &serde_json::json!("b"),
            &ArgumentType::Enum(enum_values.clone())
        ));
        assert!(!DynamicCommandRegistry::check_value_type(
            &serde_json::json!("c"),
            &ArgumentType::Enum(enum_values.clone())
        ));
        assert!(!DynamicCommandRegistry::check_value_type(
            &serde_json::json!(123),
            &ArgumentType::Enum(enum_values.clone())
        ));
    }

    #[test]
    fn test_edge_cases_json_values() {
        use super::super::command_validator::ArgumentType;

        // Null value
        let value = serde_json::json!(null);
        assert!(
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::String)
                .is_err()
        );
        assert!(
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Integer)
                .is_err()
        );
        assert!(
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Boolean)
                .is_err()
        );

        // Array value
        let value = serde_json::json!([1, 2, 3]);
        assert!(
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::String)
                .is_err()
        );
        assert!(
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Integer)
                .is_err()
        );

        // Object value
        let value = serde_json::json!({"key": "value"});
        assert!(
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::String)
                .is_err()
        );
        assert!(
            DynamicCommandRegistry::validate_json_value_type(&value, &ArgumentType::Path).is_err()
        );

        // Empty string for enum
        let value = serde_json::json!("");
        let enum_values = vec!["a".to_string(), "b".to_string()];
        let result = DynamicCommandRegistry::validate_json_value_type(
            &value,
            &ArgumentType::Enum(enum_values),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid value ''"));
    }

    #[tokio::test]
    async fn test_apply_definition_defaults_all_metadata_unset() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();
        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Create a command with no metadata set
        let mut command = Command::new("test-command".to_string());
        assert!(command.metadata.retries.is_none());
        assert!(command.metadata.timeout.is_none());
        assert!(command.metadata.continue_on_error.is_none());

        // Create a definition with all defaults
        let definition = CommandDefinition {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![],
            defaults: command::CommandMetadata {
                retries: Some(3),
                timeout: Some(5000),
                continue_on_error: Some(true),
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        // Apply defaults
        registry.apply_definition_defaults(&mut command, &definition);

        // All metadata should be set from defaults
        assert_eq!(command.metadata.retries, Some(3));
        assert_eq!(command.metadata.timeout, Some(5000));
        assert_eq!(command.metadata.continue_on_error, Some(true));
    }

    #[tokio::test]
    async fn test_apply_definition_defaults_metadata_already_set() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();
        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Create a command with metadata already set
        let mut command = Command::new("test-command".to_string());
        command.metadata.retries = Some(5);
        command.metadata.timeout = Some(10000);
        command.metadata.continue_on_error = Some(false);

        // Create a definition with different defaults
        let definition = CommandDefinition {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![],
            defaults: command::CommandMetadata {
                retries: Some(3),
                timeout: Some(5000),
                continue_on_error: Some(true),
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        // Apply defaults
        registry.apply_definition_defaults(&mut command, &definition);

        // Existing values should not be overridden
        assert_eq!(command.metadata.retries, Some(5));
        assert_eq!(command.metadata.timeout, Some(10000));
        assert_eq!(command.metadata.continue_on_error, Some(false));
    }

    #[tokio::test]
    async fn test_apply_definition_defaults_partial_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();
        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Create a command with some metadata set
        let mut command = Command::new("test-command".to_string());
        command.metadata.retries = Some(2);
        // timeout and continue_on_error are None

        // Create a definition with all defaults
        let definition = CommandDefinition {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![],
            defaults: command::CommandMetadata {
                retries: Some(3),
                timeout: Some(5000),
                continue_on_error: Some(true),
                env: HashMap::new(),
                commit_required: false,
                analysis: None,
            },
        };

        // Apply defaults
        registry.apply_definition_defaults(&mut command, &definition);

        // Only unset values should be filled
        assert_eq!(command.metadata.retries, Some(2)); // kept existing
        assert_eq!(command.metadata.timeout, Some(5000)); // filled from default
        assert_eq!(command.metadata.continue_on_error, Some(true)); // filled from default
    }

    #[tokio::test]
    async fn test_apply_definition_defaults_options_with_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();
        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Create a command with no options
        let mut command = Command::new("test-command".to_string());
        assert!(command.options.is_empty());

        // Create a definition with options that have defaults
        let definition = CommandDefinition {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![
                OptionDef {
                    name: "verbose".to_string(),
                    description: "Verbose output".to_string(),
                    option_type: ArgumentType::Boolean,
                    default: Some(serde_json::json!(true)),
                },
                OptionDef {
                    name: "count".to_string(),
                    description: "Count value".to_string(),
                    option_type: ArgumentType::Integer,
                    default: Some(serde_json::json!(10)),
                },
            ],
            defaults: command::CommandMetadata::default(),
        };

        // Apply defaults
        registry.apply_definition_defaults(&mut command, &definition);

        // Options should be set from defaults
        assert_eq!(
            command.options.get("verbose"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(command.options.get("count"), Some(&serde_json::json!(10)));
    }

    #[tokio::test]
    async fn test_apply_definition_defaults_options_already_set() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();
        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Create a command with options already set
        let mut command = Command::new("test-command".to_string());
        command
            .options
            .insert("verbose".to_string(), serde_json::json!(false));
        command
            .options
            .insert("count".to_string(), serde_json::json!(5));

        // Create a definition with different defaults
        let definition = CommandDefinition {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![
                OptionDef {
                    name: "verbose".to_string(),
                    description: "Verbose output".to_string(),
                    option_type: ArgumentType::Boolean,
                    default: Some(serde_json::json!(true)),
                },
                OptionDef {
                    name: "count".to_string(),
                    description: "Count value".to_string(),
                    option_type: ArgumentType::Integer,
                    default: Some(serde_json::json!(10)),
                },
            ],
            defaults: command::CommandMetadata::default(),
        };

        // Apply defaults
        registry.apply_definition_defaults(&mut command, &definition);

        // Existing options should not be overridden
        assert_eq!(
            command.options.get("verbose"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(command.options.get("count"), Some(&serde_json::json!(5)));
    }

    #[tokio::test]
    async fn test_apply_definition_defaults_mixed_options() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();
        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Create a command with one option set
        let mut command = Command::new("test-command".to_string());
        command
            .options
            .insert("verbose".to_string(), serde_json::json!(false));

        // Create a definition with multiple options
        let definition = CommandDefinition {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![
                OptionDef {
                    name: "verbose".to_string(),
                    description: "Verbose output".to_string(),
                    option_type: ArgumentType::Boolean,
                    default: Some(serde_json::json!(true)),
                },
                OptionDef {
                    name: "count".to_string(),
                    description: "Count value".to_string(),
                    option_type: ArgumentType::Integer,
                    default: Some(serde_json::json!(10)),
                },
                OptionDef {
                    name: "mode".to_string(),
                    description: "Mode value".to_string(),
                    option_type: ArgumentType::String,
                    default: None, // No default
                },
            ],
            defaults: command::CommandMetadata::default(),
        };

        // Apply defaults
        registry.apply_definition_defaults(&mut command, &definition);

        // Check results
        assert_eq!(
            command.options.get("verbose"),
            Some(&serde_json::json!(false))
        ); // kept existing
        assert_eq!(command.options.get("count"), Some(&serde_json::json!(10))); // filled from default
        assert_eq!(command.options.get("mode"), None); // no default, not set
    }

    #[tokio::test]
    async fn test_apply_definition_defaults_no_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();
        let registry = DynamicCommandRegistry::new(Some(commands_dir))
            .await
            .unwrap();

        // Create a command with no values
        let mut command = Command::new("test-command".to_string());

        // Create a definition with no defaults
        let definition = CommandDefinition {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            required_args: vec![],
            optional_args: vec![],
            options: vec![OptionDef {
                name: "verbose".to_string(),
                description: "Verbose output".to_string(),
                option_type: ArgumentType::Boolean,
                default: None,
            }],
            defaults: command::CommandMetadata::default(), // All None
        };

        // Apply defaults
        registry.apply_definition_defaults(&mut command, &definition);

        // Nothing should be set
        assert!(command.metadata.retries.is_none());
        assert!(command.metadata.timeout.is_none());
        assert!(command.metadata.continue_on_error.is_none());
        assert!(command.options.is_empty());
    }
}
