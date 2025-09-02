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
        // Find lines after title until first empty line or next section
        content
            .lines()
            .map(str::trim)
            .skip_while(|line| !line.starts_with("# /"))
            .skip(1) // Skip the title line itself
            .skip_while(|line| line.is_empty()) // Skip empty lines after title
            .take_while(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join(" ")
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

    /// Classify argument type based on description keywords
    fn classify_option_type(description: &str) -> ArgumentType {
        match () {
            _ if description.contains("number") || description.contains("Maximum") => {
                ArgumentType::Integer
            }
            _ if description.contains("boolean")
                || description.contains("Enable")
                || description.contains("Disable") =>
            {
                ArgumentType::Boolean
            }
            _ => ArgumentType::String,
        }
    }

    /// Parse a single option line into an OptionDef
    fn parse_option_line(line: &str) -> Option<OptionDef> {
        if !line.starts_with("- `--") {
            return None;
        }

        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 2 {
            return None;
        }

        let name = parts[0]
            .trim_start_matches("- `--")
            .trim_end_matches('`')
            .to_string();
        let description = parts[1..].join(":").trim().to_string();
        let option_type = Self::classify_option_type(&description);

        Some(OptionDef {
            name,
            description,
            option_type,
            default: None,
        })
    }

    /// Parse option lines into option definitions
    fn parse_options(&self, options: &[String]) -> Result<Vec<OptionDef>> {
        Ok(options
            .iter()
            .filter_map(|line| Self::parse_option_line(line))
            .collect())
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

    #[test]
    fn test_extract_description_with_empty_lines() {
        let content = r#"# /test-command


Description after empty lines
should still be captured.

## Variables
"#;
        let parser = MetadataParser::new();
        let desc = parser.extract_description(content);
        assert_eq!(
            desc,
            "Description after empty lines should still be captured."
        );
    }

    #[test]
    fn test_extract_description_no_title() {
        let content = "Some content without a title";
        let parser = MetadataParser::new();
        let desc = parser.extract_description(content);
        assert_eq!(desc, "");
    }

    #[test]
    fn test_extract_description_stops_at_section() {
        let content = r#"# /command
First paragraph.
## Options
Should not be included"#;
        let parser = MetadataParser::new();
        let desc = parser.extract_description(content);
        assert_eq!(desc, "First paragraph.");
    }

    #[test]
    fn test_classify_option_type_integer() {
        assert_eq!(
            MetadataParser::classify_option_type("Maximum number of items"),
            ArgumentType::Integer
        );
        assert_eq!(
            MetadataParser::classify_option_type("The number to process"),
            ArgumentType::Integer
        );
    }

    #[test]
    fn test_classify_option_type_boolean() {
        assert_eq!(
            MetadataParser::classify_option_type("Enable debug mode"),
            ArgumentType::Boolean
        );
        assert_eq!(
            MetadataParser::classify_option_type("Disable caching"),
            ArgumentType::Boolean
        );
        assert_eq!(
            MetadataParser::classify_option_type("A boolean flag"),
            ArgumentType::Boolean
        );
    }

    #[test]
    fn test_classify_option_type_string() {
        assert_eq!(
            MetadataParser::classify_option_type("The output file path"),
            ArgumentType::String
        );
        assert_eq!(
            MetadataParser::classify_option_type("Name of the resource"),
            ArgumentType::String
        );
    }

    #[test]
    fn test_parse_option_line_valid() {
        let line = "- `--output`: The output file path";
        let option = MetadataParser::parse_option_line(line).unwrap();
        assert_eq!(option.name, "output");
        assert_eq!(option.description, "The output file path");
        assert_eq!(option.option_type, ArgumentType::String);
    }

    #[test]
    fn test_parse_option_line_with_colon_in_description() {
        let line = "- `--url`: The URL to fetch (e.g., https://example.com)";
        let option = MetadataParser::parse_option_line(line).unwrap();
        assert_eq!(option.name, "url");
        assert_eq!(
            option.description,
            "The URL to fetch (e.g., https://example.com)"
        );
    }

    #[test]
    fn test_parse_option_line_invalid_format() {
        assert!(MetadataParser::parse_option_line("Not an option line").is_none());
        assert!(MetadataParser::parse_option_line("- `--missing-colon`").is_none());
        assert!(MetadataParser::parse_option_line("- --no-backticks: Description").is_none());
    }

    #[test]
    fn test_parse_options_multiple() {
        let parser = MetadataParser::new();
        let options = vec![
            "- `--verbose`: Enable verbose output".to_string(),
            "- `--max-items`: Maximum number of items".to_string(),
            "Not a valid option line".to_string(),
            "- `--file`: Input file path".to_string(),
        ];

        let parsed = parser.parse_options(&options).unwrap();
        assert_eq!(parsed.len(), 3);

        assert_eq!(parsed[0].name, "verbose");
        assert_eq!(parsed[0].option_type, ArgumentType::Boolean);

        assert_eq!(parsed[1].name, "max-items");
        assert_eq!(parsed[1].option_type, ArgumentType::Integer);

        assert_eq!(parsed[2].name, "file");
        assert_eq!(parsed[2].option_type, ArgumentType::String);
    }

    #[test]
    fn test_parse_options_empty() {
        let parser = MetadataParser::new();
        let options = vec![];
        let parsed = parser.parse_options(&options).unwrap();
        assert!(parsed.is_empty());
    }
}
