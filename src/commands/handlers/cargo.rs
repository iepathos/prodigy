//! Cargo command handler for Rust project management

use crate::commands::{
    AttributeSchema, AttributeValue, CommandHandler, CommandResult, ExecutionContext,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

/// Builds boolean flags from attributes using iterator chains
fn build_boolean_flags(attributes: &HashMap<String, AttributeValue>) -> Vec<String> {
    let flag_mappings = [
        ("release", "--release"),
        ("all_features", "--all-features"),
        ("no_default_features", "--no-default-features"),
        ("verbose", "--verbose"),
        ("quiet", "--quiet"),
    ];

    flag_mappings
        .iter()
        .filter_map(|(key, flag)| {
            attributes
                .get(*key)
                .and_then(|v| v.as_bool())
                .filter(|&enabled| enabled)
                .map(|_| (*flag).to_string())
        })
        .collect()
}

/// Builds string option arguments from attributes using iterator chains
fn build_string_option_args(attributes: &HashMap<String, AttributeValue>) -> Vec<String> {
    let option_mappings = [
        ("features", "--features"),
        ("package", "--package"),
        ("target", "--target"),
    ];

    let regular_options = option_mappings.iter().flat_map(|(key, flag)| {
        attributes
            .get(*key)
            .and_then(|v| v.as_string())
            .map(|value| vec![(*flag).to_string(), value.clone()])
    });

    let args_options = attributes
        .get("args")
        .and_then(|v| v.as_string())
        .into_iter()
        .flat_map(|args| args.split_whitespace().map(|s| s.to_string()));

    regular_options.flatten().chain(args_options).collect()
}

/// Parses cargo command output to extract metadata using functional patterns
fn parse_cargo_metadata(command: &str, stdout: &str) -> serde_json::Value {
    let build_commands = ["build", "test", "check"];

    let mut metadata = json!({
        "command": command,
    });

    if build_commands.contains(&command) {
        if stdout.contains("Finished") {
            metadata["finished"] = json!(true);
        }
        if stdout.contains("warning") {
            let warning_count = stdout.matches("warning").count();
            metadata["warnings"] = json!(warning_count);
        }
    }

    metadata
}

/// Handler for Cargo operations
pub struct CargoHandler;

impl CargoHandler {
    /// Creates a new Cargo handler
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for CargoHandler {
    fn name(&self) -> &str {
        "cargo"
    }

    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("cargo");
        schema.add_required("command", "Cargo command to run (build, test, run, etc.)");
        schema.add_optional("args", "Additional arguments for the cargo command");
        schema.add_optional("features", "Features to enable");
        schema.add_optional("package", "Package to operate on (for workspaces)");
        schema.add_optional("target", "Target triple for cross-compilation");
        schema.add_optional_with_default(
            "release",
            "Build in release mode",
            AttributeValue::Boolean(false),
        );
        schema.add_optional_with_default(
            "all_features",
            "Enable all features",
            AttributeValue::Boolean(false),
        );
        schema.add_optional_with_default(
            "no_default_features",
            "Disable default features",
            AttributeValue::Boolean(false),
        );
        schema.add_optional_with_default(
            "verbose",
            "Use verbose output",
            AttributeValue::Boolean(false),
        );
        schema.add_optional_with_default(
            "quiet",
            "Use quiet output",
            AttributeValue::Boolean(false),
        );
        schema
    }

    async fn execute(
        &self,
        context: &ExecutionContext,
        mut attributes: HashMap<String, AttributeValue>,
    ) -> CommandResult {
        // Apply defaults
        self.schema().apply_defaults(&mut attributes);

        // Extract command
        let command = match attributes.get("command").and_then(|v| v.as_string()) {
            Some(cmd) => cmd.clone(),
            None => return CommandResult::error("Missing required attribute: command".to_string()),
        };

        let start = Instant::now();

        // Build cargo command using functional composition
        let cargo_args: Vec<String> = std::iter::once(command.clone())
            .chain(build_boolean_flags(&attributes))
            .chain(build_string_option_args(&attributes))
            .collect();

        if context.dry_run {
            let duration = start.elapsed().as_millis() as u64;
            return CommandResult::success(json!({
                "dry_run": true,
                "command": format!("cargo {}", cargo_args.join(" ")),
            }))
            .with_duration(duration);
        }

        // Set environment variables for better cargo output
        let mut env = context.full_env();
        env.insert("CARGO_TERM_COLOR".to_string(), "always".to_string());

        // Execute cargo command
        let result = context
            .executor
            .execute(
                "cargo",
                &cargo_args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                Some(&context.working_dir),
                Some(env),
                None,
            )
            .await;

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    // Parse cargo output using pure function
                    let metadata = parse_cargo_metadata(&command, &stdout);

                    CommandResult::success(json!({
                        "output": stdout,
                        "metadata": metadata,
                    }))
                    .with_duration(duration)
                } else {
                    CommandResult::error(format!("Cargo command failed:\n{stderr}"))
                        .with_duration(duration)
                }
            }
            Err(e) => CommandResult::error(format!("Failed to execute cargo command: {e}"))
                .with_duration(duration),
        }
    }

    fn description(&self) -> &str {
        "Handles Rust Cargo operations for building, testing, and managing projects"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            r#"{"command": "build", "release": true}"#.to_string(),
            r#"{"command": "test", "package": "my_crate", "features": "async"}"#.to_string(),
            r#"{"command": "run", "args": "-- --help"}"#.to_string(),
            r#"{"command": "clippy", "args": "-- -W clippy::all"}"#.to_string(),
        ]
    }
}

impl Default for CargoHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::adapter::MockSubprocessExecutor;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Output;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_cargo_handler_schema() {
        let handler = CargoHandler::new();
        let schema = handler.schema();

        assert!(schema.required().contains_key("command"));
        assert!(schema.optional().contains_key("release"));
        assert!(schema.optional().contains_key("features"));
    }

    #[tokio::test]
    async fn test_cargo_build() {
        let handler = CargoHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "cargo",
            vec!["build", "--release"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"   Compiling test v0.1.0\n    Finished release [optimized] target(s)"
                    .to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "command".to_string(),
            AttributeValue::String("build".to_string()),
        );
        attributes.insert("release".to_string(), AttributeValue::Boolean(true));

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_cargo_test_with_features() {
        let handler = CargoHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "cargo",
            vec!["test", "--features", "async"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"running 10 tests\ntest result: ok. 10 passed".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "command".to_string(),
            AttributeValue::String("test".to_string()),
        );
        attributes.insert(
            "features".to_string(),
            AttributeValue::String("async".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_cargo_dry_run() {
        let handler = CargoHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test")).with_dry_run(true);

        let mut attributes = HashMap::new();
        attributes.insert(
            "command".to_string(),
            AttributeValue::String("build".to_string()),
        );
        attributes.insert("release".to_string(), AttributeValue::Boolean(true));

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());

        let data = result.data.unwrap();
        assert_eq!(data.get("dry_run"), Some(&json!(true)));
        assert!(data
            .get("command")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("cargo build --release"));
    }

    #[tokio::test]
    async fn test_cargo_missing_command_attribute() {
        let handler = CargoHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test"));

        let attributes = HashMap::new();
        let result = handler.execute(&context, attributes).await;

        assert!(!result.is_success());
        assert!(result.error.unwrap().contains("Missing required attribute: command"));
    }

    #[tokio::test]
    async fn test_cargo_dry_run_with_flags() {
        let handler = CargoHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test")).with_dry_run(true);

        let mut attributes = HashMap::new();
        attributes.insert("command".to_string(), AttributeValue::String("build".to_string()));
        attributes.insert("release".to_string(), AttributeValue::Boolean(true));
        attributes.insert("features".to_string(), AttributeValue::String("async".to_string()));

        let result = handler.execute(&context, attributes).await;

        assert!(result.is_success());
        let data = result.data.unwrap();
        assert_eq!(data.get("dry_run"), Some(&json!(true)));
        let command = data.get("command").unwrap().as_str().unwrap();
        assert!(command.contains("build"));
        assert!(command.contains("--release"));
        assert!(command.contains("--features async"));
    }

    #[tokio::test]
    async fn test_cargo_success_with_metadata() {
        let handler = CargoHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "cargo",
            vec!["build"],
            Some(PathBuf::from("/test")),
            None,
            None,
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"   Compiling test v0.1.0\nwarning: unused variable\n    Finished dev [unoptimized]".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context = ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert("command".to_string(), AttributeValue::String("build".to_string()));

        let result = handler.execute(&context, attributes).await;

        assert!(result.is_success());
        let data = result.data.unwrap();
        assert!(data.get("metadata").is_some());
        assert_eq!(data["metadata"]["command"], "build");
        assert_eq!(data["metadata"]["finished"], true);
        assert_eq!(data["metadata"]["warnings"], 1);
    }

    // Tests for pure functions
    mod pure_functions {
        use super::*;

        #[test]
        fn test_build_boolean_flags_all_true() {
            let mut attributes = HashMap::new();
            attributes.insert("release".to_string(), AttributeValue::Boolean(true));
            attributes.insert("all_features".to_string(), AttributeValue::Boolean(true));
            attributes.insert(
                "no_default_features".to_string(),
                AttributeValue::Boolean(true),
            );
            attributes.insert("verbose".to_string(), AttributeValue::Boolean(true));
            attributes.insert("quiet".to_string(), AttributeValue::Boolean(true));

            let flags = build_boolean_flags(&attributes);

            assert_eq!(flags.len(), 5);
            assert!(flags.contains(&"--release".to_string()));
            assert!(flags.contains(&"--all-features".to_string()));
            assert!(flags.contains(&"--no-default-features".to_string()));
            assert!(flags.contains(&"--verbose".to_string()));
            assert!(flags.contains(&"--quiet".to_string()));
        }

        #[test]
        fn test_build_boolean_flags_all_false() {
            let mut attributes = HashMap::new();
            attributes.insert("release".to_string(), AttributeValue::Boolean(false));
            attributes.insert("all_features".to_string(), AttributeValue::Boolean(false));
            attributes.insert(
                "no_default_features".to_string(),
                AttributeValue::Boolean(false),
            );

            let flags = build_boolean_flags(&attributes);

            assert_eq!(flags.len(), 0);
        }

        #[test]
        fn test_build_boolean_flags_mixed() {
            let mut attributes = HashMap::new();
            attributes.insert("release".to_string(), AttributeValue::Boolean(true));
            attributes.insert("all_features".to_string(), AttributeValue::Boolean(false));
            attributes.insert("verbose".to_string(), AttributeValue::Boolean(true));

            let flags = build_boolean_flags(&attributes);

            assert_eq!(flags.len(), 2);
            assert!(flags.contains(&"--release".to_string()));
            assert!(flags.contains(&"--verbose".to_string()));
        }

        #[test]
        fn test_build_string_option_args_features() {
            let mut attributes = HashMap::new();
            attributes.insert(
                "features".to_string(),
                AttributeValue::String("async tokio".to_string()),
            );

            let args = build_string_option_args(&attributes);

            assert_eq!(args, vec!["--features", "async tokio"]);
        }

        #[test]
        fn test_build_string_option_args_package() {
            let mut attributes = HashMap::new();
            attributes.insert(
                "package".to_string(),
                AttributeValue::String("my_crate".to_string()),
            );

            let args = build_string_option_args(&attributes);

            assert_eq!(args, vec!["--package", "my_crate"]);
        }

        #[test]
        fn test_build_string_option_args_with_args_splitting() {
            let mut attributes = HashMap::new();
            attributes.insert(
                "args".to_string(),
                AttributeValue::String("-- --help --verbose".to_string()),
            );

            let args = build_string_option_args(&attributes);

            assert_eq!(args, vec!["--", "--help", "--verbose"]);
        }

        #[test]
        fn test_build_string_option_args_multiple_options() {
            let mut attributes = HashMap::new();
            attributes.insert(
                "features".to_string(),
                AttributeValue::String("async".to_string()),
            );
            attributes.insert(
                "package".to_string(),
                AttributeValue::String("my_crate".to_string()),
            );
            attributes.insert(
                "target".to_string(),
                AttributeValue::String("x86_64-unknown-linux-gnu".to_string()),
            );

            let args = build_string_option_args(&attributes);

            assert_eq!(
                args,
                vec![
                    "--features",
                    "async",
                    "--package",
                    "my_crate",
                    "--target",
                    "x86_64-unknown-linux-gnu"
                ]
            );
        }

        #[test]
        fn test_parse_cargo_metadata_build_with_warnings() {
            let stdout = "   Compiling test v0.1.0\nwarning: unused variable\nwarning: dead code\n    Finished release [optimized]";

            let metadata = parse_cargo_metadata("build", stdout);

            assert_eq!(metadata["command"], "build");
            assert_eq!(metadata["finished"], true);
            assert_eq!(metadata["warnings"], 2);
        }

        #[test]
        fn test_parse_cargo_metadata_finished_detection() {
            let stdout = "    Finished dev [unoptimized + debuginfo]";

            let metadata = parse_cargo_metadata("check", stdout);

            assert_eq!(metadata["command"], "check");
            assert_eq!(metadata["finished"], true);
        }

        #[test]
        fn test_parse_cargo_metadata_non_build_command() {
            let stdout = "Some output from cargo run";

            let metadata = parse_cargo_metadata("run", stdout);

            assert_eq!(metadata["command"], "run");
            assert!(metadata.get("finished").is_none());
            assert!(metadata.get("warnings").is_none());
        }
    }
}
