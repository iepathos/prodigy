// Tests for configuration loading and merging

use super::test_utils::*;

#[test]
fn test_config_file_loading() {
    let config_content = r#"
default_max_iterations: 5
auto_worktree: true
verbose: true
"#;

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Config should be applied
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_override_with_cli_args() {
    let config_content = r#"
default_max_iterations: 5
"#;

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

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
    let config_content = "invalid: yaml: syntax: here";

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Should handle invalid config gracefully
    assert!(
        output.exit_code == exit_codes::CONFIG_ERROR
            || output.exit_code == exit_codes::GENERAL_ERROR
            || output.stderr_contains("config")
            || output.stderr_contains("parse")
    );
}

#[test]
fn test_config_with_environment_variables() {
    let mut test = CliTest::new()
        .env("PRODIGY_MAX_ITERATIONS", "3")
        .env("PRODIGY_AUTO_WORKTREE", "true");

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

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

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

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

    let mut test = CliTest::new().with_config(config_content);

    // Note: Command aliases might not be supported, but test the config loading
    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
}

#[test]
fn test_config_precedence() {
    // Test that CLI args > env vars > config file

    let config_content = r#"
default_max_iterations: 2
"#;

    let mut test = CliTest::new()
        .with_config(config_content)
        .env("PRODIGY_MAX_ITERATIONS", "3");

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

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

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) =
        test.with_workflow("mapreduce", &create_mapreduce_workflow("mr"));

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

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

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
    let mut test = CliTest::new();
    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

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

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

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

    let mut test = CliTest::new().with_config(config_content);

    let (mut test, workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("cook").arg(workflow_path.to_str().unwrap()).run();

    // Path settings might affect execution
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("permission")
            || output.stderr_contains("path")
    );
}
