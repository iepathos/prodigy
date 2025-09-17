//! Enhanced variable interpolation system
//!
//! Provides comprehensive variable interpolation with support for:
//! - MapReduce aggregate variables
//! - Cross-phase variable passing
//! - Computed variables (environment, file, command output)
//! - Variable scoping and precedence
//! - Lazy evaluation and caching

use anyhow::{anyhow, Context, Result};
use lru::LruCache;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};

/// Variable types in the system
#[derive(Clone)]
pub enum Variable {
    /// Static value that doesn't change
    Static(Value),
    /// Computed variable that evaluates on demand
    Computed(Arc<dyn ComputedVariable>),
    /// Reference to another variable
    Reference(String),
    /// Aggregate computation over a collection
    Aggregate(AggregateType),
}

impl std::fmt::Debug for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variable::Static(v) => write!(f, "Static({:?})", v),
            Variable::Computed(_) => write!(f, "Computed(<dyn>)"),
            Variable::Reference(r) => write!(f, "Reference({})", r),
            Variable::Aggregate(a) => write!(f, "Aggregate({:?})", a),
        }
    }
}

/// Types of aggregate computations
#[derive(Debug, Clone)]
pub enum AggregateType {
    Count { filter: Option<String> },
    Sum { field: String },
    Average { field: String },
    Min { field: String },
    Max { field: String },
    Collect { field: String },
}

/// Trait for computed variables that evaluate on demand
pub trait ComputedVariable: Send + Sync {
    /// Evaluate the variable in the given context
    fn evaluate(&self, context: &VariableContext) -> Result<Value>;

    /// Cache key for this variable
    fn cache_key(&self) -> String;

    /// Whether this computation is expensive
    fn is_expensive(&self) -> bool;
}

/// Environment variable resolver
#[derive(Debug, Clone)]
pub struct EnvVariable {
    name: String,
}

impl EnvVariable {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl ComputedVariable for EnvVariable {
    fn evaluate(&self, _context: &VariableContext) -> Result<Value> {
        match std::env::var(&self.name) {
            Ok(val) => Ok(Value::String(val)),
            Err(_) => Ok(Value::Null),
        }
    }

    fn cache_key(&self) -> String {
        format!("env:{}", self.name)
    }

    fn is_expensive(&self) -> bool {
        false
    }
}

/// File content resolver
#[derive(Debug, Clone)]
pub struct FileVariable {
    path: String,
}

impl FileVariable {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

impl ComputedVariable for FileVariable {
    fn evaluate(&self, _context: &VariableContext) -> Result<Value> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => Ok(Value::String(content)),
            Err(e) => Err(anyhow!("Failed to read file '{}': {}", self.path, e)),
        }
    }

    fn cache_key(&self) -> String {
        format!("file:{}", self.path)
    }

    fn is_expensive(&self) -> bool {
        true
    }
}

/// Command output resolver
#[derive(Debug, Clone)]
pub struct CommandVariable {
    command: String,
}

impl CommandVariable {
    pub fn new(command: String) -> Self {
        Self { command }
    }
}

impl ComputedVariable for CommandVariable {
    fn evaluate(&self, _context: &VariableContext) -> Result<Value> {
        use std::process::Command;

        let output = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .output()
            .context("Failed to execute command")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Value::String(stdout))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Command failed: {}", stderr))
        }
    }

    fn cache_key(&self) -> String {
        format!("cmd:{}", self.command)
    }

    fn is_expensive(&self) -> bool {
        true
    }
}

/// UUID generator
#[derive(Debug, Clone)]
pub struct UuidVariable;

impl ComputedVariable for UuidVariable {
    fn evaluate(&self, _context: &VariableContext) -> Result<Value> {
        use uuid::Uuid;
        Ok(Value::String(Uuid::new_v4().to_string()))
    }

    fn cache_key(&self) -> String {
        // UUID should not be cached - always generate new
        format!("uuid:{}", uuid::Uuid::new_v4())
    }

    fn is_expensive(&self) -> bool {
        false
    }
}

/// Date formatter
#[derive(Debug, Clone)]
pub struct DateVariable {
    format: String,
}

impl DateVariable {
    pub fn new(format: String) -> Self {
        Self { format }
    }
}

impl ComputedVariable for DateVariable {
    fn evaluate(&self, _context: &VariableContext) -> Result<Value> {
        use chrono::Local;
        let now = Local::now();
        let formatted = now.format(&self.format).to_string();
        Ok(Value::String(formatted))
    }

    fn cache_key(&self) -> String {
        format!("date:{}", self.format)
    }

    fn is_expensive(&self) -> bool {
        false
    }
}

/// Extract a value from JSON using a simple path notation
/// Supports:
/// - Simple dot notation: "field.nested.value"
/// - Array indexing: "items\\[0\\]" or "items.0"
fn extract_json_path(json: &Value, path: &str) -> Option<Value> {
    let mut current = json;

    // Split path on dots, but handle array notation
    let parts: Vec<&str> = path.split('.').collect();

    for part in parts {
        // Check for array indexing notation like "items[0]"
        if let Some(bracket_pos) = part.find('[') {
            if let Some(close_bracket) = part.find(']') {
                let field = &part[..bracket_pos];
                let index_str = &part[bracket_pos + 1..close_bracket];

                // Navigate to the field first if field is not empty
                if !field.is_empty() {
                    current = current.get(field)?;
                }

                // Then apply the index
                if let Ok(index) = index_str.parse::<usize>() {
                    current = current.get(index)?;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else if let Ok(index) = part.parse::<usize>() {
            // Handle pure numeric indices (for cases like "items.0")
            current = current.get(index)?;
        } else {
            // Regular field access
            current = current.get(part)?;
        }
    }

    Some(current.clone())
}

/// JSON path extractor
#[derive(Debug, Clone)]
pub struct JsonPathVariable {
    json_str: String,
    path: String,
}

impl JsonPathVariable {
    pub fn new(json_str: String, path: String) -> Self {
        Self { json_str, path }
    }
}

impl ComputedVariable for JsonPathVariable {
    fn evaluate(&self, _context: &VariableContext) -> Result<Value> {
        let json: Value = serde_json::from_str(&self.json_str).context("Failed to parse JSON")?;

        // Use the enhanced JSON path extraction
        extract_json_path(&json, &self.path)
            .ok_or_else(|| anyhow!("Path '{}' not found in JSON", self.path))
    }

    fn cache_key(&self) -> String {
        format!("json:{}:{}", self.path, self.json_str.len())
    }

    fn is_expensive(&self) -> bool {
        false
    }
}

/// Variable scope levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScopeLevel {
    Local,
    Phase,
    Global,
}

/// Variable scope management
#[derive(Debug, Clone)]
pub struct VariableScope {
    pub global: HashMap<String, Variable>,
    pub phase: HashMap<String, Variable>,
    pub local: HashMap<String, Variable>,
    pub precedence: Vec<ScopeLevel>,
}

impl Default for VariableScope {
    fn default() -> Self {
        Self {
            global: HashMap::new(),
            phase: HashMap::new(),
            local: HashMap::new(),
            precedence: vec![ScopeLevel::Local, ScopeLevel::Phase, ScopeLevel::Global],
        }
    }
}

/// Main variable context for interpolation
pub struct VariableContext {
    /// Variable scopes
    scope: VariableScope,
    /// Computed variable registry
    computed: HashMap<String, Arc<dyn ComputedVariable>>,
    /// Value cache for expensive computations
    cache: Arc<RwLock<LruCache<String, Value>>>,
    /// Variable extraction regex
    variable_regex: Regex,
    /// Maximum recursion depth for variable resolution
    max_recursion_depth: usize,
}

impl Default for VariableContext {
    fn default() -> Self {
        Self::new()
    }
}

impl VariableContext {
    /// Create a new variable context
    pub fn new() -> Self {
        let cache_size = NonZeroUsize::new(100).unwrap();
        Self {
            scope: VariableScope::default(),
            computed: HashMap::new(),
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            variable_regex: Regex::new(r"\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
            max_recursion_depth: 10,
        }
    }

    /// Set a global variable
    pub fn set_global(&mut self, key: impl Into<String>, value: impl Into<Variable>) {
        self.scope.global.insert(key.into(), value.into());
    }

    /// Set a phase variable
    pub fn set_phase(&mut self, key: impl Into<String>, value: impl Into<Variable>) {
        self.scope.phase.insert(key.into(), value.into());
    }

    /// Set a local variable
    pub fn set_local(&mut self, key: impl Into<String>, value: impl Into<Variable>) {
        self.scope.local.insert(key.into(), value.into());
    }

    /// Remove a local variable
    pub fn remove_local(&mut self, key: &str) {
        self.scope.local.remove(key);
    }

    /// Remove a phase variable
    pub fn remove_phase(&mut self, key: &str) {
        self.scope.phase.remove(key);
    }

    /// Remove a global variable
    pub fn remove_global(&mut self, key: &str) {
        self.scope.global.remove(key);
    }

    /// Register a computed variable
    pub fn register_computed(&mut self, key: impl Into<String>, var: Arc<dyn ComputedVariable>) {
        self.computed.insert(key.into(), var);
    }

    /// Interpolate a template string
    pub async fn interpolate(&self, template: &str) -> Result<String> {
        self.interpolate_with_depth(template, 0).await
    }

    /// Interpolate with recursion depth tracking
    async fn interpolate_with_depth(&self, template: &str, depth: usize) -> Result<String> {
        if depth > self.max_recursion_depth {
            return Err(anyhow!("Maximum variable recursion depth exceeded"));
        }

        let mut result = template.to_string();
        let variables = self.extract_variables(template);

        for var_expr in variables {
            let value = self.resolve_variable(&var_expr, depth).await?;
            let value_str = self.value_to_string(&value);

            // Replace both ${var} and $var patterns
            result = result.replace(&format!("${{{}}}", var_expr), &value_str);
            // Only replace simple $var if it's a valid identifier
            if !var_expr.contains('.') && !var_expr.contains(':') && !var_expr.contains('[') {
                result = result.replace(&format!("${}", var_expr), &value_str);
            }
        }

        Ok(result)
    }

    /// Extract variable expressions from a template
    fn extract_variables(&self, template: &str) -> Vec<String> {
        let mut variables = Vec::new();

        for cap in self.variable_regex.captures_iter(template) {
            let var_expr = if let Some(braced) = cap.get(1) {
                braced.as_str().to_string()
            } else if let Some(unbraced) = cap.get(2) {
                unbraced.as_str().to_string()
            } else {
                continue;
            };

            if !variables.contains(&var_expr) {
                variables.push(var_expr);
            }
        }

        variables
    }

    /// Resolve a variable expression
    fn resolve_variable<'a>(
        &'a self,
        expr: &'a str,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move { self.resolve_variable_impl(expr, depth).await })
    }

    /// Implementation of resolve_variable
    async fn resolve_variable_impl(&self, expr: &str, depth: usize) -> Result<Value> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.peek(expr) {
                trace!("Variable '{}' resolved from cache", expr);
                return Ok(cached.clone());
            }
        }

        // Parse the expression to determine type
        let value = if let Some(var_name) = expr.strip_prefix("env.") {
            // Environment variable
            let env_var = EnvVariable::new(var_name.to_string());
            env_var.evaluate(self)?
        } else if let Some(path) = expr.strip_prefix("file:") {
            // File content
            let file_var = FileVariable::new(path.to_string());
            file_var.evaluate(self)?
        } else if let Some(command) = expr.strip_prefix("cmd:") {
            // Command output
            let cmd_var = CommandVariable::new(command.to_string());
            cmd_var.evaluate(self)?
        } else if let Some(remainder) = expr.strip_prefix("json:") {
            // JSON extraction (format: json:path:from:data_source)
            // Split into path and data_source parts
            // Find the position of ":from:" separator
            let separator = ":from:";
            if let Some(sep_pos) = remainder.find(separator) {
                let path = &remainder[..sep_pos];
                let data_source = &remainder[sep_pos + separator.len()..];

                // Resolve the JSON data variable first
                let json_value = self.resolve_variable(data_source, depth + 1).await?;

                // Handle both string JSON and already-structured data
                let json_to_query = if json_value.is_string() {
                    // If it's a string, parse it as JSON
                    let json_str = self.value_to_string(&json_value);
                    serde_json::from_str(&json_str)
                        .context("Failed to parse JSON string from variable")?
                } else {
                    // If it's already structured data, use it directly
                    json_value.clone()
                };

                // Apply JSONPath to extract the value
                extract_json_path(&json_to_query, path)
                    .ok_or_else(|| anyhow!("JSON path '{}' not found in data", path))?
            } else {
                // Legacy format: json:path:data_source (split on first colon)
                let parts: Vec<&str> = remainder.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let json_value = self.resolve_variable(parts[1], depth + 1).await?;
                    let json_str = self.value_to_string(&json_value);
                    let json_var = JsonPathVariable::new(json_str, parts[0].to_string());
                    json_var.evaluate(self)?
                } else {
                    return Err(anyhow!(
                        "Invalid json: expression format. Use json:path:from:data_source"
                    ));
                }
            }
        } else if let Some(format) = expr.strip_prefix("date:") {
            // Date formatting
            let date_var = DateVariable::new(format.to_string());
            date_var.evaluate(self)?
        } else if expr == "uuid" {
            // UUID generation (never cached)
            return UuidVariable.evaluate(self);
        } else {
            // Standard variable lookup
            self.lookup_variable(expr)?
        };

        // Cache expensive computations
        if self.should_cache(expr) {
            let mut cache = self.cache.write().await;
            cache.put(expr.to_string(), value.clone());
            debug!("Cached variable '{}' for future use", expr);
        }

        Ok(value)
    }

    /// Look up a variable in scopes
    fn lookup_variable(&self, path: &str) -> Result<Value> {
        // Try each scope in precedence order
        for scope_level in &self.scope.precedence {
            let scope_map = match scope_level {
                ScopeLevel::Local => &self.scope.local,
                ScopeLevel::Phase => &self.scope.phase,
                ScopeLevel::Global => &self.scope.global,
            };

            if let Some(var) = scope_map.get(path) {
                return self.evaluate_variable(var);
            }

            // Try nested path resolution (e.g., "map.total")
            if path.contains('.') {
                let parts: Vec<&str> = path.split('.').collect();
                if let Some(var) = scope_map.get(parts[0]) {
                    if let Ok(value) = self.evaluate_variable(var) {
                        if let Some(nested) = self.resolve_nested_path(&value, &parts[1..]) {
                            return Ok(nested);
                        }
                    }
                }
            }
        }

        // Check if it's a registered computed variable
        if let Some(computed) = self.computed.get(path) {
            return computed.evaluate(self);
        }

        Err(anyhow!("Variable '{}' not found", path))
    }

    /// Evaluate a variable (handle references and aggregates)
    fn evaluate_variable(&self, var: &Variable) -> Result<Value> {
        match var {
            Variable::Static(value) => Ok(value.clone()),
            Variable::Computed(computed) => computed.evaluate(self),
            Variable::Reference(ref_path) => self.lookup_variable(ref_path),
            Variable::Aggregate(agg_type) => self.evaluate_aggregate(agg_type),
        }
    }

    /// Evaluate an aggregate expression
    fn evaluate_aggregate(&self, agg_type: &AggregateType) -> Result<Value> {
        // This would need access to the collection being aggregated
        // For now, return a placeholder
        match agg_type {
            AggregateType::Count { .. } => Ok(Value::Number(0.into())),
            AggregateType::Sum { .. } => Ok(Value::Number(0.into())),
            AggregateType::Average { .. } => Ok(Value::Number(0.into())),
            AggregateType::Min { .. } => Ok(Value::Null),
            AggregateType::Max { .. } => Ok(Value::Null),
            AggregateType::Collect { .. } => Ok(Value::Array(vec![])),
        }
    }

    /// Resolve nested path in a JSON value
    fn resolve_nested_path(&self, value: &Value, path: &[&str]) -> Option<Value> {
        if path.is_empty() {
            return Some(value.clone());
        }

        let mut current = value;
        for segment in path {
            current = current.get(segment)?;
        }
        Some(current.clone())
    }

    /// Convert a JSON value to string for interpolation
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            Value::Array(arr) => {
                // For string arrays, join with commas
                if arr.iter().all(|v| matches!(v, Value::String(_))) {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    // For mixed arrays, use JSON representation
                    serde_json::to_string(value).unwrap_or_default()
                }
            }
            Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
        }
    }

    /// Determine if a variable should be cached
    fn should_cache(&self, expr: &str) -> bool {
        // Cache file reads and command outputs
        expr.starts_with("file:") || expr.starts_with("cmd:")
    }

    /// Create a child context with inherited variables
    pub fn child(&self) -> Self {
        let mut child = Self::new();

        // Copy global variables
        child.scope.global = self.scope.global.clone();

        // Phase variables become parent phase
        child.scope.phase = self.scope.phase.clone();

        // Share the same cache
        child.cache = self.cache.clone();

        child
    }

    /// Export variables for persistence
    pub fn export(&self) -> HashMap<String, Value> {
        let mut exported = HashMap::new();

        // Export all scopes
        for (key, var) in &self.scope.global {
            if let Ok(value) = self.evaluate_variable(var) {
                exported.insert(format!("global.{}", key), value);
            }
        }

        for (key, var) in &self.scope.phase {
            if let Ok(value) = self.evaluate_variable(var) {
                exported.insert(format!("phase.{}", key), value);
            }
        }

        for (key, var) in &self.scope.local {
            if let Ok(value) = self.evaluate_variable(var) {
                exported.insert(format!("local.{}", key), value);
            }
        }

        exported
    }

    /// Import variables from persistence
    pub fn import(&mut self, variables: HashMap<String, Value>) {
        for (key, value) in variables {
            if let Some(var_name) = key.strip_prefix("global.") {
                self.set_global(var_name, Variable::Static(value));
            } else if let Some(var_name) = key.strip_prefix("phase.") {
                self.set_phase(var_name, Variable::Static(value));
            } else if let Some(var_name) = key.strip_prefix("local.") {
                self.set_local(var_name, Variable::Static(value));
            } else {
                // Default to global scope
                self.set_global(key, Variable::Static(value));
            }
        }
    }
}

/// Provider trait for variable sources
pub trait VariableProvider: Send + Sync {
    /// Provide variables to a context
    fn provide_variables(&self) -> HashMap<String, Value>;

    /// Update variables from external changes
    fn update_variables(&mut self, updates: HashMap<String, Value>);
}

/// Interpolator trait for template processing
#[async_trait::async_trait]
pub trait VariableInterpolator: Send + Sync {
    /// Interpolate a template with the given context
    async fn interpolate(&self, template: &str, context: &VariableContext) -> Result<String>;

    /// Extract variable names from a template
    fn extract_variables(&self, template: &str) -> Vec<String>;

    /// Validate that all variables in a template are available
    async fn validate_variables(&self, template: &str, context: &VariableContext) -> Result<()>;
}

/// Default interpolator implementation
pub struct DefaultInterpolator;

#[async_trait::async_trait]
impl VariableInterpolator for DefaultInterpolator {
    async fn interpolate(&self, template: &str, context: &VariableContext) -> Result<String> {
        context.interpolate(template).await
    }

    fn extract_variables(&self, template: &str) -> Vec<String> {
        let context = VariableContext::new();
        context.extract_variables(template)
    }

    async fn validate_variables(&self, template: &str, context: &VariableContext) -> Result<()> {
        let variables = context.extract_variables(template);

        for var in variables {
            if let Err(e) = context.resolve_variable(&var, 0).await {
                warn!("Variable '{}' validation failed: {}", var, e);
                return Err(e);
            }
        }

        Ok(())
    }
}

/// Convert from legacy InterpolationContext
impl From<&super::interpolation::InterpolationContext> for VariableContext {
    fn from(old_context: &super::interpolation::InterpolationContext) -> Self {
        let mut new_context = Self::new();

        // Import all variables as global static variables
        for (key, value) in &old_context.variables {
            new_context.set_global(key.clone(), Variable::Static(value.clone()));
        }

        // Handle parent context if present
        if let Some(parent) = &old_context.parent {
            let parent_vars: VariableContext = parent.as_ref().into();
            // Merge parent's global scope
            for (key, var) in parent_vars.scope.global {
                new_context.scope.global.entry(key).or_insert(var);
            }
        }

        new_context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_basic_interpolation() {
        let mut context = VariableContext::new();
        context.set_global("name", Variable::Static(json!("Alice")));
        context.set_global("count", Variable::Static(json!(42)));

        let result = context
            .interpolate("Hello ${name}, you have ${count} items")
            .await
            .unwrap();
        assert_eq!(result, "Hello Alice, you have 42 items");
    }

    #[tokio::test]
    async fn test_nested_variables() {
        let mut context = VariableContext::new();
        context.set_global(
            "map",
            Variable::Static(json!({
                "total": 10,
                "successful": 8,
                "failed": 2
            })),
        );

        let result = context
            .interpolate("Processed ${map.total}: ${map.successful} ok, ${map.failed} failed")
            .await
            .unwrap();
        assert_eq!(result, "Processed 10: 8 ok, 2 failed");
    }

    #[tokio::test]
    async fn test_environment_variable() {
        std::env::set_var("TEST_VAR", "test_value");

        let context = VariableContext::new();
        let result = context.interpolate("Env: ${env.TEST_VAR}").await.unwrap();
        assert_eq!(result, "Env: test_value");

        std::env::remove_var("TEST_VAR");
    }

    #[tokio::test]
    async fn test_uuid_generation() {
        let context = VariableContext::new();
        let result1 = context.interpolate("ID: ${uuid}").await.unwrap();
        let result2 = context.interpolate("ID: ${uuid}").await.unwrap();

        // UUIDs should be different
        assert_ne!(result1, result2);
        assert!(result1.starts_with("ID: "));
        assert!(result2.starts_with("ID: "));
    }

    #[tokio::test]
    async fn test_scope_precedence() {
        let mut context = VariableContext::new();
        context.set_global("var", Variable::Static(json!("global")));
        context.set_phase("var", Variable::Static(json!("phase")));
        context.set_local("var", Variable::Static(json!("local")));

        let result = context.interpolate("Value: ${var}").await.unwrap();
        assert_eq!(result, "Value: local");

        // Remove local, should fall back to phase
        context.scope.local.remove("var");
        let result = context.interpolate("Value: ${var}").await.unwrap();
        assert_eq!(result, "Value: phase");
    }
}
