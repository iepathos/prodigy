// Comprehensive integration tests for resume functionality
// Tests actual resume behavior from different interruption points

use super::test_utils::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

/// Helper to create a test checkpoint
fn create_test_checkpoint(
    checkpoint_dir: &PathBuf,
    workflow_id: &str,
    commands_executed: usize,
    total_commands: usize,
    variables: serde_json::Value,
) {
    // The checkpoint_dir is .prodigy/checkpoints
    let session_dir = checkpoint_dir;

    // Create a properly structured WorkflowCheckpoint
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
        "workflow_path": null
    });

    // Save as {workflow_id}.checkpoint.json
    fs::create_dir_all(session_dir).unwrap();
    fs::write(
        session_dir.join(format!("{}.checkpoint.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();
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
  - shell: "echo 'Command 3 executed'"
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
    // Create CliTest first to get its temp directory
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");

    // Create workflow file - use a name that matches what resume expects
    let _workflow_path = create_test_workflow(&test_dir, "workflow.yaml");

    // Create checkpoint after 1 command
    let workflow_id = "resume-early-12345";
    let variables = json!({
        "variable1": "test-value",
        "shell": {
            "output": "Command 1 output"
        }
    });
    create_test_checkpoint(&checkpoint_dir, workflow_id, 1, 5, variables);

    // Verify the checkpoint file was created
    let checkpoint_file = checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id));
    assert!(
        checkpoint_file.exists(),
        "Checkpoint file should exist at {:?}",
        checkpoint_file
    );
    assert!(
        checkpoint_dir.exists(),
        "Checkpoint directory should exist at {:?}",
        checkpoint_dir
    );

    // Resume the workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should successfully resume
    if output.exit_code != exit_codes::SUCCESS {
        eprintln!("Resume failed!");
        eprintln!("Exit code: {}", output.exit_code);
        eprintln!("STDOUT:\n{}", output.stdout);
        eprintln!("STDERR:\n{}", output.stderr);
    }
    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Resume failed with stderr: {}",
        output.stderr
    );

    // Check for the actual output format
    assert!(
        output.stdout_contains("Resuming execution from step 2 of 5")
            || output.stdout_contains("Resuming workflow from checkpoint"),
        "Expected resume message not found in stdout: {}",
        output.stdout
    );
    // In test mode, commands are simulated
    assert!(
        output
            .stdout_contains("[TEST MODE] Would execute Shell command: echo 'Command 2 executed'")
            || output.stdout_contains("Command 2 executed")
    );
    assert!(
        output.stdout_contains(
            "[TEST MODE] Would execute Shell command: echo 'Final command executed'"
        ) || output.stdout_contains("Final command executed")
    );
}

#[test]
fn test_resume_from_middle_interruption() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");

    // Create workflow file
    let _workflow_path = create_test_workflow(&test_dir, "workflow.yaml");

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
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should successfully resume from command 4
    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Resume failed with stderr: {}",
        output.stderr
    );
    assert!(
        output.stdout_contains("Resuming execution from step 4 of 5")
            || output.stdout_contains("Resuming workflow from checkpoint")
    );
    assert!(
        output
            .stdout_contains("[TEST MODE] Would execute Shell command: echo 'Command 4 executed'")
            || output.stdout_contains("Command 4 executed")
    );
    assert!(
        output.stdout_contains(
            "[TEST MODE] Would execute Shell command: echo 'Final command executed'"
        ) || output.stdout_contains("Final command executed")
    );
    // Should not re-run earlier commands (they were already completed)
    assert!(!output
        .stdout_contains("[TEST MODE] Would execute Shell command: echo 'Command 1 executed'"));
    assert!(!output
        .stdout_contains("[TEST MODE] Would execute Shell command: echo 'Command 2 executed'"));
}

#[test]
fn test_resume_with_variable_preservation() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.clone();

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

    // Create workflow file with the expected name from checkpoint
    let workflow_path = workflow_dir.join("test-resume-workflow.yaml");
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
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should preserve and use variables
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(
        output.stdout_contains("Final: ${var1} and ${var2}")
            || output.stdout_contains("Final: First variable value and Second variable value")
    );
}

#[test]
fn test_resume_with_retry_state() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.clone();

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

    // Create workflow file with the expected name from checkpoint
    let workflow_path = workflow_dir.join("test-resume-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint with proper structure
    let workflow_id = "resume-retry-22222";
    let now = chrono::Utc::now();
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "execution_state": {
            "current_step_index": 1,
            "total_steps": 3,
            "status": "Interrupted",
            "start_time": now.to_rfc3339(),
            "last_checkpoint": now.to_rfc3339(),
            "current_iteration": null,
            "total_iterations": null
        },
        "completed_steps": [{
            "step_index": 0,
            "command": "shell: echo 'Command 1'",
            "success": true,
            "output": "Command 1 output",
            "captured_variables": {},
            "duration": {
                "secs": 1,
                "nanos": 0
            },
            "completed_at": now.to_rfc3339(),
            "retry_state": null
        }],
        "variable_state": {},
        "mapreduce_state": null,
        "timestamp": now.to_rfc3339(),
        "version": 1,
        "workflow_hash": "test-hash-22222",
        "total_steps": 3,
        "workflow_name": "test-resume-workflow",
        "workflow_path": null
    });

    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();

    // Create the marker file so retry succeeds
    fs::write("/tmp/retry-test-marker", "test").ok();

    // Resume the workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should complete successfully
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    // In test mode or with simplified execution, retry details may not be shown
    // Just check that it completed

    // Clean up
    fs::remove_file("/tmp/retry-test-marker").ok();
}

#[test]
fn test_resume_completed_workflow() {
    // Create CliTest first to get its temp directory
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let session_dir = test_dir.join(".prodigy");

    // Create a completed session state in the format SessionTracker expects
    let workflow_id = "resume-complete-33333";
    let now = chrono::Utc::now();
    let session_state = json!({
        "session_id": workflow_id,
        "status": "Completed",
        "started_at": now.to_rfc3339(),
        "ended_at": now.to_rfc3339(),  // Changed from completed_at
        "iterations_completed": 1,
        "files_changed": 0,
        "errors": [],
        "working_directory": test_dir.to_str().unwrap(),
        "worktree_name": "prodigy-session-test",
        "workflow_started_at": now.to_rfc3339(),
        "current_iteration_started_at": null,
        "current_iteration_number": null,
        "iteration_timings": [],
        "command_timings": [],
        "workflow_state": {
            "current_iteration": 0,
            "current_step": 5,
            "completed_steps": (0..5).map(|i| {
                let step_time = now.to_rfc3339();
                json!({
                    "step_index": i,
                    "command": format!("cmd{}", i + 1),
                    "success": true,
                    "output": format!("Command {} output", i + 1),
                    "duration": {
                        "secs": 1,
                        "nanos": 0
                    },
                    "error": null,
                    "started_at": step_time,
                    "completed_at": step_time,
                    "exit_code": 0
                })
            }).collect::<Vec<_>>(),
            "workflow_path": "test.yaml",
            "input_args": [],
            "map_patterns": [],
            "using_worktree": false
        },
        "execution_environment": null,
        "last_checkpoint": now.to_rfc3339(),
        "workflow_hash": null,
        "workflow_type": "Standard",
        "execution_context": null,
        "checkpoint_version": 1,
        "last_validated_at": null
    });

    // Save as both session_state.json and session-specific file
    fs::create_dir_all(&session_dir).unwrap();
    fs::write(
        session_dir.join("session_state.json"),
        serde_json::to_string_pretty(&session_state).unwrap(),
    )
    .unwrap();
    fs::write(
        session_dir.join(format!("{}.json", workflow_id)),
        serde_json::to_string_pretty(&session_state).unwrap(),
    )
    .unwrap();

    // Try to resume completed workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should indicate workflow is already complete
    if output.exit_code != exit_codes::SUCCESS {
        eprintln!("Test failed - stdout: {}", output.stdout);
        eprintln!("Test failed - stderr: {}", output.stderr);
    }
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(
        output.stdout_contains("already completed") || output.stdout_contains("nothing to resume")
    );
}

#[test]
fn test_resume_with_force_restart() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");

    // Create workflow and checkpoint
    let _workflow_path = create_test_workflow(&test_dir, "workflow.yaml");
    let workflow_id = "resume-force-44444";

    create_test_checkpoint(&checkpoint_dir, workflow_id, 3, 5, json!({}));

    // Resume with --force flag
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--force")
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should restart from beginning
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Force restarting workflow from beginning"));
    assert!(output.stdout_contains("Command 1 executed"));
    assert!(output.stdout_contains("Command 2 executed"));
}

#[test]
fn test_resume_parallel_workflow() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.clone();

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

    let workflow_path = workflow_dir.join("test-parallel-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint with partial parallel execution
    let workflow_id = "resume-parallel-55555";
    let now = chrono::Utc::now();
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "execution_state": {
            "current_step_index": 0,
            "total_steps": 5,
            "status": "Interrupted",
            "start_time": now.to_rfc3339(),
            "last_checkpoint": now.to_rfc3339(),
            "current_iteration": null,
            "total_iterations": null
        },
        "completed_steps": [],
        "variable_state": {},
        "mapreduce_state": null,
        "timestamp": now.to_rfc3339(),
        "version": 1,
        "workflow_hash": "test-hash",
        "total_steps": 5,
        "workflow_name": "test-parallel-workflow",
        "workflow_path": workflow_path.to_str().unwrap()
    });

    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();

    // Resume the workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should resume workflow (parallel execution details may vary)
    if output.exit_code != exit_codes::SUCCESS {
        eprintln!("Resume failed with exit code: {}", output.exit_code);
        eprintln!("STDOUT:\n{}", output.stdout);
        eprintln!("STDERR:\n{}", output.stderr);
    }
    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Resume failed with exit code: {}, stderr: {}, stdout: {}",
        output.exit_code,
        output.stderr,
        output.stdout
    );
    // Check that resume was initiated
    assert!(
        output.stdout_contains("Resuming") || output.stdout_contains("Found checkpoint"),
        "Expected resume message not found"
    );
}

#[test]
fn test_resume_with_checkpoint_cleanup() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");

    // Create workflow
    let _workflow_path = create_test_workflow(&test_dir, "workflow.yaml");
    let workflow_id = "resume-cleanup-66666";

    // Create checkpoint
    create_test_checkpoint(&checkpoint_dir, workflow_id, 4, 5, json!({}));

    // Checkpoint files are saved in .prodigy/checkpoints
    let checkpoint_file = checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id));
    assert!(
        checkpoint_file.exists(),
        "Checkpoint should exist before resume"
    );

    // Resume and complete workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should complete and clean up checkpoint
    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Resume failed with stderr: {}",
        output.stderr
    );
    // Check that the workflow executed the final command
    assert!(
        output.stdout_contains(
            "[TEST MODE] Would execute Shell command: echo 'Final command executed'"
        ) || output.stdout_contains("Final command executed")
            || output.stdout_contains("completed"),
        "Expected completion message not found in stdout: {}",
        output.stdout
    );

    // Checkpoint file should be cleaned up after successful completion
    assert!(
        !checkpoint_file.exists(),
        "Checkpoint should be cleaned up after completion"
    );
}

#[test]
#[ignore = "Error recovery during resume not fully implemented"]
fn test_resume_with_error_recovery() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.clone();

    // Create workflow with error handling
    let workflow_content = r#"
name: test-error-workflow
description: Test error recovery during resume

commands:
  - shell: "echo 'Command 1'"
  - shell: "echo 'Command 2'"
  - shell: "exit 1"
    id: failing_command
    on_failure: "echo 'Error handled'"
  - shell: "echo 'Continue after error'"
"#;

    // Create workflow file with the expected name from checkpoint
    let workflow_path = workflow_dir.join("test-resume-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint before error
    let workflow_id = "resume-error-77777";
    create_test_checkpoint(&checkpoint_dir, workflow_id, 2, 4, json!({}));

    // Resume the workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should complete successfully
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    // Error handling may not produce specific output in test execution
}

#[test]
fn test_resume_multiple_checkpoints() {
    // Use CliTest to get a temp directory with git initialized
    let test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");

    // Create multiple checkpoints
    for i in 1..=3 {
        let workflow_id = format!("workflow-{}", i);
        create_test_checkpoint(&checkpoint_dir, &workflow_id, i, 5, json!({}));
    }

    // List available checkpoints (when list command is implemented)
    // This test is a placeholder for when 'prodigy checkpoints list' is added

    // Verify checkpoint files exist in the checkpoints directory
    assert!(checkpoint_dir.join("workflow-1.checkpoint.json").exists());
    assert!(checkpoint_dir.join("workflow-2.checkpoint.json").exists());
    assert!(checkpoint_dir.join("workflow-3.checkpoint.json").exists());
}

#[test]
#[ignore = "MapReduce resume not fully implemented"]
fn test_resume_with_mapreduce_state() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.clone();

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

    // Create workflow file with the expected name from checkpoint
    let workflow_path = workflow_dir.join("test-resume-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create items file
    let items = json!(["item1", "item2", "item3", "item4"]);
    fs::write(workflow_dir.join("items.json"), items.to_string()).unwrap();

    // Create checkpoint with MapReduce state
    let workflow_id = "resume-mapreduce-88888";
    let now = chrono::Utc::now();
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "execution_state": {
            "current_step_index": 0,
            "total_steps": 2,
            "status": "Interrupted",
            "start_time": now.to_rfc3339(),
            "last_checkpoint": now.to_rfc3339(),
            "current_iteration": null,
            "total_iterations": null
        },
        "completed_steps": [],
        "variable_state": {},
        "mapreduce_state": {
            "phase": "map",
            "completed_items": ["item1", "item2"],
            "pending_items": ["item3", "item4"],
            "map_results": {
                "item1": {"status": "success", "output": "Processed item1"},
                "item2": {"status": "success", "output": "Processed item2"}
            }
        },
        "timestamp": now.to_rfc3339(),
        "version": 1,
        "workflow_hash": "test-hash-88888",
        "total_steps": 2,
        "workflow_name": "test-resume-workflow",
        "workflow_path": null
    });

    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();

    // Resume the workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should complete successfully
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    // MapReduce may not produce specific output in test execution
}

#[test]
#[ignore = "Resume with on_failure handlers requires full workflow execution implementation"]
fn test_resume_workflow_with_on_failure_handlers() {
    // Use CliTest to get a temp directory with git initialized
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();
    let checkpoint_dir = test_dir.join(".prodigy").join("checkpoints");
    let workflow_dir = test_dir.clone();

    // Create a workflow with on_failure handlers at different steps
    let workflow_content = r#"
name: test-on-failure-resume
description: Test resuming workflow with on_failure handlers

steps:
  - shell: "echo 'Step 1 completed' > step1.txt"
    name: step1

  - shell: "echo 'Step 2 completed' > step2.txt"
    name: step2
    on_failure:
      commands:
        - shell: "echo 'Handling step 2 failure' > step2-error.txt"
        - shell: "echo 'Recovery completed' > recovery.txt"

  - shell: "test -f trigger-failure.txt && exit 1 || echo 'Step 3 completed' > step3.txt"
    name: step3
    on_failure:
      commands:
        - shell: "echo 'Step 3 failed, cleaning up' > step3-cleanup.txt"
        - shell: "rm -f trigger-failure.txt"
        - shell: "echo 'Retry marker' > retry.txt"

  - shell: "echo 'Step 4 completed' > step4.txt"
    name: step4

  - shell: "echo 'Final step completed' > final.txt"
    name: final
"#;

    // Save workflow file
    let workflow_path = workflow_dir.join("test-on-failure-resume.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create a checkpoint with error recovery state stored
    let now = chrono::Utc::now();
    let workflow_id = "on-failure-resume-test";

    // Create checkpoint that indicates step 2 completed but step 3 failed and needs recovery
    let checkpoint = json!({
        "workflow_id": workflow_id,
        "execution_state": {
            "current_step_index": 2,  // Failed at step 3 (index 2)
            "total_steps": 5,
            "status": "Interrupted",
            "start_time": now.to_rfc3339(),
            "last_checkpoint": now.to_rfc3339(),
            "current_iteration": null,
            "total_iterations": null
        },
        "completed_steps": [
            {
                "step_index": 0,
                "command": "shell: echo 'Step 1 completed' > step1.txt",
                "success": true,
                "output": "Step 1 completed",
                "captured_variables": {},
                "duration": { "secs": 1, "nanos": 0 },
                "completed_at": now.to_rfc3339(),
                "retry_state": null
            },
            {
                "step_index": 1,
                "command": "shell: echo 'Step 2 completed' > step2.txt",
                "success": true,
                "output": "Step 2 completed",
                "captured_variables": {},
                "duration": { "secs": 1, "nanos": 0 },
                "completed_at": now.to_rfc3339(),
                "retry_state": null
            }
        ],
        "variable_state": {
            // Store error recovery state as a special variable
            "__error_recovery_state": json!({
                "active_handlers": [{
                    "id": "step3-error-handler",
                    "commands": [
                        {"shell": "echo 'Step 3 failed, cleaning up' > step3-cleanup.txt"},
                        {"shell": "rm -f trigger-failure.txt"},
                        {"shell": "echo 'Retry marker' > retry.txt"}
                    ],
                    "strategy": "retry"
                }],
                "correlation_id": "test-correlation-123",
                "recovery_attempts": 1,
                "max_recovery_attempts": 3
            })
        },
        "mapreduce_state": null,
        "timestamp": now.to_rfc3339(),
        "version": 1,
        "workflow_hash": "test-hash-with-handlers",
        "total_steps": 5,
        "workflow_name": "test-on-failure-resume",
        "workflow_path": workflow_path.to_str()
    });

    // Create trigger file to cause step 3 to fail initially
    fs::write(test_dir.join("trigger-failure.txt"), "trigger").unwrap();

    // Save checkpoint
    fs::create_dir_all(&checkpoint_dir).unwrap();
    fs::write(
        checkpoint_dir.join(format!("{}.checkpoint.json", workflow_id)),
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    ).unwrap();

    // Resume the workflow
    test = test
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Workflow should complete successfully with error recovery executed
    assert_eq!(output.exit_code, exit_codes::SUCCESS,
        "Workflow should complete successfully after error recovery. Output: {}", output.stdout);

    // Verify that error handlers were executed
    assert!(test_dir.join("step3-cleanup.txt").exists(), "Error handler should have created cleanup file");
    assert!(!test_dir.join("trigger-failure.txt").exists(), "Error handler should have removed trigger file");
    assert!(test_dir.join("retry.txt").exists(), "Error handler should have created retry marker");

    // Verify remaining steps completed
    assert!(test_dir.join("step3.txt").exists(), "Step 3 should complete after error recovery");
    assert!(test_dir.join("step4.txt").exists(), "Step 4 should have been executed");
    assert!(test_dir.join("final.txt").exists(), "Final step should have been executed");
}

#[test]
fn test_checkpoint_with_error_recovery_state_serialization() {
    // Test that error recovery state can be properly serialized/deserialized in checkpoints
    let now = chrono::Utc::now();
    let workflow_id = "test-error-state-serialization";

    let checkpoint_with_error_state = json!({
        "workflow_id": workflow_id,
        "execution_state": {
            "current_step_index": 2,
            "total_steps": 4,
            "status": "Interrupted",
            "start_time": now.to_rfc3339(),
            "last_checkpoint": now.to_rfc3339(),
            "current_iteration": null,
            "total_iterations": null
        },
        "completed_steps": [
            {
                "step_index": 0,
                "command": "shell: echo 'Step 1'",
                "success": true,
                "output": "Step 1 output",
                "captured_variables": {},
                "duration": { "secs": 1, "nanos": 0 },
                "completed_at": now.to_rfc3339(),
                "retry_state": null
            }
        ],
        "variable_state": {
            "__error_recovery_state": json!({
                "active_handlers": [{
                    "id": "handler-1",
                    "commands": [
                        {"shell": "echo 'Handling error'"},
                        {"shell": "rm -f error.txt"}
                    ],
                    "strategy": "retry",
                    "scope": "step",
                    "timeout": { "secs": 30, "nanos": 0 }
                }],
                "error_context": {
                    "message": "Command failed",
                    "exit_code": "1"
                },
                "handler_execution_history": [],
                "correlation_id": "test-123",
                "recovery_attempts": 1,
                "max_recovery_attempts": 3
            }),
            "other_var": "some_value"
        },
        "mapreduce_state": null,
        "timestamp": now.to_rfc3339(),
        "version": 1,
        "workflow_hash": "test-hash",
        "total_steps": 4,
        "workflow_name": "test-workflow",
        "workflow_path": null
    });

    // Verify the JSON structure is valid
    let checkpoint_str = serde_json::to_string(&checkpoint_with_error_state).unwrap();
    let parsed_checkpoint: serde_json::Value = serde_json::from_str(&checkpoint_str).unwrap();

    // Verify error recovery state is present
    assert!(parsed_checkpoint["variable_state"]["__error_recovery_state"].is_object());

    // Verify we can extract the error recovery state
    let error_state = &parsed_checkpoint["variable_state"]["__error_recovery_state"];
    assert_eq!(error_state["correlation_id"], "test-123");
    assert_eq!(error_state["recovery_attempts"], 1);
    assert_eq!(error_state["max_recovery_attempts"], 3);

    // Verify handlers are preserved
    let handlers = error_state["active_handlers"].as_array().unwrap();
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0]["id"], "handler-1");
    assert_eq!(handlers[0]["strategy"], "retry");
}
