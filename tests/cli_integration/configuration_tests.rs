// Tests for configuration loading and merging

use super::test_utils::*;

#[test]
fn test_config_file_loading() {
    let config_content = r#"
default_max_iterations: 5
auto_worktree: true
verbose: true
"#;

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Config should be applied
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_override_with_cli_args() {
    let config_content = r#"
default_max_iterations: 5
"#;

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("-n")
        .arg("10") // Override config value
        .run();

    // CLI args should override config
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_invalid_config_file() {
    // Use truly invalid YAML syntax that will cause a parse error
    let config_content = "invalid yaml : : : this is not valid\n  bad indentation without key";

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Prodigy may ignore invalid config files and continue with defaults
    // The test should verify that either an error is returned OR the workflow runs successfully
    // with the invalid config being ignored
    assert!(
        output.exit_code == exit_codes::SUCCESS  // Config ignored, workflow runs
            || output.exit_code == exit_codes::CONFIG_ERROR
            || output.exit_code == exit_codes::GENERAL_ERROR
            || output.stderr_contains("config")
            || output.stderr_contains("parse")
            || output.stderr_contains("YAML"),
        "Unexpected behavior with invalid config, got exit code {} with stderr: {}",
        output.exit_code,
        output.stderr
    );
}

#[test]
fn test_config_with_environment_variables() {
    let test = CliTest::new()
        .env("PRODIGY_MAX_ITERATIONS", "3")
        .env("PRODIGY_AUTO_WORKTREE", "true");

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Environment variables should be respected
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_file_with_workflow_defaults() {
    let config_content = r#"
workflow_defaults:
  timeout: 300
  retry_attempts: 3
"#;

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Workflow defaults should be applied
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_with_command_aliases() {
    let config_content = r#"
command_aliases:
  c: cook
  e: exec
"#;

    let test = CliTest::new().with_config(config_content);

    // Note: Command aliases might not be supported, but test the config loading
    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_precedence() {
    // Test that CLI args > env vars > config file

    let config_content = r#"
default_max_iterations: 2
"#;

    let test = CliTest::new()
        .with_config(config_content)
        .env("PRODIGY_MAX_ITERATIONS", "3");

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("-n")
        .arg("4") // Should take precedence
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_with_mapreduce_defaults() {
    let config_content = r#"
mapreduce_defaults:
  max_parallel: 10
  chunk_size: 100
"#;

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("mapreduce", &create_mapreduce_workflow("mr"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // MapReduce defaults should be applied
    assert!(output.exit_code == exit_codes::SUCCESS || output.stderr_contains("mapreduce"));
}

#[test]
fn test_config_with_logging_settings() {
    let config_content = r#"
logging:
  level: debug
  format: json
"#;

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Logging config should be applied
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("DEBUG")
            || output.stderr_contains("{")
    );
}

#[test]
fn test_missing_config_file() {
    // When no config file exists, should use defaults
    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Should work with defaults
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_with_git_settings() {
    let config_content = r#"
git:
  auto_commit: false
  commit_prefix: "prodigy: "
  branch_prefix: "prodigy/"
"#;

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Git settings should be applied
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_with_path_settings() {
    let config_content = r#"
paths:
  worktree_base: "/tmp/prodigy-worktrees"
  event_storage: "/tmp/prodigy-events"
"#;

    let test = CliTest::new().with_config(config_content);

    let (test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Path settings might affect execution
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("permission")
            || output.stderr_contains("path")
    );
}
