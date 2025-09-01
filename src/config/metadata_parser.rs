use super::command::CommandMetadata;
use super::command_discovery::CommandFile;
use super::command_validator::{ArgumentDef, ArgumentType, CommandDefinition, OptionDef};
use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Parser for extracting command metadata from markdown files
///
/// Supports two parsing strategies:
/// 1. Frontmatter-based: YAML frontmatter with structured metadata
/// 2. Section-based: Traditional markdown sections with Variables and Options
pub struct MetadataParser {
    frontmatter_regex: Regex,
    variable_regex: Regex,
}

impl MetadataParser {
    pub fn new() -> Self {
        Self {
            frontmatter_regex: Regex::new(r"(?s)^---\n(.*?)\n---").unwrap(),
            variable_regex: Regex::new(r"^(\w+):\s*(.+)$").unwrap(),
        }
    }

    /// Parse a command file and extract its metadata
    ///
    /// Tries multiple parsing strategies in order:
    /// 1. Frontmatter parsing for structured metadata
    /// 2. Section-based parsing for existing format
    /// 3. Minimal definition as fallback
    pub fn parse_command_file(&self, file: &CommandFile) -> Result<CommandDefinition> {
        // Try frontmatter first
        if let Ok(definition) = self.parse_frontmatter(file) {
            return Ok(definition);
        }

        // Fall back to section parsing
        if let Ok(definition) = self.parse_sections(file) {
            return Ok(definition);
        }

        // Create minimal definition
        Ok(self.create_minimal_definition(file))
    }

    /// Parse YAML frontmatter from command file
    fn parse_frontmatter(&self, file: &CommandFile) -> Result<CommandDefinition> {
        let captures = self
            .frontmatter_regex
            .captures(&file.content)
            .ok_or_else(|| anyhow!("No frontmatter found"))?;

        let yaml_content = captures
            .get(1)
            .ok_or_else(|| anyhow!("Invalid frontmatter"))?
            .as_str();

        let metadata: FrontmatterMetadata = serde_yaml::from_str(yaml_content)
            .map_err(|e| anyhow!("Failed to parse frontmatter: {}", e))?;

        Ok(self.convert_frontmatter_to_definition(file, metadata))
    }

    /// Parse section-based command format
    fn parse_sections(&self, file: &CommandFile) -> Result<CommandDefinition> {
        let variables = self.extract_variables_section(&file.content)?;
        let options = self.extract_options_section(&file.content)?;

        Ok(CommandDefinition {
            name: file.name.clone(),
            description: self.extract_description(&file.content),
            required_args: self.parse_variables_to_args(&variables)?,
            optional_args: vec![],
            options: self.parse_options(&options)?,
            defaults: CommandMetadata::default(),
        })
    }

    /// Create a minimal command definition for unparseable files
    pub fn create_minimal_definition(&self, file: &CommandFile) -> CommandDefinition {
        CommandDefinition {
            name: file.name.clone(),
            description: self.extract_description(&file.content),
            required_args: vec![],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata::default(),
        }
    }

    /// Extract command description from markdown content
    fn extract_description(&self, content: &str) -> String {
        // Extract first paragraph after the title
        let lines: Vec<&str> = content.lines().collect();
        let mut found_title = false;
        let mut description_lines = Vec::new();

        for line in lines {
            let trimmed = line.trim();

            if trimmed.starts_with("# /") {
                found_title = true;
                continue;
            }

            if found_title {
                if trimmed.is_empty() {
                    if !description_lines.is_empty() {
                        break;
                    }
                    continue;
                }

                if trimmed.starts_with('#') {
                    break;
                }

                description_lines.push(trimmed);
            }
        }

        description_lines.join(" ")
    }

    /// Extract Variables section from markdown
    fn extract_variables_section(&self, content: &str) -> Result<Vec<String>> {
        self.extract_section(content, "## Variables")
    }

    /// Extract Options section from markdown
    fn extract_options_section(&self, content: &str) -> Result<Vec<String>> {
        self.extract_section(content, "## Options")
    }

    /// Generic section extraction helper
    fn extract_section(&self, content: &str, section_header: &str) -> Result<Vec<String>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_section = false;
        let mut section_lines = Vec::new();

        for line in lines {
            let trimmed = line.trim();

            if trimmed == section_header {
                in_section = true;
                continue;
            }

            if in_section {
                if trimmed.starts_with("##") {
                    break;
                }

                if !trimmed.is_empty() {
                    section_lines.push(trimmed.to_string());
                }
            }
        }

        Ok(section_lines)
    }

    /// Parse variable lines into argument definitions
    fn parse_variables_to_args(&self, variables: &[String]) -> Result<Vec<ArgumentDef>> {
        let mut args = Vec::new();

        for var_line in variables {
            if let Some(captures) = self.variable_regex.captures(var_line) {
                let name = captures.get(1).unwrap().as_str();
                let spec = captures.get(2).unwrap().as_str();

                // Skip environment variables and optional arguments
                if spec.contains("$PRODIGY_")
                    || spec.contains("Environment variable")
                    || spec.contains("optional")
                {
                    continue;
                }

                // Check if it's a required argument
                if spec.contains("$ARGUMENTS") {
                    args.push(ArgumentDef {
                        name: name.to_lowercase(),
                        description: spec.to_string(),
                        arg_type: ArgumentType::String,
                    });
                }
            }
        }

        Ok(args)
    }

    /// Parse option lines into option definitions
    fn parse_options(&self, options: &[String]) -> Result<Vec<OptionDef>> {
        let mut opts = Vec::new();

        for opt_line in options {
            if opt_line.starts_with("- `--") {
                let parts: Vec<&str> = opt_line.split(':').collect();
                if parts.len() >= 2 {
                    let name_part = parts[0].trim_start_matches("- `--").trim_end_matches('`');
                    let desc = parts[1..].join(":").trim().to_string();

                    // Determine option type from description
                    let option_type = if desc.contains("number") || desc.contains("Maximum") {
                        ArgumentType::Integer
                    } else if desc.contains("boolean")
                        || desc.contains("Enable")
                        || desc.contains("Disable")
                    {
                        ArgumentType::Boolean
                    } else {
                        ArgumentType::String
                    };

                    opts.push(OptionDef {
                        name: name_part.to_string(),
                        description: desc,
                        option_type,
                        default: None,
                    });
                }
            }
        }

        Ok(opts)
    }

    /// Convert frontmatter metadata to command definition
    fn convert_frontmatter_to_definition(
        &self,
        file: &CommandFile,
        metadata: FrontmatterMetadata,
    ) -> CommandDefinition {
        CommandDefinition {
            name: metadata.name.unwrap_or_else(|| file.name.clone()),
            description: metadata
                .description
                .unwrap_or_else(|| self.extract_description(&file.content)),
            required_args: metadata
                .arguments
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|arg| arg.required.unwrap_or(false))
                .map(std::convert::Into::into)
                .collect(),
            optional_args: metadata
                .arguments
                .unwrap_or_default()
                .into_iter()
                .filter(|arg| !arg.required.unwrap_or(false))
                .map(std::convert::Into::into)
                .collect(),
            options: metadata
                .options
                .unwrap_or_default()
                .into_iter()
                .map(std::convert::Into::into)
                .collect(),
            defaults: metadata.metadata.unwrap_or_default(),
        }
    }
}

impl Default for MetadataParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Frontmatter metadata structure
#[derive(Debug, Deserialize, Serialize)]
struct FrontmatterMetadata {
    name: Option<String>,
    description: Option<String>,
    arguments: Option<Vec<FrontmatterArgument>>,
    options: Option<Vec<FrontmatterOption>>,
    metadata: Option<CommandMetadata>,
}

/// Frontmatter argument definition
#[derive(Debug, Deserialize, Serialize, Clone)]
struct FrontmatterArgument {
    name: String,
    #[serde(rename = "type")]
    arg_type: Option<String>,
    required: Option<bool>,
    description: Option<String>,
}

/// Frontmatter option definition
#[derive(Debug, Deserialize, Serialize, Clone)]
struct FrontmatterOption {
    name: String,
    #[serde(rename = "type")]
    option_type: Option<String>,
    default: Option<serde_json::Value>,
    description: Option<String>,
}

impl From<FrontmatterArgument> for ArgumentDef {
    fn from(arg: FrontmatterArgument) -> Self {
        Self {
            name: arg.name,
            description: arg.description.unwrap_or_default(),
            arg_type: match arg.arg_type.as_deref() {
                Some("integer") => ArgumentType::Integer,
                Some("boolean") => ArgumentType::Boolean,
                Some("path") => ArgumentType::Path,
                _ => ArgumentType::String,
            },
        }
    }
}

impl From<FrontmatterOption> for OptionDef {
    fn from(opt: FrontmatterOption) -> Self {
        Self {
            name: opt.name,
            description: opt.description.unwrap_or_default(),
            option_type: match opt.option_type.as_deref() {
                Some("integer") => ArgumentType::Integer,
                Some("boolean") => ArgumentType::Boolean,
                Some("path") => ArgumentType::Path,
                _ => ArgumentType::String,
            },
            default: opt.default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_file(name: &str, content: &str) -> CommandFile {
        CommandFile {
            path: PathBuf::from(format!("{name}.md")),
            name: name.to_string(),
            content: content.to_string(),
            modified: SystemTime::now(),
        }
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: prodigy-test-command
description: "Test command for unit tests"
arguments:
  - name: target
    type: string
    required: true
    description: "Target file"
options:
  - name: verbose
    type: boolean
    default: false
    description: "Enable verbose output"
metadata:
  retries: 3
  timeout: 300
---

# /prodigy-test-command

This is the command documentation.
"#;

        let file = create_test_file("prodigy-test-command", content);
        let parser = MetadataParser::new();
        let definition = parser.parse_command_file(&file).unwrap();

        assert_eq!(definition.name, "prodigy-test-command");
        assert_eq!(definition.description, "Test command for unit tests");
        assert_eq!(definition.required_args.len(), 1);
        assert_eq!(definition.required_args[0].name, "target");
        assert_eq!(definition.options.len(), 1);
        assert_eq!(definition.options[0].name, "verbose");
        assert_eq!(definition.defaults.retries, Some(3));
        assert_eq!(definition.defaults.timeout, Some(300));
    }

    #[test]
    fn test_parse_sections() {
        let content = r#"# /prodigy-code-review

Analyze code and generate improvement specs.

## Variables

SCOPE: $ARGUMENTS (optional - specify scope)
FOCUS: $PRODIGY_FOCUS (optional - focus directive)

## Options

- `--max-issues`: Maximum number of issues (default: 10)
- `--severity`: Minimum severity level

## Execute

Command implementation details...
"#;

        let file = create_test_file("prodigy-code-review", content);
        let parser = MetadataParser::new();
        let definition = parser.parse_command_file(&file).unwrap();

        assert_eq!(definition.name, "prodigy-code-review");
        assert_eq!(
            definition.description,
            "Analyze code and generate improvement specs."
        );
        assert_eq!(definition.required_args.len(), 0); // SCOPE is optional
        assert_eq!(definition.options.len(), 2);
        assert_eq!(definition.options[0].name, "max-issues");
        assert_eq!(definition.options[1].name, "severity");
    }

    #[test]
    fn test_minimal_definition() {
        let content = r#"# /prodigy-simple

A simple command without metadata sections.

Just some documentation here.
"#;

        let file = create_test_file("prodigy-simple", content);
        let parser = MetadataParser::new();
        let definition = parser.parse_command_file(&file).unwrap();

        assert_eq!(definition.name, "prodigy-simple");
        assert_eq!(
            definition.description,
            "A simple command without metadata sections."
        );
        assert_eq!(definition.required_args.len(), 0);
        assert_eq!(definition.options.len(), 0);
    }

    #[test]
    fn test_extract_description_multiline() {
        let content = r#"# /prodigy-test

This is a longer description
that spans multiple lines
and should be joined together.

## Next Section
"#;

        let parser = MetadataParser::new();
        let desc = parser.extract_description(content);

        assert_eq!(
            desc,
            "This is a longer description that spans multiple lines and should be joined together."
        );
    }
}
