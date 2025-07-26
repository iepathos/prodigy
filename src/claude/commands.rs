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
    pub structured_output: bool,
    pub output_schema: Option<String>,
    pub automation_friendly: bool,
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
        // Load Claude CLI commands if available
        self.load_claude_cli_commands()?;

        // Implement command (fallback if Claude CLI not available)
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
                structured_output: false,
                output_schema: None,
                automation_friendly: false,
            },
            interactive: false,
        })?;

        // Code Review command (fallback if Claude CLI not available)
        self.register_command(CommandConfig {
            name: "mmm-code-review".to_string(),
            aliases: vec!["mmm-r".to_string()],
            description: "Review code for quality and correctness".to_string(),
            template: "code-review".to_string(),
            task_type: "review".to_string(),
            pre_processors: vec!["gather-code".to_string()],
            post_processors: vec!["format-review".to_string()],
            settings: CommandSettings {
                temperature: Some(0.3),
                max_tokens: Some(2000),
                model_override: None,
                structured_output: true,
                output_schema: Some("mmm-code-review-schema".to_string()),
                automation_friendly: true,
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
                structured_output: false,
                output_schema: None,
                automation_friendly: false,
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
                structured_output: false,
                output_schema: None,
                automation_friendly: false,
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
                structured_output: false,
                output_schema: None,
                automation_friendly: false,
            },
            interactive: false,
        })?;

        Ok(())
    }

    /// Load Claude CLI commands from .claude/commands directory
    fn load_claude_cli_commands(&mut self) -> Result<()> {
        let claude_dir = PathBuf::from(".claude/commands");

        if !claude_dir.exists() {
            return Ok(()); // No Claude CLI commands available
        }

        let commands = [
            (
                "mmm-lint",
                "Automatically detect and fix linting issues in the codebase",
            ),
            (
                "mmm-code-review",
                "Conduct comprehensive code review with quality analysis",
            ),
            (
                "mmm-implement-spec",
                "Implement specifications by reading spec files and executing implementation",
            ),
            (
                "mmm-add-spec",
                "Generate new specification documents from feature descriptions",
            ),
        ];

        for (cmd_name, description) in commands {
            let cmd_file = claude_dir.join(format!("{cmd_name}.md"));

            if cmd_file.exists() {
                self.register_command(CommandConfig {
                    name: format!("claude-{cmd_name}"),
                    aliases: vec![cmd_name.to_string()],
                    description: description.to_string(),
                    template: format!("claude-cli-{cmd_name}"),
                    task_type: "claude-cli".to_string(),
                    pre_processors: vec!["prepare-claude-context".to_string()],
                    post_processors: vec!["process-claude-output".to_string()],
                    settings: CommandSettings {
                        temperature: Some(0.7),
                        max_tokens: Some(8000),
                        model_override: None,
                        structured_output: false,
                        output_schema: None,
                        automation_friendly: false,
                    },
                    interactive: true,
                })?;
            }
        }

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

/// Structured output wrapper for automated processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredCommandOutput {
    pub command: String,
    pub execution_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success: bool,
    pub data: serde_json::Value,
    pub metadata: CommandMetadata,
}

/// Command metadata for context and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub duration: std::time::Duration,
    pub token_usage: Option<TokenUsage>,
    pub model_used: String,
    pub context_size: usize,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

impl CommandRegistry {
    /// Execute command with structured output for automation
    pub async fn execute_structured(
        &self,
        command_name: &str,
        args: Vec<String>,
        context: &crate::workflow::WorkflowContext,
    ) -> Result<StructuredCommandOutput> {
        let config = self.get_command(command_name)?;

        if !config.settings.automation_friendly {
            return Err(Error::Config(format!(
                "Command '{command_name}' is not configured for automation"
            )));
        }

        // Execute command with enhanced context
        let start_time = chrono::Utc::now();
        let execution_id = uuid::Uuid::new_v4().to_string();

        let result = self
            .execute_command_internal(
                command_name,
                args,
                Some(context),
                config.settings.structured_output,
            )
            .await;

        let duration = chrono::Utc::now().signed_duration_since(start_time);

        match result {
            Ok(output) => {
                let data = if config.settings.structured_output {
                    self.parse_structured_output(&output, config.settings.output_schema.as_deref())?
                } else {
                    serde_json::Value::String(output)
                };

                Ok(StructuredCommandOutput {
                    command: command_name.to_string(),
                    execution_id,
                    timestamp: start_time,
                    success: true,
                    data,
                    metadata: CommandMetadata {
                        duration: duration.to_std().unwrap_or_default(),
                        token_usage: None, // TODO: Extract from response
                        model_used: config
                            .settings
                            .model_override
                            .clone()
                            .unwrap_or_else(|| "default".to_string()),
                        context_size: 0, // TODO: Calculate
                    },
                })
            }
            Err(e) => Ok(StructuredCommandOutput {
                command: command_name.to_string(),
                execution_id,
                timestamp: start_time,
                success: false,
                data: serde_json::json!({
                    "error": e.to_string(),
                    "error_type": "execution_failed"
                }),
                metadata: CommandMetadata {
                    duration: duration.to_std().unwrap_or_default(),
                    token_usage: None,
                    model_used: "unknown".to_string(),
                    context_size: 0,
                },
            }),
        }
    }

    /// Internal command execution method (placeholder)
    async fn execute_command_internal(
        &self,
        command_name: &str,
        _args: Vec<String>,
        _context: Option<&crate::workflow::WorkflowContext>,
        _structured: bool,
    ) -> Result<String> {
        // This is a placeholder for the actual command execution
        // In the full implementation, this would integrate with the Claude API
        tracing::info!("Executing command: {}", command_name);
        Ok(format!("Mock output for command: {command_name}"))
    }

    /// Parse structured output from command response
    fn parse_structured_output(
        &self,
        output: &str,
        schema: Option<&str>,
    ) -> Result<serde_json::Value> {
        // Look for structured output marker
        if let Some(start) = output.find("```json") {
            if let Some(end) = output[start..].find("```") {
                let json_str = &output[start + 7..start + end];
                return serde_json::from_str(json_str)
                    .map_err(|e| Error::Parse(format!("Invalid JSON in structured output: {e}")));
            }
        }

        // Look for mmm_structured_output marker
        if let Some(marker_start) = output.find("\"mmm_structured_output\"") {
            // Find the JSON object containing the marker
            let mut brace_count = 0;
            let mut start_pos = None;
            let mut end_pos = None;

            for (i, ch) in output[..marker_start].char_indices().rev() {
                if ch == '}' {
                    brace_count += 1;
                } else if ch == '{' {
                    brace_count -= 1;
                    if brace_count == 0 {
                        start_pos = Some(i);
                        break;
                    }
                }
            }

            if let Some(start) = start_pos {
                brace_count = 0;
                for (i, ch) in output[start..].char_indices() {
                    if ch == '{' {
                        brace_count += 1;
                    } else if ch == '}' {
                        brace_count -= 1;
                        if brace_count == 0 {
                            end_pos = Some(start + i + 1);
                            break;
                        }
                    }
                }

                if let Some(end) = end_pos {
                    let json_str = &output[start..end];
                    return serde_json::from_str(json_str).map_err(|e| {
                        Error::Parse(format!("Invalid JSON in structured output: {e}"))
                    });
                }
            }
        }

        // Fallback: validate against schema if provided
        if let Some(_schema) = schema {
            // TODO: Implement schema validation
        }

        // Default: return as string value
        Ok(serde_json::Value::String(output.to_string()))
    }
}
