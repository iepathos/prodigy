use super::ProdigyError;
use serde::{Deserialize, Serialize};

/// Serializable error representation for JSON output
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializableError {
    /// Error kind as string
    pub kind: String,
    /// User-facing error message
    pub message: String,
    /// Error code
    pub code: u16,
    /// Context chain (operation history)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<String>,
    /// Source error if present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Box<SerializableError>>,
}

impl From<&ProdigyError> for SerializableError {
    fn from(error: &ProdigyError) -> Self {
        let kind = match error {
            ProdigyError::Config { .. } => "Config",
            ProdigyError::Session { .. } => "Session",
            ProdigyError::Storage { .. } => "Storage",
            ProdigyError::Execution { .. } => "Execution",
            ProdigyError::Workflow { .. } => "Workflow",
            ProdigyError::Git { .. } => "Git",
            ProdigyError::Validation { .. } => "Validation",
            ProdigyError::Other { .. } => "Other",
        }
        .to_string();

        let context: Vec<String> = error.chain().iter().map(|c| c.message.clone()).collect();

        let source = error.error_source().map(|s| Box::new(s.into()));

        Self {
            kind,
            message: error.user_message(),
            code: error.code(),
            context,
            source,
        }
    }
}

impl ProdigyError {
    /// Convert error to JSON value
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(SerializableError::from(self))
            .unwrap_or_else(|_| serde_json::json!({ "error": "Serialization failed" }))
    }

    /// Convert error to JSON string
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(&SerializableError::from(self))
            .unwrap_or_else(|_| r#"{"error":"Serialization failed"}"#.to_string())
    }

    /// Convert error to pretty-printed JSON string
    pub fn to_json_string_pretty(&self) -> String {
        serde_json::to_string_pretty(&SerializableError::from(self))
            .unwrap_or_else(|_| r#"{"error":"Serialization failed"}"#.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes::ErrorCode;

    #[test]
    fn test_basic_serialization() {
        let error = ProdigyError::config("Invalid configuration");

        let serialized = SerializableError::from(&error);
        assert_eq!(serialized.kind, "Config");
        assert_eq!(serialized.code, ErrorCode::CONFIG_GENERIC);
        assert!(serialized.context.is_empty());
        assert!(serialized.source.is_none());
    }

    #[test]
    fn test_serialization_with_context() {
        let error = ProdigyError::storage("File not found")
            .context("Loading configuration")
            .context("Starting application");

        let serialized = SerializableError::from(&error);
        assert_eq!(serialized.kind, "Storage");
        assert_eq!(serialized.context.len(), 2);
        assert_eq!(serialized.context[0], "Loading configuration");
        assert_eq!(serialized.context[1], "Starting application");
    }

    #[test]
    fn test_serialization_with_error_source() {
        let source_error = ProdigyError::storage("Disk full");
        let error = ProdigyError::execution("Command failed").with_error_source(source_error);

        let serialized = SerializableError::from(&error);
        assert_eq!(serialized.kind, "Execution");
        assert!(serialized.source.is_some());
        assert_eq!(serialized.source.unwrap().kind, "Storage");
    }

    #[test]
    fn test_to_json() {
        let error = ProdigyError::validation("Invalid input").context("Validating workflow");

        let json = error.to_json();
        assert_eq!(json["kind"], "Validation");
        assert_eq!(json["context"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_to_json_string() {
        let error = ProdigyError::workflow("Workflow failed");
        let json_str = error.to_json_string();
        assert!(json_str.contains("Workflow"));
        assert!(json_str.contains("kind"));
    }
}
