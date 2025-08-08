//! Attribute schema system for command validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a value that can be passed as an attribute to a command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<AttributeValue>),
    Object(HashMap<String, AttributeValue>),
    Null,
}

impl AttributeValue {
    /// Attempts to get the value as a string
    pub fn as_string(&self) -> Option<&String> {
        match self {
            AttributeValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Attempts to get the value as a number
    pub fn as_number(&self) -> Option<f64> {
        match self {
            AttributeValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempts to get the value as a boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AttributeValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempts to get the value as an array
    pub fn as_array(&self) -> Option<&Vec<AttributeValue>> {
        match self {
            AttributeValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Attempts to get the value as an object
    pub fn as_object(&self) -> Option<&HashMap<String, AttributeValue>> {
        match self {
            AttributeValue::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Checks if the value is null
    pub fn is_null(&self) -> bool {
        matches!(self, AttributeValue::Null)
    }
}

impl From<String> for AttributeValue {
    fn from(s: String) -> Self {
        AttributeValue::String(s)
    }
}

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self {
        AttributeValue::String(s.to_string())
    }
}

impl From<bool> for AttributeValue {
    fn from(b: bool) -> Self {
        AttributeValue::Boolean(b)
    }
}

impl From<i32> for AttributeValue {
    fn from(n: i32) -> Self {
        AttributeValue::Number(n as f64)
    }
}

impl From<f64> for AttributeValue {
    fn from(n: f64) -> Self {
        AttributeValue::Number(n)
    }
}

/// Defines the expected attributes for a command handler
#[derive(Debug, Clone)]
pub struct AttributeSchema {
    name: String,
    required: HashMap<String, String>,
    optional: HashMap<String, String>,
    defaults: HashMap<String, AttributeValue>,
}

impl AttributeSchema {
    /// Creates a new schema for a command
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            required: HashMap::new(),
            optional: HashMap::new(),
            defaults: HashMap::new(),
        }
    }

    /// Adds a required attribute
    pub fn add_required(&mut self, name: &str, description: &str) -> &mut Self {
        self.required
            .insert(name.to_string(), description.to_string());
        self
    }

    /// Adds an optional attribute
    pub fn add_optional(&mut self, name: &str, description: &str) -> &mut Self {
        self.optional
            .insert(name.to_string(), description.to_string());
        self
    }

    /// Adds an optional attribute with a default value
    pub fn add_optional_with_default(
        &mut self,
        name: &str,
        description: &str,
        default: AttributeValue,
    ) -> &mut Self {
        self.optional
            .insert(name.to_string(), description.to_string());
        self.defaults.insert(name.to_string(), default);
        self
    }

    /// Validates a set of attributes against this schema
    pub fn validate(
        &self,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Result<(), crate::commands::CommandError> {
        // Check all required attributes are present
        for required_key in self.required.keys() {
            if !attributes.contains_key(required_key) {
                return Err(crate::commands::CommandError::ValidationError(format!(
                    "Missing required attribute: {}",
                    required_key
                )));
            }
        }

        // Check for unknown attributes
        for provided_key in attributes.keys() {
            if !self.required.contains_key(provided_key)
                && !self.optional.contains_key(provided_key)
            {
                return Err(crate::commands::CommandError::ValidationError(format!(
                    "Unknown attribute: {}",
                    provided_key
                )));
            }
        }

        Ok(())
    }

    /// Applies defaults to a set of attributes
    pub fn apply_defaults(&self, attributes: &mut HashMap<String, AttributeValue>) {
        for (key, default_value) in &self.defaults {
            attributes
                .entry(key.clone())
                .or_insert_with(|| default_value.clone());
        }
    }

    /// Gets the name of this schema
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the required attributes
    pub fn required(&self) -> &HashMap<String, String> {
        &self.required
    }

    /// Gets the optional attributes
    pub fn optional(&self) -> &HashMap<String, String> {
        &self.optional
    }

    /// Gets the default values
    pub fn defaults(&self) -> &HashMap<String, AttributeValue> {
        &self.defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_value_conversions() {
        let string_val = AttributeValue::from("test");
        assert_eq!(string_val.as_string(), Some(&"test".to_string()));

        let num_val = AttributeValue::from(42);
        assert_eq!(num_val.as_number(), Some(42.0));

        let bool_val = AttributeValue::from(true);
        assert_eq!(bool_val.as_bool(), Some(true));
    }

    #[test]
    fn test_schema_validation() {
        let mut schema = AttributeSchema::new("test");
        schema.add_required("command", "The command to run");
        schema.add_optional("timeout", "Timeout in seconds");

        let mut attrs = HashMap::new();
        attrs.insert("command".to_string(), AttributeValue::from("echo test"));

        assert!(schema.validate(&attrs).is_ok());

        let empty_attrs = HashMap::new();
        assert!(schema.validate(&empty_attrs).is_err());

        let mut unknown_attrs = HashMap::new();
        unknown_attrs.insert("command".to_string(), AttributeValue::from("echo test"));
        unknown_attrs.insert("unknown".to_string(), AttributeValue::from("value"));
        assert!(schema.validate(&unknown_attrs).is_err());
    }

    #[test]
    fn test_schema_defaults() {
        let mut schema = AttributeSchema::new("test");
        schema.add_optional_with_default("timeout", "Timeout", AttributeValue::from(30));

        let mut attrs = HashMap::new();
        schema.apply_defaults(&mut attrs);

        assert_eq!(attrs.get("timeout"), Some(&AttributeValue::from(30)));
    }
}
