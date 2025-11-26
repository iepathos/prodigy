//! Pure workflow processing functions
//!
//! These functions handle workflow logic without performing any I/O operations.
//! They process workflow data, validate conditions, and compute state changes.

use std::collections::HashMap;

/// Workflow variable resolution result
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedVariable {
    pub name: String,
    pub raw_expression: String,
    pub resolved_value: String,
}

/// Pure variable interpolation
pub fn interpolate_variables(template: &str, variables: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in variables {
        let placeholder = format!("${{{}}}", key);
        result = result.replace(&placeholder, value);

        // Also handle without braces for backwards compatibility
        let placeholder_simple = format!("${}", key);
        if !result.contains(&placeholder) {
            result = result.replace(&placeholder_simple, value);
        }
    }

    result
}

/// Extract variable names from a template string
pub fn extract_variable_names(template: &str) -> Vec<String> {
    let mut variables = Vec::new();
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

    for cap in re.captures_iter(template) {
        if let Some(var_name) = cap.get(1) {
            variables.push(var_name.as_str().to_string());
        }
    }

    // Also check for simple $VAR format
    let simple_re = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in simple_re.captures_iter(template) {
        if let Some(var_name) = cap.get(1) {
            let var = var_name.as_str().to_string();
            if !variables.contains(&var) {
                variables.push(var);
            }
        }
    }

    variables
}

/// Determine command type from a command string
#[derive(Debug, Clone, PartialEq)]
pub enum CommandType {
    Shell,
    Claude,
    Test,
    Foreach,
}

/// Parse command type from command prefix
pub fn parse_command_type(command: &str) -> Option<CommandType> {
    let trimmed = command.trim();

    if trimmed.starts_with("shell:") {
        Some(CommandType::Shell)
    } else if trimmed.starts_with("claude:") {
        Some(CommandType::Claude)
    } else if trimmed.starts_with("test:") {
        Some(CommandType::Test)
    } else if trimmed.starts_with("foreach:") {
        Some(CommandType::Foreach)
    } else {
        None
    }
}

/// Extract command content after the type prefix
pub fn extract_command_content(command: &str) -> String {
    let trimmed = command.trim();

    for prefix in &["shell:", "claude:", "test:", "foreach:"] {
        if let Some(content) = trimmed.strip_prefix(prefix) {
            return content.trim().to_string();
        }
    }

    trimmed.to_string()
}

/// Calculate step progress
#[derive(Debug, Clone)]
pub struct StepProgress {
    pub current: usize,
    pub total: usize,
    pub percentage: f64,
}

pub fn calculate_step_progress(current: usize, total: usize) -> StepProgress {
    let percentage = if total > 0 {
        (current as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    StepProgress {
        current,
        total,
        percentage,
    }
}

/// Workflow validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate workflow structure
pub fn validate_workflow_structure(
    commands: &[String],
    max_iterations: Option<usize>,
) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check for empty workflow
    if commands.is_empty() {
        errors.push("Workflow has no commands".to_string());
    }

    // Check for invalid command formats
    for (i, cmd) in commands.iter().enumerate() {
        if cmd.trim().is_empty() {
            errors.push(format!("Command {} is empty", i + 1));
        }

        // Check for unrecognized command types
        if parse_command_type(cmd).is_none() && !cmd.contains(':') {
            warnings.push(format!(
                "Command {} may have invalid format: '{}'",
                i + 1,
                cmd
            ));
        }
    }

    // Check iteration limits
    if let Some(max) = max_iterations {
        if max == 0 {
            errors.push("Maximum iterations cannot be zero".to_string());
        } else if max > 100 {
            warnings.push(format!(
                "High iteration count ({}) may take a long time",
                max
            ));
        }
    }

    ValidationResult {
        is_valid: errors.is_empty(),
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_variables() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "test".to_string());
        vars.insert("version".to_string(), "1.0".to_string());

        let template = "Project ${name} version ${version}";
        let result = interpolate_variables(template, &vars);
        assert_eq!(result, "Project test version 1.0");

        // Test simple format
        let template2 = "Project $name version $version";
        let result2 = interpolate_variables(template2, &vars);
        assert_eq!(result2, "Project test version 1.0");
    }

    #[test]
    fn test_extract_variable_names() {
        let template = "Project ${name} version ${version} and $simple";
        let vars = extract_variable_names(template);
        assert_eq!(vars.len(), 3);
        assert!(vars.contains(&"name".to_string()));
        assert!(vars.contains(&"version".to_string()));
        assert!(vars.contains(&"simple".to_string()));
    }

    #[test]
    fn test_parse_command_type() {
        assert_eq!(
            parse_command_type("shell: ls -la"),
            Some(CommandType::Shell)
        );
        assert_eq!(
            parse_command_type("claude: /help"),
            Some(CommandType::Claude)
        );
        assert_eq!(
            parse_command_type("test: cargo test"),
            Some(CommandType::Test)
        );
        assert_eq!(parse_command_type("unknown command"), None);
    }

    #[test]
    fn test_extract_command_content() {
        assert_eq!(extract_command_content("shell: ls -la"), "ls -la");
        assert_eq!(extract_command_content("claude: /help"), "/help");
        assert_eq!(extract_command_content("no prefix"), "no prefix");
    }

    #[test]
    fn test_calculate_step_progress() {
        let progress = calculate_step_progress(5, 10);
        assert_eq!(progress.current, 5);
        assert_eq!(progress.total, 10);
        assert_eq!(progress.percentage, 50.0);

        let zero_progress = calculate_step_progress(0, 0);
        assert_eq!(zero_progress.percentage, 0.0);
    }

    #[test]
    fn test_validate_workflow_structure() {
        // Valid workflow
        let commands = vec!["shell: echo hello".to_string(), "claude: /help".to_string()];
        let result = validate_workflow_structure(&commands, Some(5));
        assert!(result.is_valid);
        assert!(result.errors.is_empty());

        // Empty workflow
        let empty: Vec<String> = vec![];
        let result = validate_workflow_structure(&empty, None);
        assert!(!result.is_valid);
        assert!(result
            .errors
            .contains(&"Workflow has no commands".to_string()));

        // Invalid iterations
        let result = validate_workflow_structure(&commands, Some(0));
        assert!(!result.is_valid);
        assert!(result
            .errors
            .contains(&"Maximum iterations cannot be zero".to_string()));
    }
}
