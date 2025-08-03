use super::command::{Command, CommandArg};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;

static VAR_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex pattern"));

/// Parse a command string into a structured Command
/// Supports formats like:
/// - "mmm-code-review"
/// - "/mmm-code-review"
/// - `"mmm-implement-spec ${SPEC_ID}"`
/// - "mmm-code-review --focus security"
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
            let key = part.trim_start_matches("--");

            // Check if next part exists and doesn't start with --
            if i + 1 < parts.len() && !parts[i + 1].starts_with("--") {
                let next_part = parts[i + 1];

                // Heuristic: if the key suggests it's a boolean flag, treat it as one
                // Common boolean flags that don't take values
                let boolean_flags = [
                    "verbose", "help", "version", "debug", "quiet", "force", "dry-run",
                ];

                if boolean_flags.contains(&key) {
                    // Boolean flag - don't consume next part
                    cmd.options.insert(key.to_string(), serde_json::json!(true));
                    i += 1;
                } else {
                    // Option with value
                    cmd.options
                        .insert(key.to_string(), serde_json::json!(next_part));
                    i += 2;
                }
            } else {
                // Boolean flag (no next part or next part is another option)
                cmd.options.insert(key.to_string(), serde_json::json!(true));
                i += 1;
            }
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
        let cmd = parse_command_string("mmm-code-review").unwrap();
        assert_eq!(cmd.name, "mmm-code-review");
        assert!(cmd.args.is_empty());
        assert!(cmd.options.is_empty());
    }

    #[test]
    fn test_parse_command_with_slash() {
        let cmd = parse_command_string("/mmm-lint").unwrap();
        assert_eq!(cmd.name, "mmm-lint");
    }

    #[test]
    fn test_parse_command_with_args() {
        let cmd = parse_command_string("mmm-implement-spec iteration-123").unwrap();
        assert_eq!(cmd.name, "mmm-implement-spec");
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(
            cmd.args[0],
            CommandArg::Literal("iteration-123".to_string())
        );
    }

    #[test]
    fn test_parse_command_with_options() {
        let cmd = parse_command_string("mmm-code-review --focus security --verbose").unwrap();
        assert_eq!(cmd.name, "mmm-code-review");
        assert_eq!(
            cmd.options.get("focus"),
            Some(&serde_json::json!("security"))
        );
        assert_eq!(cmd.options.get("verbose"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_parse_command_with_variable() {
        let cmd = parse_command_string("mmm-implement-spec ${SPEC_ID}").unwrap();
        assert_eq!(cmd.name, "mmm-implement-spec");
        assert_eq!(cmd.args[0], CommandArg::Variable("SPEC_ID".to_string()));
    }

    #[test]
    fn test_expand_variables() {
        let mut cmd = Command::new("mmm-implement-spec")
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
            "mmm-code-review --focus security --max-issues 10 --verbose file1.rs file2.rs",
        )
        .unwrap();

        assert_eq!(cmd.name, "mmm-code-review");
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
        let result = parse_command_string("echo 'hello world'");
        assert!(result.is_ok());

        let command = result.unwrap();
        assert_eq!(command.name, "echo");
        assert_eq!(command.args.len(), 1);
        assert_eq!(command.args[0], CommandArg::Literal("'hello".to_string()));
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
