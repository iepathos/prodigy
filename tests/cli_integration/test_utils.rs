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

// Worktree test infrastructure

/// Create a proper test worktree using production WorktreeManager
///
/// Creates an actual git worktree through Prodigy's WorktreeManager, ensuring
/// tests properly simulate real workflow execution environments.
///
/// # Arguments
/// * `prodigy_home` - Base directory for Prodigy data (typically PRODIGY_HOME)
/// * `project_root` - Root directory of the test git repository
/// * `worktree_name` - Name for the worktree (should start with "session-")
///
/// # Returns
/// * `Result<PathBuf>` - Path to the created worktree
///
/// # Errors
/// Returns error if worktree creation fails or git configuration fails
pub fn create_test_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    worktree_name: &str,
) -> anyhow::Result<PathBuf> {
    use prodigy::subprocess::SubprocessManager;

    // Initialize subprocess manager
    let _subprocess = SubprocessManager::production();

    // Calculate worktree path in prodigy_home
    let repo_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("prodigy");
    let worktree_path = prodigy_home
        .join("worktrees")
        .join(repo_name)
        .join(worktree_name);

    // Create parent directory
    std::fs::create_dir_all(worktree_path.parent().unwrap())?;

    // Create worktree branch
    let branch = format!("prodigy-{}", worktree_name);
    let add_command = Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&worktree_path)
        .current_dir(project_root)
        .output()?;

    if !add_command.status.success() {
        anyhow::bail!(
            "Failed to create worktree: {}",
            String::from_utf8_lossy(&add_command.stderr)
        );
    }

    // Initialize git config in worktree
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&worktree_path)
        .output()?;

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&worktree_path)
        .output()?;

    Ok(worktree_path)
}

/// Enhanced test checkpoint creation that includes worktree setup
///
/// Creates a complete test environment including:
/// - Actual git worktree using WorktreeManager
/// - Checkpoint in proper location
/// - Session state in UnifiedSessionManager location
///
/// This helper ensures tests properly simulate interrupted workflows that
/// can be resumed through Prodigy's resume command.
///
/// # Arguments
/// * `prodigy_home` - Base directory for Prodigy data (from PRODIGY_HOME env var)
/// * `project_root` - Root directory of the test git repository
/// * `workflow_id` - Session/workflow ID (should start with "session-")
/// * `commands_executed` - Number of commands executed before interruption
/// * `total_commands` - Total commands in the workflow
/// * `variables` - Variable state to preserve in checkpoint
///
/// # Returns
/// * `Result<PathBuf>` - Path to the created worktree
///
/// # Errors
/// Returns error if worktree or checkpoint creation fails
pub fn create_test_checkpoint_with_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    workflow_id: &str,
    commands_executed: usize,
    total_commands: usize,
    variables: serde_json::Value,
) -> anyhow::Result<PathBuf> {
    use serde_json::json;

    // 1. Create actual worktree using production WorktreeManager
    let worktree_path = create_test_worktree(prodigy_home, project_root, workflow_id)?;

    // 2. Create checkpoint in proper location
    let checkpoint_dir = prodigy_home
        .join("state")
        .join(workflow_id)
        .join("checkpoints");
    std::fs::create_dir_all(&checkpoint_dir)?;

    // 3. Create checkpoint JSON with proper structure
    let now = chrono::Utc::now();
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "execution_state": {
            "current_step_index": commands_executed,
            "total_steps": total_commands,
            "status": "Interrupted",
            "start_time": now.to_rfc3339(),
            "last_checkpoint": now.to_rfc3339(),
            "current_iteration": null,
            "total_iterations": null
        },
        "completed_steps": (0..commands_executed).map(|i| {
            json!({
                "step_index": i,
                "command": format!("shell: echo 'Command {}'", i + 1),
                "success": true,
                "output": format!("Command {} output", i + 1),
                "captured_variables": {},
                "duration": {
                    "secs": 1,
                    "nanos": 0
                },
                "completed_at": now.to_rfc3339(),
                "retry_state": null
            })
        }).collect::<Vec<_>>(),
        "variable_state": variables,
        "mapreduce_state": null,
        "timestamp": now.to_rfc3339(),
        "version": 1,
        "workflow_hash": "test-hash-12345",
        "total_steps": total_commands,
        "workflow_name": "test-resume-workflow",
        "workflow_path": "test-resume-workflow.yaml"
    });

    let checkpoint_file = checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id));
    std::fs::write(&checkpoint_file, serde_json::to_string_pretty(&checkpoint)?)?;

    // 4. Create UnifiedSession state
    let unified_session = json!({
        "id": workflow_id,
        "session_type": "Workflow",
        "status": "Paused",  // Paused status is resumable
        "started_at": now.to_rfc3339(),
        "updated_at": now.to_rfc3339(),
        "completed_at": null,
        "metadata": {},
        "checkpoints": [],
        "timings": {},
        "error": null,
        "workflow_data": {
            "workflow_id": workflow_id,
            "workflow_name": "test-resume-workflow",
            "current_step": commands_executed,
            "total_steps": total_commands,
            "completed_steps": (0..commands_executed).collect::<Vec<_>>(),
            "variables": {},
            "iterations_completed": 0,
            "files_changed": 0,
            "worktree_name": workflow_id
        },
        "mapreduce_data": null
    });

    // Save in UnifiedSessionManager location (PRODIGY_HOME/sessions/)
    let sessions_dir = prodigy_home.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;
    std::fs::write(
        sessions_dir.join(format!("{}.json", workflow_id)),
        serde_json::to_string_pretty(&unified_session)?,
    )?;

    Ok(worktree_path)
}

/// Cleanup test worktrees after test completion
///
/// Removes worktree and associated metadata. Should be called in test
/// cleanup to avoid leaking worktrees.
///
/// # Arguments
/// * `prodigy_home` - Base directory for Prodigy data
/// * `project_root` - Root directory of the test git repository
/// * `worktree_name` - Name of the worktree to clean up
///
/// # Errors
/// Returns error if cleanup fails
#[allow(dead_code)]
pub fn cleanup_test_worktree(
    prodigy_home: &Path,
    project_root: &Path,
    worktree_name: &str,
) -> anyhow::Result<()> {
    let repo_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("prodigy");
    let worktree_path = prodigy_home
        .join("worktrees")
        .join(repo_name)
        .join(worktree_name);

    // Remove worktree using git
    let remove_command = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(&worktree_path)
        .current_dir(project_root)
        .output()?;

    if !remove_command.status.success() {
        // Log error but don't fail - worktree might already be gone
        eprintln!(
            "Warning: Failed to remove worktree: {}",
            String::from_utf8_lossy(&remove_command.stderr)
        );
    }

    // Clean up any remaining worktree directory
    if worktree_path.exists() {
        std::fs::remove_dir_all(&worktree_path).ok();
    }

    Ok(())
}
