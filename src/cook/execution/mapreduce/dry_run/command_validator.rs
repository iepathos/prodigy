//! Command validation for dry-run mode
//!
//! Validates workflow commands without executing them.

use super::types::{
    CommandType, CommandValidation, ValidationIssue, VariableContext, VariableReference,
};
use crate::cook::workflow::WorkflowStep;
use regex::Regex;
use std::time::Duration;
use tracing::debug;

/// Validator for workflow commands
pub struct CommandValidator {
    variable_regex: Regex,
}

impl CommandValidator {
    /// Create a new command validator
    pub fn new() -> Self {
        Self {
            variable_regex: Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex pattern"),
        }
    }

    /// Validate a single command
    pub fn validate_command(&self, command: &WorkflowStep) -> CommandValidation {
        debug!("Validating command: {:?}", command);

        let command_type = self.get_command_type(command);
        let mut issues = Vec::new();
        let mut valid = true;

        // Validate command structure
        self.validate_command_structure(command, &mut issues, &mut valid);

        // Extract and validate variable references
        let variable_references = self.extract_variables(command);
        self.validate_variable_references(&variable_references, &mut issues);

        // Validate command-specific syntax
        self.validate_command_syntax(command, &mut issues, &mut valid);

        // Estimate execution duration
        let estimated_duration = self.estimate_duration(command);

        CommandValidation {
            command_type,
            valid,
            issues,
            variable_references,
            estimated_duration,
        }
    }

    /// Validate multiple commands
    pub fn validate_commands(&self, commands: &[WorkflowStep]) -> Vec<CommandValidation> {
        commands
            .iter()
            .map(|cmd| self.validate_command(cmd))
            .collect()
    }

    /// Get command type from workflow step
    fn get_command_type(&self, command: &WorkflowStep) -> CommandType {
        if command.claude.is_some() {
            CommandType::Claude
        } else if command.shell.is_some() {
            CommandType::Shell
        } else if command.goal_seek.is_some() {
            CommandType::GoalSeek
        } else if command.foreach.is_some() {
            CommandType::Foreach
        } else {
            CommandType::Shell // Default fallback
        }
    }

    /// Validate command structure
    fn validate_command_structure(
        &self,
        command: &WorkflowStep,
        issues: &mut Vec<ValidationIssue>,
        valid: &mut bool,
    ) {
        let mut command_count = 0;

        if command.claude.is_some() {
            command_count += 1;
            if let Some(cmd) = &command.claude {
                if !cmd.starts_with('/') && !cmd.is_empty() {
                    issues.push(ValidationIssue::Warning(
                        "Claude command should start with '/' for slash commands".to_string(),
                    ));
                }
                if cmd.trim().is_empty() {
                    issues.push(ValidationIssue::Error(
                        "Claude command cannot be empty".to_string(),
                    ));
                    *valid = false;
                }
            }
        }

        if command.shell.is_some() {
            command_count += 1;
            if let Some(cmd) = &command.shell {
                if cmd.trim().is_empty() {
                    issues.push(ValidationIssue::Error(
                        "Shell command cannot be empty".to_string(),
                    ));
                    *valid = false;
                }

                // Check for potentially dangerous commands
                self.check_dangerous_shell_command(cmd, issues);
            }
        }

        if command.goal_seek.is_some() {
            command_count += 1;
            // Goal seek validation would go here
        }

        if command.foreach.is_some() {
            command_count += 1;
            // Foreach validation would go here
        }

        // Check that exactly one command type is specified
        if command_count == 0 {
            issues.push(ValidationIssue::Error(
                "Command must specify one of: claude, shell, goal_seek, or foreach".to_string(),
            ));
            *valid = false;
        } else if command_count > 1 {
            issues.push(ValidationIssue::Error(
                "Command cannot specify multiple types simultaneously".to_string(),
            ));
            *valid = false;
        }
    }

    /// Check for dangerous shell commands
    fn check_dangerous_shell_command(&self, cmd: &str, issues: &mut Vec<ValidationIssue>) {
        let dangerous_patterns = [
            ("rm -rf /", "Dangerous recursive delete from root"),
            ("rm -rf /*", "Dangerous recursive delete of all files"),
            (":(){ :|:& };:", "Fork bomb detected"),
            ("dd if=/dev/zero", "Dangerous disk write operation"),
            ("mkfs", "Filesystem formatting command"),
            ("> /dev/sda", "Direct disk write"),
        ];

        for (pattern, warning) in dangerous_patterns.iter() {
            if cmd.contains(pattern) {
                issues.push(ValidationIssue::Error(format!("{}: {}", warning, pattern)));
            }
        }

        // Warn about sudo usage
        if cmd.starts_with("sudo") || cmd.contains("| sudo") {
            issues.push(ValidationIssue::Warning(
                "Command uses sudo which may require interactive authentication".to_string(),
            ));
        }
    }

    /// Extract variable references from command
    fn extract_variables(&self, command: &WorkflowStep) -> Vec<VariableReference> {
        let mut variables = Vec::new();

        if let Some(cmd) = &command.claude {
            variables.extend(self.extract_from_string(cmd));
        }

        if let Some(cmd) = &command.shell {
            variables.extend(self.extract_from_string(cmd));
        }

        // Extract from other command types as needed
        if let Some(goal_seek) = &command.goal_seek {
            if let Some(claude) = &goal_seek.claude {
                variables.extend(self.extract_from_string(claude));
            }
            variables.extend(self.extract_from_string(&goal_seek.validate));
        }

        variables
    }

    /// Extract variables from a string
    fn extract_from_string(&self, text: &str) -> Vec<VariableReference> {
        self.variable_regex
            .captures_iter(text)
            .map(|cap| VariableReference {
                name: cap[1].to_string(),
                context: self.determine_context(&cap[1]),
            })
            .collect()
    }

    /// Determine variable context from name
    fn determine_context(&self, var_name: &str) -> VariableContext {
        if var_name.starts_with("item.") {
            VariableContext::Item
        } else if var_name.starts_with("map.") {
            VariableContext::Map
        } else if var_name.starts_with("setup.") {
            VariableContext::Setup
        } else if var_name.starts_with("shell.") {
            VariableContext::Shell
        } else if var_name.starts_with("merge.") {
            VariableContext::Merge
        } else {
            VariableContext::Unknown
        }
    }

    /// Validate variable references
    fn validate_variable_references(
        &self,
        references: &[VariableReference],
        issues: &mut Vec<ValidationIssue>,
    ) {
        for var_ref in references {
            match var_ref.context {
                VariableContext::Unknown => {
                    issues.push(ValidationIssue::Warning(format!(
                        "Variable '{}' has unknown context - may not be available",
                        var_ref.name
                    )));
                }
                _ => {
                    // Context is known, variable should be available
                    debug!(
                        "Variable {} has context {:?}",
                        var_ref.name, var_ref.context
                    );
                }
            }
        }
    }

    /// Validate command-specific syntax
    fn validate_command_syntax(
        &self,
        command: &WorkflowStep,
        issues: &mut Vec<ValidationIssue>,
        valid: &mut bool,
    ) {
        // Validate shell command syntax
        if let Some(cmd) = &command.shell {
            // Check for unclosed quotes
            let single_quotes = cmd.matches('\'').count();
            let double_quotes = cmd.matches('"').count();

            if single_quotes % 2 != 0 {
                issues.push(ValidationIssue::Error(
                    "Unclosed single quote in shell command".to_string(),
                ));
                *valid = false;
            }

            if double_quotes % 2 != 0 {
                issues.push(ValidationIssue::Error(
                    "Unclosed double quote in shell command".to_string(),
                ));
                *valid = false;
            }

            // Check for unescaped special characters that might cause issues
            if cmd.contains("$((") && !cmd.contains("))") {
                issues.push(ValidationIssue::Warning(
                    "Possible unclosed arithmetic expansion in shell command".to_string(),
                ));
            }
        }

        // Validate Claude command syntax
        if let Some(cmd) = &command.claude {
            // Check for common Claude command patterns
            if cmd.starts_with('/') {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    issues.push(ValidationIssue::Error(
                        "Claude slash command is incomplete".to_string(),
                    ));
                    *valid = false;
                }
            }
        }
    }

    /// Estimate command execution duration
    fn estimate_duration(&self, command: &WorkflowStep) -> Duration {
        if command.claude.is_some() {
            // Claude commands typically take 30-120 seconds
            Duration::from_secs(60)
        } else if let Some(shell_cmd) = &command.shell {
            // Estimate based on command type
            if shell_cmd.contains("npm install") || shell_cmd.contains("cargo build") {
                Duration::from_secs(120)
            } else if shell_cmd.contains("test") || shell_cmd.contains("pytest") {
                Duration::from_secs(60)
            } else {
                Duration::from_secs(10)
            }
        } else if command.goal_seek.is_some() {
            // Goal seek can take multiple iterations
            Duration::from_secs(180)
        } else if command.foreach.is_some() {
            // Foreach depends on iterations
            Duration::from_secs(120)
        } else {
            Duration::from_secs(30)
        }
    }
}

impl Default for CommandValidator {
    fn default() -> Self {
        Self::new()
    }
}
