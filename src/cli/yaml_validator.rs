//! YAML workflow validator to check format and suggest improvements

use anyhow::{Context, Result};
use serde_yaml::Value;
use std::fs;
use std::path::Path;

pub struct YamlValidator {
    check_simplified: bool,
}

#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

impl YamlValidator {
    pub fn new(check_simplified: bool) -> Self {
        Self { check_simplified }
    }

    /// Validate a YAML workflow file
    pub fn validate_file(&self, path: &Path) -> Result<ValidationResult> {
        // Read the file
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Parse YAML
        let yaml: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML: {}", path.display()))?;

        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        // Check if it's a MapReduce workflow
        if let Value::Mapping(ref root) = yaml {
            if let Some(Value::String(mode)) = root.get("mode") {
                if mode == "mapreduce" {
                    self.validate_mapreduce_workflow(root, &mut issues, &mut suggestions)?;
                }
            }
        }

        // Check if it's a regular workflow
        if let Value::Sequence(ref steps) = yaml {
            self.validate_regular_workflow(steps, &mut issues, &mut suggestions)?;
        }

        let is_valid = issues.is_empty();

        Ok(ValidationResult {
            is_valid,
            issues,
            suggestions,
        })
    }

    /// Validate required fields in map section
    fn validate_map_section(map: &serde_yaml::Mapping) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        if !map.contains_key("input") {
            issues.push("Map section missing required field 'input'".to_string());
        }
        if !map.contains_key("json_path") {
            issues.push("Map section missing required field 'json_path'".to_string());
        }

        Ok(issues)
    }

    /// Validate agent_template structure and syntax
    /// Returns (issues, suggestions)
    fn validate_agent_template(template: &Value, check_simplified: bool) -> Result<(Vec<String>, Vec<String>)> {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        if check_simplified {
            match template {
                Value::Sequence(_) => {
                    // Good - simplified syntax
                }
                Value::Mapping(template_map) => {
                    if template_map.contains_key("commands") {
                        issues.push("MapReduce workflow uses nested 'commands' syntax. Use simplified syntax with commands directly under 'agent_template'".to_string());
                        suggestions.push("Run 'prodigy migrate-yaml' to automatically convert to simplified syntax".to_string());
                    }
                }
                _ => {
                    issues.push("Invalid agent_template structure".to_string());
                }
            }
        }

        Ok((issues, suggestions))
    }

    /// Check for deprecated parameters in map section
    fn check_deprecated_map_params(map: &serde_yaml::Mapping) -> (Vec<String>, Vec<String>) {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        if map.contains_key("timeout_per_agent") {
            issues.push(
                "Error: Deprecated parameter 'timeout_per_agent' is no longer supported"
                    .to_string(),
            );
            suggestions.push("Remove 'timeout_per_agent' from your workflow file. See MIGRATION.md for updated syntax.".to_string());
        }
        if map.contains_key("retry_on_failure") {
            issues.push(
                "Error: Deprecated parameter 'retry_on_failure' is no longer supported"
                    .to_string(),
            );
            suggestions.push("Remove 'retry_on_failure' from your workflow file. See MIGRATION.md for updated syntax.".to_string());
        }

        (issues, suggestions)
    }

    /// Validate reduce section structure and syntax
    /// Returns (issues, suggestions)
    fn validate_reduce_section(reduce: &Value, check_simplified: bool) -> (Vec<String>, Vec<String>) {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        if check_simplified {
            match reduce {
                Value::Sequence(_) => {
                    // Good - simplified syntax
                }
                Value::Mapping(reduce_map) => {
                    if reduce_map.contains_key("commands") {
                        issues.push("Reduce section uses nested 'commands' syntax. Use simplified syntax with commands directly under 'reduce'".to_string());
                        suggestions.push("Run 'prodigy migrate-yaml' to automatically convert to simplified syntax".to_string());
                    }
                }
                _ => {
                    issues.push("Invalid reduce structure".to_string());
                }
            }
        }

        (issues, suggestions)
    }

    /// Validate required top-level fields
    fn validate_required_fields(workflow: &serde_yaml::Mapping) -> Vec<String> {
        let mut issues = Vec::new();

        if !workflow.contains_key("name") {
            issues.push("Missing required field 'name'".to_string());
        }

        issues
    }

    /// Validate MapReduce workflow structure
    fn validate_mapreduce_workflow(
        &self,
        workflow: &serde_yaml::Mapping,
        issues: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) -> Result<()> {
        // Check for required fields
        let mut required_field_issues = Self::validate_required_fields(workflow);
        issues.append(&mut required_field_issues);

        // Check map section
        if let Some(Value::Mapping(map)) = workflow.get("map") {
            let mut map_issues = Self::validate_map_section(map)?;
            issues.append(&mut map_issues);

            if let Some(agent_template) = map.get("agent_template") {
                let (mut template_issues, mut template_suggestions) = Self::validate_agent_template(agent_template, self.check_simplified)?;
                issues.append(&mut template_issues);
                suggestions.append(&mut template_suggestions);
            } else {
                issues.push("Map section missing required field 'agent_template'".to_string());
            }

            let (mut deprecated_issues, mut deprecated_suggestions) = Self::check_deprecated_map_params(map);
            issues.append(&mut deprecated_issues);
            suggestions.append(&mut deprecated_suggestions);
        } else {
            issues.push("Missing required 'map' section for MapReduce workflow".to_string());
        }

        // Check reduce section
        if let Some(reduce) = workflow.get("reduce") {
            let (mut reduce_issues, mut reduce_suggestions) = Self::validate_reduce_section(reduce, self.check_simplified);
            issues.append(&mut reduce_issues);
            suggestions.append(&mut reduce_suggestions);
        }

        // Check for common issues in command definitions
        self.check_commands_recursive(&Value::Mapping(workflow.clone()), issues, suggestions)?;

        Ok(())
    }

    /// Validate regular workflow structure
    fn validate_regular_workflow(
        &self,
        steps: &[Value],
        issues: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) -> Result<()> {
        if steps.is_empty() {
            issues.push("Workflow has no steps defined".to_string());
        }

        for (idx, step) in steps.iter().enumerate() {
            if let Value::Mapping(step_map) = step {
                // Check for command type
                let has_command = step_map.contains_key("claude")
                    || step_map.contains_key("shell")
                    || step_map.contains_key("test")
                    || step_map.contains_key("analyze");

                if !has_command {
                    issues.push(format!("Step {} has no command defined", idx + 1));
                }

                // Check for deprecated 'test' command
                if step_map.contains_key("test") {
                    issues.push(format!(
                        "Step {} uses deprecated 'test' command type",
                        idx + 1
                    ));
                    suggestions.push("Replace 'test:' with 'shell:' for test commands".to_string());
                }
            } else {
                issues.push(format!("Step {} is not a valid mapping", idx + 1));
            }
        }

        Ok(())
    }

    /// Recursively check commands for issues
    fn check_commands_recursive(
        &self,
        value: &Value,
        issues: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) -> Result<()> {
        Self::check_commands_recursive_impl(value, issues, suggestions)
    }

    fn check_commands_recursive_impl(
        value: &Value,
        issues: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) -> Result<()> {
        match value {
            Value::Mapping(map) => {
                // Check for on_failure with deprecated parameters
                if let Some(Value::Mapping(on_failure)) = map.get("on_failure") {
                    if on_failure.contains_key("max_attempts") {
                        issues.push("Error: Deprecated parameter 'max_attempts' in on_failure is no longer supported".to_string());
                        suggestions.push("Remove 'max_attempts' from on_failure. See MIGRATION.md for updated syntax.".to_string());
                    }
                    if on_failure.contains_key("fail_workflow") {
                        issues.push("Error: Deprecated parameter 'fail_workflow' in on_failure is no longer supported".to_string());
                        suggestions.push("Remove 'fail_workflow' from on_failure. See MIGRATION.md for updated syntax.".to_string());
                    }
                }

                // Recurse into all values
                for (_key, val) in map.iter() {
                    Self::check_commands_recursive_impl(val, issues, suggestions)?;
                }
            }
            Value::Sequence(seq) => {
                // Recurse into all items
                for item in seq.iter() {
                    Self::check_commands_recursive_impl(item, issues, suggestions)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    /// Helper function to create a temp file with YAML content
    fn create_temp_yaml(content: &str) -> Result<NamedTempFile> {
        let temp_file = NamedTempFile::new()?;
        fs::write(temp_file.path(), content)?;
        Ok(temp_file)
    }

    #[test]
    fn test_validator_creation() {
        let validator = YamlValidator::new(true);
        assert!(validator.check_simplified);

        let validator = YamlValidator::new(false);
        assert!(!validator.check_simplified);
    }

    #[test]
    fn test_missing_name_field() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Missing required field 'name'")));

        Ok(())
    }

    #[test]
    fn test_missing_map_section() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.contains("Missing required 'map' section")));

        Ok(())
    }

    #[test]
    fn test_missing_input_field() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Map section missing required field 'input'")));

        Ok(())
    }

    #[test]
    fn test_missing_json_path_field() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  agent_template:
    - claude: "/process ${item}"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Map section missing required field 'json_path'")));

        Ok(())
    }

    #[test]
    fn test_missing_agent_template_field() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Map section missing required field 'agent_template'")));

        Ok(())
    }

    #[test]
    fn test_simplified_syntax_agent_template_valid() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
    - shell: "echo done"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(result.is_valid);
        assert!(result.issues.is_empty());

        Ok(())
    }

    #[test]
    fn test_nested_commands_syntax_in_agent_template() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    commands:
      - claude: "/process ${item}"
      - shell: "echo done"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("nested 'commands' syntax")));
        assert!(result
            .suggestions
            .iter()
            .any(|s| s.contains("prodigy migrate-yaml")));

        Ok(())
    }

    #[test]
    fn test_invalid_agent_template_structure() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template: "invalid string"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Invalid agent_template structure")));

        Ok(())
    }

    #[test]
    fn test_deprecated_timeout_per_agent() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
  timeout_per_agent: 300
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Deprecated parameter 'timeout_per_agent'")));
        assert!(result
            .suggestions
            .iter()
            .any(|s| s.contains("Remove 'timeout_per_agent'")));

        Ok(())
    }

    #[test]
    fn test_deprecated_retry_on_failure() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
  retry_on_failure: true
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Deprecated parameter 'retry_on_failure'")));
        assert!(result
            .suggestions
            .iter()
            .any(|s| s.contains("Remove 'retry_on_failure'")));

        Ok(())
    }

    #[test]
    fn test_simplified_reduce_syntax_valid() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"

reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo complete"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(result.is_valid);
        assert!(result.issues.is_empty());

        Ok(())
    }

    #[test]
    fn test_nested_commands_syntax_in_reduce() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"

reduce:
  commands:
    - claude: "/summarize ${map.results}"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Reduce section uses nested 'commands' syntax")));
        assert!(result
            .suggestions
            .iter()
            .any(|s| s.contains("prodigy migrate-yaml")));

        Ok(())
    }

    #[test]
    fn test_invalid_reduce_structure() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"

reduce: "invalid string"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.contains("Invalid reduce structure")));

        Ok(())
    }

    #[test]
    fn test_valid_mapreduce_workflow() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
    - shell: "test ${item.path}"

reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Complete'"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(result.is_valid);
        assert!(result.issues.is_empty());

        Ok(())
    }

    #[test]
    fn test_check_simplified_false_skips_syntax_checks() -> Result<()> {
        let validator = YamlValidator::new(false);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    commands:
      - claude: "/process ${item}"

reduce:
  commands:
    - claude: "/summarize ${map.results}"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        // Should be valid when check_simplified is false
        assert!(result.is_valid);
        assert!(result.issues.is_empty());

        Ok(())
    }

    #[test]
    fn test_deprecated_on_failure_max_attempts() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
      on_failure:
        max_attempts: 3
        claude: "/fix ${item}"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Deprecated parameter 'max_attempts' in on_failure")));

        Ok(())
    }

    #[test]
    fn test_deprecated_on_failure_fail_workflow() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
name: test-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process ${item}"
      on_failure:
        fail_workflow: true
        claude: "/fix ${item}"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("Deprecated parameter 'fail_workflow' in on_failure")));

        Ok(())
    }

    #[test]
    fn test_regular_workflow_validation() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"
- claude: "/command one"
- shell: "echo hello"
"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(result.is_valid);
        assert!(result.issues.is_empty());

        Ok(())
    }

    #[test]
    fn test_regular_workflow_empty_steps() -> Result<()> {
        let validator = YamlValidator::new(true);

        let yaml_content = r#"[]"#;

        let temp_file = create_temp_yaml(yaml_content)?;
        let result = validator.validate_file(temp_file.path())?;

        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.contains("no steps defined")));

        Ok(())
    }

    // Tests for extracted functions

    #[test]
    fn test_validate_map_section_all_fields_present() -> Result<()> {
        let yaml_content = r#"
input: "items.json"
json_path: "$.items[*]"
agent_template:
  - claude: "/test"
"#;
        let map: serde_yaml::Mapping = serde_yaml::from_str(yaml_content)?;
        let issues = YamlValidator::validate_map_section(&map)?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn test_validate_map_section_missing_input() -> Result<()> {
        let yaml_content = r#"
json_path: "$.items[*]"
agent_template:
  - claude: "/test"
"#;
        let map: serde_yaml::Mapping = serde_yaml::from_str(yaml_content)?;
        let issues = YamlValidator::validate_map_section(&map)?;
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("missing required field 'input'"));
        Ok(())
    }

    #[test]
    fn test_validate_map_section_missing_json_path() -> Result<()> {
        let yaml_content = r#"
input: "items.json"
agent_template:
  - claude: "/test"
"#;
        let map: serde_yaml::Mapping = serde_yaml::from_str(yaml_content)?;
        let issues = YamlValidator::validate_map_section(&map)?;
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("missing required field 'json_path'"));
        Ok(())
    }

    #[test]
    fn test_validate_agent_template_simplified_valid() -> Result<()> {
        let yaml_content = r#"
- claude: "/test"
- shell: "echo done"
"#;
        let template: Value = serde_yaml::from_str(yaml_content)?;
        let (issues, suggestions) = YamlValidator::validate_agent_template(&template, true)?;
        assert!(issues.is_empty());
        assert!(suggestions.is_empty());
        Ok(())
    }

    #[test]
    fn test_validate_agent_template_nested_commands() -> Result<()> {
        let yaml_content = r#"
commands:
  - claude: "/test"
"#;
        let template: Value = serde_yaml::from_str(yaml_content)?;
        let (issues, suggestions) = YamlValidator::validate_agent_template(&template, true)?;
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("nested 'commands' syntax"));
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].contains("prodigy migrate-yaml"));
        Ok(())
    }

    #[test]
    fn test_validate_agent_template_invalid_structure() -> Result<()> {
        let template = Value::String("invalid".to_string());
        let (issues, suggestions) = YamlValidator::validate_agent_template(&template, true)?;
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("Invalid agent_template structure"));
        assert!(suggestions.is_empty());
        Ok(())
    }

    #[test]
    fn test_check_deprecated_map_params_none() {
        let yaml_content = r#"
input: "items.json"
json_path: "$.items[*]"
"#;
        let map: serde_yaml::Mapping = serde_yaml::from_str(yaml_content).unwrap();
        let (issues, suggestions) = YamlValidator::check_deprecated_map_params(&map);
        assert!(issues.is_empty());
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_check_deprecated_map_params_timeout_per_agent() {
        let yaml_content = r#"
input: "items.json"
json_path: "$.items[*]"
timeout_per_agent: 300
"#;
        let map: serde_yaml::Mapping = serde_yaml::from_str(yaml_content).unwrap();
        let (issues, suggestions) = YamlValidator::check_deprecated_map_params(&map);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("timeout_per_agent"));
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].contains("Remove 'timeout_per_agent'"));
    }

    #[test]
    fn test_check_deprecated_map_params_retry_on_failure() {
        let yaml_content = r#"
input: "items.json"
json_path: "$.items[*]"
retry_on_failure: true
"#;
        let map: serde_yaml::Mapping = serde_yaml::from_str(yaml_content).unwrap();
        let (issues, suggestions) = YamlValidator::check_deprecated_map_params(&map);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("retry_on_failure"));
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].contains("Remove 'retry_on_failure'"));
    }

    #[test]
    fn test_validate_reduce_section_simplified_valid() {
        let yaml_content = r#"
- claude: "/summarize"
- shell: "echo done"
"#;
        let reduce: Value = serde_yaml::from_str(yaml_content).unwrap();
        let (issues, suggestions) = YamlValidator::validate_reduce_section(&reduce, true);
        assert!(issues.is_empty());
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_validate_reduce_section_nested_commands() {
        let yaml_content = r#"
commands:
  - claude: "/summarize"
"#;
        let reduce: Value = serde_yaml::from_str(yaml_content).unwrap();
        let (issues, suggestions) = YamlValidator::validate_reduce_section(&reduce, true);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("nested 'commands' syntax"));
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].contains("prodigy migrate-yaml"));
    }

    #[test]
    fn test_validate_reduce_section_invalid_structure() {
        let reduce = Value::String("invalid".to_string());
        let (issues, suggestions) = YamlValidator::validate_reduce_section(&reduce, true);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("Invalid reduce structure"));
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_validate_reduce_section_check_simplified_false() {
        let yaml_content = r#"
commands:
  - claude: "/summarize"
"#;
        let reduce: Value = serde_yaml::from_str(yaml_content).unwrap();
        let (issues, suggestions) = YamlValidator::validate_reduce_section(&reduce, false);
        assert!(issues.is_empty());
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_validate_required_fields_all_present() {
        let yaml_content = r#"
name: "test-workflow"
mode: mapreduce
"#;
        let workflow: serde_yaml::Mapping = serde_yaml::from_str(yaml_content).unwrap();
        let issues = YamlValidator::validate_required_fields(&workflow);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_validate_required_fields_missing_name() {
        let yaml_content = r#"
mode: mapreduce
"#;
        let workflow: serde_yaml::Mapping = serde_yaml::from_str(yaml_content).unwrap();
        let issues = YamlValidator::validate_required_fields(&workflow);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("Missing required field 'name'"));
    }
}
