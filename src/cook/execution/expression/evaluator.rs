//! Expression evaluator for executing compiled expressions

use super::parser::{Expression, NullHandling, SortDirection, SortKey};
use anyhow::Result;
use regex::Regex;
use serde_json::Value;
use std::cmp::Ordering;

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
}

impl CompiledSort {
    /// Create a new compiled sort
    pub fn new(sort_keys: Vec<SortKey>) -> Self {
        Self { sort_keys }
    }

    /// Apply the sort to a vector of items
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
            (Value::String(a), Value::String(b)) => a.cmp(b),
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
}

/// Expression evaluator
#[derive(Clone)]
pub struct ExpressionEvaluator {
    // Could add caching or context here
}

impl ExpressionEvaluator {
    /// Create a new evaluator
    pub fn new() -> Self {
        Self {}
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
            // Literals
            Expression::Number(n) => Ok(Value::Number(
                serde_json::Number::from_f64(*n).unwrap_or_else(|| serde_json::Number::from(0)),
            )),
            Expression::String(s) => Ok(Value::String(s.clone())),
            Expression::Boolean(b) => Ok(Value::Bool(*b)),
            Expression::Null => Ok(Value::Null),

            // Field access
            Expression::Field(path) => Ok(self.get_field_value(item, path).unwrap_or(Value::Null)),
            Expression::Index(expr, index) => {
                let base = self.evaluate(expr, item)?;
                let idx = self.evaluate(index, item)?;
                Ok(self.index_value(&base, &idx).unwrap_or(Value::Null))
            }

            // Special variables
            Expression::Variable(name) => self.get_variable_value(name, item),

            // Comparison operators
            Expression::Equal(left, right) => {
                let l = self.evaluate(left, item)?;
                let r = self.evaluate(right, item)?;
                Ok(Value::Bool(l == r))
            }
            Expression::NotEqual(left, right) => {
                let l = self.evaluate(left, item)?;
                let r = self.evaluate(right, item)?;
                Ok(Value::Bool(l != r))
            }
            Expression::GreaterThan(left, right) => {
                let l = self.evaluate(left, item)?;
                let r = self.evaluate(right, item)?;
                Ok(Value::Bool(
                    self.compare_values(&l, &r) == Ordering::Greater,
                ))
            }
            Expression::LessThan(left, right) => {
                let l = self.evaluate(left, item)?;
                let r = self.evaluate(right, item)?;
                Ok(Value::Bool(self.compare_values(&l, &r) == Ordering::Less))
            }
            Expression::GreaterEqual(left, right) => {
                let l = self.evaluate(left, item)?;
                let r = self.evaluate(right, item)?;
                let ord = self.compare_values(&l, &r);
                Ok(Value::Bool(
                    ord == Ordering::Greater || ord == Ordering::Equal,
                ))
            }
            Expression::LessEqual(left, right) => {
                let l = self.evaluate(left, item)?;
                let r = self.evaluate(right, item)?;
                let ord = self.compare_values(&l, &r);
                Ok(Value::Bool(ord == Ordering::Less || ord == Ordering::Equal))
            }

            // Logical operators
            Expression::And(left, right) => {
                let l = self.evaluate_bool(left, item)?;
                if !l {
                    return Ok(Value::Bool(false)); // Short-circuit
                }
                let r = self.evaluate_bool(right, item)?;
                Ok(Value::Bool(r))
            }
            Expression::Or(left, right) => {
                let l = self.evaluate_bool(left, item)?;
                if l {
                    return Ok(Value::Bool(true)); // Short-circuit
                }
                let r = self.evaluate_bool(right, item)?;
                Ok(Value::Bool(r))
            }
            Expression::Not(expr) => {
                let v = self.evaluate_bool(expr, item)?;
                Ok(Value::Bool(!v))
            }

            // String functions
            Expression::Contains(str_expr, pattern) => {
                let s = self.evaluate(str_expr, item)?;
                let p = self.evaluate(pattern, item)?;
                if let (Value::String(s), Value::String(p)) = (s, p) {
                    Ok(Value::Bool(s.contains(&p)))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            Expression::StartsWith(str_expr, prefix) => {
                let s = self.evaluate(str_expr, item)?;
                let p = self.evaluate(prefix, item)?;
                if let (Value::String(s), Value::String(p)) = (s, p) {
                    Ok(Value::Bool(s.starts_with(&p)))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            Expression::EndsWith(str_expr, suffix) => {
                let s = self.evaluate(str_expr, item)?;
                let p = self.evaluate(suffix, item)?;
                if let (Value::String(s), Value::String(p)) = (s, p) {
                    Ok(Value::Bool(s.ends_with(&p)))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            Expression::Matches(str_expr, pattern) => {
                let s = self.evaluate(str_expr, item)?;
                let p = self.evaluate(pattern, item)?;
                if let (Value::String(s), Value::String(p)) = (s, p) {
                    match Regex::new(&p) {
                        Ok(re) => Ok(Value::Bool(re.is_match(&s))),
                        Err(_) => Ok(Value::Bool(false)),
                    }
                } else {
                    Ok(Value::Bool(false))
                }
            }

            // Type checking
            Expression::IsNull(expr) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(v.is_null()))
            }
            Expression::IsNotNull(expr) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(!v.is_null()))
            }
            Expression::IsNumber(expr) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(matches!(v, Value::Number(_))))
            }
            Expression::IsString(expr) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(matches!(v, Value::String(_))))
            }
            Expression::IsBool(expr) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(matches!(v, Value::Bool(_))))
            }
            Expression::IsArray(expr) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(matches!(v, Value::Array(_))))
            }
            Expression::IsObject(expr) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(matches!(v, Value::Object(_))))
            }

            // Aggregate functions
            Expression::Length(expr) => {
                let v = self.evaluate(expr, item)?;
                let len = match v {
                    Value::String(s) => s.len(),
                    Value::Array(arr) => arr.len(),
                    Value::Object(obj) => obj.len(),
                    _ => 0,
                };
                Ok(Value::Number(serde_json::Number::from(len as u64)))
            }
            Expression::Sum(expr) => {
                let v = self.evaluate(expr, item)?;
                if let Value::Array(arr) = v {
                    let sum: f64 = arr.iter().filter_map(|v| v.as_f64()).sum();
                    Ok(Value::Number(
                        serde_json::Number::from_f64(sum)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(Value::Number(serde_json::Number::from(0)))
                }
            }
            Expression::Count(expr) => {
                let v = self.evaluate(expr, item)?;
                if let Value::Array(arr) = v {
                    Ok(Value::Number(serde_json::Number::from(arr.len() as u64)))
                } else {
                    Ok(Value::Number(serde_json::Number::from(0)))
                }
            }
            Expression::Min(expr) => {
                let v = self.evaluate(expr, item)?;
                if let Value::Array(arr) = v {
                    let min = arr
                        .iter()
                        .filter_map(|v| v.as_f64())
                        .min_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                    if let Some(min) = min {
                        Ok(Value::Number(
                            serde_json::Number::from_f64(min)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::Max(expr) => {
                let v = self.evaluate(expr, item)?;
                if let Value::Array(arr) = v {
                    let max = arr
                        .iter()
                        .filter_map(|v| v.as_f64())
                        .max_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                    if let Some(max) = max {
                        Ok(Value::Number(
                            serde_json::Number::from_f64(max)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::Avg(expr) => {
                let v = self.evaluate(expr, item)?;
                if let Value::Array(arr) = v {
                    let values: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
                    if !values.is_empty() {
                        let avg = values.iter().sum::<f64>() / values.len() as f64;
                        Ok(Value::Number(
                            serde_json::Number::from_f64(avg)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }

            // Array operations
            Expression::In(expr, values) => {
                let v = self.evaluate(expr, item)?;
                Ok(Value::Bool(values.contains(&v)))
            }
        }
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
            "_index" => Ok(Value::Number(serde_json::Number::from(0))), // Would need context
            "_key" => Ok(Value::String("".to_string())),                // Would need context
            "_value" => Ok(Value::Null),                                // Would need context
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
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
