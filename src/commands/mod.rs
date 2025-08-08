//! Modular command handler architecture for extensible workflow commands
//!
//! This module provides a plugin-like interface for extending MMM's command
//! support without modifying core execution logic. Each command type (shell,
//! claude, git, etc.) implements the `CommandHandler` trait.

use async_trait::async_trait;
use std::collections::HashMap;

pub mod attributes;
pub mod context;
pub mod handlers;
pub mod registry;
pub mod result;

pub use attributes::{AttributeSchema, AttributeValue};
pub use context::ExecutionContext;
pub use registry::CommandRegistry;
pub use result::{CommandError, CommandResult};

/// Core trait that all command handlers must implement
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Returns the unique identifier for this handler
    fn name(&self) -> &str;

    /// Returns the attribute schema that this handler expects
    fn schema(&self) -> AttributeSchema;

    /// Validates the provided attributes against the schema
    fn validate(&self, attributes: &HashMap<String, AttributeValue>) -> Result<(), CommandError> {
        self.schema().validate(attributes)
    }

    /// Executes the command with the given context and attributes
    async fn execute(
        &self,
        context: &ExecutionContext,
        attributes: HashMap<String, AttributeValue>,
    ) -> CommandResult;

    /// Returns a description of what this handler does
    fn description(&self) -> &str;

    /// Returns example usage for this handler
    fn examples(&self) -> Vec<String> {
        vec![]
    }
}

/// Builder for creating and configuring command handlers
pub struct CommandHandlerBuilder {
    registry: CommandRegistry,
}

impl CommandHandlerBuilder {
    /// Creates a new builder with an empty registry
    pub fn new() -> Self {
        Self {
            registry: CommandRegistry::new(),
        }
    }

    /// Registers a command handler
    pub fn register(self, handler: Box<dyn CommandHandler>) -> Self {
        self.registry.register(handler);
        self
    }

    /// Builds and returns the configured registry
    pub fn build(self) -> CommandRegistry {
        self.registry
    }
}

impl Default for CommandHandlerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDirFixture;

    struct MockHandler {
        name: String,
        schema: AttributeSchema,
    }

    #[async_trait]
    impl CommandHandler for MockHandler {
        fn name(&self) -> &str {
            &self.name
        }

        fn schema(&self) -> AttributeSchema {
            self.schema.clone()
        }

        async fn execute(
            &self,
            _context: &ExecutionContext,
            _attributes: HashMap<String, AttributeValue>,
        ) -> CommandResult {
            CommandResult::success(Value::String("Mock executed".to_string()))
        }

        fn description(&self) -> &str {
            "Mock command handler for testing"
        }
    }

    #[test]
    fn test_command_handler_builder() {
        let handler = Box::new(MockHandler {
            name: "mock".to_string(),
            schema: AttributeSchema::new("mock"),
        });

        let registry = CommandHandlerBuilder::new().register(handler).build();

        assert!(registry.get("mock").is_some());
    }

    #[tokio::test]
    async fn test_handler_validation() {
        let mut schema = AttributeSchema::new("test");
        schema.add_required("command", "The command to run");

        let handler = MockHandler {
            name: "test".to_string(),
            schema,
        };

        let mut attrs = HashMap::new();
        attrs.insert(
            "command".to_string(),
            AttributeValue::String("echo test".to_string()),
        );

        assert!(handler.validate(&attrs).is_ok());

        let empty_attrs = HashMap::new();
        assert!(handler.validate(&empty_attrs).is_err());
    }
}
