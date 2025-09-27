//! Variable processing and preview for dry-run mode
//!
//! Processes and previews variable interpolation for MapReduce workflows.

use super::types::{DryRunError, VariablePreview};
use crate::cook::execution::mapreduce::{MapPhase, ReducePhase, SetupPhase};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// Processor for variable interpolation preview
pub struct VariableProcessor;

impl VariableProcessor {
    /// Create a new variable processor
    pub fn new() -> Self {
        Self
    }

    /// Create a preview of variable interpolation
    pub fn create_preview(
        &self,
        map_phase: &MapPhase,
        work_items: &[Value],
        setup_phase: Option<&SetupPhase>,
        reduce_phase: Option<&ReducePhase>,
    ) -> Result<VariablePreview, DryRunError> {
        debug!("Creating variable interpolation preview");

        let setup_variables = self.extract_setup_variables(setup_phase);
        let item_variables = self.extract_item_variables(work_items, 5)?; // Sample first 5 items
        let reduce_variables = self.extract_reduce_variables(reduce_phase, work_items.len());
        let undefined_references = self.find_undefined_references(
            map_phase,
            &setup_variables,
            &item_variables,
            &reduce_variables,
        );

        Ok(VariablePreview {
            setup_variables,
            item_variables,
            reduce_variables,
            undefined_references,
        })
    }

    /// Extract variables available from setup phase
    fn extract_setup_variables(&self, setup_phase: Option<&SetupPhase>) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        if let Some(setup) = setup_phase {
            // Setup phase can capture outputs
            for (var_name, _) in &setup.capture_outputs {
                variables.insert(
                    format!("setup.{}", var_name),
                    "<captured from setup command>".to_string(),
                );
            }
        }

        // Add default setup variables
        variables.insert("setup.phase".to_string(), "setup".to_string());
        variables.insert("setup.status".to_string(), "completed".to_string());

        variables
    }

    /// Extract variables for sample work items
    fn extract_item_variables(
        &self,
        work_items: &[Value],
        sample_size: usize,
    ) -> Result<Vec<HashMap<String, String>>, DryRunError> {
        let mut item_variables = Vec::new();

        for (idx, item) in work_items.iter().take(sample_size).enumerate() {
            let mut vars = HashMap::new();

            // Add item index
            vars.insert("item.index".to_string(), idx.to_string());

            // Extract fields from the item
            match item {
                Value::Object(map) => {
                    for (key, value) in map {
                        let var_name = format!("item.{}", key);
                        let var_value = self.value_to_string(value);
                        vars.insert(var_name, var_value);
                    }
                }
                Value::String(s) => {
                    vars.insert("item".to_string(), s.clone());
                    vars.insert("item.value".to_string(), s.clone());
                }
                Value::Number(n) => {
                    vars.insert("item".to_string(), n.to_string());
                    vars.insert("item.value".to_string(), n.to_string());
                }
                _ => {
                    vars.insert("item".to_string(), serde_json::to_string(item)?);
                }
            }

            item_variables.push(vars);
        }

        Ok(item_variables)
    }

    /// Extract variables available in reduce phase
    fn extract_reduce_variables(
        &self,
        reduce_phase: Option<&ReducePhase>,
        work_item_count: usize,
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        if reduce_phase.is_some() {
            // Map phase results variables
            variables.insert(
                "map.successful".to_string(),
                "<count of successful items>".to_string(),
            );
            variables.insert(
                "map.failed".to_string(),
                "<count of failed items>".to_string(),
            );
            variables.insert("map.total".to_string(), work_item_count.to_string());
            variables.insert(
                "map.results".to_string(),
                "<aggregated results>".to_string(),
            );

            // Reduce phase variables
            variables.insert("reduce.phase".to_string(), "reduce".to_string());
        }

        variables
    }

    /// Find undefined variable references
    fn find_undefined_references(
        &self,
        map_phase: &MapPhase,
        setup_variables: &HashMap<String, String>,
        item_variables: &[HashMap<String, String>],
        reduce_variables: &HashMap<String, String>,
    ) -> Vec<String> {
        let mut undefined = HashSet::new();
        let mut all_defined = HashSet::new();

        // Collect all defined variables
        for var in setup_variables.keys() {
            all_defined.insert(var.clone());
        }

        for var in reduce_variables.keys() {
            all_defined.insert(var.clone());
        }

        // For item variables, use the first sample as representative
        if let Some(first_item) = item_variables.first() {
            for var in first_item.keys() {
                all_defined.insert(var.clone());
            }
        }

        // Add shell output variable (available after shell commands)
        all_defined.insert("shell.output".to_string());
        all_defined.insert("shell.stdout".to_string());
        all_defined.insert("shell.stderr".to_string());
        all_defined.insert("shell.exit_code".to_string());

        // Check variable references in commands
        for command in &map_phase.agent_template {
            let references = self.extract_variable_references(command);
            for var_ref in references {
                if !all_defined.contains(&var_ref) && !self.is_dynamic_variable(&var_ref) {
                    undefined.insert(var_ref);
                }
            }
        }

        undefined.into_iter().collect()
    }

    /// Extract variable references from a command
    fn extract_variable_references(
        &self,
        command: &crate::cook::workflow::WorkflowStep,
    ) -> Vec<String> {
        let mut references = Vec::new();
        let regex = regex::Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex");

        // Check all command types for variables
        let command_texts = vec![
            command.claude.as_deref(),
            command.shell.as_deref(),
            command
                .goal_seek
                .as_ref()
                .and_then(|gs| gs.claude.as_deref()),
            command
                .goal_seek
                .as_ref()
                .map(|gs| gs.validate.as_str()),
        ];

        for text in command_texts.into_iter().flatten() {
            for cap in regex.captures_iter(text) {
                references.push(cap[1].to_string());
            }
        }

        references
    }

    /// Check if a variable is dynamically generated
    fn is_dynamic_variable(&self, var_name: &str) -> bool {
        // Some variables are dynamically generated and not known at dry-run time
        let dynamic_prefixes = [
            "env.",       // Environment variables
            "runtime.",   // Runtime variables
            "timestamp.", // Timestamp variables
            "random.",    // Random values
        ];

        dynamic_prefixes
            .iter()
            .any(|prefix| var_name.starts_with(prefix))
    }

    /// Convert JSON value to string for preview
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(arr) => format!("[array with {} items]", arr.len()),
            Value::Object(obj) => format!("[object with {} fields]", obj.len()),
        }
    }
}

impl Default for VariableProcessor {
    fn default() -> Self {
        Self::new()
    }
}
