//! Custom Claude commands framework

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Command configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub template: String,
    pub task_type: String,
    pub pre_processors: Vec<String>,
    pub post_processors: Vec<String>,
    pub settings: CommandSettings,
    pub interactive: bool,
}

/// Command-specific settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandSettings {
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
    pub model_override: Option<String>,
}

/// Registry for Claude commands
pub struct CommandRegistry {
    commands: HashMap<String, CommandConfig>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    /// Create a new command registry
    pub fn new() -> Result<Self> {
        let mut registry = Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        };

        // Load built-in commands
        registry.load_builtin_commands()?;

        // Load custom commands from config
        if let Ok(custom_path) = std::env::var("MMM_CLAUDE_COMMANDS") {
            registry.load_commands_from_file(&PathBuf::from(custom_path))?;
        } else if PathBuf::from(".mmm/commands.toml").exists() {
            registry.load_commands_from_file(&PathBuf::from(".mmm/commands.toml"))?;
        }

        Ok(registry)
    }

    /// Load built-in commands
    fn load_builtin_commands(&mut self) -> Result<()> {
        // Implement command
        self.register_command(CommandConfig {
            name: "implement".to_string(),
            aliases: vec!["impl".to_string(), "i".to_string()],
            description: "Implement a feature based on specification".to_string(),
            template: "implement-feature".to_string(),
            task_type: "implementation".to_string(),
            pre_processors: vec![
                "gather-context".to_string(),
                "analyze-dependencies".to_string(),
            ],
            post_processors: vec!["extract-code".to_string(), "update-state".to_string()],
            settings: CommandSettings {
                temperature: Some(0.7),
                max_tokens: Some(4096),
                model_override: None,
            },
            interactive: false,
        })?;

        // Review command
        self.register_command(CommandConfig {
            name: "review".to_string(),
            aliases: vec!["r".to_string()],
            description: "Review code for quality and correctness".to_string(),
            template: "code-review".to_string(),
            task_type: "review".to_string(),
            pre_processors: vec!["gather-code".to_string()],
            post_processors: vec!["format-review".to_string()],
            settings: CommandSettings {
                temperature: Some(0.3),
                max_tokens: Some(2000),
                model_override: None,
            },
            interactive: false,
        })?;

        // Debug command
        self.register_command(CommandConfig {
            name: "debug".to_string(),
            aliases: vec!["d".to_string()],
            description: "Debug an issue or error".to_string(),
            template: "debug-issue".to_string(),
            task_type: "debug".to_string(),
            pre_processors: vec!["gather-error-context".to_string()],
            post_processors: vec!["extract-solution".to_string()],
            settings: CommandSettings {
                temperature: Some(0.5),
                max_tokens: Some(3000),
                model_override: Some("claude-3-opus".to_string()),
            },
            interactive: true,
        })?;

        // Plan command
        self.register_command(CommandConfig {
            name: "plan".to_string(),
            aliases: vec!["p".to_string()],
            description: "Plan implementation approach".to_string(),
            template: "plan-implementation".to_string(),
            task_type: "planning".to_string(),
            pre_processors: vec!["analyze-requirements".to_string()],
            post_processors: vec!["format-plan".to_string()],
            settings: CommandSettings {
                temperature: Some(0.8),
                max_tokens: Some(2000),
                model_override: Some("claude-3-opus".to_string()),
            },
            interactive: false,
        })?;

        // Explain command
        self.register_command(CommandConfig {
            name: "explain".to_string(),
            aliases: vec!["e".to_string()],
            description: "Explain code or concept".to_string(),
            template: "explain-code".to_string(),
            task_type: "explanation".to_string(),
            pre_processors: vec!["gather-context".to_string()],
            post_processors: vec!["format-explanation".to_string()],
            settings: CommandSettings {
                temperature: Some(0.4),
                max_tokens: Some(2000),
                model_override: None,
            },
            interactive: false,
        })?;

        Ok(())
    }

    /// Register a command
    pub fn register_command(&mut self, config: CommandConfig) -> Result<()> {
        // Check for conflicts
        if self.commands.contains_key(&config.name) {
            return Err(Error::Config(format!(
                "Command '{}' already exists",
                config.name
            )));
        }

        // Register aliases
        for alias in &config.aliases {
            if self.aliases.contains_key(alias) {
                return Err(Error::Config(format!("Alias '{alias}' already in use")));
            }
            self.aliases.insert(alias.clone(), config.name.clone());
        }

        self.commands.insert(config.name.clone(), config);
        Ok(())
    }

    /// Get a command by name or alias
    pub fn get_command(&self, name_or_alias: &str) -> Result<&CommandConfig> {
        // Check if it's an alias
        let name = self
            .aliases
            .get(name_or_alias)
            .map(|s| s.as_str())
            .unwrap_or(name_or_alias);

        self.commands
            .get(name)
            .ok_or_else(|| Error::NotFound(format!("Command '{name_or_alias}' not found")))
    }

    /// List all available commands
    pub fn list_commands(&self) -> Vec<&CommandConfig> {
        let mut commands: Vec<_> = self.commands.values().collect();
        commands.sort_by_key(|c| &c.name);
        commands
    }

    /// Load commands from a TOML file
    pub fn load_commands_from_file(&mut self, path: &PathBuf) -> Result<()> {
        let content = fs::read_to_string(path).map_err(Error::Io)?;

        let file_config: CommandsFile = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Invalid commands TOML: {e}")))?;

        for command in file_config.commands {
            self.register_command(command)?;
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct CommandsFile {
    commands: Vec<CommandConfig>,
}

/// Pre-processor trait for commands
pub trait PreProcessor: Send + Sync {
    fn process(&self, context: &mut HashMap<String, String>) -> Result<()>;
}

/// Post-processor trait for commands
pub trait PostProcessor: Send + Sync {
    fn process(&self, response: &str, context: &HashMap<String, String>) -> Result<String>;
}

/// Built-in pre-processors
pub struct GatherContextProcessor;

impl PreProcessor for GatherContextProcessor {
    fn process(&self, context: &mut HashMap<String, String>) -> Result<()> {
        // Gather relevant context files
        // This would integrate with the project and spec systems
        context.insert(
            "project_context".to_string(),
            "TODO: Gather project context".to_string(),
        );
        Ok(())
    }
}

/// Built-in post-processors
pub struct ExtractCodeProcessor;

impl PostProcessor for ExtractCodeProcessor {
    fn process(&self, response: &str, _context: &HashMap<String, String>) -> Result<String> {
        // Extract code blocks from response
        // This would use the response parser
        Ok(response.to_string())
    }
}
