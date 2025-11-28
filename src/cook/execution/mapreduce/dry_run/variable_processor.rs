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
        // Build captured outputs from setup phase
        let captured_outputs: HashMap<String, String> = setup_phase
            .map(|setup| {
                setup
                    .capture_outputs
                    .keys()
                    .map(|var_name| {
                        (
                            format!("setup.{}", var_name),
                            "<captured from setup command>".to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Combine with default setup variables
        let default_vars = [
            ("setup.phase".to_string(), "setup".to_string()),
            ("setup.status".to_string(), "completed".to_string()),
        ];

        default_vars.into_iter().chain(captured_outputs).collect()
    }

    /// Extract variables for sample work items
    fn extract_item_variables(
        &self,
        work_items: &[Value],
        sample_size: usize,
    ) -> Result<Vec<HashMap<String, String>>, DryRunError> {
        work_items
            .iter()
            .take(sample_size)
            .enumerate()
            .map(|(idx, item)| self.extract_single_item_variables(idx, item))
            .collect()
    }

    /// Extract variables from a single work item
    fn extract_single_item_variables(
        &self,
        idx: usize,
        item: &Value,
    ) -> Result<HashMap<String, String>, DryRunError> {
        // Start with item index
        let index_var = ("item.index".to_string(), idx.to_string());

        // Extract item-specific variables based on type
        let item_vars: Vec<(String, String)> = match item {
            Value::Object(map) => map
                .iter()
                .map(|(key, value)| (format!("item.{}", key), self.value_to_string(value)))
                .collect(),
            Value::String(s) => vec![
                ("item".to_string(), s.clone()),
                ("item.value".to_string(), s.clone()),
            ],
            Value::Number(n) => vec![
                ("item".to_string(), n.to_string()),
                ("item.value".to_string(), n.to_string()),
            ],
            _ => vec![("item".to_string(), serde_json::to_string(item)?)],
        };

        Ok(std::iter::once(index_var).chain(item_vars).collect())
    }

    /// Extract variables available in reduce phase
    fn extract_reduce_variables(
        &self,
        reduce_phase: Option<&ReducePhase>,
        work_item_count: usize,
    ) -> HashMap<String, String> {
        reduce_phase
            .map(|_| {
                [
                    (
                        "map.successful".to_string(),
                        "<count of successful items>".to_string(),
                    ),
                    (
                        "map.failed".to_string(),
                        "<count of failed items>".to_string(),
                    ),
                    ("map.total".to_string(), work_item_count.to_string()),
                    (
                        "map.results".to_string(),
                        "<aggregated results>".to_string(),
                    ),
                    ("reduce.phase".to_string(), "reduce".to_string()),
                ]
                .into_iter()
                .collect()
            })
            .unwrap_or_default()
    }

    /// Find undefined variable references
    fn find_undefined_references(
        &self,
        map_phase: &MapPhase,
        setup_variables: &HashMap<String, String>,
        item_variables: &[HashMap<String, String>],
        reduce_variables: &HashMap<String, String>,
    ) -> Vec<String> {
        // Collect all defined variables using functional chaining
        let shell_vars = [
            "shell.output",
            "shell.stdout",
            "shell.stderr",
            "shell.exit_code",
        ]
        .into_iter()
        .map(String::from);

        let all_defined: HashSet<String> = setup_variables
            .keys()
            .cloned()
            .chain(reduce_variables.keys().cloned())
            .chain(
                item_variables
                    .first()
                    .into_iter()
                    .flat_map(|item| item.keys().cloned()),
            )
            .chain(shell_vars)
            .collect();

        // Find undefined references using functional composition
        map_phase
            .agent_template
            .iter()
            .flat_map(|cmd| self.extract_variable_references(cmd))
            .filter(|var_ref| !all_defined.contains(var_ref) && !self.is_dynamic_variable(var_ref))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Extract variable references from a command
    fn extract_variable_references(
        &self,
        command: &crate::cook::workflow::WorkflowStep,
    ) -> Vec<String> {
        let regex = regex::Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex");

        // Check all command types for variables using functional composition
        [command.claude.as_deref(), command.shell.as_deref()]
            .into_iter()
            .flatten()
            .flat_map(|text| {
                regex
                    .captures_iter(text)
                    .map(|cap| cap[1].to_string())
                    .collect::<Vec<_>>()
            })
            .collect()
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
