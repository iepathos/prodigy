use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Standard variable names that work in ALL execution modes
/// These are the ONLY variable names that should be used
pub struct StandardVariables;

impl StandardVariables {
    // Input variables - consistent regardless of source
    pub const ITEM: &'static str = "item"; // Current item being processed
    pub const INDEX: &'static str = "item_index"; // Zero-based index
    pub const TOTAL: &'static str = "item_total"; // Total number of items

    // For backwards compatibility during migration
    pub const ITEM_VALUE: &'static str = "item.value"; // The actual value
    pub const ITEM_PATH: &'static str = "item.path"; // For file inputs
    pub const ITEM_NAME: &'static str = "item.name"; // Display name

    // Workflow context variables
    pub const WORKFLOW_NAME: &'static str = "workflow.name";
    pub const WORKFLOW_ID: &'static str = "workflow.id";
    pub const ITERATION: &'static str = "workflow.iteration";

    // Step context variables
    pub const STEP_NAME: &'static str = "step.name";
    pub const STEP_INDEX: &'static str = "step.index";

    // Output capture variables
    pub const LAST_OUTPUT: &'static str = "last.output";
    pub const LAST_EXIT_CODE: &'static str = "last.exit_code";

    // MapReduce specific (only available in those contexts)
    pub const MAP_KEY: &'static str = "map.key"; // Key for map output
    pub const MAP_RESULTS: &'static str = "map.results"; // Aggregated map results
    pub const WORKER_ID: &'static str = "worker.id"; // Parallel worker ID
}

/// Represents different types of execution inputs
#[derive(Debug, Clone)]
pub enum ExecutionInput {
    Argument(String),
    FilePath(String),
    JsonObject(Value),
}

/// Execution mode for the workflow
#[derive(Debug, Clone)]
pub enum ExecutionMode {
    Standard,
    WithArguments,
    WithFilePattern,
    MapReduce,
}

/// Unified variable context that ALL paths use
#[derive(Debug, Clone)]
pub struct VariableContext {
    variables: HashMap<String, Value>, // ALL variables stored here
    aliases: HashMap<String, String>,  // For backwards compatibility
}

impl VariableContext {
    /// Create context for any execution mode with STANDARD variable names
    pub fn from_execution_input(
        _mode: &ExecutionMode,
        input: &ExecutionInput,
        index: usize,
        total: usize,
    ) -> Self {
        let mut variables = HashMap::new();
        let mut aliases = HashMap::new();

        // Standard variables that work everywhere
        match input {
            ExecutionInput::Argument(arg) => {
                variables.insert(StandardVariables::ITEM.into(), json!(arg));
                variables.insert(StandardVariables::ITEM_VALUE.into(), json!(arg));
                // Legacy compatibility
                aliases.insert("ARG".into(), StandardVariables::ITEM_VALUE.into());
                aliases.insert("ARGUMENT".into(), StandardVariables::ITEM_VALUE.into());
            }
            ExecutionInput::FilePath(path) => {
                variables.insert(StandardVariables::ITEM.into(), json!(path));
                variables.insert(StandardVariables::ITEM_PATH.into(), json!(path));
                // Legacy compatibility
                aliases.insert("FILE".into(), StandardVariables::ITEM_PATH.into());
                aliases.insert("FILE_PATH".into(), StandardVariables::ITEM_PATH.into());
            }
            ExecutionInput::JsonObject(obj) => {
                // MapReduce items - use the SAME variable names!
                variables.insert(StandardVariables::ITEM.into(), obj.clone());
                // Flatten for convenience
                if let Some(path) = obj.get("file_path") {
                    variables.insert(StandardVariables::ITEM_PATH.into(), path.clone());
                }
                if let Some(name) = obj.get("name") {
                    variables.insert(StandardVariables::ITEM_NAME.into(), name.clone());
                }
            }
        }

        // Always set standard context variables
        variables.insert(StandardVariables::INDEX.into(), json!(index));
        variables.insert(StandardVariables::TOTAL.into(), json!(total));

        Self { variables, aliases }
    }

    /// Create an empty context for testing
    pub fn empty() -> Self {
        Self {
            variables: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// Add a variable to the context
    pub fn add_variable(&mut self, key: impl Into<String>, value: Value) {
        self.variables.insert(key.into(), value);
    }

    /// Add an alias for backwards compatibility
    pub fn add_alias(&mut self, old_name: impl Into<String>, new_name: impl Into<String>) {
        self.aliases.insert(old_name.into(), new_name.into());
    }

    /// Get a variable value
    pub fn get(&self, key: &str) -> Option<&Value> {
        // Check if it's an alias first
        if let Some(actual_key) = self.aliases.get(key) {
            self.variables.get(actual_key)
        } else {
            self.variables.get(key)
        }
    }

    /// Use the SAME interpolation engine for ALL paths
    /// This ensures consistent behavior across all execution modes
    pub fn interpolate(&self, template: &str) -> Result<String> {
        // First resolve aliases for backwards compatibility
        let template = self.resolve_aliases(template);

        // Convert our variables to InterpolationContext
        // We need to organize nested variables properly
        let mut context = InterpolationContext::new();
        let mut nested_objects: HashMap<String, HashMap<String, Value>> = HashMap::new();

        for (key, value) in &self.variables {
            // Handle nested keys like "item.value" by grouping them
            if key.contains('.') {
                let parts: Vec<&str> = key.split('.').collect();
                if parts.len() == 2 {
                    // Add to nested object
                    nested_objects
                        .entry(parts[0].to_string())
                        .or_default()
                        .insert(parts[1].to_string(), value.clone());
                } else {
                    // Complex nesting not supported yet
                    context.set(key.clone(), value.clone());
                }
            } else {
                context.set(key.clone(), value.clone());
            }
        }

        // Add nested objects to context
        for (obj_name, fields) in nested_objects {
            context.set(obj_name, json!(fields));
        }

        // Use the existing MapReduce InterpolationEngine for ALL paths!
        // This gives everyone nested access, defaults, etc.
        let mut engine = InterpolationEngine::new(false);

        engine
            .interpolate(&template, &context)
            .context("Failed to interpolate variables")
    }

    fn resolve_aliases(&self, template: &str) -> String {
        self.aliases
            .iter()
            .fold(template.to_string(), |acc, (old, new)| {
                acc.replace(&format!("${{{}}}", old), &format!("${{{}}}", new))
                    .replace(&format!("${}", old), &format!("${}", new))
            })
    }

    /// Convert to a format the InterpolationEngine can use
    pub fn to_interpolation_context(&self) -> InterpolationContext {
        let mut context = InterpolationContext::new();
        for (key, value) in &self.variables {
            context.set(key.clone(), value.clone());
        }
        context
    }

    /// Set workflow metadata
    pub fn set_workflow_metadata(&mut self, name: &str, id: &str, iteration: usize) {
        self.variables
            .insert(StandardVariables::WORKFLOW_NAME.into(), json!(name));
        self.variables
            .insert(StandardVariables::WORKFLOW_ID.into(), json!(id));
        self.variables
            .insert(StandardVariables::ITERATION.into(), json!(iteration));
    }

    /// Set step metadata
    pub fn set_step_metadata(&mut self, name: &str, index: usize) {
        self.variables
            .insert(StandardVariables::STEP_NAME.into(), json!(name));
        self.variables
            .insert(StandardVariables::STEP_INDEX.into(), json!(index));
    }

    /// Set command output results
    pub fn set_last_output(&mut self, output: &str, exit_code: i32) {
        self.variables
            .insert(StandardVariables::LAST_OUTPUT.into(), json!(output));
        self.variables
            .insert(StandardVariables::LAST_EXIT_CODE.into(), json!(exit_code));
    }

    /// Set MapReduce specific variables
    pub fn set_mapreduce_metadata(&mut self, worker_id: Option<usize>, map_key: Option<&str>) {
        if let Some(id) = worker_id {
            self.variables
                .insert(StandardVariables::WORKER_ID.into(), json!(id));
        }
        if let Some(key) = map_key {
            self.variables
                .insert(StandardVariables::MAP_KEY.into(), json!(key));
        }
    }

    /// Set aggregated map results for reduce phase
    pub fn set_map_results(&mut self, results: Value) {
        self.variables
            .insert(StandardVariables::MAP_RESULTS.into(), results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_variables_from_argument() {
        let input = ExecutionInput::Argument("test_arg".to_string());
        let ctx =
            VariableContext::from_execution_input(&ExecutionMode::WithArguments, &input, 0, 3);

        assert_eq!(ctx.get("item"), Some(&json!("test_arg")));
        assert_eq!(ctx.get("item.value"), Some(&json!("test_arg")));
        assert_eq!(ctx.get("item_index"), Some(&json!(0)));
        assert_eq!(ctx.get("item_total"), Some(&json!(3)));

        // Test legacy alias
        assert_eq!(ctx.get("ARG"), Some(&json!("test_arg")));
    }

    #[test]
    fn test_standard_variables_from_file() {
        let input = ExecutionInput::FilePath("/path/to/file.txt".to_string());
        let ctx =
            VariableContext::from_execution_input(&ExecutionMode::WithFilePattern, &input, 1, 5);

        assert_eq!(ctx.get("item"), Some(&json!("/path/to/file.txt")));
        assert_eq!(ctx.get("item.path"), Some(&json!("/path/to/file.txt")));
        assert_eq!(ctx.get("item_index"), Some(&json!(1)));
        assert_eq!(ctx.get("item_total"), Some(&json!(5)));

        // Test legacy aliases
        assert_eq!(ctx.get("FILE"), Some(&json!("/path/to/file.txt")));
        assert_eq!(ctx.get("FILE_PATH"), Some(&json!("/path/to/file.txt")));
    }

    #[test]
    fn test_standard_variables_from_json() {
        let obj = json!({
            "file_path": "/path/to/data.json",
            "name": "Test Item",
            "value": 42
        });
        let input = ExecutionInput::JsonObject(obj.clone());
        let ctx = VariableContext::from_execution_input(&ExecutionMode::MapReduce, &input, 2, 10);

        assert_eq!(ctx.get("item"), Some(&obj));
        assert_eq!(ctx.get("item.path"), Some(&json!("/path/to/data.json")));
        assert_eq!(ctx.get("item.name"), Some(&json!("Test Item")));
        assert_eq!(ctx.get("item_index"), Some(&json!(2)));
        assert_eq!(ctx.get("item_total"), Some(&json!(10)));
    }

    #[test]
    fn test_variable_interpolation() {
        let input = ExecutionInput::Argument("test_file.txt".to_string());
        let mut ctx =
            VariableContext::from_execution_input(&ExecutionMode::WithArguments, &input, 0, 1);

        ctx.set_workflow_metadata("test_workflow", "wf-123", 1);
        ctx.set_step_metadata("process_file", 0);

        let template = "Processing ${item.value} in workflow ${workflow.name} (step ${step.index})";
        let result = ctx.interpolate(template).unwrap();

        assert_eq!(
            result,
            "Processing test_file.txt in workflow test_workflow (step 0)"
        );
    }

    #[test]
    fn test_alias_resolution() {
        let input = ExecutionInput::FilePath("/data/file.txt".to_string());
        let ctx =
            VariableContext::from_execution_input(&ExecutionMode::WithFilePattern, &input, 0, 1);

        // Test that legacy variable names work through aliases
        let template = "File: ${FILE} or ${FILE_PATH} or ${item.path}";
        let resolved = ctx.resolve_aliases(template);

        assert!(resolved.contains("${item.path}"));
        assert_eq!(resolved.matches("${item.path}").count(), 3);
    }

    #[test]
    fn test_mapreduce_metadata() {
        let mut ctx = VariableContext::empty();

        ctx.set_mapreduce_metadata(Some(3), Some("key_123"));
        ctx.set_map_results(json!({"total": 100, "processed": 95}));

        assert_eq!(ctx.get("worker.id"), Some(&json!(3)));
        assert_eq!(ctx.get("map.key"), Some(&json!("key_123")));
        assert_eq!(
            ctx.get("map.results"),
            Some(&json!({"total": 100, "processed": 95}))
        );
    }

    #[test]
    fn test_output_capture() {
        let mut ctx = VariableContext::empty();

        ctx.set_last_output("Command completed successfully", 0);

        assert_eq!(
            ctx.get("last.output"),
            Some(&json!("Command completed successfully"))
        );
        assert_eq!(ctx.get("last.exit_code"), Some(&json!(0)));
    }
}

