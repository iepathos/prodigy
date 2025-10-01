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

        // Build cargo command
        let mut cargo_args = vec![command.clone()];

        // Add common flags using pure function
        cargo_args.extend(build_boolean_flags(&attributes));

        // Add features if specified
        if let Some(features) = attributes.get("features").and_then(|v| v.as_string()) {
            cargo_args.push("--features".to_string());
            cargo_args.push(features.clone());
        }

        // Add package if specified
        if let Some(package) = attributes.get("package").and_then(|v| v.as_string()) {
            cargo_args.push("--package".to_string());
            cargo_args.push(package.clone());
        }

        // Add target if specified
        if let Some(target) = attributes.get("target").and_then(|v| v.as_string()) {
            cargo_args.push("--target".to_string());
            cargo_args.push(target.clone());
        }

        // Add additional args if provided
        if let Some(args) = attributes.get("args").and_then(|v| v.as_string()) {
            for arg in args.split_whitespace() {
                cargo_args.push(arg.to_string());
            }
        }

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
                    // Parse cargo output for useful information
                    let mut metadata = json!({
                        "command": command,
                    });

                    // Try to extract compilation info if it's a build command
                    if command == "build" || command == "test" || command == "check" {
                        if stdout.contains("Finished") {
                            metadata["finished"] = json!(true);
                        }
                        if stdout.contains("warning") {
                            let warning_count = stdout.matches("warning").count();
                            metadata["warnings"] = json!(warning_count);
                        }
                    }

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
}
