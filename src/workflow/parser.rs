use anyhow::{Context, Result};
use serde_yaml;
use std::fs;
use std::path::Path;

use super::{Workflow, WorkflowContext};

pub struct WorkflowParser {
    template_resolver: super::template::TemplateResolver,
}

impl Default for WorkflowParser {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowParser {
    pub fn new() -> Self {
        Self {
            template_resolver: super::template::TemplateResolver::new(),
        }
    }

    pub fn parse_file<P: AsRef<Path>>(&mut self, path: P) -> Result<Workflow> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read workflow file: {:?}", path.as_ref()))?;

        self.parse_string(&content)
    }

    pub fn parse_string(&mut self, content: &str) -> Result<Workflow> {
        let mut workflow: Workflow =
            serde_yaml::from_str(content).context("Failed to parse workflow YAML")?;

        if let Some(extends) = &workflow.extends {
            workflow = self
                .template_resolver
                .resolve_inheritance(&workflow, extends)?;
        }

        self.validate_workflow(&workflow)?;

        Ok(workflow)
    }

    fn validate_workflow(&self, workflow: &Workflow) -> Result<()> {
        if workflow.name.is_empty() {
            anyhow::bail!("Workflow name cannot be empty");
        }

        if workflow.version.is_empty() {
            anyhow::bail!("Workflow version cannot be empty");
        }

        if workflow.stages.is_empty() {
            anyhow::bail!("Workflow must have at least one stage");
        }

        for stage in &workflow.stages {
            if stage.name.is_empty() {
                anyhow::bail!("Stage name cannot be empty");
            }
            if stage.steps.is_empty() {
                anyhow::bail!("Stage '{}' must have at least one step", stage.name);
            }

            for step in &stage.steps {
                if step.name.is_empty() {
                    anyhow::bail!("Step name cannot be empty in stage '{}'", stage.name);
                }

                if step.command.is_none() && step.step_type.is_none() {
                    anyhow::bail!("Step '{}' must have either a command or type", step.name);
                }
            }
        }

        Ok(())
    }

    pub fn expand_variables(
        &self,
        workflow: &Workflow,
        context: &WorkflowContext,
    ) -> Result<Workflow> {
        let mut template_engine = tera::Tera::default();
        let mut tera_context = tera::Context::new();

        tera_context.insert("parameters", &context.parameters);
        tera_context.insert("variables", &context.variables);
        tera_context.insert("outputs", &context.outputs);
        tera_context.insert("project", &context.project);
        tera_context.insert("spec", &context.spec_id);

        let serialized = serde_json::to_string(workflow)?;
        let expanded = template_engine
            .render_str(&serialized, &tera_context)
            .context("Failed to expand workflow variables")?;

        let expanded_workflow: Workflow =
            serde_json::from_str(&expanded).context("Failed to parse expanded workflow")?;

        Ok(expanded_workflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_workflow() {
        let yaml = r#"
name: test-workflow
version: 1.0.0
description: Test workflow

triggers:
  - type: manual

parameters: {}

stages:
  - name: test-stage
    steps:
      - name: test-step
        command: echo "test"

on_success: []
on_failure: []
"#;

        let mut parser = WorkflowParser::new();
        let workflow = parser.parse_string(yaml).unwrap();

        assert_eq!(workflow.name, "test-workflow");
        assert_eq!(workflow.version, "1.0.0");
        assert_eq!(workflow.stages.len(), 1);
        assert_eq!(workflow.stages[0].steps.len(), 1);
    }

    #[test]
    fn test_validate_empty_name() {
        let yaml = r#"
name: ""
version: 1.0.0

triggers:
  - type: manual

parameters: {}

stages:
  - name: test-stage
    steps:
      - name: test-step
        command: echo "test"

on_success: []
on_failure: []
"#;

        let mut parser = WorkflowParser::new();
        let result = parser.parse_string(yaml);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Workflow name cannot be empty"));
    }
}
