//! Command registry for managing and discovering command handlers

use super::{AttributeValue, CommandHandler, CommandResult, ExecutionContext};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry that manages all available command handlers
pub struct CommandRegistry {
    handlers: Arc<RwLock<HashMap<String, Arc<dyn CommandHandler>>>>,
}

impl CommandRegistry {
    /// Creates a new empty registry
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a registry with default built-in handlers
    pub async fn with_defaults() -> Self {
        let registry = Self::new();

        // Register built-in handlers
        use crate::commands::handlers::{
            CargoHandler, ClaudeHandler, FileHandler, GitHandler, ShellHandler,
        };

        registry.register(Box::new(ShellHandler::new()));
        registry.register(Box::new(ClaudeHandler::new()));
        registry.register(Box::new(GitHandler::new()));
        registry.register(Box::new(CargoHandler::new()));
        registry.register(Box::new(FileHandler::new()));

        registry
    }

    /// Registers a new command handler
    pub fn register(&self, handler: Box<dyn CommandHandler>) {
        let handlers = self.handlers.clone();
        let handler: Arc<dyn CommandHandler> = Arc::from(handler);
        tokio::spawn(async move {
            let mut handlers = handlers.write().await;
            let name = handler.name().to_string();
            handlers.insert(name, handler);
        });
    }

    /// Registers a handler synchronously (for use in non-async contexts)
    pub fn register_sync(&self, handler: Box<dyn CommandHandler>) {
        let handlers = self.handlers.clone();
        let handler: Arc<dyn CommandHandler> = Arc::from(handler);
        let name = handler.name().to_string();

        // Use try_current to check if we're in an async context
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // We're in an async context, spawn the task
            handle.spawn(async move {
                let mut handlers = handlers.write().await;
                handlers.insert(name, handler);
            });
        } else {
            // We're not in an async context, create a new runtime
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut handlers = handlers.write().await;
                handlers.insert(name, handler);
            });
        }
    }

    /// Gets a handler by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn CommandHandler>> {
        let handlers = self.handlers.read().await;
        handlers.get(name).cloned()
    }

    /// Lists all registered handler names
    pub async fn list(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        handlers.keys().cloned().collect()
    }

    /// Executes a command using the appropriate handler
    pub async fn execute(
        &self,
        handler_name: &str,
        context: &ExecutionContext,
        attributes: HashMap<String, AttributeValue>,
    ) -> CommandResult {
        let handlers = self.handlers.read().await;

        if let Some(handler) = handlers.get(handler_name) {
            handler.execute(context, attributes).await
        } else {
            CommandResult::error(format!("Unknown command handler: {handler_name}"))
        }
    }

    /// Validates attributes for a specific handler
    pub async fn validate(
        &self,
        handler_name: &str,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Result<(), String> {
        let handlers = self.handlers.read().await;

        if let Some(handler) = handlers.get(handler_name) {
            handler.validate(attributes).map_err(|e| e.to_string())
        } else {
            Err(format!("Unknown command handler: {handler_name}"))
        }
    }

    /// Gets the schema for a specific handler
    pub async fn get_schema(&self, handler_name: &str) -> Option<super::AttributeSchema> {
        let handlers = self.handlers.read().await;
        handlers.get(handler_name).map(|h| h.schema())
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CommandRegistry {
    fn clone(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::AttributeSchema;
    use async_trait::async_trait;
    use serde_json::Value;

    struct TestHandler {
        name: String,
    }

    #[async_trait]
    impl CommandHandler for TestHandler {
        fn name(&self) -> &str {
            &self.name
        }

        fn schema(&self) -> AttributeSchema {
            AttributeSchema::new(&self.name)
        }

        async fn execute(
            &self,
            _context: &ExecutionContext,
            _attributes: HashMap<String, AttributeValue>,
        ) -> CommandResult {
            CommandResult::success(Value::String(format!("{} executed", self.name)))
        }

        fn description(&self) -> &str {
            "Test handler"
        }
    }

    #[tokio::test]
    async fn test_registry_registration() {
        let registry = CommandRegistry::new();
        let handler = Box::new(TestHandler {
            name: "test".to_string(),
        });

        registry.register_sync(handler);

        // Wait a bit for the async registration to complete
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let names = registry.list().await;
        assert!(names.contains(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_registry_get_handler() {
        let registry = CommandRegistry::new();
        let handler = Box::new(TestHandler {
            name: "test".to_string(),
        });

        registry.register_sync(handler);

        // Wait a bit for the async registration to complete
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let retrieved = registry.get("test").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_registry_execute() {
        let registry = CommandRegistry::new();
        let handler = Box::new(TestHandler {
            name: "test".to_string(),
        });

        registry.register_sync(handler);

        // Wait a bit for the async registration to complete
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let context = ExecutionContext::new(std::env::current_dir().unwrap());
        let result = registry.execute("test", &context, HashMap::new()).await;

        assert!(result.is_success());
    }
}
