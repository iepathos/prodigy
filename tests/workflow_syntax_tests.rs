use anyhow::Result;
use mmm::cook::workflow::WorkflowStep;
use std::collections::HashMap;

// Define WorkflowContext for testing
#[derive(Debug, Clone, Default)]
pub struct WorkflowContext {
    pub variables: HashMap<String, String>,
    pub captured_outputs: HashMap<String, String>,
    pub iteration_vars: HashMap<String, String>,
}

impl WorkflowContext {
    /// Interpolate variables in a template string
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = template.to_string();

        // Replace ${VAR} and $VAR patterns
        for (key, value) in &self.variables {
            result = result.replace(&format!("${{{key}}}"), value);
            result = result.replace(&format!("${key}"), value);
        }

        for (key, value) in &self.captured_outputs {
            result = result.replace(&format!("${{{key}}}"), value);
            result = result.replace(&format!("${key}"), value);
        }

        for (key, value) in &self.iteration_vars {
            result = result.replace(&format!("${{{key}}}"), value);
            result = result.replace(&format!("${key}"), value);
        }

        result
    }
}

#[test]
fn test_workflow_context_interpolation() {
    let mut ctx = WorkflowContext::default();
    ctx.variables
        .insert("ARG".to_string(), "spec-123".to_string());
    ctx.variables
        .insert("PROJECT_ROOT".to_string(), "/home/user/project".to_string());
    ctx.captured_outputs
        .insert("CAPTURED_OUTPUT".to_string(), "test output".to_string());
    ctx.iteration_vars
        .insert("ITERATION".to_string(), "3".to_string());

    // Test various interpolation patterns
    assert_eq!(ctx.interpolate("$ARG"), "spec-123");
    assert_eq!(ctx.interpolate("${ARG}"), "spec-123");
    assert_eq!(
        ctx.interpolate("/mmm-implement-spec $ARG"),
        "/mmm-implement-spec spec-123"
    );
    assert_eq!(
        ctx.interpolate("${PROJECT_ROOT}/src"),
        "/home/user/project/src"
    );
    assert_eq!(ctx.interpolate("Iteration $ITERATION"), "Iteration 3");
    assert_eq!(
        ctx.interpolate("Output: '$CAPTURED_OUTPUT'"),
        "Output: 'test output'"
    );

    // Test missing variables (should remain as-is)
    assert_eq!(ctx.interpolate("$MISSING_VAR"), "$MISSING_VAR");
    assert_eq!(ctx.interpolate("${MISSING_VAR}"), "${MISSING_VAR}");
}

#[test]
fn test_parse_claude_command_syntax() -> Result<()> {
    let yaml = r#"
claude: "/mmm-implement-spec $ARG"
capture_output: true
commit_required: false
analysis:
  max_cache_age: 300
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml)?;
    assert_eq!(step.claude, Some("/mmm-implement-spec $ARG".to_string()));
    assert!(step.capture_output);
    assert!(!step.commit_required);
    assert!(step.analysis.is_some());
    assert_eq!(step.analysis.unwrap().max_cache_age, 300);

    Ok(())
}

#[test]
fn test_parse_shell_command_syntax() -> Result<()> {
    let yaml = r#"
shell: "cargo test --lib"
timeout: 120
capture_output: true
on_failure:
  claude: "/mmm-fix-test-failures '$CAPTURED_OUTPUT'"
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml)?;
    assert_eq!(step.shell, Some("cargo test --lib".to_string()));
    assert_eq!(step.timeout, Some(120));
    assert!(step.capture_output);
    assert!(step.on_failure.is_some());

    let on_failure = step.on_failure.unwrap();
    assert_eq!(
        on_failure.claude,
        Some("/mmm-fix-test-failures '$CAPTURED_OUTPUT'".to_string())
    );

    Ok(())
}

#[test]
fn test_parse_legacy_syntax() -> Result<()> {
    let yaml = r#"
name: mmm-lint
commit_required: false
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml)?;
    assert_eq!(step.name, Some("mmm-lint".to_string()));
    assert!(!step.commit_required);
    assert!(step.claude.is_none());
    assert!(step.shell.is_none());

    Ok(())
}

#[test]
fn test_parse_conditional_execution() -> Result<()> {
    let yaml = r#"
shell: "cargo test"
on_success:
  shell: "echo 'Tests passed!'"
on_failure:
  claude: "/mmm-fix-tests"
on_exit_code:
  101:
    claude: "/mmm-fix-compilation"
  102:
    shell: "cargo clean && cargo build"
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml)?;
    assert!(step.on_success.is_some());
    assert!(step.on_failure.is_some());
    assert_eq!(step.on_exit_code.len(), 2);
    assert!(step.on_exit_code.contains_key(&101));
    assert!(step.on_exit_code.contains_key(&102));

    Ok(())
}

#[test]
fn test_parse_complete_workflow() -> Result<()> {
    let yaml = r#"
commands:
  - claude: "/mmm-implement-spec $ARG"
    analysis:
      max_cache_age: 300
      
  - shell: "cargo test"
    capture_output: true
    on_failure:
      claude: "/mmm-fix-test-failures '$CAPTURED_OUTPUT'"
      on_success:
        shell: "cargo test"
        
  - name: mmm-lint
    commit_required: false
"#;

    let workflow: serde_yaml::Value = serde_yaml::from_str(yaml)?;
    let commands = workflow["commands"].as_sequence().unwrap();

    assert_eq!(commands.len(), 3);

    // First command uses claude syntax
    let step1: WorkflowStep = serde_yaml::from_value(commands[0].clone())?;
    assert!(step1.claude.is_some());
    assert!(step1.analysis.is_some());

    // Second command uses shell syntax with conditional
    let step2: WorkflowStep = serde_yaml::from_value(commands[1].clone())?;
    assert!(step2.shell.is_some());
    assert!(step2.capture_output);
    assert!(step2.on_failure.is_some());

    // Third command uses legacy syntax
    let step3: WorkflowStep = serde_yaml::from_value(commands[2].clone())?;
    assert!(step3.name.is_some());
    assert!(!step3.commit_required);

    Ok(())
}

#[test]
fn test_environment_variables() -> Result<()> {
    let yaml = r#"
shell: "npm test"
env:
  NODE_ENV: "test"
  TEST_TIMEOUT: "30000"
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml)?;
    assert_eq!(step.env.len(), 2);
    assert_eq!(step.env.get("NODE_ENV"), Some(&"test".to_string()));
    assert_eq!(step.env.get("TEST_TIMEOUT"), Some(&"30000".to_string()));

    Ok(())
}

#[test]
fn test_workflow_step_defaults() -> Result<()> {
    let yaml = r#"
claude: "/mmm-code-review"
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml)?;

    // Check defaults
    assert!(!step.capture_output); // defaults to false
    assert!(step.commit_required); // defaults to true
    assert!(step.timeout.is_none());
    assert!(step.working_dir.is_none());
    assert!(step.env.is_empty());
    assert!(step.on_failure.is_none());
    assert!(step.on_success.is_none());
    assert!(step.on_exit_code.is_empty());

    Ok(())
}
