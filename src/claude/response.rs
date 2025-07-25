//! Response processing with parsers and validators

use crate::error::{Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Parsed response content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedResponse {
    pub content: String,
    pub code_blocks: Vec<CodeBlock>,
    pub commands: Vec<MmmCommand>,
    pub metadata: ResponseMetadata,
}

/// Code block extracted from response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub language: String,
    pub content: String,
    pub file_path: Option<String>,
    pub line_range: Option<(usize, usize)>,
}

/// MMM command extracted from response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmmCommand {
    pub command: String,
    pub args: Vec<String>,
}

/// Response metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub has_code: bool,
    pub has_commands: bool,
    pub needs_review: bool,
    pub is_complete: bool,
    pub confidence: f32,
}

/// Trait for response parsers
pub trait ResponseParser: Send + Sync {
    fn can_parse(&self, response: &str) -> bool;
    fn parse(&self, response: &str) -> Result<ParsedResponse>;
}

/// Trait for response validators
pub trait ResponseValidator: Send + Sync {
    fn validate(&self, response: &ParsedResponse) -> Result<ValidationResult>;
}

/// Validation result
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Response processor with multiple parsers and validators
pub struct ResponseProcessor {
    parsers: Vec<Box<dyn ResponseParser>>,
    validators: Vec<Box<dyn ResponseValidator>>,
}

impl Default for ResponseProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseProcessor {
    /// Create a new response processor
    pub fn new() -> Self {
        let mut processor = Self {
            parsers: Vec::new(),
            validators: Vec::new(),
        };

        // Add default parsers
        processor.add_parser(Box::new(CodeBlockParser::new()));
        processor.add_parser(Box::new(CommandParser::new()));
        processor.add_parser(Box::new(MetadataParser::new()));

        // Add default validators
        processor.add_validator(Box::new(CodeValidator::new()));
        processor.add_validator(Box::new(CompletionValidator::new()));

        processor
    }

    /// Add a parser
    pub fn add_parser(&mut self, parser: Box<dyn ResponseParser>) {
        self.parsers.push(parser);
    }

    /// Add a validator
    pub fn add_validator(&mut self, validator: Box<dyn ResponseValidator>) {
        self.validators.push(validator);
    }

    /// Process a response
    pub fn process(&self, response: &str) -> Result<ParsedResponse> {
        // Find suitable parser
        let parser = self
            .parsers
            .iter()
            .find(|p| p.can_parse(response))
            .ok_or_else(|| Error::Parse("No suitable parser found".to_string()))?;

        // Parse response
        let mut parsed = parser.parse(response)?;

        // Run validators
        for validator in &self.validators {
            let result = validator.validate(&parsed)?;
            if !result.is_valid {
                return Err(Error::Validation(format!(
                    "Validation failed: {}",
                    result.errors.join(", ")
                )));
            }
            // Add warnings to metadata
            if !result.warnings.is_empty() {
                parsed.metadata.needs_review = true;
            }
        }

        Ok(parsed)
    }
}

/// Parser for code blocks
struct CodeBlockParser {
    code_block_regex: Regex,
    file_path_regex: Regex,
}

impl CodeBlockParser {
    fn new() -> Self {
        Self {
            code_block_regex: Regex::new(r"```(\w+)?\n([\s\S]*?)```").unwrap(),
            file_path_regex: Regex::new(r"#\s*(?:File:|file:)\s*([^\n]+)").unwrap(),
        }
    }
}

impl ResponseParser for CodeBlockParser {
    fn can_parse(&self, response: &str) -> bool {
        response.contains("```")
    }

    fn parse(&self, response: &str) -> Result<ParsedResponse> {
        let mut code_blocks = Vec::new();
        let mut file_path = None;

        // Check for file path comment
        if let Some(cap) = self.file_path_regex.captures(response) {
            file_path = Some(cap[1].trim().to_string());
        }

        // Extract code blocks
        for cap in self.code_block_regex.captures_iter(response) {
            let language = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let content = cap[2].to_string();

            code_blocks.push(CodeBlock {
                language,
                content,
                file_path: file_path.clone(),
                line_range: None,
            });
        }

        Ok(ParsedResponse {
            content: response.to_string(),
            code_blocks: code_blocks.clone(),
            commands: Vec::new(),
            metadata: ResponseMetadata {
                has_code: !code_blocks.is_empty(),
                has_commands: false,
                needs_review: false,
                is_complete: true,
                confidence: 0.8,
            },
        })
    }
}

/// Parser for MMM commands
struct CommandParser {
    command_regex: Regex,
}

impl CommandParser {
    fn new() -> Self {
        Self {
            command_regex: Regex::new(r"@mmm:(\w+)(?:\((.*?)\))?").unwrap(),
        }
    }
}

impl ResponseParser for CommandParser {
    fn can_parse(&self, response: &str) -> bool {
        response.contains("@mmm:")
    }

    fn parse(&self, response: &str) -> Result<ParsedResponse> {
        let mut commands = Vec::new();

        for cap in self.command_regex.captures_iter(response) {
            let command = cap[1].to_string();
            let args = cap
                .get(2)
                .map(|m| {
                    m.as_str()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect()
                })
                .unwrap_or_default();

            commands.push(MmmCommand { command, args });
        }

        // Parse code blocks too
        let code_parser = CodeBlockParser::new();
        let mut parsed = code_parser.parse(response)?;
        parsed.commands = commands;
        parsed.metadata.has_commands = !parsed.commands.is_empty();

        Ok(parsed)
    }
}

/// Parser for metadata indicators
struct MetadataParser;

impl MetadataParser {
    fn new() -> Self {
        Self
    }
}

impl ResponseParser for MetadataParser {
    fn can_parse(&self, _response: &str) -> bool {
        true // Always parse for metadata
    }

    fn parse(&self, response: &str) -> Result<ParsedResponse> {
        let mut metadata = ResponseMetadata {
            has_code: response.contains("```"),
            has_commands: response.contains("@mmm:"),
            needs_review: false,
            is_complete: true,
            confidence: 0.8,
        };

        // Check for review indicators
        if response.contains("TODO")
            || response.contains("FIXME")
            || response.contains("needs review")
        {
            metadata.needs_review = true;
        }

        // Check for completion indicators
        if response.contains("incomplete")
            || response.contains("partial")
            || response.contains("...")
        {
            metadata.is_complete = false;
            metadata.confidence = 0.5;
        }

        // Check for high confidence indicators
        if response.contains("tested")
            || response.contains("verified")
            || response.contains("complete")
        {
            metadata.confidence = 0.95;
        }

        // Use code parser as base
        let code_parser = CodeBlockParser::new();
        let mut parsed = code_parser.parse(response)?;
        parsed.metadata = metadata;

        Ok(parsed)
    }
}

/// Validator for code blocks
struct CodeValidator;

impl CodeValidator {
    fn new() -> Self {
        Self
    }
}

impl ResponseValidator for CodeValidator {
    fn validate(&self, response: &ParsedResponse) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for block in &response.code_blocks {
            // Check for empty code blocks
            if block.content.trim().is_empty() {
                errors.push("Empty code block found".to_string());
            }

            // Check for syntax indicators
            if block.content.contains("// TODO") || block.content.contains("# TODO") {
                warnings.push("Code contains TODO markers".to_string());
            }

            // Language-specific checks could go here
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }
}

/// Validator for completion status
struct CompletionValidator;

impl CompletionValidator {
    fn new() -> Self {
        Self
    }
}

impl ResponseValidator for CompletionValidator {
    fn validate(&self, response: &ParsedResponse) -> Result<ValidationResult> {
        let mut warnings = Vec::new();

        if !response.metadata.is_complete {
            warnings.push("Response appears to be incomplete".to_string());
        }

        if response.metadata.confidence < 0.7 {
            warnings.push("Low confidence response".to_string());
        }

        Ok(ValidationResult {
            is_valid: true, // Warnings don't invalidate
            errors: Vec::new(),
            warnings,
        })
    }
}
