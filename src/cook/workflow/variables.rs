use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

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

/// Format for captured output
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaptureFormat {
    /// Raw string output (default)
    #[default]
    String,
    /// Parse as JSON
    Json,
    /// Split into array of lines
    Lines,
    /// Parse as number
    Number,
    /// Parse as boolean
    Boolean,
}

/// Which streams to capture from command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStreams {
    #[serde(default = "default_true")]
    pub stdout: bool,
    #[serde(default)]
    pub stderr: bool,
    #[serde(default = "default_true")]
    pub exit_code: bool,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default = "default_true")]
    pub duration: bool,
}

impl Default for CaptureStreams {
    fn default() -> Self {
        Self {
            stdout: true,
            stderr: false,
            exit_code: true,
            success: true,
            duration: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Captured value from command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CapturedValue {
    /// Simple string value
    String(String),
    /// Numeric value
    Number(f64),
    /// Boolean value
    Boolean(bool),
    /// JSON value
    Json(Value),
    /// Array of values
    Array(Vec<CapturedValue>),
    /// Object with key-value pairs
    Object(HashMap<String, CapturedValue>),
}

impl std::fmt::Display for CapturedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapturedValue::String(s) => write!(f, "{}", s),
            CapturedValue::Number(n) => write!(f, "{}", n),
            CapturedValue::Boolean(b) => write!(f, "{}", b),
            CapturedValue::Json(j) => write!(f, "{}", j),
            CapturedValue::Array(arr) => {
                let strings: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", strings.join(", "))
            }
            CapturedValue::Object(map) => {
                let pairs: Vec<String> = map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{{{}}}", pairs.join(", "))
            }
        }
    }
}

impl CapturedValue {

    /// Convert to JSON value
    pub fn to_json(&self) -> Value {
        match self {
            CapturedValue::String(s) => Value::String(s.clone()),
            CapturedValue::Number(n) => json!(n),
            CapturedValue::Boolean(b) => Value::Bool(*b),
            CapturedValue::Json(j) => j.clone(),
            CapturedValue::Array(arr) => {
                let values: Vec<Value> = arr.iter().map(|v| v.to_json()).collect();
                Value::Array(values)
            }
            CapturedValue::Object(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in map {
                    obj.insert(k.clone(), v.to_json());
                }
                Value::Object(obj)
            }
        }
    }
}

impl From<Value> for CapturedValue {
    fn from(value: Value) -> Self {
        match value {
            Value::String(s) => CapturedValue::String(s),
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    CapturedValue::Number(f)
                } else if let Some(i) = n.as_i64() {
                    CapturedValue::Number(i as f64)
                } else if let Some(u) = n.as_u64() {
                    CapturedValue::Number(u as f64)
                } else {
                    CapturedValue::Json(Value::Number(n))
                }
            }
            Value::Bool(b) => CapturedValue::Boolean(b),
            Value::Array(arr) => {
                let values: Vec<CapturedValue> = arr.into_iter().map(Into::into).collect();
                CapturedValue::Array(values)
            }
            Value::Object(obj) => {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    map.insert(k, v.into());
                }
                CapturedValue::Object(map)
            }
            Value::Null => CapturedValue::String("null".to_string()),
        }
    }
}

/// Command execution result for variable capture
pub struct CommandResult {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: i32,
    pub success: bool,
    pub duration: Duration,
}

/// Thread-safe variable storage for captured outputs
#[derive(Debug, Clone)]
pub struct VariableStore {
    variables: Arc<RwLock<HashMap<String, CapturedValue>>>,
    parent: Option<Arc<VariableStore>>,
}

impl Default for VariableStore {
    fn default() -> Self {
        Self::new()
    }
}

impl VariableStore {
    /// Create a new variable store
    pub fn new() -> Self {
        Self {
            variables: Arc::new(RwLock::new(HashMap::new())),
            parent: None,
        }
    }

    /// Create a child store with this store as parent
    pub fn child(&self) -> Self {
        Self {
            variables: Arc::new(RwLock::new(HashMap::new())),
            parent: Some(Arc::new(self.clone())),
        }
    }

    /// Set a variable value
    pub async fn set(&self, name: impl Into<String>, value: CapturedValue) {
        let mut vars = self.variables.write().await;
        vars.insert(name.into(), value);
    }

    /// Get a variable value
    pub fn get<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<CapturedValue>> + Send + 'a>>
    {
        Box::pin(async move {
            // Check local variables first
            let vars = self.variables.read().await;
            if let Some(value) = vars.get(name) {
                return Some(value.clone());
            }
            drop(vars); // Release lock before checking parent

            // Check parent if not found locally
            if let Some(parent) = &self.parent {
                parent.get(name).await
            } else {
                None
            }
        })
    }

    /// Capture command result into variables
    pub async fn capture_command_result(
        &self,
        name: &str,
        result: CommandResult,
        format: CaptureFormat,
        streams: &CaptureStreams,
    ) -> Result<()> {
        // Capture main output based on format
        if streams.stdout {
            let value = match format {
                CaptureFormat::String => {
                    CapturedValue::String(result.stdout.clone().unwrap_or_default())
                }
                CaptureFormat::Json => {
                    let json_str = result.stdout.as_deref().unwrap_or("null");
                    let json_value: Value = serde_json::from_str(json_str)
                        .map_err(|e| anyhow!("Failed to parse JSON output: {}", e))?;
                    CapturedValue::from(json_value)
                }
                CaptureFormat::Lines => {
                    let lines = result
                        .stdout
                        .as_deref()
                        .unwrap_or("")
                        .lines()
                        .map(|s| CapturedValue::String(s.to_string()))
                        .collect();
                    CapturedValue::Array(lines)
                }
                CaptureFormat::Number => {
                    let num_str = result.stdout.as_deref().unwrap_or("0").trim();
                    let num = num_str
                        .parse::<f64>()
                        .map_err(|e| anyhow!("Failed to parse number '{}': {}", num_str, e))?;
                    CapturedValue::Number(num)
                }
                CaptureFormat::Boolean => {
                    let bool_str = result.stdout.as_deref().unwrap_or("false").trim();
                    let val = bool_str.parse::<bool>().unwrap_or(result.success);
                    CapturedValue::Boolean(val)
                }
            };
            self.set(name, value).await;
        }

        // Capture stderr if requested
        if streams.stderr {
            if let Some(stderr) = &result.stderr {
                self.set(
                    format!("{}.stderr", name),
                    CapturedValue::String(stderr.clone()),
                )
                .await;
            }
        }

        // Capture metadata fields
        if streams.exit_code {
            self.set(
                format!("{}.exit_code", name),
                CapturedValue::Number(result.exit_code as f64),
            )
            .await;
        }

        if streams.success {
            self.set(
                format!("{}.success", name),
                CapturedValue::Boolean(result.success),
            )
            .await;
        }

        if streams.duration {
            self.set(
                format!("{}.duration", name),
                CapturedValue::Number(result.duration.as_secs_f64()),
            )
            .await;
        }

        Ok(())
    }

    /// Resolve a variable path (e.g., "var.field.subfield")
    pub async fn resolve_path(&self, path: &str) -> Result<CapturedValue> {
        let parts: Vec<&str> = path.split('.').collect();

        // Get base variable
        let base_value = self
            .get(parts[0])
            .await
            .ok_or_else(|| anyhow!("Variable '{}' not found", parts[0]))?;

        // Navigate nested path
        let mut current = base_value;
        for part in &parts[1..] {
            current = match current {
                CapturedValue::Json(ref obj) => {
                    if let Some(value) = obj.get(*part) {
                        value.clone().into()
                    } else {
                        return Err(anyhow!("Field '{}' not found in JSON object", part));
                    }
                }
                CapturedValue::Object(ref map) => map
                    .get(*part)
                    .ok_or_else(|| anyhow!("Field '{}' not found in object", part))?
                    .clone(),
                _ => {
                    return Err(anyhow!(
                        "Cannot access field '{}' on non-object value",
                        part
                    ))
                }
            };
        }

        Ok(current)
    }

    /// Get all variables as a HashMap for interpolation
    pub fn to_hashmap(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HashMap<String, String>> + Send + '_>>
    {
        Box::pin(async move {
            let mut result = HashMap::new();

            // Get parent variables first
            if let Some(parent) = &self.parent {
                result.extend(parent.to_hashmap().await);
            }

            // Override with local variables
            let vars = self.variables.read().await;
            for (key, value) in vars.iter() {
                result.insert(key.clone(), value.to_string());
            }

            result
        })
    }

    /// Get all variables as JSON for debugging
    pub fn to_json(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Value> + Send + '_>> {
        Box::pin(async move {
            let mut result = serde_json::Map::new();

            // Get parent variables first
            if let Some(parent) = &self.parent {
                if let Value::Object(parent_map) = parent.to_json().await {
                    result.extend(parent_map);
                }
            }

            // Override with local variables
            let vars = self.variables.read().await;
            for (key, value) in vars.iter() {
                result.insert(key.clone(), value.to_json());
            }

            Value::Object(result)
        })
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

    #[tokio::test]
    async fn test_variable_store_basic() {
        let store = VariableStore::new();

        // Set and get simple values
        store
            .set("name", CapturedValue::String("test".to_string()))
            .await;
        store.set("count", CapturedValue::Number(42.0)).await;
        store.set("enabled", CapturedValue::Boolean(true)).await;

        assert_eq!(store.get("name").await.unwrap().to_string(), "test");
        assert_eq!(store.get("count").await.unwrap().to_string(), "42");
        assert_eq!(store.get("enabled").await.unwrap().to_string(), "true");
    }

    #[tokio::test]
    async fn test_variable_store_hierarchy() {
        let parent = VariableStore::new();
        parent
            .set("parent_var", CapturedValue::String("parent".to_string()))
            .await;

        let child = parent.child();
        child
            .set("child_var", CapturedValue::String("child".to_string()))
            .await;

        // Child can access both parent and own variables
        assert_eq!(child.get("parent_var").await.unwrap().to_string(), "parent");
        assert_eq!(child.get("child_var").await.unwrap().to_string(), "child");

        // Parent cannot access child variables
        assert!(parent.get("child_var").await.is_none());
    }

    #[tokio::test]
    async fn test_capture_command_result() {
        let store = VariableStore::new();

        let result = CommandResult {
            stdout: Some("hello world".to_string()),
            stderr: Some("warning".to_string()),
            exit_code: 0,
            success: true,
            duration: Duration::from_secs(5),
        };

        store
            .capture_command_result(
                "cmd",
                result,
                CaptureFormat::String,
                &CaptureStreams {
                    stdout: true,
                    stderr: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(store.get("cmd").await.unwrap().to_string(), "hello world");
        assert_eq!(
            store.get("cmd.stderr").await.unwrap().to_string(),
            "warning"
        );
        assert_eq!(store.get("cmd.exit_code").await.unwrap().to_string(), "0");
        assert_eq!(store.get("cmd.success").await.unwrap().to_string(), "true");
    }

    #[tokio::test]
    async fn test_json_capture() {
        let store = VariableStore::new();

        let result = CommandResult {
            stdout: Some(r#"{"name": "test", "count": 42}"#.to_string()),
            stderr: None,
            exit_code: 0,
            success: true,
            duration: Duration::from_secs(1),
        };

        store
            .capture_command_result(
                "data",
                result,
                CaptureFormat::Json,
                &CaptureStreams::default(),
            )
            .await
            .unwrap();

        // Test nested path resolution
        let name = store.resolve_path("data.name").await.unwrap();
        assert_eq!(name.to_string(), "test");

        let count = store.resolve_path("data.count").await.unwrap();
        assert_eq!(count.to_string(), "42");
    }

    #[tokio::test]
    async fn test_lines_capture() {
        let store = VariableStore::new();

        let result = CommandResult {
            stdout: Some("line1\nline2\nline3".to_string()),
            stderr: None,
            exit_code: 0,
            success: true,
            duration: Duration::from_secs(1),
        };

        store
            .capture_command_result(
                "lines",
                result,
                CaptureFormat::Lines,
                &CaptureStreams::default(),
            )
            .await
            .unwrap();

        let lines = store.get("lines").await.unwrap();
        match lines {
            CapturedValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0].to_string(), "line1");
                assert_eq!(arr[1].to_string(), "line2");
                assert_eq!(arr[2].to_string(), "line3");
            }
            _ => panic!("Expected array value"),
        }
    }
}
