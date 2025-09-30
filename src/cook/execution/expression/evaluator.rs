//! Expression evaluator for executing compiled expressions

use super::parser::{Expression, NullHandling, SortDirection, SortKey};
use anyhow::Result;
use regex::Regex;
use serde_json::Value;
use std::cmp::Ordering;

/// String collation options for locale-specific sorting
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Collation {
    /// Default binary/lexicographic comparison
    #[default]
    Default,
    /// Case-insensitive comparison
    CaseInsensitive,
    /// Numeric-aware comparison (e.g., "item2" < "item10")
    Numeric,
    /// Case-insensitive and numeric-aware
    CaseInsensitiveNumeric,
}

/// Compiled filter ready for execution
#[derive(Clone)]
pub struct CompiledFilter {
    expression: Expression,
    evaluator: ExpressionEvaluator,
}

impl CompiledFilter {
    /// Create a new compiled filter
    pub fn new(expression: Expression, evaluator: ExpressionEvaluator) -> Self {
        Self {
            expression,
            evaluator,
        }
    }

    /// Evaluate the filter against an item
    pub fn evaluate(&self, item: &Value) -> Result<bool> {
        self.evaluator.evaluate_bool(&self.expression, item)
    }
}

/// Compiled sort specification ready for execution
pub struct CompiledSort {
    sort_keys: Vec<SortKey>,
    collation: Collation,
}

impl CompiledSort {
    /// Create a new compiled sort
    pub fn new(sort_keys: Vec<SortKey>) -> Self {
        Self {
            sort_keys,
            collation: Collation::Default,
        }
    }

    /// Create a new compiled sort with custom collation
    pub fn with_collation(sort_keys: Vec<SortKey>, collation: Collation) -> Self {
        Self {
            sort_keys,
            collation,
        }
    }

    /// Apply the sort to a vector of items
    #[allow(clippy::ptr_arg)]
    pub fn apply(&self, items: &mut Vec<Value>) -> Result<()> {
        let evaluator = ExpressionEvaluator::new();
        items.sort_by(|a, b| self.compare_items(a, b, &evaluator));
        Ok(())
    }

    /// Compare two items according to the sort keys
    fn compare_items(&self, a: &Value, b: &Value, evaluator: &ExpressionEvaluator) -> Ordering {
        for key in &self.sort_keys {
            let a_value = evaluator.evaluate(&key.expression, a).ok();
            let b_value = evaluator.evaluate(&key.expression, b).ok();

            let ordering =
                self.compare_values(a_value.as_ref(), b_value.as_ref(), &key.null_handling);

            let ordering = match key.direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            };

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        Ordering::Equal
    }

    /// Compare two values with null handling
    fn compare_values(
        &self,
        a: Option<&Value>,
        b: Option<&Value>,
        null_handling: &NullHandling,
    ) -> Ordering {
        match (a, b) {
            (None, None) | (Some(Value::Null), Some(Value::Null)) => Ordering::Equal,
            (None, Some(v)) | (Some(Value::Null), Some(v)) if !v.is_null() => match null_handling {
                NullHandling::First => Ordering::Less,
                NullHandling::Last => Ordering::Greater,
            },
            (Some(v), None) | (Some(v), Some(Value::Null)) if !v.is_null() => match null_handling {
                NullHandling::First => Ordering::Greater,
                NullHandling::Last => Ordering::Less,
            },
            (Some(a), Some(b)) => self.compare_json_values(a, b),
            _ => Ordering::Equal,
        }
    }

    /// Compare two JSON values
    fn compare_json_values(&self, a: &Value, b: &Value) -> Ordering {
        match (a, b) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Number(a), Value::Number(b)) => {
                let a_f64 = a.as_f64().unwrap_or(0.0);
                let b_f64 = b.as_f64().unwrap_or(0.0);
                a_f64.partial_cmp(&b_f64).unwrap_or(Ordering::Equal)
            }
            (Value::String(a), Value::String(b)) => self.compare_strings(a, b),
            (Value::Array(a), Value::Array(b)) => a.len().cmp(&b.len()),
            (Value::Object(a), Value::Object(b)) => a.len().cmp(&b.len()),
            // Different types - use type ordering
            _ => {
                let type_order = |v: &Value| match v {
                    Value::Null => 0,
                    Value::Bool(_) => 1,
                    Value::Number(_) => 2,
                    Value::String(_) => 3,
                    Value::Array(_) => 4,
                    Value::Object(_) => 5,
                };
                type_order(a).cmp(&type_order(b))
            }
        }
    }

    /// Compare strings according to the configured collation
    fn compare_strings(&self, a: &str, b: &str) -> Ordering {
        match &self.collation {
            Collation::Default => a.cmp(b),
            Collation::CaseInsensitive => a.to_lowercase().cmp(&b.to_lowercase()),
            Collation::Numeric => self.natural_compare(a, b),
            Collation::CaseInsensitiveNumeric => {
                self.natural_compare(&a.to_lowercase(), &b.to_lowercase())
            }
        }
    }

    /// Natural/numeric-aware string comparison
    /// Handles strings like "item2" < "item10" correctly
    fn natural_compare(&self, a: &str, b: &str) -> Ordering {
        let a_parts = self.split_numeric_parts(a);
        let b_parts = self.split_numeric_parts(b);

        for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
            let ord = match (a_part, b_part) {
                (NumericPart::Text(a_text), NumericPart::Text(b_text)) => a_text.cmp(b_text),
                (NumericPart::Number(a_num), NumericPart::Number(b_num)) => a_num.cmp(b_num),
                (NumericPart::Text(_), NumericPart::Number(_)) => Ordering::Less,
                (NumericPart::Number(_), NumericPart::Text(_)) => Ordering::Greater,
            };
            if ord != Ordering::Equal {
                return ord;
            }
        }

        a_parts.len().cmp(&b_parts.len())
    }

    /// Split a string into text and numeric parts for natural comparison
    fn split_numeric_parts(&self, s: &str) -> Vec<NumericPart> {
        let mut parts = Vec::new();
        let mut current_text = String::new();
        let mut current_num = String::new();
        let mut in_number = false;

        for ch in s.chars() {
            if ch.is_ascii_digit() {
                if !in_number {
                    if !current_text.is_empty() {
                        parts.push(NumericPart::Text(current_text.clone()));
                        current_text.clear();
                    }
                    in_number = true;
                }
                current_num.push(ch);
            } else {
                if in_number {
                    if let Ok(num) = current_num.parse::<u64>() {
                        parts.push(NumericPart::Number(num));
                    } else {
                        parts.push(NumericPart::Text(current_num.clone()));
                    }
                    current_num.clear();
                    in_number = false;
                }
                current_text.push(ch);
            }
        }

        // Handle remaining parts
        if in_number && !current_num.is_empty() {
            if let Ok(num) = current_num.parse::<u64>() {
                parts.push(NumericPart::Number(num));
            } else {
                parts.push(NumericPart::Text(current_num));
            }
        } else if !current_text.is_empty() {
            parts.push(NumericPart::Text(current_text));
        }

        if parts.is_empty() {
            parts.push(NumericPart::Text(s.to_string()));
        }

        parts
    }
}

/// Part of a string for natural sorting
#[derive(Debug)]
enum NumericPart {
    Text(String),
    Number(u64),
}

/// Evaluation context for special variables
#[derive(Clone, Debug, Default)]
pub struct EvaluationContext {
    pub index: Option<usize>,
    pub key: Option<String>,
    pub value: Option<Value>,
}

/// Expression evaluator
#[derive(Clone)]
pub struct ExpressionEvaluator {
    context: EvaluationContext,
}

impl ExpressionEvaluator {
    /// Create a new evaluator
    pub fn new() -> Self {
        Self {
            context: EvaluationContext::default(),
        }
    }

    /// Create an evaluator with context
    pub fn with_context(context: EvaluationContext) -> Self {
        Self { context }
    }

    /// Evaluate an expression and return a boolean result
    pub fn evaluate_bool(&self, expr: &Expression, item: &Value) -> Result<bool> {
        match self.evaluate(expr, item)? {
            Value::Bool(b) => Ok(b),
            Value::Null => Ok(false),
            _ => Ok(true), // Non-null values are truthy
        }
    }

    /// Evaluate an expression
    pub fn evaluate(&self, expr: &Expression, item: &Value) -> Result<Value> {
        match expr {
            // Literals - delegated to pure function
            Expression::Number(n) => Ok(Self::evaluate_literal_number(*n)),
            Expression::String(s) => Ok(Value::String(s.clone())),
            Expression::Boolean(b) => Ok(Value::Bool(*b)),
            Expression::Null => Ok(Value::Null),

            // Field access
            Expression::Field(path) => Ok(self.get_field_value(item, path).unwrap_or(Value::Null)),
            Expression::Index(expr, index) => self.evaluate_index_access(expr, index, item),

            // Special variables
            Expression::Variable(name) => self.get_variable_value(name, item),

            // Comparison operators - already extracted as helpers
            Expression::Equal(left, right) => {
                self.evaluate_binary_comparison(left, right, item, |l, r| l == r)
            }
            Expression::NotEqual(left, right) => {
                self.evaluate_binary_comparison(left, right, item, |l, r| l != r)
            }
            Expression::GreaterThan(left, right) => {
                self.evaluate_comparison_gt(left, right, item)
            }
            Expression::LessThan(left, right) => {
                self.evaluate_comparison_lt(left, right, item)
            }
            Expression::GreaterEqual(left, right) => {
                self.evaluate_comparison_gte(left, right, item)
            }
            Expression::LessEqual(left, right) => {
                self.evaluate_comparison_lte(left, right, item)
            }

            // Logical operators - already extracted
            Expression::And(left, right) => self.evaluate_logical_and(left, right, item),
            Expression::Or(left, right) => self.evaluate_logical_or(left, right, item),
            Expression::Not(expr) => self.evaluate_logical_not(expr, item),

            // String functions - already extracted as helpers
            Expression::Contains(str_expr, pattern) => {
                self.evaluate_string_operation(str_expr, pattern, item, |s, p| s.contains(p))
            }
            Expression::StartsWith(str_expr, prefix) => {
                self.evaluate_string_operation(str_expr, prefix, item, |s, p| s.starts_with(p))
            }
            Expression::EndsWith(str_expr, suffix) => {
                self.evaluate_string_operation(str_expr, suffix, item, |s, p| s.ends_with(p))
            }
            Expression::Matches(str_expr, pattern) => {
                self.evaluate_string_operation(str_expr, pattern, item, |s, p| {
                    Regex::new(p).map_or(false, |re| re.is_match(s))
                })
            }

            // Type checking - already extracted as helpers
            Expression::IsNull(expr) => {
                self.evaluate_type_check(expr, item, |v| v.is_null())
            }
            Expression::IsNotNull(expr) => {
                self.evaluate_type_check(expr, item, |v| !v.is_null())
            }
            Expression::IsNumber(expr) => {
                self.evaluate_type_check(expr, item, |v| matches!(v, Value::Number(_)))
            }
            Expression::IsString(expr) => {
                self.evaluate_type_check(expr, item, |v| matches!(v, Value::String(_)))
            }
            Expression::IsBool(expr) => {
                self.evaluate_type_check(expr, item, |v| matches!(v, Value::Bool(_)))
            }
            Expression::IsArray(expr) => {
                self.evaluate_type_check(expr, item, |v| matches!(v, Value::Array(_)))
            }
            Expression::IsObject(expr) => {
                self.evaluate_type_check(expr, item, |v| matches!(v, Value::Object(_)))
            }

            // Aggregate functions - extracted to separate methods
            Expression::Length(expr) => self.evaluate_length(expr, item),
            Expression::Sum(expr) => self.evaluate_sum(expr, item),
            Expression::Count(expr) => self.evaluate_count(expr, item),
            Expression::Min(expr) => self.evaluate_min(expr, item),
            Expression::Max(expr) => self.evaluate_max(expr, item),
            Expression::Avg(expr) => self.evaluate_avg(expr, item),

            // Array operations - extracted to separate methods
            Expression::In(expr, values) => self.evaluate_in_operation(expr, values, item),
            Expression::ArrayWildcard(base_expr, path) => {
                self.evaluate_array_wildcard(base_expr, path, item)
            }
        }
    }

    /// Pure function: Evaluate a literal number
    fn evaluate_literal_number(n: f64) -> Value {
        Value::Number(
            serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0)),
        )
    }

    /// Evaluate index access operation
    fn evaluate_index_access(
        &self,
        expr: &Expression,
        index: &Expression,
        item: &Value,
    ) -> Result<Value> {
        let base = self.evaluate(expr, item)?;
        let idx = self.evaluate(index, item)?;
        Ok(self.index_value(&base, &idx).unwrap_or(Value::Null))
    }

    /// Evaluate greater-than comparison
    fn evaluate_comparison_gt(
        &self,
        left: &Expression,
        right: &Expression,
        item: &Value,
    ) -> Result<Value> {
        self.evaluate_binary_comparison(left, right, item, |l, r| {
            self.compare_values(l, r) == Ordering::Greater
        })
    }

    /// Evaluate less-than comparison
    fn evaluate_comparison_lt(
        &self,
        left: &Expression,
        right: &Expression,
        item: &Value,
    ) -> Result<Value> {
        self.evaluate_binary_comparison(left, right, item, |l, r| {
            self.compare_values(l, r) == Ordering::Less
        })
    }

    /// Evaluate greater-than-or-equal comparison
    fn evaluate_comparison_gte(
        &self,
        left: &Expression,
        right: &Expression,
        item: &Value,
    ) -> Result<Value> {
        self.evaluate_binary_comparison(left, right, item, |l, r| {
            let ord = self.compare_values(l, r);
            ord == Ordering::Greater || ord == Ordering::Equal
        })
    }

    /// Evaluate less-than-or-equal comparison
    fn evaluate_comparison_lte(
        &self,
        left: &Expression,
        right: &Expression,
        item: &Value,
    ) -> Result<Value> {
        self.evaluate_binary_comparison(left, right, item, |l, r| {
            let ord = self.compare_values(l, r);
            ord == Ordering::Less || ord == Ordering::Equal
        })
    }

    /// Evaluate logical AND with short-circuit evaluation
    fn evaluate_logical_and(
        &self,
        left: &Expression,
        right: &Expression,
        item: &Value,
    ) -> Result<Value> {
        let l = self.evaluate_bool(left, item)?;
        if !l {
            return Ok(Value::Bool(false)); // Short-circuit
        }
        let r = self.evaluate_bool(right, item)?;
        Ok(Value::Bool(r))
    }

    /// Evaluate logical OR with short-circuit evaluation
    fn evaluate_logical_or(
        &self,
        left: &Expression,
        right: &Expression,
        item: &Value,
    ) -> Result<Value> {
        let l = self.evaluate_bool(left, item)?;
        if l {
            return Ok(Value::Bool(true)); // Short-circuit
        }
        let r = self.evaluate_bool(right, item)?;
        Ok(Value::Bool(r))
    }

    /// Evaluate logical NOT
    fn evaluate_logical_not(&self, expr: &Expression, item: &Value) -> Result<Value> {
        let v = self.evaluate_bool(expr, item)?;
        Ok(Value::Bool(!v))
    }

    /// Evaluate length aggregate function
    fn evaluate_length(&self, expr: &Expression, item: &Value) -> Result<Value> {
        let v = self.evaluate(expr, item)?;
        let len = Self::compute_length(&v);
        Ok(Value::Number(serde_json::Number::from(len as u64)))
    }

    /// Pure function: Compute length of a value
    fn compute_length(v: &Value) -> usize {
        match v {
            Value::String(s) => s.len(),
            Value::Array(arr) => arr.len(),
            Value::Object(obj) => obj.len(),
            _ => 0,
        }
    }

    /// Evaluate sum aggregate function
    fn evaluate_sum(&self, expr: &Expression, item: &Value) -> Result<Value> {
        let v = self.evaluate(expr, item)?;
        match v {
            Value::Array(arr) => {
                let sum = Self::sum_numeric_array(&arr);
                Ok(Self::to_number_value(sum))
            }
            _ => Ok(Value::Number(serde_json::Number::from(0))),
        }
    }

    /// Pure function: Sum numeric values in an array
    fn sum_numeric_array(arr: &[Value]) -> f64 {
        arr.iter()
            .filter_map(|v| v.as_f64())
            .filter(|f| !f.is_nan())
            .sum()
    }

    /// Evaluate count aggregate function
    fn evaluate_count(&self, expr: &Expression, item: &Value) -> Result<Value> {
        let v = self.evaluate(expr, item)?;
        match v {
            Value::Array(arr) => Ok(Value::Number(serde_json::Number::from(arr.len() as u64))),
            _ => Ok(Value::Number(serde_json::Number::from(0))),
        }
    }

    /// Evaluate min aggregate function
    fn evaluate_min(&self, expr: &Expression, item: &Value) -> Result<Value> {
        let v = self.evaluate(expr, item)?;
        match v {
            Value::Array(arr) => Ok(Self::find_min_in_array(&arr)),
            _ => Ok(Value::Null),
        }
    }

    /// Pure function: Find minimum value in numeric array
    fn find_min_in_array(arr: &[Value]) -> Value {
        let min = arr
            .iter()
            .filter_map(|v| v.as_f64())
            .filter(|f| !f.is_nan())
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        min.map(Self::to_number_value).unwrap_or(Value::Null)
    }

    /// Evaluate max aggregate function
    fn evaluate_max(&self, expr: &Expression, item: &Value) -> Result<Value> {
        let v = self.evaluate(expr, item)?;
        match v {
            Value::Array(arr) => Ok(Self::find_max_in_array(&arr)),
            _ => Ok(Value::Null),
        }
    }

    /// Pure function: Find maximum value in numeric array
    fn find_max_in_array(arr: &[Value]) -> Value {
        let max = arr
            .iter()
            .filter_map(|v| v.as_f64())
            .filter(|f| !f.is_nan())
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        max.map(Self::to_number_value).unwrap_or(Value::Null)
    }

    /// Evaluate average aggregate function
    fn evaluate_avg(&self, expr: &Expression, item: &Value) -> Result<Value> {
        let v = self.evaluate(expr, item)?;
        match v {
            Value::Array(arr) => Ok(Self::compute_average(&arr)),
            _ => Ok(Value::Null),
        }
    }

    /// Pure function: Compute average of numeric array
    fn compute_average(arr: &[Value]) -> Value {
        let values: Vec<f64> = arr
            .iter()
            .filter_map(|v| v.as_f64())
            .filter(|f| !f.is_nan())
            .collect();

        if values.is_empty() {
            Value::Null
        } else {
            let avg = values.iter().sum::<f64>() / values.len() as f64;
            Self::to_number_value(avg)
        }
    }

    /// Pure function: Convert f64 to JSON Number Value
    fn to_number_value(n: f64) -> Value {
        Value::Number(serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0)))
    }

    /// Evaluate 'in' operation
    fn evaluate_in_operation(
        &self,
        expr: &Expression,
        values: &[Value],
        item: &Value,
    ) -> Result<Value> {
        let v = self.evaluate(expr, item)?;
        Ok(Value::Bool(values.contains(&v)))
    }

    /// Evaluate array wildcard access
    fn evaluate_array_wildcard(
        &self,
        base_expr: &Expression,
        path: &[String],
        item: &Value,
    ) -> Result<Value> {
        let base = self.evaluate(base_expr, item)?;
        match base {
            Value::Array(arr) => Ok(Value::Array(self.collect_array_wildcard_values(&arr, path))),
            _ => Ok(Value::Null),
        }
    }

    /// Collect values from array wildcard access
    fn collect_array_wildcard_values(&self, arr: &[Value], path: &[String]) -> Vec<Value> {
        arr.iter()
            .filter_map(|item| {
                if path.is_empty() {
                    Some(item.clone())
                } else {
                    self.get_field_value(item, path)
                }
            })
            .collect()
    }

    /// Get a field value from an object using a path
    fn get_field_value(&self, item: &Value, path: &[String]) -> Option<Value> {
        let mut current = item.clone();

        for segment in path {
            // Handle array access notation in field names
            if segment.contains('[') && segment.contains(']') {
                let parts: Vec<&str> = segment.split('[').collect();
                let field = parts[0];
                let index_str = parts[1].trim_end_matches(']');

                // Get the field first
                current = current.get(field)?.clone();

                // Then apply the index
                if let Ok(index) = index_str.parse::<usize>() {
                    if let Value::Array(arr) = current {
                        current = arr.get(index)?.clone();
                    } else {
                        return None;
                    }
                }
            } else {
                current = current.get(segment)?.clone();
            }
        }

        Some(current)
    }

    /// Index into a value
    fn index_value(&self, base: &Value, index: &Value) -> Option<Value> {
        match (base, index) {
            (Value::Array(arr), Value::Number(n)) => {
                let idx = n.as_u64()? as usize;
                arr.get(idx).cloned()
            }
            (Value::Object(obj), Value::String(key)) => obj.get(key).cloned(),
            _ => None,
        }
    }

    /// Get a special variable value
    fn get_variable_value(&self, name: &str, _item: &Value) -> Result<Value> {
        match name {
            "_index" => match self.context.index {
                Some(idx) => Ok(Value::Number(serde_json::Number::from(idx as u64))),
                None => Ok(Value::Null),
            },
            "_key" => match &self.context.key {
                Some(key) => Ok(Value::String(key.clone())),
                None => Ok(Value::Null),
            },
            "_value" => match &self.context.value {
                Some(val) => Ok(val.clone()),
                None => Ok(Value::Null),
            },
            _ => Ok(Value::Null),
        }
    }

    /// Compare two values for ordering
    fn compare_values(&self, a: &Value, b: &Value) -> Ordering {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                let a_f64 = a.as_f64().unwrap_or(0.0);
                let b_f64 = b.as_f64().unwrap_or(0.0);
                a_f64.partial_cmp(&b_f64).unwrap_or(Ordering::Equal)
            }
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }

    /// Helper: Evaluate a binary comparison operation
    /// Takes left and right expressions and a comparison function
    fn evaluate_binary_comparison<F>(
        &self,
        left: &Expression,
        right: &Expression,
        item: &Value,
        comparator: F,
    ) -> Result<Value>
    where
        F: FnOnce(&Value, &Value) -> bool,
    {
        let left_val = self.evaluate(left, item)?;
        let right_val = self.evaluate(right, item)?;
        Ok(Value::Bool(comparator(&left_val, &right_val)))
    }

    /// Helper: Evaluate a string operation
    /// Takes string expression, pattern expression, and a string comparison function
    fn evaluate_string_operation<F>(
        &self,
        str_expr: &Expression,
        pattern_expr: &Expression,
        item: &Value,
        operation: F,
    ) -> Result<Value>
    where
        F: FnOnce(&str, &str) -> bool,
    {
        let str_val = self.evaluate(str_expr, item)?;
        let pattern_val = self.evaluate(pattern_expr, item)?;

        let result = match (str_val, pattern_val) {
            (Value::String(s), Value::String(p)) => operation(&s, &p),
            _ => false,
        };

        Ok(Value::Bool(result))
    }

    /// Helper: Evaluate a type checking operation
    /// Takes an expression and a type checking predicate
    fn evaluate_type_check<F>(
        &self,
        expr: &Expression,
        item: &Value,
        type_predicate: F,
    ) -> Result<Value>
    where
        F: FnOnce(&Value) -> bool,
    {
        let value = self.evaluate(expr, item)?;
        Ok(Value::Bool(type_predicate(&value)))
    }
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
