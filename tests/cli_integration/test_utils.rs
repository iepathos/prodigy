// Test utilities for CLI integration tests

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Output from CLI command execution
#[derive(Debug)]
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

impl CliOutput {
    /// Check if stdout contains a string
    pub fn stdout_contains(&self, text: &str) -> bool {
        self.stdout.contains(text)
    }

    /// Check if stderr contains a string
    pub fn stderr_contains(&self, text: &str) -> bool {
        self.stderr.contains(text)
    }
}

/// Test harness for CLI commands
pub struct CliTest {
    temp_dir: TempDir,
    command: Command,
    config_content: Option<String>,
    env_vars: Vec<(String, String)>,
}

impl CliTest {
    /// Create a new CLI test with temporary directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Initialize git repo in temp dir for testing worktree commands
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to initialize git repo");

        // Add initial commit to avoid empty repo issues
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .ok();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .ok();

        let readme = temp_dir.path().join("README.md");
        std::fs::write(&readme, "# Test Project\n").ok();

        Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .ok();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .ok();

        // Use the compiled binary directly, not cargo run
        // This allows tests to work from temp directories
        let binary_path = std::env::current_exe()
            .ok()
            .and_then(|path| {
                // The test binary is in target/debug/deps/
                // The actual binary is in target/debug/
                path.parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.join("prodigy"))
            })
            .filter(|p| p.exists())
            .unwrap_or_else(|| {
                // Fallback to searching for the binary
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("target")
                    .join("debug")
                    .join("prodigy")
            });

        let mut command = Command::new(binary_path);
        command.current_dir(temp_dir.path());

        Self {
            temp_dir,
            command,
            config_content: None,
            env_vars: Vec::new(),
        }
    }

    /// Add a command argument
    pub fn arg(mut self, arg: &str) -> Self {
        self.command.arg(arg);
        self
    }

    /// Set environment variable
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env_vars.push((key.to_string(), value.to_string()));
        self.command.env(key, value);
        self
    }

    /// Set configuration content
    pub fn with_config(mut self, content: &str) -> Self {
        self.config_content = Some(content.to_string());
        self
    }

    /// Create a workflow file and return its path
    pub fn with_workflow(self, name: &str, content: &str) -> (Self, PathBuf) {
        let workflow_path = self.temp_dir.path().join(format!("{}.yaml", name));
        std::fs::write(&workflow_path, content).expect("Failed to write workflow");
        (self, workflow_path)
    }

    /// Get path to temp directory
    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Run the command and return output
    pub fn run(&mut self) -> CliOutput {
        // Write config if provided
        if let Some(ref config) = self.config_content {
            let config_dir = self.temp_dir.path().join(".prodigy");
            std::fs::create_dir_all(&config_dir).ok();
            let config_path = config_dir.join("config.yaml");
            std::fs::write(&config_path, config).expect("Failed to write config");
        }

        // Set PRODIGY_AUTOMATION to avoid interactive prompts in tests
        self.command.env("PRODIGY_AUTOMATION", "true");

        // Execute command
        let output = self
            .command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to execute command");

        CliOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
        }
    }

    /// Spawn command for signal testing
    pub fn spawn(&mut self) -> std::process::Child {
        self.command.env("PRODIGY_AUTOMATION", "true");
        self.command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn command")
    }
}

// Helper functions

/// Create a simple test workflow
pub fn create_test_workflow(name: &str) -> String {
    format!(
        r#"
name: {}
commands:
  - shell: "echo 'Test workflow {}'"
"#,
        name, name
    )
}

/// Create a workflow with error
#[allow(dead_code)]
pub fn create_failing_workflow(name: &str) -> String {
    format!(
        r#"
name: {}
commands:
  - shell: "exit 1"
"#,
        name
    )
}

/// Create a long-running workflow for interrupt testing
pub fn create_long_workflow(name: &str) -> String {
    format!(
        r#"
name: {}
commands:
  - shell: "sleep 30"
"#,
        name
    )
}

/// Create a mapreduce workflow
pub fn create_mapreduce_workflow(name: &str) -> String {
    format!(
        r#"
name: {}
mode: mapreduce

setup:
  - shell: "echo '[\"item1\", \"item2\"]' > work-items.json"

map:
  input: work-items.json
  json_path: "$[*]"
  agent_template:
    - shell: "echo 'Processing ${{item}}'"

reduce:
  - shell: "echo 'Reduce complete'"
"#,
        name
    )
}

/// Standard exit codes
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const GENERAL_ERROR: i32 = 1;
    pub const ARGUMENT_ERROR: i32 = 2;
    pub const CONFIG_ERROR: i32 = 3;
    pub const INTERRUPTED: i32 = 130;
    pub const TERMINATED: i32 = 143;
}

/// Assert that output matches expectations
pub fn assert_output(
    output: &CliOutput,
    expected_exit: i32,
    stdout_contains: Option<&str>,
    stderr_contains: Option<&str>,
) {
    assert_eq!(
        output.exit_code, expected_exit,
        "Expected exit code {}, got {}. Stdout: {}, Stderr: {}",
        expected_exit, output.exit_code, output.stdout, output.stderr
    );

    if let Some(expected) = stdout_contains {
        assert!(
            output.stdout_contains(expected),
            "Expected stdout to contain '{}', got: {}",
            expected,
            output.stdout
        );
    }

    if let Some(expected) = stderr_contains {
        assert!(
            output.stderr_contains(expected),
            "Expected stderr to contain '{}', got: {}",
            expected,
            output.stderr
        );
    }
}
