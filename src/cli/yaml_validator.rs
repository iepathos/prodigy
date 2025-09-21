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

    /// Validate MapReduce workflow structure
    fn validate_mapreduce_workflow(
        &self,
        workflow: &serde_yaml::Mapping,
        issues: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) -> Result<()> {
        // Check for required fields
        if !workflow.contains_key("name") {
            issues.push("Missing required field 'name'".to_string());
        }

        // Check map section
        if let Some(Value::Mapping(map)) = workflow.get("map") {
            // Check for required map fields
            if !map.contains_key("input") {
                issues.push("Map section missing required field 'input'".to_string());
            }
            if !map.contains_key("json_path") {
                issues.push("Map section missing required field 'json_path'".to_string());
            }

            // Check agent_template
            if let Some(agent_template) = map.get("agent_template") {
                if self.check_simplified {
                    // Check for simplified syntax
                    match agent_template {
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
            } else {
                issues.push("Map section missing required field 'agent_template'".to_string());
            }

            // Reject deprecated parameters
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
        } else {
            issues.push("Missing required 'map' section for MapReduce workflow".to_string());
        }

        // Check reduce section
        if let Some(reduce) = workflow.get("reduce") {
            if self.check_simplified {
                // Check for simplified syntax
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
