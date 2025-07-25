use anyhow::{Context, Result};
use serde_yaml;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{Stage, Step, Workflow};

pub struct TemplateResolver {
    template_dir: PathBuf,
    cache: HashMap<String, Workflow>,
}

impl TemplateResolver {
    pub fn new() -> Self {
        Self {
            template_dir: PathBuf::from(".mmm/workflow-templates"),
            cache: HashMap::new(),
        }
    }

    pub fn with_template_dir<P: AsRef<Path>>(template_dir: P) -> Self {
        Self {
            template_dir: template_dir.as_ref().to_path_buf(),
            cache: HashMap::new(),
        }
    }

    pub fn resolve_inheritance(
        &mut self,
        workflow: &Workflow,
        parent_name: &str,
    ) -> Result<Workflow> {
        let parent = self.load_template(parent_name)?;
        let merged = self.merge_workflows(&parent, workflow)?;
        Ok(merged)
    }

    fn load_template(&mut self, template_name: &str) -> Result<Workflow> {
        if let Some(cached) = self.cache.get(template_name) {
            return Ok(cached.clone());
        }

        let template_path = self.template_dir.join(format!("{}.yaml", template_name));
        let content = fs::read_to_string(&template_path)
            .with_context(|| format!("Failed to read template: {:?}", template_path))?;

        let mut template: Workflow =
            serde_yaml::from_str(&content).context("Failed to parse template YAML")?;

        if let Some(parent) = &template.extends {
            template = self.resolve_inheritance(&template, parent)?;
        }

        self.cache
            .insert(template_name.to_string(), template.clone());
        Ok(template)
    }

    fn merge_workflows(&self, parent: &Workflow, child: &Workflow) -> Result<Workflow> {
        let mut merged = parent.clone();

        merged.name = child.name.clone();

        if child.description.is_some() {
            merged.description = child.description.clone();
        }

        merged.version = child.version.clone();

        if !child.triggers.is_empty() {
            merged.triggers = child.triggers.clone();
        }

        for (key, value) in &child.parameters {
            merged.parameters.insert(key.clone(), value.clone());
        }

        if !child.stages.is_empty() {
            merged.stages = self.merge_stages(&parent.stages, &child.stages)?;
        }

        if !child.on_success.is_empty() {
            merged.on_success = child.on_success.clone();
        }

        if !child.on_failure.is_empty() {
            merged.on_failure = child.on_failure.clone();
        }

        merged.extends = None;

        Ok(merged)
    }

    fn merge_stages(&self, parent_stages: &[Stage], child_stages: &[Stage]) -> Result<Vec<Stage>> {
        let mut merged_stages = Vec::new();
        let mut processed_names = std::collections::HashSet::new();

        for child_stage in child_stages {
            if child_stage.name.starts_with("use:") {
                let ref_name = child_stage.name.strip_prefix("use:").unwrap().trim();
                if let Some(parent_stage) = parent_stages.iter().find(|s| s.name == ref_name) {
                    let mut stage = parent_stage.clone();

                    if child_stage.condition.is_some() {
                        stage.condition = child_stage.condition.clone();
                    }
                    if child_stage.parallel.is_some() {
                        stage.parallel = child_stage.parallel;
                    }

                    merged_stages.push(stage);
                    processed_names.insert(ref_name.to_string());
                }
            } else {
                let merged_stage = if let Some(parent_stage) =
                    parent_stages.iter().find(|s| s.name == child_stage.name)
                {
                    self.merge_stage(parent_stage, child_stage)?
                } else {
                    child_stage.clone()
                };

                merged_stages.push(merged_stage);
                processed_names.insert(child_stage.name.clone());
            }
        }

        for parent_stage in parent_stages {
            if !processed_names.contains(&parent_stage.name) {
                merged_stages.push(parent_stage.clone());
            }
        }

        Ok(merged_stages)
    }

    fn merge_stage(&self, parent_stage: &Stage, child_stage: &Stage) -> Result<Stage> {
        let mut merged = parent_stage.clone();

        merged.name = child_stage.name.clone();

        if child_stage.condition.is_some() {
            merged.condition = child_stage.condition.clone();
        }

        if child_stage.parallel.is_some() {
            merged.parallel = child_stage.parallel;
        }

        if child_stage.for_each.is_some() {
            merged.for_each = child_stage.for_each.clone();
        }

        if !child_stage.steps.is_empty() {
            merged.steps = self.merge_steps(&parent_stage.steps, &child_stage.steps)?;
        }

        Ok(merged)
    }

    fn merge_steps(&self, parent_steps: &[Step], child_steps: &[Step]) -> Result<Vec<Step>> {
        let mut merged_steps = Vec::new();
        let mut processed_names = std::collections::HashSet::new();

        for child_step in child_steps {
            if child_step.name.starts_with("use:") {
                let ref_name = child_step.name.strip_prefix("use:").unwrap().trim();

                let parts: Vec<&str> = ref_name.split('.').collect();
                if parts.len() == 2 && parts[0] == "common_steps" {
                    if let Some(parent_step) = parent_steps.iter().find(|s| s.name == parts[1]) {
                        merged_steps.push(parent_step.clone());
                        processed_names.insert(parts[1].to_string());
                    }
                }
            } else {
                merged_steps.push(child_step.clone());
                processed_names.insert(child_step.name.clone());
            }
        }

        Ok(merged_steps)
    }

    pub fn validate_template(&self, workflow: &Workflow) -> Result<()> {
        if workflow.name == "base" || workflow.name.ends_with("-base") {
            if workflow.stages.is_empty() {
                anyhow::bail!("Base template must define at least one stage or common steps");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{FailureStrategy, Parameter, ParameterType, Trigger, TriggerType};

    #[test]
    fn test_workflow_merge() {
        let parent = Workflow {
            name: "base".to_string(),
            description: Some("Base template".to_string()),
            version: "1.0.0".to_string(),
            triggers: vec![Trigger {
                trigger_type: TriggerType::Manual,
                filter: None,
                cron: None,
            }],
            parameters: HashMap::from([(
                "timeout".to_string(),
                Parameter {
                    param_type: ParameterType::Integer,
                    default: Some(serde_json::json!(3600)),
                    description: Some("Timeout in seconds".to_string()),
                    required: Some(false),
                },
            )]),
            stages: vec![Stage {
                name: "test".to_string(),
                condition: None,
                parallel: Some(false),
                for_each: None,
                steps: vec![Step {
                    name: "run-tests".to_string(),
                    step_type: None,
                    command: Some("cargo test".to_string()),
                    condition: None,
                    outputs: None,
                    on_failure: Some(FailureStrategy::Simple("fail".to_string())),
                    max_retries: None,
                    timeout: None,
                    message: None,
                }],
            }],
            on_success: vec![],
            on_failure: vec![],
            extends: None,
        };

        let child = Workflow {
            name: "feature".to_string(),
            description: None,
            version: "1.0.0".to_string(),
            triggers: vec![],
            parameters: HashMap::from([(
                "feature_flag".to_string(),
                Parameter {
                    param_type: ParameterType::Boolean,
                    default: Some(serde_json::json!(true)),
                    description: Some("Feature flag".to_string()),
                    required: Some(false),
                },
            )]),
            stages: vec![
                Stage {
                    name: "build".to_string(),
                    condition: None,
                    parallel: Some(false),
                    for_each: None,
                    steps: vec![Step {
                        name: "cargo-build".to_string(),
                        step_type: None,
                        command: Some("cargo build".to_string()),
                        condition: None,
                        outputs: None,
                        on_failure: None,
                        max_retries: None,
                        timeout: None,
                        message: None,
                    }],
                },
                Stage {
                    name: "test".to_string(),
                    condition: None,
                    parallel: Some(true),
                    for_each: None,
                    steps: vec![Step {
                        name: "run-tests".to_string(),
                        step_type: None,
                        command: Some("cargo test --all".to_string()),
                        condition: None,
                        outputs: None,
                        on_failure: Some(FailureStrategy::Simple("retry".to_string())),
                        max_retries: Some(2),
                        timeout: None,
                        message: None,
                    }],
                },
            ],
            on_success: vec![],
            on_failure: vec![],
            extends: Some("base".to_string()),
        };

        let resolver = TemplateResolver::new();
        let merged = resolver.merge_workflows(&parent, &child).unwrap();

        assert_eq!(merged.name, "feature");
        assert_eq!(merged.parameters.len(), 2);
        assert!(merged.parameters.contains_key("timeout"));
        assert!(merged.parameters.contains_key("feature_flag"));

        assert_eq!(merged.stages.len(), 2);

        let test_stage = merged.stages.iter().find(|s| s.name == "test").unwrap();
        assert_eq!(test_stage.parallel, Some(true));
        assert_eq!(
            test_stage.steps[0].command,
            Some("cargo test --all".to_string())
        );
        assert_eq!(test_stage.steps[0].max_retries, Some(2));
    }
}
