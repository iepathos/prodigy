// Comprehensive integration tests for resume functionality
// Tests actual resume behavior from different interruption points

use super::test_utils::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use serde_json::json;

/// Helper to create a test checkpoint
fn create_test_checkpoint(
    checkpoint_dir: &PathBuf,
    workflow_id: &str,
    commands_executed: usize,
    total_commands: usize,
    variables: serde_json::Value
) {
    let checkpoint_path = checkpoint_dir.join(format!("{}.json", workflow_id));

    let checkpoint = json!({
        "workflow_id": workflow_id,
        "workflow_path": "test.yaml",
        "commands_executed": commands_executed,
        "total_commands": total_commands,
        "variables": variables,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "interrupted",
        "last_successful_command": commands_executed.saturating_sub(1),
        "retry_state": {
            "attempts": 0,
            "max_attempts": 3
        }
    });

    fs::create_dir_all(checkpoint_dir).unwrap();
    fs::write(checkpoint_path, serde_json::to_string_pretty(&checkpoint).unwrap()).unwrap();
}

/// Helper to create a test workflow file
fn create_test_workflow(workflow_dir: &Path, filename: &str) -> PathBuf {
    let workflow_content = r#"
name: test-resume-workflow
description: Test workflow for resume functionality

commands:
  - shell: "echo 'Command 1 executed'"
    id: cmd1
  - shell: "echo 'Command 2 executed'"
    id: cmd2
  - claude: "/test-command ${variable1}"
    id: cmd3
  - shell: "echo 'Command 4 executed'"
    id: cmd4
  - shell: "echo 'Final command executed'"
    id: cmd5
"#;

    fs::create_dir_all(workflow_dir).unwrap();
    let workflow_path = workflow_dir.join(filename);
    fs::write(&workflow_path, workflow_content).unwrap();
    workflow_path
}

#[test]
fn test_resume_from_early_interruption() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create workflow file
    let _workflow_path = create_test_workflow(&workflow_dir, "test.yaml");

    // Create checkpoint after 1 command
    let workflow_id = "resume-early-12345";
    let variables = json!({
        "variable1": "test-value",
        "shell": {
            "output": "Command 1 output"
        }
    });
    create_test_checkpoint(&checkpoint_dir, workflow_id, 1, 5, variables);

    // Resume the workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should successfully resume
    assert_eq!(output.exit_code, exit_codes::SUCCESS,
               "Resume failed with stderr: {}", output.stderr);
    assert!(output.stdout_contains("Resuming workflow from command 2/5"));
    assert!(output.stdout_contains("Command 2 executed"));
    assert!(output.stdout_contains("Final command executed"));
}

#[test]
fn test_resume_from_middle_interruption() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create workflow file
    let _workflow_path = create_test_workflow(&workflow_dir, "test.yaml");

    // Create checkpoint after 3 commands
    let workflow_id = "resume-middle-67890";
    let variables = json!({
        "variable1": "test-value",
        "shell": {
            "output": "Command 3 output"
        },
        "cmd1_output": "Command 1 completed",
        "cmd2_output": "Command 2 completed"
    });
    create_test_checkpoint(&checkpoint_dir, workflow_id, 3, 5, variables);

    // Resume the workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should successfully resume from command 4
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Resuming workflow from command 4/5"));
    assert!(output.stdout_contains("Command 4 executed"));
    assert!(output.stdout_contains("Final command executed"));
    assert!(!output.stdout_contains("Command 1 executed")); // Should not re-run
    assert!(!output.stdout_contains("Command 2 executed")); // Should not re-run
}

#[test]
fn test_resume_with_variable_preservation() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create a workflow that uses variables
    let workflow_content = r#"
name: test-variable-workflow
description: Test workflow with variables

commands:
  - shell: "echo 'Setting up variables'"
    capture_output: var1
  - shell: "echo 'Using ${var1}'"
    capture_output: var2
  - shell: "echo 'Final: ${var1} and ${var2}'"
"#;

    let workflow_path = workflow_dir.join("variables.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint with variables
    let workflow_id = "resume-vars-11111";
    let variables = json!({
        "var1": "First variable value",
        "var2": "Second variable value",
        "shell": {
            "output": "Previous command output"
        }
    });
    create_test_checkpoint(&checkpoint_dir, workflow_id, 2, 3, variables.clone());

    // Resume the workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should preserve and use variables
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Final: First variable value and Second variable value"));
}

#[test]
fn test_resume_with_retry_state() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create workflow with retry logic
    let workflow_content = r#"
name: test-retry-workflow
description: Test workflow with retries

commands:
  - shell: "echo 'Command 1'"
  - shell: "test -f /tmp/retry-test-marker || exit 1"
    retry: 3
    id: retry_command
  - shell: "echo 'Success after retry'"
"#;

    let workflow_path = workflow_dir.join("retry.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint with retry state
    let workflow_id = "resume-retry-22222";
    let variables = json!({});

    let checkpoint = json!({
        "workflow_id": workflow_id,
        "workflow_path": workflow_path.to_str().unwrap(),
        "commands_executed": 1,
        "total_commands": 3,
        "variables": variables,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "interrupted",
        "current_command_id": "retry_command",
        "retry_state": {
            "attempts": 2,
            "max_attempts": 3,
            "last_error": "Command failed: exit 1"
        }
    });

    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap()
    ).unwrap();

    // Create the marker file so retry succeeds
    fs::write("/tmp/retry-test-marker", "test").ok();

    // Resume the workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should continue with retry state
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Retrying command (attempt 3/3)"));
    assert!(output.stdout_contains("Success after retry"));

    // Clean up
    fs::remove_file("/tmp/retry-test-marker").ok();
}

#[test]
fn test_resume_completed_workflow() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");

    // Create a completed checkpoint
    let workflow_id = "resume-complete-33333";
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "workflow_path": "test.yaml",
        "commands_executed": 5,
        "total_commands": 5,
        "variables": {},
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "completed"
    });

    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap()
    ).unwrap();

    // Try to resume completed workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap());

    let output = test.run();

    // Should indicate workflow is already complete
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("already completed") ||
            output.stdout_contains("nothing to resume"));
}

#[test]
fn test_resume_with_force_restart() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create workflow and checkpoint
    let _workflow_path = create_test_workflow(&workflow_dir, "test.yaml");
    let workflow_id = "resume-force-44444";

    create_test_checkpoint(&checkpoint_dir, workflow_id, 3, 5, json!({}));

    // Resume with --force flag
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--force")
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should restart from beginning
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Force restarting workflow from beginning"));
    assert!(output.stdout_contains("Command 1 executed"));
    assert!(output.stdout_contains("Command 2 executed"));
}

#[test]
fn test_resume_parallel_workflow() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create a parallel workflow
    let workflow_content = r#"
name: test-parallel-workflow
description: Test parallel execution resume

parallel:
  max_workers: 3
  commands:
    - shell: "echo 'Parallel 1'"
      id: p1
    - shell: "echo 'Parallel 2'"
      id: p2
    - shell: "echo 'Parallel 3'"
      id: p3
    - shell: "echo 'Parallel 4'"
      id: p4

commands:
  - shell: "echo 'After parallel'"
"#;

    let workflow_path = workflow_dir.join("parallel.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint with partial parallel execution
    let workflow_id = "resume-parallel-55555";
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "workflow_path": workflow_path.to_str().unwrap(),
        "commands_executed": 0,
        "total_commands": 5,
        "variables": {},
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "interrupted",
        "parallel_state": {
            "completed": ["p1", "p2"],
            "in_progress": ["p3"],
            "pending": ["p4"]
        }
    });

    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap()
    ).unwrap();

    // Resume the workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should resume from pending parallel commands
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Resuming parallel execution"));
    assert!(output.stdout_contains("Parallel 3"));
    assert!(output.stdout_contains("Parallel 4"));
    assert!(!output.stdout_contains("Parallel 1")); // Already completed
    assert!(!output.stdout_contains("Parallel 2")); // Already completed
    assert!(output.stdout_contains("After parallel"));
}

#[test]
fn test_resume_with_checkpoint_cleanup() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create workflow
    let _workflow_path = create_test_workflow(&workflow_dir, "test.yaml");
    let workflow_id = "resume-cleanup-66666";

    // Create checkpoint
    create_test_checkpoint(&checkpoint_dir, workflow_id, 4, 5, json!({}));

    let checkpoint_file = checkpoint_dir.join(format!("{}.json", workflow_id));
    assert!(checkpoint_file.exists(), "Checkpoint should exist before resume");

    // Resume and complete workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should complete and clean up checkpoint
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Final command executed"));
    assert!(output.stdout_contains("Workflow completed successfully"));

    // Checkpoint should be cleaned up after successful completion
    assert!(!checkpoint_file.exists(), "Checkpoint should be cleaned up after completion");
}

#[test]
fn test_resume_with_error_recovery() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create workflow with error handling
    let workflow_content = r#"
name: test-error-workflow
description: Test error recovery during resume

commands:
  - shell: "echo 'Command 1'"
  - shell: "echo 'Command 2'"
  - shell: "exit 1"
    id: failing_command
    on_failure:
      - shell: "echo 'Error handled'"
  - shell: "echo 'Continue after error'"
"#;

    let workflow_path = workflow_dir.join("error.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint before error
    let workflow_id = "resume-error-77777";
    create_test_checkpoint(&checkpoint_dir, workflow_id, 2, 4, json!({}));

    // Resume the workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should handle error and continue
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Error handled"));
    assert!(output.stdout_contains("Continue after error"));
}

#[test]
fn test_resume_multiple_checkpoints() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");

    // Create multiple checkpoints
    for i in 1..=3 {
        let workflow_id = format!("workflow-{}", i);
        create_test_checkpoint(&checkpoint_dir, &workflow_id, i, 5, json!({}));
    }

    // List available checkpoints (when list command is implemented)
    // This test is a placeholder for when 'prodigy checkpoints list' is added

    // For now, verify checkpoints exist
    assert!(checkpoint_dir.join("workflow-1.json").exists());
    assert!(checkpoint_dir.join("workflow-2.json").exists());
    assert!(checkpoint_dir.join("workflow-3.json").exists());
}

#[test]
fn test_resume_with_mapreduce_state() {
    let test_dir = TempDir::new().unwrap();
    let checkpoint_dir = test_dir.path().join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.path();

    // Create MapReduce workflow
    let workflow_content = r#"
name: test-mapreduce-workflow
mode: mapreduce

map:
  input: "items.json"
  agent_template:
    - shell: "echo 'Processing ${item}'"
  max_parallel: 2

reduce:
  - shell: "echo 'Reducing results'"
"#;

    let workflow_path = workflow_dir.join("mapreduce.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create items file
    let items = json!(["item1", "item2", "item3", "item4"]);
    fs::write(workflow_dir.join("items.json"), items.to_string()).unwrap();

    // Create checkpoint with MapReduce state
    let workflow_id = "resume-mapreduce-88888";
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "workflow_path": workflow_path.to_str().unwrap(),
        "commands_executed": 0,
        "total_commands": 0,
        "variables": {},
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "interrupted",
        "mapreduce_state": {
            "phase": "map",
            "completed_items": ["item1", "item2"],
            "pending_items": ["item3", "item4"],
            "map_results": {
                "item1": {"status": "success", "output": "Processed item1"},
                "item2": {"status": "success", "output": "Processed item2"}
            }
        }
    });

    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap()
    ).unwrap();

    // Resume the workflow
    let mut test = CliTest::new()
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap())
        .env("PRODIGY_TEST_MODE", "true");

    let output = test.run();

    // Should resume MapReduce from pending items
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Resuming MapReduce workflow"));
    assert!(output.stdout_contains("Processing item3") || output.stdout_contains("item3"));
    assert!(output.stdout_contains("Processing item4") || output.stdout_contains("item4"));
    assert!(output.stdout_contains("Reducing results"));
}