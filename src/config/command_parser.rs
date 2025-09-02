use super::command::{Command, CommandArg};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;

static VAR_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex pattern"));

/// Validate command string preconditions
fn validate_preconditions(s: &str) -> Result<Vec<&str>> {
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

    Ok(parts)
}

/// Check if a command option is a boolean flag
fn is_boolean_flag(key: &str) -> bool {
    const BOOLEAN_FLAGS: &[&str] = &[
        "verbose", "help", "version", "debug", "quiet", "force", "dry-run",
    ];
    BOOLEAN_FLAGS.contains(&key)
}

/// Parse a single command-line option
fn parse_option(cmd: &mut Command, parts: &[&str], i: usize) -> usize {
    let part = parts[i];
    let key = part.trim_start_matches("--");

    // Check if next part exists and doesn't start with --
    if i + 1 < parts.len() && !parts[i + 1].starts_with("--") && !is_boolean_flag(key) {
        // Option with value
        cmd.options
            .insert(key.to_string(), serde_json::json!(parts[i + 1]));
        i + 2
    } else {
        // Boolean flag (no next part or next part is another option or it's a known boolean flag)
        cmd.options.insert(key.to_string(), serde_json::json!(true));
        i + 1
    }
}

/// Parse a command string into a structured Command
/// Supports formats like:
/// - "prodigy-code-review"
/// - "/prodigy-code-review"
/// - `"prodigy-implement-spec ${SPEC_ID}"`
/// - "prodigy-code-review --focus security"
pub fn parse_command_string(s: &str) -> Result<Command> {
    let parts = validate_preconditions(s)?;
    let mut cmd = Command::new(parts[0]);

    // Parse remaining parts as arguments or options
    let mut i = 1;
    while i < parts.len() {
        let part = parts[i];

        if part.starts_with("--") {
            i = parse_option(&mut cmd, &parts, i);
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
    fn test_validate_preconditions_empty() {
        assert!(validate_preconditions("").is_err());
        assert!(validate_preconditions("  ").is_err());
        assert!(validate_preconditions("\t\n").is_err());
    }

    #[test]
    fn test_validate_preconditions_with_slash() {
        let parts = validate_preconditions("/command").unwrap();
        assert_eq!(parts, vec!["command"]);
    }

    #[test]
    fn test_validate_preconditions_multiple_parts() {
        let parts = validate_preconditions("cmd arg1 arg2 --opt").unwrap();
        assert_eq!(parts, vec!["cmd", "arg1", "arg2", "--opt"]);
    }

    #[test]
    fn test_is_boolean_flag() {
        assert!(is_boolean_flag("verbose"));
        assert!(is_boolean_flag("help"));
        assert!(is_boolean_flag("version"));
        assert!(is_boolean_flag("debug"));
        assert!(is_boolean_flag("quiet"));
        assert!(is_boolean_flag("force"));
        assert!(is_boolean_flag("dry-run"));
        assert!(!is_boolean_flag("focus"));
        assert!(!is_boolean_flag("max-issues"));
        assert!(!is_boolean_flag("unknown"));
    }

    #[test]
    fn test_parse_option_boolean_flag() {
        let mut cmd = Command::new("test");
        let parts = vec!["cmd", "--verbose", "next"];
        let next_i = parse_option(&mut cmd, &parts, 1);
        assert_eq!(next_i, 2);
        assert_eq!(cmd.options.get("verbose"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_parse_option_with_value() {
        let mut cmd = Command::new("test");
        let parts = vec!["cmd", "--focus", "security", "--verbose"];
        let next_i = parse_option(&mut cmd, &parts, 1);
        assert_eq!(next_i, 3);
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );
    }

    #[test]
    fn test_parse_option_at_end() {
        let mut cmd = Command::new("test");
        let parts = vec!["cmd", "--verbose"];
        let next_i = parse_option(&mut cmd, &parts, 1);
        assert_eq!(next_i, 2);
        assert_eq!(cmd.options.get("verbose"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_parse_option_before_another_option() {
        let mut cmd = Command::new("test");
        let parts = vec!["cmd", "--focus", "--verbose"];
        let next_i = parse_option(&mut cmd, &parts, 1);
        assert_eq!(next_i, 2);
        assert_eq!(cmd.options.get("focus"), Some(&serde_json::json!(true)));
    }

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
}
