use super::command::{Command, CommandArg};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;

static VAR_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex pattern"));

/// Classify whether a flag is a boolean flag that doesn't take a value
fn is_boolean_flag(flag_name: &str) -> bool {
    const BOOLEAN_FLAGS: &[&str] = &[
        "verbose", "help", "version", "debug", "quiet", "force", "dry-run",
    ];
    BOOLEAN_FLAGS.contains(&flag_name)
}

/// Parse an option and determine how many parts it consumes
fn parse_option_part(parts: &[&str], index: usize) -> (String, serde_json::Value, usize) {
    let key = parts[index].trim_start_matches("--");

    // Check if this is a boolean flag or has a value
    let has_value =
        index + 1 < parts.len() && !parts[index + 1].starts_with("--") && !is_boolean_flag(key);

    if has_value {
        // Option with value
        (key.to_string(), serde_json::json!(parts[index + 1]), 2)
    } else {
        // Boolean flag
        (key.to_string(), serde_json::json!(true), 1)
    }
}

/// Parse a command string into a structured Command
/// Supports formats like:
/// - "prodigy-code-review"
/// - "/prodigy-code-review"
/// - `"prodigy-implement-spec ${SPEC_ID}"`
/// - "prodigy-code-review --focus security"
pub fn parse_command_string(s: &str) -> Result<Command> {
    let s = s.trim();
    if s.is_empty() {
        return Err(anyhow!("Empty command string"));
    }

    // Remove leading slash if present
    let s = s.strip_prefix('/').unwrap_or(s);

    // Split into parts
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow!("Invalid command format"));
    }

    let mut cmd = Command::new(parts[0]);

    // Parse remaining parts as arguments or options
    let mut i = 1;
    while i < parts.len() {
        let part = parts[i];

        if part.starts_with("--") {
            // This is an option
            let (key, value, consumed) = parse_option_part(&parts, i);
            cmd.options.insert(key, value);
            i += consumed;
        } else {
            // This is a positional argument
            cmd.args.push(CommandArg::parse(part));
            i += 1;
        }
    }

    Ok(cmd)
}

/// Expand variables in command arguments
/// Supports `${VAR_NAME}` and `$VAR` syntax
pub fn expand_variables(cmd: &mut Command, variables: &std::collections::HashMap<String, String>) {
    // Args are already CommandArg, no need to expand - they'll be resolved at execution time

    // Expand in string option values
    for value in cmd.options.values_mut() {
        if let Some(s) = value.as_str() {
            *value = serde_json::json!(expand_string(s, variables));
        }
    }

    // Expand in environment variables
    let mut new_env = std::collections::HashMap::new();
    for (key, value) in &cmd.metadata.env {
        new_env.insert(key.clone(), expand_string(value, variables));
    }
    cmd.metadata.env = new_env;
}

fn expand_string(s: &str, variables: &std::collections::HashMap<String, String>) -> String {
    let mut result = s.to_string();

    // Find all ${VAR_NAME} patterns
    for cap in VAR_REGEX.captures_iter(s) {
        if let Some(var_name) = cap.get(1) {
            if let Some(value) = variables.get(var_name.as_str()) {
                result = result.replace(&cap[0], value);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_simple_command() {
        let cmd = parse_command_string("prodigy-code-review").unwrap();
        assert_eq!(cmd.name, "prodigy-code-review");
        assert!(cmd.args.is_empty());
        assert!(cmd.options.is_empty());
    }

    #[test]
    fn test_parse_command_with_slash() {
        let cmd = parse_command_string("/prodigy-lint").unwrap();
        assert_eq!(cmd.name, "prodigy-lint");
    }

    #[test]
    fn test_parse_command_with_args() {
        let cmd = parse_command_string("prodigy-implement-spec iteration-123").unwrap();
        assert_eq!(cmd.name, "prodigy-implement-spec");
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(
            cmd.args[0],
            CommandArg::Literal("iteration-123".to_string())
        );
    }

    #[test]
    fn test_parse_command_with_options() {
        let cmd = parse_command_string("prodigy-code-review --focus security --verbose").unwrap();
        assert_eq!(cmd.name, "prodigy-code-review");
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );
        assert_eq!(cmd.options.get("verbose"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_parse_command_with_variable() {
        let cmd = parse_command_string("prodigy-implement-spec ${SPEC_ID}").unwrap();
        assert_eq!(cmd.name, "prodigy-implement-spec");
        assert_eq!(cmd.args[0], CommandArg::Variable("SPEC_ID".to_string()));
    }

    #[test]
    fn test_expand_variables() {
        let mut cmd = Command::new("prodigy-implement-spec")
            .with_arg("${SPEC_ID}")
            .with_option("focus", serde_json::json!("${FOCUS_AREA}"));

        let mut vars = HashMap::new();
        vars.insert("SPEC_ID".to_string(), "iteration-123".to_string());
        vars.insert("FOCUS_AREA".to_string(), "performance".to_string());

        expand_variables(&mut cmd, &vars);

        // Note: expand_variables doesn't change CommandArg anymore as it's resolved at execution time
        assert_eq!(cmd.args[0], CommandArg::Variable("SPEC_ID".to_string()));
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("performance"))
        );
    }

    #[test]
    fn test_parse_empty_command() {
        assert!(parse_command_string("").is_err());
        assert!(parse_command_string("  ").is_err());
    }

    #[test]
    fn test_parse_complex_command() {
        let cmd = parse_command_string(
            "prodigy-code-review --focus security --max-issues 10 --verbose file1.rs file2.rs",
        )
        .unwrap();

        assert_eq!(cmd.name, "prodigy-code-review");
        assert_eq!(cmd.args.len(), 2);
        assert_eq!(cmd.args[0], CommandArg::Literal("file1.rs".to_string()));
        assert_eq!(cmd.args[1], CommandArg::Literal("file2.rs".to_string()));
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );
        assert_eq!(
            cmd.options.get("max-issues"),
            Some(&serde_json::json!("10"))
        );
        assert_eq!(cmd.options.get("verbose"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_parse_command_string_simple() {
        // Test parsing a simple command string
        let result = parse_command_string("echo hello");
        assert!(result.is_ok());

        let command = result.unwrap();
        assert_eq!(command.name, "echo");
        assert_eq!(command.args.len(), 1);
        assert_eq!(command.args[0], CommandArg::Literal("hello".to_string()));
    }

    #[test]
    fn test_parse_command_string_with_variables() {
        // Test parsing command with variables
        let result = parse_command_string("echo ${USER}");
        assert!(result.is_ok());

        let command = result.unwrap();
        assert_eq!(command.name, "echo");
        assert_eq!(command.args.len(), 1);
        assert_eq!(command.args[0], CommandArg::Variable("USER".to_string()));
    }

    #[test]
    fn test_parse_command_string_empty() {
        // Test error for empty string
        let result = parse_command_string("");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_boolean_flag() {
        // Test classification of boolean flags
        assert!(is_boolean_flag("verbose"));
        assert!(is_boolean_flag("help"));
        assert!(is_boolean_flag("version"));
        assert!(is_boolean_flag("debug"));
        assert!(is_boolean_flag("quiet"));
        assert!(is_boolean_flag("force"));
        assert!(is_boolean_flag("dry-run"));

        // Test non-boolean flags
        assert!(!is_boolean_flag("focus"));
        assert!(!is_boolean_flag("max-issues"));
        assert!(!is_boolean_flag("output"));
        assert!(!is_boolean_flag("file"));
    }

    #[test]
    fn test_parse_option_part_with_value() {
        // Test parsing option with value
        let parts = vec!["--focus", "security", "file.rs"];
        let (key, value, consumed) = parse_option_part(&parts, 0);

        assert_eq!(key, "focus");
        assert_eq!(value, serde_json::json!("security"));
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_parse_option_part_boolean() {
        // Test parsing boolean option
        let parts = vec!["--verbose", "file.rs"];
        let (key, value, consumed) = parse_option_part(&parts, 0);

        assert_eq!(key, "verbose");
        assert_eq!(value, serde_json::json!(true));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_parse_option_part_at_end() {
        // Test parsing option at end of command
        let parts = vec!["command", "--verbose"];
        let (key, value, consumed) = parse_option_part(&parts, 1);

        assert_eq!(key, "verbose");
        assert_eq!(value, serde_json::json!(true));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_parse_option_part_before_another_option() {
        // Test parsing option followed by another option
        let parts = vec!["--output", "--verbose"];
        let (key, value, consumed) = parse_option_part(&parts, 0);

        assert_eq!(key, "output");
        assert_eq!(value, serde_json::json!(true));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_parse_mixed_arguments_and_options() {
        // Test complex command with mixed arguments and options
        let cmd = parse_command_string(
            "prodigy-analyze --verbose file1.rs --output report.txt file2.rs --debug",
        )
        .unwrap();

        assert_eq!(cmd.name, "prodigy-analyze");
        assert_eq!(cmd.args.len(), 2);
        assert_eq!(cmd.args[0], CommandArg::Literal("file1.rs".to_string()));
        assert_eq!(cmd.args[1], CommandArg::Literal("file2.rs".to_string()));
        assert_eq!(cmd.options.get("verbose"), Some(&serde_json::json!(true)));
        assert_eq!(
            cmd.options.get("output"),
            Some(&serde_json::json!("report.txt"))
        );
        assert_eq!(cmd.options.get("debug"), Some(&serde_json::json!(true)));
    }
}
