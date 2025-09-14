//! Value types for expression evaluation

use serde::{Deserialize, Serialize};

/// Value type for expression evaluation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Boolean value
    Bool(bool),
    /// Numeric value
    Number(f64),
    /// String value
    String(String),
    /// Null value
    Null,
}

impl Value {
    /// Check if the value is truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty() && s != "false" && s != "0",
            Value::Null => false,
        }
    }

    /// Convert to a boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Number(n) => Some(*n != 0.0),
            Value::String(s) => match s.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                "1" => Some(true),
                "0" => Some(false),
                "" => Some(false),
                _ => None,
            },
            Value::Null => Some(false),
        }
    }

    /// Convert to a number
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::String(s) => s.parse().ok(),
            Value::Null => Some(0.0),
        }
    }

    /// Convert to a string
    pub fn as_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Null => String::new(),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Number(n as f64)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_truthiness() {
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Number(1.0).is_truthy());
        assert!(!Value::Number(0.0).is_truthy());
        assert!(Value::String("hello".to_string()).is_truthy());
        assert!(!Value::String("".to_string()).is_truthy());
        assert!(!Value::String("false".to_string()).is_truthy());
        assert!(!Value::Null.is_truthy());
    }

    #[test]
    fn test_value_conversions() {
        assert_eq!(Value::String("true".to_string()).as_bool(), Some(true));
        assert_eq!(Value::String("42".to_string()).as_number(), Some(42.0));
        assert_eq!(Value::Bool(true).as_string(), "true");
        assert_eq!(Value::Number(42.0).as_string(), "42");
    }
}
