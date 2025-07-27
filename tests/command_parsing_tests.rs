use std::collections::HashMap;

/// Test command string parsing into components
#[test]
fn test_parse_command_string() {
    // Test simple command
    let (cmd, args) = parse_command_string("mmm-lint");
    assert_eq!(cmd, "mmm-lint");
    assert!(args.is_empty());

    // Test command with single argument
    let (cmd, args) = parse_command_string("mmm-code-review --focus architecture");
    assert_eq!(cmd, "mmm-code-review");
    assert_eq!(args, vec!["--focus", "architecture"]);

    // Test command with multiple arguments
    let (cmd, args) = parse_command_string("mmm-analyze --verbose --depth 3 --output report.md");
    assert_eq!(cmd, "mmm-analyze");
    assert_eq!(
        args,
        vec!["--verbose", "--depth", "3", "--output", "report.md"]
    );
}

/// Test command string parsing with quoted arguments
#[test]
fn test_parse_command_with_quotes() {
    // Test single quoted argument
    let (cmd, args) = parse_command_string("mmm-commit --message 'Fix bug in parser'");
    assert_eq!(cmd, "mmm-commit");
    assert_eq!(args, vec!["--message", "Fix bug in parser"]);

    // Test double quoted argument
    let (cmd, args) = parse_command_string("mmm-review --focus \"error handling\"");
    assert_eq!(cmd, "mmm-review");
    assert_eq!(args, vec!["--focus", "error handling"]);

    // Test mixed quotes
    let (cmd, args) = parse_command_string("mmm-test --name 'integration test' --tag \"v1.0\"");
    assert_eq!(cmd, "mmm-test");
    assert_eq!(args, vec!["--name", "integration test", "--tag", "v1.0"]);
}

/// Test edge cases in command parsing
#[test]
fn test_parse_command_edge_cases() {
    // Empty string
    let (cmd, args) = parse_command_string("");
    assert_eq!(cmd, "");
    assert!(args.is_empty());

    // Only whitespace
    let (cmd, args) = parse_command_string("   ");
    assert_eq!(cmd, "");
    assert!(args.is_empty());

    // Command with leading/trailing whitespace
    let (cmd, args) = parse_command_string("  mmm-lint  ");
    assert_eq!(cmd, "mmm-lint");
    assert!(args.is_empty());

    // Multiple spaces between arguments
    let (cmd, args) = parse_command_string("mmm-review    --focus    architecture");
    assert_eq!(cmd, "mmm-review");
    assert_eq!(args, vec!["--focus", "architecture"]);
}

/// Test conversion from string command to structured command
#[test]
fn test_string_to_structured_command() {
    // Simple command
    let cmd = Command::from_string("mmm-lint");
    assert_eq!(cmd.name, "mmm-lint");
    assert!(cmd.args.is_empty());

    // Command with focus argument
    let cmd = Command::from_string("mmm-code-review --focus architecture");
    assert_eq!(cmd.name, "mmm-code-review");
    assert_eq!(cmd.args.get("focus"), Some(&"architecture".to_string()));

    // Command with multiple arguments
    let cmd = Command::from_string("mmm-analyze --verbose --depth 3");
    assert_eq!(cmd.name, "mmm-analyze");
    assert_eq!(cmd.args.get("verbose"), Some(&"true".to_string()));
    assert_eq!(cmd.args.get("depth"), Some(&"3".to_string()));
}

/// Test structured command validation
#[test]
fn test_command_validation() {
    // Valid command
    let cmd = Command {
        name: "mmm-code-review".to_string(),
        args: HashMap::from([("focus".to_string(), "architecture".to_string())]),
        timeout: None,
        retry_on_failure: false,
        continue_on_error: false,
        env: HashMap::new(),
        working_dir: None,
    };
    assert!(validate_command(&cmd).is_ok());

    // Invalid command name
    let cmd = Command {
        name: "invalid-command".to_string(),
        args: HashMap::new(),
        timeout: None,
        retry_on_failure: false,
        continue_on_error: false,
        env: HashMap::new(),
        working_dir: None,
    };
    assert!(validate_command(&cmd).is_err());

    // Invalid argument for command
    let cmd = Command {
        name: "mmm-lint".to_string(),
        args: HashMap::from([("focus".to_string(), "architecture".to_string())]), // mmm-lint doesn't support focus
        timeout: None,
        retry_on_failure: false,
        continue_on_error: false,
        env: HashMap::new(),
        working_dir: None,
    };
    assert!(validate_command(&cmd).is_err());
}

// Helper functions that would be implemented in the actual code
fn parse_command_string(input: &str) -> (String, Vec<&str>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), vec![]);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), vec![]);
    }

    (parts[0].to_string(), parts[1..].to_vec())
}

#[derive(Debug, PartialEq)]
struct Command {
    name: String,
    args: HashMap<String, String>,
    timeout: Option<u64>,
    retry_on_failure: bool,
    continue_on_error: bool,
    env: HashMap<String, String>,
    working_dir: Option<String>,
}

impl Command {
    fn from_string(input: &str) -> Self {
        let (name, args_vec) = parse_command_string(input);
        let mut args = HashMap::new();

        // Simple argument parsing for demonstration
        let mut i = 0;
        while i < args_vec.len() {
            if args_vec[i].starts_with("--") {
                let key = args_vec[i].trim_start_matches("--");
                let value = if i + 1 < args_vec.len() && !args_vec[i + 1].starts_with("--") {
                    i += 1;
                    args_vec[i]
                } else {
                    "true"
                };
                args.insert(key.to_string(), value.to_string());
            }
            i += 1;
        }

        Command {
            name,
            args,
            timeout: None,
            retry_on_failure: false,
            continue_on_error: false,
            env: HashMap::new(),
            working_dir: None,
        }
    }
}

fn validate_command(cmd: &Command) -> Result<(), String> {
    // Placeholder validation logic
    let valid_commands = vec![
        "mmm-code-review",
        "mmm-implement-spec",
        "mmm-lint",
        "mmm-test",
        "mmm-analyze",
    ];

    if !valid_commands.contains(&cmd.name.as_str()) {
        return Err(format!("Unknown command: {}", cmd.name));
    }

    // Validate command-specific arguments
    match cmd.name.as_str() {
        "mmm-code-review" => {
            // Valid args: focus, depth, verbose
            for key in cmd.args.keys() {
                if !["focus", "depth", "verbose"].contains(&key.as_str()) {
                    return Err(format!("Invalid argument '{}' for mmm-code-review", key));
                }
            }
        }
        "mmm-lint" => {
            // mmm-lint doesn't support any custom arguments
            if !cmd.args.is_empty() {
                return Err("mmm-lint doesn't support custom arguments".to_string());
            }
        }
        _ => {}
    }

    Ok(())
}
