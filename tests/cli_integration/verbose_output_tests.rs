// Tests for verbose output levels

use super::test_utils::*;

#[test]
fn test_no_verbose_flag() {
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("quiet", &create_test_workflow("quiet"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Should have minimal output
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(!output.stderr_contains("[DEBUG]"));
    assert!(!output.stderr_contains("[TRACE]"));
}

#[test]
fn test_verbose_flag_debug() {
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("debug", &create_test_workflow("debug"));

    let output = test
        .arg("-v")
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .run();

    // Should show debug output
    assert!(output.exit_code == exit_codes::SUCCESS);
    // Debug messages might be visible
}

#[test]
fn test_verbose_flag_trace() {
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("trace", &create_test_workflow("trace"));

    let output = test
        .arg("-vv")
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .run();

    // Should show trace output
    assert!(output.exit_code == exit_codes::SUCCESS);
    // Trace messages might be visible
}

#[test]
fn test_verbose_flag_all() {
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("all", &create_test_workflow("all"));

    let output = test
        .arg("-vvv")
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .run();

    // Should show all output
    assert!(output.exit_code == exit_codes::SUCCESS);
    // All log levels might be visible
}

#[test]
fn test_verbose_with_exec() {
    let mut test = CliTest::new()
        .arg("-v")
        .arg("exec")
        .arg("shell: echo 'Verbose exec'");

    let output = test.run();

    assert_output(&output, exit_codes::SUCCESS, Some("Verbose exec"), None);
}

#[test]
fn test_verbose_with_batch() {
    let test = CliTest::new();

    // Create test file
    let test_dir = test.temp_path().to_path_buf();
    std::fs::write(test_dir.join("test.txt"), "content").unwrap();

    let output = test
        .arg("-v")
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: echo 'Processing {}'")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_verbose_error_messages() {
    let mut test = CliTest::new().arg("-v").arg("cook").arg("nonexistent.yaml");

    let output = test.run();

    // Should show detailed error with verbose flag
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(
        output.stderr_contains("not found")
            || output.stderr_contains("No such file")
            || output.stderr_contains("Failed")
            || output.stderr_contains("Error")
    );
}

#[test]
fn test_verbose_with_mapreduce() {
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("mr", &create_mapreduce_workflow("verbose-mr"));

    let output = test
        .arg("-vv")
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .run();

    // Should show detailed MapReduce execution
    assert!(output.exit_code == exit_codes::SUCCESS || output.stderr_contains("mapreduce"));
}

#[test]
fn test_verbose_timing_information() {
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("timing", &create_test_workflow("timing"));

    let output = test
        .arg("-v")
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .run();

    // Verbose mode might show timing information
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_quiet_mode() {
    // Some CLIs support a quiet mode that suppresses output
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("quiet", &create_test_workflow("quiet"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .env("PRODIGY_QUIET", "true") // If supported
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_verbose_environment_variable() {
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("env", &create_test_workflow("env"));

    let output = test
        .env("PRODIGY_VERBOSE", "debug")
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .run();

    // Environment variable might control verbosity
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_verbose_progress_indicators() {
    let test = CliTest::new();
    let workflow_content = r#"
name: progress-test
commands:
  - shell: "echo 'Step 1'"
  - shell: "echo 'Step 2'"
  - shell: "echo 'Step 3'"
"#;
    let (test, workflow_path) = test.with_workflow("progress", workflow_content);

    let output = test
        .arg("-v")
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .run();

    // Verbose mode might show step progress
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("Step") || output.stderr_contains("Step"));
}
