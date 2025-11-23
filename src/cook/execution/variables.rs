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
    Count {
        filter: Option<String>,
    },
    Sum {
        field: String,
    },
    Average {
        field: String,
    },
    Min {
        field: String,
    },
    Max {
        field: String,
    },
    Collect {
        field: String,
    },
    // Additional statistical functions
    Median {
        field: String,
    },
    StdDev {
        field: String,
    },
    Variance {
        field: String,
    },
    // Collection functions
    Unique {
        field: String,
    },
    Concat {
        field: String,
        separator: Option<String>,
    },
    Merge {
        field: String,
    },
    Flatten {
        field: String,
    },
    Sort {
        field: String,
        descending: bool,
    },
    GroupBy {
        field: String,
        key: String,
    },
}

/// Types of variables based on their prefix or format
#[derive(Debug, Clone, PartialEq)]
enum VariableType {
    /// Environment variable (env.*)
    Environment,
    /// File content (file:*)
    File,
    /// Command output (cmd:*)
    Command,
    /// JSON extraction (json:*)
    Json,
    /// Date formatting (date:*)
    Date,
    /// UUID generation (uuid)
    Uuid,
    /// Standard variable lookup (no prefix)
    Standard,
}

/// Parse the variable type from an expression based on its prefix or format
///
/// Detects the variable type by examining the expression prefix:
/// - `env.*` → Environment variable
/// - `file:*` → File content
/// - `cmd:*` → Command output
/// - `json:*` → JSON extraction
/// - `date:*` → Date formatting
/// - `uuid` → UUID generation
/// - Otherwise → Standard variable lookup
///
/// This function is pure and has low cyclomatic complexity (5).
fn parse_variable_type(expr: &str) -> VariableType {
    if expr.starts_with("env.") {
        VariableType::Environment
    } else if expr.starts_with("file:") {
        VariableType::File
    } else if expr.starts_with("cmd:") {
        VariableType::Command
    } else if expr.starts_with("json:") {
        VariableType::Json
    } else if expr.starts_with("date:") {
        VariableType::Date
    } else if expr == "uuid" {
        VariableType::Uuid
    } else {
        VariableType::Standard
    }
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

/// Parse array indexing notation like "items[0]" into (field, index)
/// Returns None for malformed bracket expressions
fn parse_bracket_notation(part: &str) -> Option<(&str, usize)> {
    let bracket_pos = part.find('[')?;
    let close_bracket = part.find(']')?;
    let field = &part[..bracket_pos];
    let index_str = &part[bracket_pos + 1..close_bracket];
    let index = index_str.parse::<usize>().ok()?;
    Some((field, index))
}

/// Navigate to a field in JSON value, then apply array index
fn navigate_field_with_index<'a>(
    current: &'a Value,
    field: &str,
    index: usize,
) -> Option<&'a Value> {
    let after_field = if field.is_empty() {
        current
    } else {
        current.get(field)?
    };
    after_field.get(index)
}

/// Process a single path segment and return the next value
fn process_path_segment<'a>(current: &'a Value, part: &str) -> Option<&'a Value> {
    if let Some((field, index)) = parse_bracket_notation(part) {
        navigate_field_with_index(current, field, index)
    } else if let Ok(index) = part.parse::<usize>() {
        current.get(index)
    } else {
        current.get(part)
    }
}

/// Extract a value from JSON using a simple path notation
/// Supports:
/// - Simple dot notation: "field.nested.value"
/// - Array indexing: "items\\[0\\]" or "items.0"
fn extract_json_path(json: &Value, path: &str) -> Option<Value> {
    path.split('.')
        .try_fold(json, |current, part| process_path_segment(current, part))
        .cloned()
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
        let cache_size = NonZeroUsize::new(100).expect("Cache size must be non-zero");
        let variable_regex = Regex::new(r"\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*)")
            .expect("Variable regex pattern is valid");

        Self {
            scope: VariableScope::default(),
            computed: HashMap::new(),
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            variable_regex,
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

    /// Check if variable expression is a simple identifier (no special chars)
    fn is_simple_identifier(var_expr: &str) -> bool {
        !var_expr.contains('.') && !var_expr.contains(':') && !var_expr.contains('[')
    }

    /// Replace a variable expression in the template with its value
    fn replace_variable_in_template(template: String, var_expr: &str, value_str: &str) -> String {
        let with_braces = template.replace(&format!("${{{}}}", var_expr), value_str);
        if Self::is_simple_identifier(var_expr) {
            with_braces.replace(&format!("${}", var_expr), value_str)
        } else {
            with_braces
        }
    }

    /// Interpolate with recursion depth tracking
    async fn interpolate_with_depth(&self, template: &str, depth: usize) -> Result<String> {
        if depth > self.max_recursion_depth {
            return Err(anyhow!("Maximum variable recursion depth exceeded"));
        }

        let variables = self.extract_variables(template);
        let mut result = template.to_string();

        for var_expr in variables {
            let value = self.resolve_variable(&var_expr, depth).await?;
            let value_str = self.value_to_string(&value);
            result = Self::replace_variable_in_template(result, &var_expr, &value_str);
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

    /// Strip a prefix from expression or return error
    fn strip_prefix_or_error<'a>(expr: &'a str, prefix: &str, var_type: &str) -> Result<&'a str> {
        expr.strip_prefix(prefix)
            .ok_or_else(|| anyhow!("Invalid {} variable format: {}", var_type, expr))
    }

    /// Resolve environment variable (env.*)
    fn resolve_env_variable(&self, expr: &str) -> Result<Value> {
        let var_name = Self::strip_prefix_or_error(expr, "env.", "environment")?;
        EnvVariable::new(var_name.to_string()).evaluate(self)
    }

    /// Resolve file variable (file:*)
    fn resolve_file_variable(&self, expr: &str) -> Result<Value> {
        let path = Self::strip_prefix_or_error(expr, "file:", "file")?;
        FileVariable::new(path.to_string()).evaluate(self)
    }

    /// Resolve command variable (cmd:*)
    fn resolve_cmd_variable(&self, expr: &str) -> Result<Value> {
        let command = Self::strip_prefix_or_error(expr, "cmd:", "command")?;
        CommandVariable::new(command.to_string()).evaluate(self)
    }

    /// Resolve date variable (date:*)
    fn resolve_date_variable(&self, expr: &str) -> Result<Value> {
        let format = Self::strip_prefix_or_error(expr, "date:", "date")?;
        DateVariable::new(format.to_string()).evaluate(self)
    }

    /// Resolve a variable by dispatching to the appropriate type-specific resolver
    async fn resolve_by_type(
        &self,
        var_type: VariableType,
        expr: &str,
        depth: usize,
    ) -> Result<Value> {
        match var_type {
            VariableType::Environment => self.resolve_env_variable(expr),
            VariableType::File => self.resolve_file_variable(expr),
            VariableType::Command => self.resolve_cmd_variable(expr),
            VariableType::Date => self.resolve_date_variable(expr),
            VariableType::Json => {
                let remainder = Self::strip_prefix_or_error(expr, "json:", "JSON")?;
                self.resolve_json_variable(remainder, depth).await
            }
            VariableType::Uuid => UuidVariable.evaluate(self),
            VariableType::Standard => self.lookup_variable(expr),
        }
    }

    /// Parse JSON variable format into (path, data_source)
    /// Returns Some for modern format, None for legacy
    fn parse_json_format(remainder: &str) -> Option<(&str, &str)> {
        let separator = ":from:";
        let sep_pos = remainder.find(separator)?;
        let path = &remainder[..sep_pos];
        let data_source = &remainder[sep_pos + separator.len()..];
        Some((path, data_source))
    }

    /// Convert a JSON value to queryable form (parse string or use as-is)
    fn prepare_json_for_query(&self, json_value: &Value) -> Result<Value> {
        if json_value.is_string() {
            let json_str = self.value_to_string(json_value);
            serde_json::from_str(&json_str).context("Failed to parse JSON string from variable")
        } else {
            Ok(json_value.clone())
        }
    }

    /// Resolve JSON variable using modern :from: syntax
    async fn resolve_modern_json_format(
        &self,
        path: &str,
        data_source: &str,
        depth: usize,
    ) -> Result<Value> {
        let json_value = self.resolve_variable(data_source, depth + 1).await?;
        let json_to_query = self.prepare_json_for_query(&json_value)?;
        extract_json_path(&json_to_query, path)
            .ok_or_else(|| anyhow!("JSON path '{}' not found in data", path))
    }

    /// Resolve JSON variable using legacy path:source syntax
    async fn resolve_legacy_json_format(
        &self,
        path: &str,
        data_source: &str,
        depth: usize,
    ) -> Result<Value> {
        let json_value = self.resolve_variable(data_source, depth + 1).await?;
        let json_str = self.value_to_string(&json_value);
        JsonPathVariable::new(json_str, path.to_string()).evaluate(self)
    }

    /// Resolve a JSON variable expression with path extraction
    /// Supports: `json:path:from:data_source` and `json:path:data_source`
    async fn resolve_json_variable(&self, remainder: &str, depth: usize) -> Result<Value> {
        if let Some((path, data_source)) = Self::parse_json_format(remainder) {
            self.resolve_modern_json_format(path, data_source, depth)
                .await
        } else {
            let parts: Vec<&str> = remainder.splitn(2, ':').collect();
            match parts.as_slice() {
                [path, data_source] => {
                    self.resolve_legacy_json_format(path, data_source, depth)
                        .await
                }
                _ => Err(anyhow!(
                    "Invalid json: expression format. Use json:path:from:data_source"
                )),
            }
        }
    }

    /// Implementation of resolve_variable with caching
    async fn resolve_variable_impl(&self, expr: &str, depth: usize) -> Result<Value> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.peek(expr) {
                trace!("Variable '{}' resolved from cache", expr);
                return Ok(cached.clone());
            }
        }

        // Resolve the variable without caching
        let value = self.resolve_without_cache(expr, depth).await?;

        // Cache expensive computations
        if self.should_cache(expr) {
            let mut cache = self.cache.write().await;
            cache.put(expr.to_string(), value.clone());
            debug!("Cached variable '{}' for future use", expr);
        }

        Ok(value)
    }

    /// Resolve a variable without caching - pure resolution logic
    ///
    /// This function contains the core variable resolution logic without any
    /// caching concerns. It:
    /// 1. Parses the variable type from the expression
    /// 2. Handles UUID as a special case (never cached)
    /// 3. Delegates to the type-specific resolver
    ///
    /// Separating this from caching logic improves testability and maintains
    /// single responsibility principle. Low complexity (~2).
    async fn resolve_without_cache(&self, expr: &str, depth: usize) -> Result<Value> {
        // Parse the expression to determine type
        let var_type = parse_variable_type(expr);

        // Special case: UUID is never cached, so handle it directly
        if var_type == VariableType::Uuid {
            return UuidVariable.evaluate(self);
        }

        // Resolve the variable using the appropriate resolver
        self.resolve_by_type(var_type, expr, depth).await
    }

    /// Get scope map for a given scope level
    fn get_scope_map(&self, level: &ScopeLevel) -> &HashMap<String, Variable> {
        match level {
            ScopeLevel::Local => &self.scope.local,
            ScopeLevel::Phase => &self.scope.phase,
            ScopeLevel::Global => &self.scope.global,
        }
    }

    /// Try direct lookup in a scope map
    fn try_direct_lookup(
        &self,
        scope_map: &HashMap<String, Variable>,
        path: &str,
    ) -> Option<Value> {
        scope_map
            .get(path)
            .and_then(|var| self.evaluate_variable(var).ok())
    }

    /// Try nested path lookup (e.g., "map.total" -> get "map" then navigate to "total")
    fn try_nested_lookup(
        &self,
        scope_map: &HashMap<String, Variable>,
        path: &str,
    ) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let base_var = scope_map.get(parts[0])?;
        let base_value = self.evaluate_variable(base_var).ok()?;
        self.resolve_nested_path(&base_value, &parts[1..])
    }

    /// Try lookup in a single scope (direct or nested)
    fn try_scope_lookup(&self, scope_level: &ScopeLevel, path: &str) -> Option<Value> {
        let scope_map = self.get_scope_map(scope_level);
        self.try_direct_lookup(scope_map, path).or_else(|| {
            if path.contains('.') {
                self.try_nested_lookup(scope_map, path)
            } else {
                None
            }
        })
    }

    /// Look up a variable in scopes
    fn lookup_variable(&self, path: &str) -> Result<Value> {
        // Try each scope in precedence order
        for scope_level in &self.scope.precedence {
            if let Some(value) = self.try_scope_lookup(scope_level, path) {
                return Ok(value);
            }
        }

        // Check if it's a registered computed variable
        self.computed
            .get(path)
            .map(|computed| computed.evaluate(self))
            .unwrap_or_else(|| Err(anyhow!("Variable '{}' not found", path)))
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
        match agg_type {
            AggregateType::Count { filter } => self.aggregate_count(filter.as_deref()),
            AggregateType::Sum { field } => self.aggregate_sum(field),
            AggregateType::Average { field } => self.aggregate_average(field),
            AggregateType::Min { field } => self.aggregate_min(field),
            AggregateType::Max { field } => self.aggregate_max(field),
            AggregateType::Collect { field } => self.aggregate_collect(field),
            AggregateType::Median { field } => self.aggregate_median(field),
            AggregateType::StdDev { field } => self.aggregate_stddev(field),
            AggregateType::Variance { field } => self.aggregate_variance(field),
            AggregateType::Unique { field } => self.aggregate_unique(field),
            AggregateType::Concat { field, separator } => {
                self.aggregate_concat(field, separator.as_deref())
            }
            AggregateType::Merge { field } => self.aggregate_merge(field),
            AggregateType::Flatten { field } => self.aggregate_flatten(field),
            AggregateType::Sort { field, descending } => self.aggregate_sort(field, *descending),
            AggregateType::GroupBy { field, key } => self.aggregate_group_by(field, key),
        }
    }

    /// Get the default collection for MapReduce operations
    fn get_default_collection(&self) -> Value {
        self.lookup_variable("map.results")
            .or_else(|_| self.lookup_variable("map"))
            .unwrap_or(Value::Array(vec![]))
    }

    /// Count items in an array (with optional filter)
    fn count_array_items(items: &[Value], _filter: Option<&str>) -> usize {
        // TODO: Implement filter expression evaluation
        items.len()
    }

    /// Count items in an object (check for "results" field or count keys)
    fn count_object_items(map: &serde_json::Map<String, Value>) -> usize {
        map.get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(map.len())
    }

    /// Count items in a collection, optionally filtered
    fn aggregate_count(&self, filter: Option<&str>) -> Result<Value> {
        let collection = self.get_default_collection();
        let count = match &collection {
            Value::Array(items) => Self::count_array_items(items, filter),
            Value::Object(map) => Self::count_object_items(map),
            _ => 0,
        };
        Ok(Value::Number(serde_json::Number::from(count)))
    }

    /// Sum numeric values from a field in a collection
    fn aggregate_sum(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let sum = items
                    .iter()
                    .filter_map(|item| {
                        self.extract_field_value(item, &field_name)
                            .and_then(|v| v.as_f64())
                    })
                    .sum::<f64>();
                Ok(Value::Number(
                    serde_json::Number::from_f64(sum)
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                ))
            }
            _ => Ok(Value::Number(serde_json::Number::from(0))),
        }
    }

    /// Calculate average of numeric values from a field
    fn aggregate_average(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let values: Vec<f64> = items
                    .iter()
                    .filter_map(|item| {
                        self.extract_field_value(item, &field_name)
                            .and_then(|v| v.as_f64())
                    })
                    .collect();

                if values.is_empty() {
                    Ok(Value::Null)
                } else {
                    let avg = values.iter().sum::<f64>() / values.len() as f64;
                    Ok(Value::Number(
                        serde_json::Number::from_f64(avg)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ))
                }
            }
            _ => Ok(Value::Null),
        }
    }

    /// Compare two JSON values (numeric if possible, otherwise string)
    fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
        if let (Some(a_num), Some(b_num)) = (a.as_f64(), b.as_f64()) {
            a_num
                .partial_cmp(&b_num)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.to_string().cmp(&b.to_string())
        }
    }

    /// Find minimum value from a field
    fn aggregate_min(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let min_val = items
                    .iter()
                    .filter_map(|item| self.extract_field_value(item, &field_name))
                    .min_by(|a, b| Self::compare_values(a, b));
                Ok(min_val.cloned().unwrap_or(Value::Null))
            }
            _ => Ok(Value::Null),
        }
    }

    /// Find maximum value from a field
    fn aggregate_max(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let max_val = items
                    .iter()
                    .filter_map(|item| self.extract_field_value(item, &field_name))
                    .max_by(|a, b| Self::compare_values(a, b));
                Ok(max_val.cloned().unwrap_or(Value::Null))
            }
            _ => Ok(Value::Null),
        }
    }

    /// Collect all values from a field into an array
    fn aggregate_collect(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let collected: Vec<Value> = items
                    .iter()
                    .filter_map(|item| self.extract_field_value(item, &field_name))
                    .cloned()
                    .collect();
                Ok(Value::Array(collected))
            }
            _ => Ok(Value::Array(vec![])),
        }
    }

    /// Get the collection to operate on based on the field specification
    fn get_collection_for_field(&self, field: &str) -> Result<Value> {
        // Field can be like "map.results.score" or just "score"
        // If it starts with a collection path, use that collection
        if field.contains('.') {
            let parts: Vec<&str> = field.split('.').collect();
            if parts.len() > 1 {
                // Try to get the collection from the first parts
                let collection_path = parts[0..parts.len() - 1].join(".");
                return self
                    .lookup_variable(&collection_path)
                    .or_else(|_| Ok(Value::Array(vec![])));
            }
        }

        // Default to map.results for MapReduce context
        self.lookup_variable("map.results")
            .or_else(|_| self.lookup_variable("map"))
            .or_else(|_| Ok(Value::Array(vec![])))
    }

    /// Extract the field name from a path like "map.results.score" -> "score"
    fn extract_field_name(&self, field: &str) -> String {
        field.split('.').next_back().unwrap_or(field).to_string()
    }

    /// Extract a field value from an item
    fn extract_field_value<'a>(&self, item: &'a Value, field: &str) -> Option<&'a Value> {
        match item {
            Value::Object(map) => map.get(field),
            _ => None,
        }
    }

    /// Extract numeric field values from items
    fn extract_numeric_values(&self, items: &[Value], field_name: &str) -> Vec<f64> {
        items
            .iter()
            .filter_map(|item| {
                self.extract_field_value(item, field_name)
                    .and_then(|v| v.as_f64())
            })
            .collect()
    }

    /// Calculate median from sorted values
    fn calculate_median(sorted_values: &[f64]) -> f64 {
        let len = sorted_values.len();
        if len.is_multiple_of(2) {
            (sorted_values[len / 2 - 1] + sorted_values[len / 2]) / 2.0
        } else {
            sorted_values[len / 2]
        }
    }

    /// Calculate median of numeric values
    fn aggregate_median(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let mut values = self.extract_numeric_values(items, &field_name);
                if values.is_empty() {
                    return Ok(Value::Null);
                }
                values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let median = Self::calculate_median(&values);
                Ok(Value::Number(
                    serde_json::Number::from_f64(median)
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                ))
            }
            _ => Ok(Value::Null),
        }
    }

    /// Calculate standard deviation
    fn aggregate_stddev(&self, field: &str) -> Result<Value> {
        let variance = self.aggregate_variance(field)?;
        match variance {
            Value::Number(var) => {
                let val = var.as_f64().unwrap_or(0.0);
                Ok(Value::Number(
                    serde_json::Number::from_f64(val.sqrt())
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                ))
            }
            _ => Ok(Value::Null),
        }
    }

    /// Calculate mean of numeric values
    fn calculate_mean(values: &[f64]) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }

    /// Calculate sample variance from values and mean
    fn calculate_sample_variance(values: &[f64], mean: f64) -> f64 {
        let sum_squared_diffs: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
        sum_squared_diffs / (values.len() - 1) as f64
    }

    /// Calculate variance
    fn aggregate_variance(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let values = self.extract_numeric_values(items, &field_name);
                if values.len() < 2 {
                    return Ok(Value::Null);
                }
                let mean = Self::calculate_mean(&values);
                let variance = Self::calculate_sample_variance(&values, mean);
                Ok(Value::Number(
                    serde_json::Number::from_f64(variance)
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                ))
            }
            _ => Ok(Value::Null),
        }
    }

    /// Collect unique values using HashSet for deduplication
    fn collect_unique_values(&self, items: &[Value], field_name: &str) -> Result<Vec<Value>> {
        let mut unique_keys = std::collections::HashSet::new();
        let mut result = Vec::new();

        for item in items {
            if let Some(value) = self.extract_field_value(item, field_name) {
                let key = serde_json::to_string(value)?;
                if unique_keys.insert(key) {
                    result.push(value.clone());
                }
            }
        }
        Ok(result)
    }

    /// Get unique values from a field
    fn aggregate_unique(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let unique = self.collect_unique_values(items, &field_name)?;
                Ok(Value::Array(unique))
            }
            _ => Ok(Value::Array(vec![])),
        }
    }

    /// Concatenate string values from a field
    fn aggregate_concat(&self, field: &str, separator: Option<&str>) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);
        let sep = separator.unwrap_or("");

        match &collection {
            Value::Array(items) => {
                let strings: Vec<String> = items
                    .iter()
                    .filter_map(|item| {
                        self.extract_field_value(item, &field_name)
                            .map(|v| match v {
                                Value::String(s) => s.clone(),
                                _ => v.to_string(),
                            })
                    })
                    .collect();

                Ok(Value::String(strings.join(sep)))
            }
            _ => Ok(Value::String(String::new())),
        }
    }

    /// Merge objects from a field
    fn aggregate_merge(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let mut merged = serde_json::Map::new();

                for item in items {
                    if let Some(Value::Object(obj)) = self.extract_field_value(item, &field_name) {
                        for (k, v) in obj {
                            merged.insert(k.clone(), v.clone());
                        }
                    }
                }

                Ok(Value::Object(merged))
            }
            _ => Ok(Value::Object(serde_json::Map::new())),
        }
    }

    /// Flatten nested arrays
    fn aggregate_flatten(&self, field: &str) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let mut flattened = Vec::new();

                for item in items {
                    match self.extract_field_value(item, &field_name) {
                        Some(Value::Array(arr)) => {
                            flattened.extend(arr.clone());
                        }
                        Some(value) => {
                            flattened.push(value.clone());
                        }
                        None => {}
                    }
                }

                Ok(Value::Array(flattened))
            }
            _ => Ok(Value::Array(vec![])),
        }
    }

    /// Sort values from a field
    fn aggregate_sort(&self, field: &str, descending: bool) -> Result<Value> {
        let collection = self.get_collection_for_field(field)?;
        let field_name = self.extract_field_name(field);

        match &collection {
            Value::Array(items) => {
                let mut values: Vec<Value> = items
                    .iter()
                    .filter_map(|item| self.extract_field_value(item, &field_name).cloned())
                    .collect();

                values.sort_by(|a, b| {
                    let ordering = if let (Some(a_num), Some(b_num)) = (a.as_f64(), b.as_f64()) {
                        a_num
                            .partial_cmp(&b_num)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.to_string().cmp(&b.to_string())
                    };

                    if descending {
                        ordering.reverse()
                    } else {
                        ordering
                    }
                });

                Ok(Value::Array(values))
            }
            _ => Ok(Value::Array(vec![])),
        }
    }

    /// Get collection for group_by operation (field or default)
    fn get_groupby_collection(&self, field: &str) -> Value {
        if field.contains('.') {
            self.lookup_variable(field).unwrap_or(Value::Array(vec![]))
        } else {
            self.get_default_collection()
        }
    }

    /// Convert a JSON value to a string key for grouping
    fn value_to_group_key(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }

    /// Build groups HashMap from items and key field
    fn build_groups(&self, items: &[Value], key: &str) -> HashMap<String, Vec<Value>> {
        let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
        for item in items {
            if let Some(key_value) = self.extract_field_value(item, key) {
                let key_str = Self::value_to_group_key(key_value);
                groups.entry(key_str).or_default().push(item.clone());
            }
        }
        groups
    }

    /// Group items by a key field
    fn aggregate_group_by(&self, field: &str, key: &str) -> Result<Value> {
        let collection = self.get_groupby_collection(field);

        match &collection {
            Value::Array(items) => {
                let groups = self.build_groups(items, key);
                let result: serde_json::Map<_, _> = groups
                    .into_iter()
                    .map(|(k, v)| (k, Value::Array(v)))
                    .collect();
                Ok(Value::Object(result))
            }
            _ => Ok(Value::Object(serde_json::Map::new())),
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

    /// Check if all values in array are strings
    fn is_string_array(arr: &[Value]) -> bool {
        arr.iter().all(|v| matches!(v, Value::String(_)))
    }

    /// Convert string array to comma-separated string
    fn string_array_to_string(arr: &[Value]) -> String {
        arr.iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Convert array to string (comma-separated for strings, JSON otherwise)
    fn array_to_string(arr: &[Value]) -> String {
        if Self::is_string_array(arr) {
            Self::string_array_to_string(arr)
        } else {
            serde_json::to_string(&Value::Array(arr.to_vec())).unwrap_or_default()
        }
    }

    /// Convert a JSON value to string for interpolation
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            Value::Array(arr) => Self::array_to_string(arr),
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

    /// Export variables from a single scope with prefix
    fn export_scope(
        &self,
        scope_map: &HashMap<String, Variable>,
        prefix: &str,
    ) -> HashMap<String, Value> {
        scope_map
            .iter()
            .filter_map(|(key, var)| {
                self.evaluate_variable(var)
                    .ok()
                    .map(|value| (format!("{}.{}", prefix, key), value))
            })
            .collect()
    }

    /// Export variables for persistence
    pub fn export(&self) -> HashMap<String, Value> {
        let mut exported = HashMap::new();
        exported.extend(self.export_scope(&self.scope.global, "global"));
        exported.extend(self.export_scope(&self.scope.phase, "phase"));
        exported.extend(self.export_scope(&self.scope.local, "local"));
        exported
    }

    /// Parse scope prefix from variable key
    fn parse_scope_prefix(key: &str) -> Option<(&str, &str)> {
        key.strip_prefix("global.")
            .map(|name| ("global", name))
            .or_else(|| key.strip_prefix("phase.").map(|name| ("phase", name)))
            .or_else(|| key.strip_prefix("local.").map(|name| ("local", name)))
    }

    /// Import a single variable into the appropriate scope
    fn import_variable(&mut self, key: String, value: Value) {
        if let Some((scope, var_name)) = Self::parse_scope_prefix(&key) {
            match scope {
                "global" => self.set_global(var_name, Variable::Static(value)),
                "phase" => self.set_phase(var_name, Variable::Static(value)),
                "local" => self.set_local(var_name, Variable::Static(value)),
                _ => unreachable!(),
            }
        } else {
            // Default to global scope if no prefix
            self.set_global(key, Variable::Static(value));
        }
    }

    /// Import variables from persistence
    pub fn import(&mut self, variables: HashMap<String, Value>) {
        for (key, value) in variables {
            self.import_variable(key, value);
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

    #[test]
    fn test_parse_variable_type_environment() {
        assert_eq!(parse_variable_type("env.PATH"), VariableType::Environment);
        assert_eq!(parse_variable_type("env.HOME"), VariableType::Environment);
    }

    #[test]
    fn test_parse_variable_type_file() {
        assert_eq!(
            parse_variable_type("file:/path/to/file.txt"),
            VariableType::File
        );
    }

    #[test]
    fn test_parse_variable_type_command() {
        assert_eq!(parse_variable_type("cmd:echo hello"), VariableType::Command);
    }

    #[test]
    fn test_parse_variable_type_json() {
        assert_eq!(
            parse_variable_type("json:$.path:from:variable"),
            VariableType::Json
        );
    }

    #[test]
    fn test_parse_variable_type_date() {
        assert_eq!(parse_variable_type("date:%Y-%m-%d"), VariableType::Date);
    }

    #[test]
    fn test_parse_variable_type_uuid() {
        assert_eq!(parse_variable_type("uuid"), VariableType::Uuid);
    }

    #[test]
    fn test_parse_variable_type_standard() {
        assert_eq!(parse_variable_type("some.variable"), VariableType::Standard);
        assert_eq!(parse_variable_type("simple"), VariableType::Standard);
    }

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

    #[test]
    fn test_aggregate_count() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "status": "success"},
                {"id": 2, "status": "failure"},
                {"id": 3, "status": "success"},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Count { filter: None })
            .unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_aggregate_sum() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "score": 10},
                {"id": 2, "score": 20},
                {"id": 3, "score": 30},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Sum {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(result.as_f64(), Some(60.0));
    }

    #[test]
    fn test_aggregate_average() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "score": 10},
                {"id": 2, "score": 20},
                {"id": 3, "score": 30},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Average {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(result.as_f64(), Some(20.0));
    }

    #[test]
    fn test_aggregate_min_max() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "score": 30},
                {"id": 2, "score": 10},
                {"id": 3, "score": 20},
            ])),
        );

        let min_result = context
            .evaluate_aggregate(&AggregateType::Min {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(min_result, json!(10));

        let max_result = context
            .evaluate_aggregate(&AggregateType::Max {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(max_result, json!(30));
    }

    #[test]
    fn test_aggregate_median() {
        let mut context = VariableContext::new();

        // Odd number of values
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "score": 10},
                {"id": 2, "score": 30},
                {"id": 3, "score": 20},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Median {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(result.as_f64(), Some(20.0));

        // Even number of values
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "score": 10},
                {"id": 2, "score": 20},
                {"id": 3, "score": 30},
                {"id": 4, "score": 40},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Median {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(result.as_f64(), Some(25.0));
    }

    #[test]
    fn test_aggregate_variance_stddev() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "score": 2},
                {"id": 2, "score": 4},
                {"id": 3, "score": 6},
            ])),
        );

        let variance = context
            .evaluate_aggregate(&AggregateType::Variance {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(variance.as_f64(), Some(4.0)); // Sample variance

        let stddev = context
            .evaluate_aggregate(&AggregateType::StdDev {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(stddev.as_f64(), Some(2.0)); // sqrt(4) = 2
    }

    #[test]
    fn test_aggregate_unique() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "status": "success"},
                {"id": 2, "status": "failure"},
                {"id": 3, "status": "success"},
                {"id": 4, "status": "pending"},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Unique {
                field: "status".to_string(),
            })
            .unwrap();

        if let Value::Array(arr) = result {
            assert_eq!(arr.len(), 3);
            let values: Vec<String> = arr
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            assert!(values.contains(&"success".to_string()));
            assert!(values.contains(&"failure".to_string()));
            assert!(values.contains(&"pending".to_string()));
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_aggregate_concat() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"},
                {"id": 3, "name": "Charlie"},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Concat {
                field: "name".to_string(),
                separator: Some(", ".to_string()),
            })
            .unwrap();
        assert_eq!(result, json!("Alice, Bob, Charlie"));

        let result_no_sep = context
            .evaluate_aggregate(&AggregateType::Concat {
                field: "name".to_string(),
                separator: None,
            })
            .unwrap();
        assert_eq!(result_no_sep, json!("AliceBobCharlie"));
    }

    #[test]
    fn test_aggregate_merge() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "config": {"a": 1, "b": 2}},
                {"id": 2, "config": {"c": 3, "d": 4}},
                {"id": 3, "config": {"b": 5}}, // Override b
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Merge {
                field: "config".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!({"a": 1, "b": 5, "c": 3, "d": 4}));
    }

    #[test]
    fn test_aggregate_flatten() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "tags": ["rust", "async"]},
                {"id": 2, "tags": ["tokio"]},
                {"id": 3, "tags": ["serde", "json"]},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Flatten {
                field: "tags".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!(["rust", "async", "tokio", "serde", "json"]));
    }

    #[test]
    fn test_aggregate_sort() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "score": 30},
                {"id": 2, "score": 10},
                {"id": 3, "score": 20},
            ])),
        );

        let asc_result = context
            .evaluate_aggregate(&AggregateType::Sort {
                field: "score".to_string(),
                descending: false,
            })
            .unwrap();
        assert_eq!(asc_result, json!([10, 20, 30]));

        let desc_result = context
            .evaluate_aggregate(&AggregateType::Sort {
                field: "score".to_string(),
                descending: true,
            })
            .unwrap();
        assert_eq!(desc_result, json!([30, 20, 10]));
    }

    #[test]
    fn test_aggregate_group_by() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "status": "success", "score": 10},
                {"id": 2, "status": "failure", "score": 5},
                {"id": 3, "status": "success", "score": 15},
                {"id": 4, "status": "failure", "score": 3},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::GroupBy {
                field: "map.results".to_string(),
                key: "status".to_string(),
            })
            .unwrap();

        if let Value::Object(groups) = result {
            assert_eq!(groups.len(), 2);
            assert!(groups.contains_key("success"));
            assert!(groups.contains_key("failure"));

            if let Some(Value::Array(success_items)) = groups.get("success") {
                assert_eq!(success_items.len(), 2);
            }
            if let Some(Value::Array(failure_items)) = groups.get("failure") {
                assert_eq!(failure_items.len(), 2);
            }
        } else {
            panic!("Expected object result");
        }
    }

    #[test]
    fn test_aggregate_with_empty_collection() {
        let mut context = VariableContext::new();
        context.set_global("map.results", Variable::Static(json!([])));

        let count = context
            .evaluate_aggregate(&AggregateType::Count { filter: None })
            .unwrap();
        assert_eq!(count, json!(0));

        let sum = context
            .evaluate_aggregate(&AggregateType::Sum {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(sum.as_f64(), Some(0.0));

        let avg = context
            .evaluate_aggregate(&AggregateType::Average {
                field: "score".to_string(),
            })
            .unwrap();
        assert_eq!(avg, json!(null));
    }

    #[test]
    fn test_aggregate_collect() {
        let mut context = VariableContext::new();
        context.set_global(
            "map.results",
            Variable::Static(json!([
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"},
                {"id": 3, "name": "Charlie"},
            ])),
        );

        let result = context
            .evaluate_aggregate(&AggregateType::Collect {
                field: "name".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!(["Alice", "Bob", "Charlie"]));
    }

    // Phase 1: Tests for uncovered variable resolution paths

    #[tokio::test]
    async fn test_variable_cache_hit() {
        let temp_file = "/tmp/test-cache-file.txt";
        std::fs::write(temp_file, "initial value").unwrap();

        let context = VariableContext::new();

        // First access - should cache it
        let result1 = context
            .interpolate("${file:/tmp/test-cache-file.txt}")
            .await
            .unwrap();
        assert_eq!(result1, "initial value");

        // Change the file content
        std::fs::write(temp_file, "changed value").unwrap();

        // Second access - should hit cache and return the same initial value
        let result2 = context
            .interpolate("${file:/tmp/test-cache-file.txt}")
            .await
            .unwrap();
        assert_eq!(result2, "initial value"); // Cache hit - same as first

        // Cleanup
        std::fs::remove_file(temp_file).unwrap();
    }

    #[tokio::test]
    async fn test_env_variable_missing() {
        let context = VariableContext::new();

        // Ensure the env var doesn't exist
        std::env::remove_var("NONEXISTENT_TEST_VAR");

        let result = context
            .interpolate("Value: ${env.NONEXISTENT_TEST_VAR}")
            .await
            .unwrap();
        // Missing env vars should resolve to empty string (Null becomes empty)
        assert_eq!(result, "Value: ");
    }

    #[tokio::test]
    async fn test_env_variable_with_special_chars() {
        std::env::set_var("TEST_SPECIAL_VAR", "value with spaces and $pecial ch@rs");

        let context = VariableContext::new();
        let result = context
            .interpolate("Env: ${env.TEST_SPECIAL_VAR}")
            .await
            .unwrap();
        assert_eq!(result, "Env: value with spaces and $pecial ch@rs");

        std::env::remove_var("TEST_SPECIAL_VAR");
    }

    #[tokio::test]
    async fn test_file_variable_missing_file() {
        let context = VariableContext::new();

        let result = context
            .interpolate("${file:/nonexistent/path/to/file.txt}")
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read file"));
    }

    #[tokio::test]
    async fn test_file_variable_empty_file() {
        let temp_file = "/tmp/test-empty-file.txt";
        std::fs::write(temp_file, "").unwrap();

        let context = VariableContext::new();
        let result = context
            .interpolate("Content: ${file:/tmp/test-empty-file.txt}")
            .await
            .unwrap();
        assert_eq!(result, "Content: ");

        std::fs::remove_file(temp_file).unwrap();
    }

    #[tokio::test]
    async fn test_file_variable_with_content() {
        let temp_file = "/tmp/test-file-content.txt";
        std::fs::write(temp_file, "Hello from file").unwrap();

        let context = VariableContext::new();
        let result = context
            .interpolate("Content: ${file:/tmp/test-file-content.txt}")
            .await
            .unwrap();
        assert_eq!(result, "Content: Hello from file");

        std::fs::remove_file(temp_file).unwrap();
    }

    #[tokio::test]
    async fn test_cmd_variable_command_failure() {
        let context = VariableContext::new();

        let result = context.interpolate("${cmd:false}").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Command failed"));
    }

    #[tokio::test]
    async fn test_cmd_variable_empty_output() {
        let context = VariableContext::new();

        let result = context.interpolate("Output: ${cmd:true}").await.unwrap();
        assert_eq!(result, "Output: ");
    }

    #[tokio::test]
    async fn test_cmd_variable_multiline_output() {
        let context = VariableContext::new();

        let result = context
            .interpolate("${cmd:echo 'line1\nline2\nline3'}")
            .await
            .unwrap();
        // Command output is trimmed
        assert!(result.contains("line1"));
    }

    #[tokio::test]
    async fn test_json_from_syntax_with_string_source() {
        let mut context = VariableContext::new();
        context.set_global(
            "json_data",
            Variable::Static(json!(r#"{"name": "Alice", "age": 30}"#)),
        );

        let result = context
            .interpolate("${json:name:from:json_data}")
            .await
            .unwrap();
        assert_eq!(result, "Alice");
    }

    #[tokio::test]
    async fn test_json_from_syntax_with_structured_source() {
        let mut context = VariableContext::new();
        context.set_global("data", Variable::Static(json!({"name": "Bob", "age": 25})));

        let result = context.interpolate("${json:age:from:data}").await.unwrap();
        assert_eq!(result, "25");
    }

    #[tokio::test]
    async fn test_json_from_syntax_missing_path() {
        let mut context = VariableContext::new();
        context.set_global("data", Variable::Static(json!({"name": "Charlie"})));

        let result = context.interpolate("${json:missing_field:from:data}").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_json_from_syntax_invalid_json() {
        let mut context = VariableContext::new();
        context.set_global("invalid", Variable::Static(json!("not valid json {")));

        let result = context.interpolate("${json:field:from:invalid}").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to parse"));
    }

    #[tokio::test]
    async fn test_json_legacy_syntax_valid() {
        let mut context = VariableContext::new();
        context.set_global("legacy_data", Variable::Static(json!(r#"{"value": 42}"#)));

        let result = context
            .interpolate("${json:value:legacy_data}")
            .await
            .unwrap();
        assert_eq!(result, "42");
    }

    #[tokio::test]
    async fn test_json_legacy_syntax_invalid_format() {
        let context = VariableContext::new();

        // json: without path separator should fail
        let result = context.interpolate("${json:invalid}").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid json: expression"));
    }

    #[tokio::test]
    async fn test_json_path_with_array_indexing() {
        let mut context = VariableContext::new();
        context.set_global(
            "array_data",
            Variable::Static(json!({"items": [{"id": 1}, {"id": 2}, {"id": 3}]})),
        );

        let result = context
            .interpolate("${json:items.1.id:from:array_data}")
            .await
            .unwrap();
        assert_eq!(result, "2");
    }

    #[tokio::test]
    async fn test_date_variable_invalid_format() {
        let context = VariableContext::new();

        // chrono should handle most format strings, but let's test a valid one
        let result = context.interpolate("${date:%Y-%m-%d}").await;
        assert!(result.is_ok());
        assert!(result.unwrap().len() >= 10); // At least YYYY-MM-DD
    }

    #[tokio::test]
    async fn test_date_variable_various_formats() {
        let context = VariableContext::new();

        // Test year format
        let year = context.interpolate("${date:%Y}").await.unwrap();
        assert!(year.len() == 4);

        // Test full datetime
        let datetime = context
            .interpolate("${date:%Y-%m-%d %H:%M:%S}")
            .await
            .unwrap();
        assert!(datetime.len() >= 19);
    }

    #[tokio::test]
    async fn test_should_cache_expensive_operations() {
        let context = VariableContext::new();

        // File operations should be cached
        assert!(context.should_cache("file:/tmp/test.txt"));

        // Command operations should be cached
        assert!(context.should_cache("cmd:echo hello"));
    }

    #[tokio::test]
    async fn test_should_not_cache_uuid() {
        let context = VariableContext::new();

        // UUID should not be cached (returns early before caching)
        let result1 = context.resolve_variable("uuid", 0).await.unwrap();
        let result2 = context.resolve_variable("uuid", 0).await.unwrap();

        // UUIDs should be different
        assert_ne!(result1, result2);
    }

    #[tokio::test]
    async fn test_caching_behavior_for_file_operations() {
        let temp_file = "/tmp/test-caching.txt";
        std::fs::write(temp_file, "initial").unwrap();

        let context = VariableContext::new();

        // First read
        let result1 = context
            .interpolate("${file:/tmp/test-caching.txt}")
            .await
            .unwrap();
        assert_eq!(result1, "initial");

        // Change file content
        std::fs::write(temp_file, "changed").unwrap();

        // Second read - should return cached value
        let result2 = context
            .interpolate("${file:/tmp/test-caching.txt}")
            .await
            .unwrap();
        assert_eq!(result2, "initial"); // Still cached

        std::fs::remove_file(temp_file).unwrap();
    }

    #[tokio::test]
    async fn test_json_nested_path_resolution() {
        let mut context = VariableContext::new();
        context.set_global(
            "nested",
            Variable::Static(json!({
                "level1": {
                    "level2": {
                        "level3": "deep value"
                    }
                }
            })),
        );

        let result = context
            .interpolate("${json:level1.level2.level3:from:nested}")
            .await
            .unwrap();
        assert_eq!(result, "deep value");
    }

    #[tokio::test]
    async fn test_json_from_syntax_with_nested_objects() {
        let mut context = VariableContext::new();
        // Test extracting from deeply nested JSON structures
        context.set_global(
            "config",
            Variable::Static(json!({
                "database": {
                    "connection": {
                        "host": "localhost",
                        "port": 5432
                    }
                }
            })),
        );

        let result = context
            .interpolate("${json:database.connection.host:from:config}")
            .await
            .unwrap();
        assert_eq!(result, "localhost");

        let port_result = context
            .interpolate("${json:database.connection.port:from:config}")
            .await
            .unwrap();
        assert_eq!(port_result, "5432");
    }
}
