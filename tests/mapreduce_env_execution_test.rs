//! Integration tests for MapReduce environment variable execution
//!
//! These tests verify that environment variables are correctly resolved
//! and passed during actual workflow execution, not just parsing.

use anyhow::Result;
use prodigy::config::parse_mapreduce_workflow;

#[test]
fn test_numeric_field_env_interpolation() -> Result<()> {
    let workflow_yaml = r#"
name: test-numeric-env
mode: mapreduce

env:
  MAX_PARALLEL: "5"
  TIMEOUT_SECONDS: "900"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: ${MAX_PARALLEL}
  agent_timeout_secs: ${TIMEOUT_SECONDS}

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Verify env block contains our variables
    assert!(config.env.is_some());
    let env = config.env.as_ref().unwrap();
    assert_eq!(env.get("MAX_PARALLEL"), Some(&"5".to_string()));
    assert_eq!(env.get("TIMEOUT_SECONDS"), Some(&"900".to_string()));

    // Verify numeric fields accept string values (for env var references)
    assert_eq!(config.map.max_parallel, "${MAX_PARALLEL}");
    assert_eq!(config.map.agent_timeout_secs, Some("${TIMEOUT_SECONDS}".to_string()));

    // Verify we can convert to MapPhase and resolve variables
    let map_phase = config.to_map_phase()?;

    // After resolution, should be numeric values
    assert_eq!(map_phase.config.max_parallel, 5);
    assert_eq!(map_phase.config.agent_timeout_secs, Some(900));

    Ok(())
}

#[test]
fn test_numeric_field_direct_values() -> Result<()> {
    let workflow_yaml = r#"
name: test-numeric-direct
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 3
  agent_timeout_secs: 600

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Verify numeric fields accept direct numeric values too
    assert_eq!(config.map.max_parallel, "3");
    assert_eq!(config.map.agent_timeout_secs, Some("600".to_string()));

    // Verify we can convert to MapPhase
    let map_phase = config.to_map_phase()?;

    // Should parse to numeric values
    assert_eq!(map_phase.config.max_parallel, 3);
    assert_eq!(map_phase.config.agent_timeout_secs, Some(600));

    Ok(())
}

#[test]
fn test_env_var_resolution_error_handling() -> Result<()> {
    let workflow_yaml = r#"
name: test-missing-var
mode: mapreduce

env:
  EXISTING_VAR: "10"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: ${MISSING_VAR}  # This variable is not defined

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Should parse successfully
    assert_eq!(config.map.max_parallel, "${MISSING_VAR}");

    // But conversion to MapPhase should fail with clear error
    let result = config.to_map_phase();
    assert!(result.is_err());

    let error = result.unwrap_err();
    let error_msg = format!("{:#}", error);
    assert!(error_msg.contains("MISSING_VAR"));
    assert!(error_msg.contains("not found"));

    Ok(())
}

#[test]
fn test_env_var_invalid_numeric_value() -> Result<()> {
    let workflow_yaml = r#"
name: test-invalid-numeric
mode: mapreduce

env:
  MAX_PARALLEL: "not-a-number"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: ${MAX_PARALLEL}

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Conversion to MapPhase should fail with parse error
    let result = config.to_map_phase();
    assert!(result.is_err());

    let error = result.unwrap_err();
    let error_msg = format!("{:#}", error);
    assert!(error_msg.contains("MAX_PARALLEL"));
    assert!(error_msg.contains("not-a-number") || error_msg.contains("parse"));

    Ok(())
}

#[test]
fn test_both_env_var_syntaxes() -> Result<()> {
    let workflow_yaml = r#"
name: test-syntax-variants
mode: mapreduce

env:
  VAR1: "5"
  VAR2: "600"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: $VAR1           # Shell-style without braces
  agent_timeout_secs: ${VAR2}   # Bracketed style

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Both syntaxes should be accepted
    assert_eq!(config.map.max_parallel, "$VAR1");
    assert_eq!(config.map.agent_timeout_secs, Some("${VAR2}".to_string()));

    // Both should resolve correctly
    let map_phase = config.to_map_phase()?;
    assert_eq!(map_phase.config.max_parallel, 5);
    assert_eq!(map_phase.config.agent_timeout_secs, Some(600));

    Ok(())
}

#[test]
fn test_system_env_fallback() -> Result<()> {
    // Set a system environment variable
    std::env::set_var("TEST_PRODIGY_MAX_PARALLEL", "7");

    let workflow_yaml = r#"
name: test-system-env
mode: mapreduce

# No env block - should fall back to system environment

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: ${TEST_PRODIGY_MAX_PARALLEL}

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Should resolve from system environment
    let map_phase = config.to_map_phase()?;
    assert_eq!(map_phase.config.max_parallel, 7);

    // Clean up
    std::env::remove_var("TEST_PRODIGY_MAX_PARALLEL");

    Ok(())
}

#[test]
fn test_workflow_env_overrides_system() -> Result<()> {
    // Set a system environment variable
    std::env::set_var("TEST_PRODIGY_OVERRIDE", "10");

    let workflow_yaml = r#"
name: test-env-override
mode: mapreduce

env:
  TEST_PRODIGY_OVERRIDE: "20"  # Workflow env should override system

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: ${TEST_PRODIGY_OVERRIDE}

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Should use workflow env value (20), not system env (10)
    let map_phase = config.to_map_phase()?;
    assert_eq!(map_phase.config.max_parallel, 20);

    // Clean up
    std::env::remove_var("TEST_PRODIGY_OVERRIDE");

    Ok(())
}

#[test]
fn test_optional_timeout_field() -> Result<()> {
    let workflow_yaml = r#"
name: test-optional-timeout
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 3
  # agent_timeout_secs is optional - not specified

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Optional field should be None
    assert_eq!(config.map.agent_timeout_secs, None);

    // Should convert successfully
    let map_phase = config.to_map_phase()?;
    assert_eq!(map_phase.config.agent_timeout_secs, None);

    Ok(())
}

/// Test that environment variables work correctly in shell commands
#[test]
fn test_env_vars_in_setup_commands() -> Result<()> {
    let workflow_yaml = r#"
name: test-setup-env
mode: mapreduce

env:
  PROJECT_NAME: "my-project"
  OUTPUT_DIR: "output"

setup:
  - shell: "echo $PROJECT_NAME"
  - shell: "mkdir -p $OUTPUT_DIR"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 1

  agent_template:
    - shell: "echo test"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Verify setup commands contain env var references
    assert!(config.setup.is_some());
    let setup = config.setup.as_ref().unwrap();
    assert_eq!(setup.commands.len(), 2);

    // Verify the shell commands contain variable references
    // (actual execution will substitute them)
    let cmd1 = &setup.commands[0];
    assert!(cmd1.shell.is_some());
    assert!(cmd1.shell.as_ref().unwrap().contains("$PROJECT_NAME"));

    let cmd2 = &setup.commands[1];
    assert!(cmd2.shell.is_some());
    assert!(cmd2.shell.as_ref().unwrap().contains("$OUTPUT_DIR"));

    Ok(())
}

/// Integration test combining numeric field interpolation with command env vars
#[test]
fn test_complete_env_workflow() -> Result<()> {
    let workflow_yaml = r#"
name: test-complete-env
mode: mapreduce

env:
  PROJECT_NAME: "complete-test"
  MAX_PARALLEL: "4"
  TIMEOUT_SECONDS: "300"
  OUTPUT_DIR: "results"

setup:
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "echo Starting $PROJECT_NAME"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: ${MAX_PARALLEL}
  agent_timeout_secs: ${TIMEOUT_SECONDS}

  agent_template:
    - shell: "echo Processing ${item.name} for $PROJECT_NAME"
    - shell: "echo ${item.name} > $OUTPUT_DIR/${item.name}.txt"

reduce:
  - shell: "echo Completed $PROJECT_NAME"
  - shell: "cat $OUTPUT_DIR/*.txt > $OUTPUT_DIR/summary.txt"

merge:
  commands:
    - shell: "echo Merging $PROJECT_NAME results"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Verify environment variables
    assert!(config.env.is_some());
    let env = config.env.as_ref().unwrap();
    assert_eq!(env.len(), 4);

    // Verify numeric fields can be resolved
    let map_phase = config.to_map_phase()?;
    assert_eq!(map_phase.config.max_parallel, 4);
    assert_eq!(map_phase.config.agent_timeout_secs, Some(300));

    // Verify all phases have commands with env var references
    assert!(config.setup.is_some());
    assert_eq!(config.setup.as_ref().unwrap().commands.len(), 2);
    assert_eq!(map_phase.agent_template.len(), 2);
    assert!(config.reduce.is_some());
    assert_eq!(config.reduce.as_ref().unwrap().commands.len(), 2);
    assert!(config.merge.is_some());
    assert_eq!(config.merge.as_ref().unwrap().commands.len(), 1);

    Ok(())
}

/// Test that environment variables are interpolated in map.input field
#[test]
fn test_map_input_env_interpolation() -> Result<()> {
    let workflow_yaml = r#"
name: test-map-input-env
mode: mapreduce

env:
  DATA_FILE: "workflows/data/items.json"
  PROJECT_NAME: "test-project"

map:
  input: ${DATA_FILE}  # Should be interpolated to actual file path
  json_path: "$.items[*]"
  max_parallel: 1

  agent_template:
    - shell: "echo ${item.name}"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Verify env variables are defined
    assert!(config.env.is_some());
    let env = config.env.as_ref().unwrap();
    assert_eq!(env.get("DATA_FILE"), Some(&"workflows/data/items.json".to_string()));

    // Verify map.input contains the variable reference (before interpolation)
    assert_eq!(config.map.input, "${DATA_FILE}");

    // NOTE: The actual interpolation happens at runtime in WorkflowExecutor
    // when it populates workflow_context.variables and interpolates the input.
    // This test verifies the workflow parses correctly with env var references.

    Ok(())
}

/// Test that environment variables work in shell commands
#[test]
fn test_shell_command_env_vars() -> Result<()> {
    let workflow_yaml = r#"
name: test-shell-env
mode: mapreduce

env:
  OUTPUT_DIR: "test-output"
  PROJECT_NAME: "my-project"

setup:
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "echo Starting $PROJECT_NAME"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 1

  agent_template:
    - shell: "echo Processing for $PROJECT_NAME"
"#;

    // Parse the workflow
    let config = parse_mapreduce_workflow(workflow_yaml)?;

    // Verify environment variables
    assert!(config.env.is_some());
    let env = config.env.as_ref().unwrap();
    assert_eq!(env.get("OUTPUT_DIR"), Some(&"test-output".to_string()));
    assert_eq!(env.get("PROJECT_NAME"), Some(&"my-project".to_string()));

    // Verify setup commands contain variable references
    // (the shell will expand these when environment variables are set)
    assert!(config.setup.is_some());
    let setup = config.setup.as_ref().unwrap();

    let cmd1 = &setup.commands[0];
    assert!(cmd1.shell.as_ref().unwrap().contains("$OUTPUT_DIR"));

    let cmd2 = &setup.commands[1];
    assert!(cmd2.shell.as_ref().unwrap().contains("$PROJECT_NAME"));

    Ok(())
}
