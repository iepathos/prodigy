// Integration tests for UnifiedSession file creation and management
// Verifies that workflow execution creates UnifiedSession files and that resume can load them

use super::test_utils::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to setup isolated PRODIGY_HOME for a test
fn setup_test_prodigy_home() -> TempDir {
    TempDir::new().expect("Failed to create temp directory for PRODIGY_HOME")
}

#[test]
fn test_workflow_creates_unified_session_file() {
    // This test verifies that running a workflow creates a UnifiedSession file
    // in ~/.prodigy/sessions/ with the correct structure

    let prodigy_home = setup_test_prodigy_home();
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();

    // Create a simple workflow
    let workflow_content = r#"
name: test-unified-session-workflow
description: Test that workflow creates UnifiedSession

commands:
  - shell: "echo 'Step 1'"
    id: step1
  - shell: "echo 'Step 2'"
    id: step2
"#;

    let workflow_path = test_dir.join("test-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Run the workflow with isolated PRODIGY_HOME
    test = test
        .env("PRODIGY_HOME", prodigy_home.path().to_str().unwrap())
        .arg("run")
        .arg(workflow_path.to_str().unwrap());

    let output = test.run();

    // Workflow should complete successfully
    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Workflow should complete successfully. Stdout: {}\nStderr: {}",
        output.stdout,
        output.stderr
    );

    // Verify UnifiedSession file was created in PRODIGY_HOME/sessions/
    let sessions_dir = prodigy_home.path().join("sessions");
    assert!(
        sessions_dir.exists(),
        "Sessions directory should be created at {:?}",
        sessions_dir
    );

    // Find the session file - there should be exactly one
    let session_files: Vec<PathBuf> = fs::read_dir(&sessions_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();

    assert_eq!(
        session_files.len(),
        1,
        "Expected exactly one session file, found: {:?}",
        session_files
    );

    // Read and verify the session file structure
    let session_file = &session_files[0];
    let session_content = fs::read_to_string(session_file).unwrap();
    let session: serde_json::Value = serde_json::from_str(&session_content).unwrap();

    // Verify required fields
    assert!(session["id"].is_string(), "Session should have id field");
    assert_eq!(
        session["session_type"].as_str(),
        Some("Workflow"),
        "Session type should be Workflow"
    );
    assert!(
        session["status"].is_string(),
        "Session should have status field"
    );
    assert!(
        session["started_at"].is_string(),
        "Session should have started_at timestamp"
    );
    assert!(
        session["workflow_data"].is_object(),
        "Session should have workflow_data"
    );

    // Verify workflow_data structure
    let workflow_data = &session["workflow_data"];
    assert_eq!(
        workflow_data["workflow_name"].as_str(),
        Some("test-unified-session-workflow"),
        "Workflow name should match"
    );
    assert!(
        workflow_data["workflow_id"].is_string(),
        "Workflow data should have workflow_id"
    );
}

#[test]
fn test_unified_session_contains_correct_workflow_id_and_status() {
    // Verifies that UnifiedSession file contains correct workflow_id and status

    let prodigy_home = setup_test_prodigy_home();
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();

    // Create workflow
    let workflow_content = r#"
name: test-status-workflow
description: Test workflow status tracking

commands:
  - shell: "echo 'Command executed'"
"#;

    let workflow_path = test_dir.join("status-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Run the workflow
    test = test
        .env("PRODIGY_HOME", prodigy_home.path().to_str().unwrap())
        .arg("run")
        .arg(workflow_path.to_str().unwrap());

    let output = test.run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Workflow should complete. Stderr: {}",
        output.stderr
    );

    // Find and read the session file
    let sessions_dir = prodigy_home.path().join("sessions");
    let session_files: Vec<PathBuf> = fs::read_dir(&sessions_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();

    assert_eq!(session_files.len(), 1, "Expected one session file");

    let session_content = fs::read_to_string(&session_files[0]).unwrap();
    let session: serde_json::Value = serde_json::from_str(&session_content).unwrap();

    // Verify workflow_id matches session id
    assert_eq!(
        session["id"].as_str(),
        session["workflow_data"]["workflow_id"].as_str(),
        "Session id should match workflow_id"
    );

    // Verify status is valid (should be Completed or Active)
    let status = session["status"].as_str().unwrap();
    assert!(
        status == "Completed" || status == "Active" || status == "Paused",
        "Status should be valid, got: {}",
        status
    );
}

#[test]
fn test_resume_can_load_unified_session() {
    // Integration test that verifies resume can load UnifiedSession files

    let prodigy_home = setup_test_prodigy_home();
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();

    // Create workflow
    let workflow_content = r#"
name: test-resume-workflow
description: Test resume from UnifiedSession

commands:
  - shell: "echo 'Step 1'"
    id: step1
  - shell: "echo 'Step 2'"
    id: step2
  - shell: "echo 'Step 3'"
    id: step3
"#;

    let workflow_path = test_dir.join("test-resume-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create a checkpoint with worktree to simulate interrupted workflow
    let workflow_id = "session-resume-unified-test-123";
    let variables = json!({});

    let _worktree_path = create_test_checkpoint_with_worktree(
        prodigy_home.path(),
        &test_dir,
        workflow_id,
        1, // Completed 1 command
        3, // Total 3 commands
        variables,
    )
    .expect("Failed to create test checkpoint");

    // Verify UnifiedSession file exists
    let session_file = prodigy_home
        .path()
        .join("sessions")
        .join(format!("{}.json", workflow_id));
    assert!(
        session_file.exists(),
        "UnifiedSession file should exist at {:?}",
        session_file
    );

    // Resume the workflow
    test = test
        .env("PRODIGY_HOME", prodigy_home.path().to_str().unwrap())
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    // Should successfully resume and complete
    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Resume should succeed. Stdout: {}\nStderr: {}",
        output.stdout,
        output.stderr
    );

    // Verify resume output indicates successful resumption
    assert!(
        output.stdout_contains("Resuming")
            || output.stdout_contains("Resumed")
            || output.stdout_contains("completed"),
        "Expected resume/completion message. Stdout: {}",
        output.stdout
    );
}

#[test]
fn test_unified_session_tracks_multiple_workflows() {
    // Verifies that multiple workflow executions create separate UnifiedSession files

    let prodigy_home = setup_test_prodigy_home();
    let test_dir = TempDir::new().expect("Failed to create temp dir");

    // Initialize git repo in temp dir
    std::process::Command::new("git")
        .arg("init")
        .current_dir(test_dir.path())
        .output()
        .expect("Failed to init git");

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(test_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(test_dir.path())
        .output()
        .ok();

    let readme = test_dir.path().join("README.md");
    fs::write(&readme, "# Test\n").ok();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(test_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(test_dir.path())
        .output()
        .ok();

    // Create two different workflows
    let workflow1 = test_dir.path().join("workflow1.yaml");
    fs::write(
        &workflow1,
        r#"
name: workflow-one
commands:
  - shell: "echo 'Workflow 1'"
"#,
    )
    .unwrap();

    let workflow2 = test_dir.path().join("workflow2.yaml");
    fs::write(
        &workflow2,
        r#"
name: workflow-two
commands:
  - shell: "echo 'Workflow 2'"
"#,
    )
    .unwrap();

    // Run first workflow
    let mut test1 = CliTest::new();
    test1 = test1
        .env("PRODIGY_HOME", prodigy_home.path().to_str().unwrap())
        .arg("run")
        .arg(workflow1.to_str().unwrap());

    let output1 = test1.run();
    assert_eq!(output1.exit_code, exit_codes::SUCCESS);

    // Run second workflow
    let mut test2 = CliTest::new();
    test2 = test2
        .env("PRODIGY_HOME", prodigy_home.path().to_str().unwrap())
        .arg("run")
        .arg(workflow2.to_str().unwrap());

    let output2 = test2.run();
    assert_eq!(output2.exit_code, exit_codes::SUCCESS);

    // Verify two separate session files exist
    let sessions_dir = prodigy_home.path().join("sessions");
    let session_files: Vec<PathBuf> = fs::read_dir(&sessions_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();

    assert_eq!(
        session_files.len(),
        2,
        "Expected two session files for two workflows"
    );

    // Verify each session has a unique workflow name
    let mut workflow_names = Vec::new();
    for session_file in &session_files {
        let content = fs::read_to_string(session_file).unwrap();
        let session: serde_json::Value = serde_json::from_str(&content).unwrap();
        let name = session["workflow_data"]["workflow_name"]
            .as_str()
            .unwrap()
            .to_string();
        workflow_names.push(name);
    }

    assert!(
        workflow_names.contains(&"workflow-one".to_string()),
        "Should have workflow-one"
    );
    assert!(
        workflow_names.contains(&"workflow-two".to_string()),
        "Should have workflow-two"
    );
}

#[test]
fn test_unified_session_persists_across_interruption() {
    // Verifies that UnifiedSession file persists when workflow is interrupted
    // and can be used for resume

    let prodigy_home = setup_test_prodigy_home();
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();

    // Create a workflow
    let workflow_content = r#"
name: test-resume-workflow
description: Test session persistence

commands:
  - shell: "echo 'Step 1'"
  - shell: "echo 'Step 2'"
  - shell: "echo 'Step 3'"
"#;

    let workflow_path = test_dir.join("test-resume-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Simulate interrupted workflow with checkpoint
    let workflow_id = "session-interrupted-persist-456";
    let _worktree_path = create_test_checkpoint_with_worktree(
        prodigy_home.path(),
        &test_dir,
        workflow_id,
        1, // Interrupted after 1 command
        3,
        json!({}),
    )
    .expect("Failed to create checkpoint");

    // Verify session file was created and has Paused status
    let session_file = prodigy_home
        .path()
        .join("sessions")
        .join(format!("{}.json", workflow_id));
    assert!(session_file.exists(), "Session file should exist");

    let session_content = fs::read_to_string(&session_file).unwrap();
    let session: serde_json::Value = serde_json::from_str(&session_content).unwrap();

    assert_eq!(
        session["status"].as_str(),
        Some("Paused"),
        "Interrupted session should have Paused status"
    );

    // Now resume and verify it completes
    test = test
        .env("PRODIGY_HOME", prodigy_home.path().to_str().unwrap())
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Resume should complete successfully"
    );
}

#[test]
fn test_checkpoint_and_session_state_consistency() {
    // This test verifies that session state remains consistent with checkpoint state
    // throughout execution (Acceptance Criteria 10 from spec 154)

    let prodigy_home = setup_test_prodigy_home();
    let mut test = CliTest::new();
    let test_dir = test.temp_path().to_path_buf();

    // Create a workflow
    let workflow_content = r#"
name: test-resume-workflow
description: Test checkpoint consistency

commands:
  - shell: "echo 'Step 1'"
  - shell: "echo 'Step 2'"
  - shell: "echo 'Step 3'"
"#;

    let workflow_path = test_dir.join("test-resume-workflow.yaml");
    fs::write(&workflow_path, workflow_content).unwrap();

    // Create checkpoint and UnifiedSession
    let workflow_id = "session-consistency-test-789";
    let _worktree_path = create_test_checkpoint_with_worktree(
        prodigy_home.path(),
        &test_dir,
        workflow_id,
        2, // Completed 2 steps
        3,
        json!({}),
    )
    .expect("Failed to create checkpoint");

    // Load the checkpoint file
    let checkpoint_file = prodigy_home
        .path()
        .join("state")
        .join(workflow_id)
        .join("checkpoints")
        .join(format!("{}.checkpoint.json", workflow_id));
    let checkpoint_content = fs::read_to_string(&checkpoint_file).unwrap();
    let checkpoint: serde_json::Value = serde_json::from_str(&checkpoint_content).unwrap();

    // Load the UnifiedSession file
    let session_file = prodigy_home
        .path()
        .join("sessions")
        .join(format!("{}.json", workflow_id));
    let session_content = fs::read_to_string(&session_file).unwrap();
    let session: serde_json::Value = serde_json::from_str(&session_content).unwrap();

    // Verify consistency between checkpoint and session
    // 1. Session ID should match workflow ID in checkpoint
    assert_eq!(
        session["id"].as_str(),
        checkpoint["workflow_id"].as_str(),
        "Session ID should match checkpoint workflow_id"
    );

    // 2. Current step should be consistent
    assert_eq!(
        checkpoint["execution_state"]["current_step_index"],
        session["workflow_data"]["current_step"],
        "Current step should match between checkpoint and session"
    );

    // 3. Total steps should be consistent
    assert_eq!(
        checkpoint["execution_state"]["total_steps"], session["workflow_data"]["total_steps"],
        "Total steps should match between checkpoint and session"
    );

    // 4. Session status should reflect checkpoint state
    // Checkpoint status "Interrupted" should map to "Paused" in UnifiedSession
    assert_eq!(
        checkpoint["execution_state"]["status"].as_str(),
        Some("Interrupted"),
        "Checkpoint should have Interrupted status"
    );
    assert_eq!(
        session["status"].as_str(),
        Some("Paused"),
        "Session should have Paused status for interrupted checkpoint"
    );

    // 5. Timing should be present
    assert!(
        session["started_at"].is_string(),
        "Session should have started_at timestamp"
    );

    // Now resume and verify consistency is maintained after completion
    test = test
        .env("PRODIGY_HOME", prodigy_home.path().to_str().unwrap())
        .arg("resume")
        .arg(workflow_id)
        .arg("--path")
        .arg(test_dir.to_str().unwrap());

    let output = test.run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "Resume should complete successfully"
    );

    // After completion, verify final state
    let final_session_content = fs::read_to_string(&session_file).unwrap();
    let final_session: serde_json::Value = serde_json::from_str(&final_session_content).unwrap();

    // Session should now be completed
    assert!(
        final_session["status"].as_str() == Some("Completed")
            || final_session["status"].as_str() == Some("Active"),
        "Session should be Completed or Active after successful resume. Got: {:?}",
        final_session["status"]
    );

    // Should have completed_at timestamp
    if final_session["status"].as_str() == Some("Completed") {
        assert!(
            final_session["completed_at"].is_string(),
            "Completed session should have completed_at timestamp"
        );
    }
}

#[test]
fn test_checkpoint_consistency_during_execution() {
    // Test that creates a checkpoint during execution and verifies
    // that the corresponding UnifiedSession has matching state

    let prodigy_home = setup_test_prodigy_home();
    let test_dir = TempDir::new().expect("Failed to create temp dir");

    // Initialize git repo
    std::process::Command::new("git")
        .arg("init")
        .current_dir(test_dir.path())
        .output()
        .expect("Failed to init git");

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(test_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(test_dir.path())
        .output()
        .ok();

    let readme = test_dir.path().join("README.md");
    fs::write(&readme, "# Test\n").ok();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(test_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(test_dir.path())
        .output()
        .ok();

    // Create multiple checkpoints at different execution points
    let workflow_id = "session-multi-checkpoint-999";

    // First checkpoint after 1 step
    let _worktree_path_1 = create_test_checkpoint_with_worktree(
        prodigy_home.path(),
        test_dir.path(),
        &format!("{}-step1", workflow_id),
        1,
        5,
        json!({"step": 1}),
    )
    .expect("Failed to create first checkpoint");

    // Verify first checkpoint state
    let session_file_1 = prodigy_home
        .path()
        .join("sessions")
        .join(format!("{}-step1.json", workflow_id));
    assert!(session_file_1.exists(), "First session file should exist");

    let session_1: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&session_file_1).unwrap()).unwrap();
    assert_eq!(
        session_1["workflow_data"]["current_step"], 1,
        "First checkpoint should show 1 step completed"
    );

    // Second checkpoint after 3 steps
    let _worktree_path_2 = create_test_checkpoint_with_worktree(
        prodigy_home.path(),
        test_dir.path(),
        &format!("{}-step3", workflow_id),
        3,
        5,
        json!({"step": 3}),
    )
    .expect("Failed to create second checkpoint");

    // Verify second checkpoint state
    let session_file_2 = prodigy_home
        .path()
        .join("sessions")
        .join(format!("{}-step3.json", workflow_id));
    assert!(session_file_2.exists(), "Second session file should exist");

    let session_2: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&session_file_2).unwrap()).unwrap();
    assert_eq!(
        session_2["workflow_data"]["current_step"], 3,
        "Second checkpoint should show 3 steps completed"
    );

    // Both sessions should be independent with their own IDs
    assert_ne!(
        session_1["id"], session_2["id"],
        "Each session should have unique ID"
    );
}
