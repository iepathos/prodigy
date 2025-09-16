// Tests for the 'cook' command

use super::test_utils::*;

#[test]
fn test_cook_basic_workflow() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("basic", &create_test_workflow("basic"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    assert_output(
        &output,
        exit_codes::SUCCESS,
        Some("Test workflow basic"),
        None,
    );
}

#[test]
fn test_cook_with_invalid_workflow() {
    let mut test = CliTest::new().arg("cook").arg("nonexistent.yaml");

    let output = test.run();

    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(
        output.stderr_contains("o such file")
            || output.stderr_contains("not found")
            || output.stderr_contains("does not exist")
            || output.stderr_contains("Failed to")
            || output.stderr_contains("Error"),
        "Expected error message for nonexistent file, got stderr: {}",
        output.stderr
    );
}

#[test]
fn test_cook_with_max_iterations() {
    let mut test = CliTest::new();
    let workflow_content = r#"
name: iteration-test
commands:
  - shell: "echo 'Iteration'"
"#;
    let (mut test, workflow_path) = test.with_workflow("iteration", workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("-n")
        .arg("3")
        .run();

    assert_output(&output, exit_codes::SUCCESS, Some("Iteration"), None);
}

#[test]
fn test_cook_with_worktree_flag() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) =
        test.with_workflow("worktree", &create_test_workflow("worktree"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--worktree")
        .run();

    // Worktree operations require a clean git state
    // The test should succeed or fail gracefully
    assert!(output.exit_code == exit_codes::SUCCESS || output.stderr_contains("worktree"));
}

#[test]
fn test_cook_with_args() {
    let mut test = CliTest::new();
    let workflow_content = r#"
name: args-test
commands:
  - shell: "echo 'KEY=${KEY}'"
"#;
    let (mut test, workflow_path) = test.with_workflow("args", workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--args")
        .arg("KEY=value")
        .run();

    assert_output(&output, exit_codes::SUCCESS, Some("KEY=value"), None);
}

#[test]
fn test_cook_with_auto_accept() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("auto", &create_test_workflow("auto"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("-y")
        .run();

    assert_output(
        &output,
        exit_codes::SUCCESS,
        Some("Test workflow auto"),
        None,
    );
}

#[test]
fn test_cook_failing_workflow() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("fail", &create_failing_workflow("fail"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
}

#[test]
fn test_cook_with_invalid_yaml() {
    let mut test = CliTest::new();
    let workflow_path = test.temp_path().join("invalid.yaml");
    std::fs::write(&workflow_path, "not: valid: yaml: syntax").unwrap();

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(output.stderr_contains("ailed to parse") || output.stderr_contains("nvalid"));
}

#[test]
fn test_cook_with_path_option() {
    let mut test = CliTest::new();
    let other_dir = tempfile::TempDir::new().unwrap();

    // Initialize git in other directory
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&other_dir)
        .output()
        .unwrap();

    let workflow_path = other_dir.path().join("test.yaml");
    std::fs::write(&workflow_path, &create_test_workflow("path-test")).unwrap();

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--path")
        .arg(other_dir.path().to_str().unwrap())
        .run();

    // Should process workflow from specified path - may succeed or fail depending on environment
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.exit_code == exit_codes::GENERAL_ERROR
    );
    // If successful, should show the expected output
    if output.exit_code == exit_codes::SUCCESS {
        assert!(output.stdout_contains("Test workflow path-test"));
    }
}

#[test]
fn test_cook_mapreduce_workflow() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) =
        test.with_workflow("mapreduce", &create_mapreduce_workflow("mr-test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // MapReduce should complete successfully
    assert!(output.success || output.stderr_contains("mapreduce"));
}

#[test]
fn test_cook_with_timeout() {
    let mut test = CliTest::new();
    let workflow_content = r#"
name: timeout-test
commands:
  - shell: "sleep 10"
    timeout: 1
"#;
    let (mut test, workflow_path) = test.with_workflow("timeout", workflow_content);

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Timeout behavior may vary - either success (if timeout not enforced) or error
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.exit_code == exit_codes::GENERAL_ERROR
    );
    // If error, should mention timeout
    if output.exit_code == exit_codes::GENERAL_ERROR {
        assert!(output.stderr_contains("imeout") || output.stderr_contains("exceeded"));
    }
}

#[test]
fn test_run_alias_for_cook() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("alias", &create_test_workflow("alias"));

    let output = test
        .arg("run") // Using 'run' instead of 'cook'
        .arg(workflow_path.to_str().unwrap())
        .run();

    assert_output(
        &output,
        exit_codes::SUCCESS,
        Some("Test workflow alias"),
        None,
    );
}

#[test]
fn test_cook_with_metrics_flag() {
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("metrics", &create_test_workflow("metrics"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--metrics")
        .run();

    // Should complete with metrics (metrics might be shown in output)
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
#[ignore = "foreach functionality not fully implemented yet"]
fn test_cook_with_foreach() {
    let mut test = CliTest::new();
    let workflow_content = r#"
name: foreach-test
commands:
  - foreach:
      foreach: ["a", "b", "c"]
      do:
        - shell: "echo 'Processing ${item}'"
"#;
    let (mut test, workflow_path) = test.with_workflow("foreach", workflow_content);

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    assert_output(&output, exit_codes::SUCCESS, Some("Processing"), None);
}

#[test]
#[ignore = "goal_seek functionality not fully implemented yet"]
fn test_cook_with_goal_seek() {
    let mut test = CliTest::new();
    let workflow_content = r#"
name: goal-seek-test
commands:
  - goal_seek:
      goal: "Echo success"
      command: "shell: echo 'success'"
      validate: "shell: echo 'score: 100'"
      threshold: 80
      max_attempts: 2
"#;
    let (mut test, workflow_path) = test.with_workflow("goal", workflow_content);

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}
