//! File operation handler for filesystem manipulation

use crate::commands::{
    AttributeSchema, AttributeValue, CommandHandler, CommandResult, ExecutionContext,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Handler for file operations
pub struct FileHandler;

impl FileHandler {
    /// Creates a new file handler
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for FileHandler {
    fn name(&self) -> &str {
        "file"
    }

    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("file");
        schema.add_required(
            "operation",
            "File operation (read, write, append, delete, copy, move, exists)",
        );
        schema.add_required("path", "Path to the file");
        schema.add_optional("content", "Content to write (for write/append operations)");
        schema.add_optional("destination", "Destination path (for copy/move operations)");
        schema.add_optional("encoding", "File encoding (default: utf-8)");
        schema.add_optional_with_default(
            "create_dirs",
            "Create parent directories if they don't exist",
            AttributeValue::Boolean(true),
        );
        schema.add_optional_with_default(
            "overwrite",
            "Overwrite existing files",
            AttributeValue::Boolean(false),
        );
        schema
    }

    async fn execute(
        &self,
        context: &ExecutionContext,
        mut attributes: HashMap<String, AttributeValue>,
    ) -> CommandResult {
        // Apply defaults
        self.schema().apply_defaults(&mut attributes);

        // Extract operation
        let operation = match attributes.get("operation").and_then(|v| v.as_string()) {
            Some(op) => op.clone(),
            None => {
                return CommandResult::error("Missing required attribute: operation".to_string())
            }
        };

        // Extract path
        let path = match attributes.get("path").and_then(|v| v.as_string()) {
            Some(p) => context.resolve_path(p.as_ref()),
            None => return CommandResult::error("Missing required attribute: path".to_string()),
        };

        let start = Instant::now();

        if context.dry_run {
            let duration = start.elapsed().as_millis() as u64;
            return CommandResult::success(json!({
                "dry_run": true,
                "operation": operation,
                "path": path.display().to_string(),
            }))
            .with_duration(duration);
        }

        // Execute operation
        let result = match operation.as_str() {
            "read" => match fs::read_to_string(&path).await {
                Ok(content) => CommandResult::success(json!({
                    "content": content,
                    "path": path.display().to_string(),
                    "size": content.len(),
                })),
                Err(e) => CommandResult::error(format!("Failed to read file: {e}")),
            },
            "write" => {
                let content = match attributes.get("content").and_then(|v| v.as_string()) {
                    Some(c) => c.clone(),
                    None => {
                        return CommandResult::error(
                            "Write operation requires 'content' attribute".to_string(),
                        )
                    }
                };

                let overwrite = attributes
                    .get("overwrite")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let create_dirs = attributes
                    .get("create_dirs")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                // Check if file exists and overwrite is false
                if !overwrite && path.exists() {
                    return CommandResult::error(
                        "File already exists and overwrite is false".to_string(),
                    );
                }

                // Create parent directories if needed
                if create_dirs {
                    if let Some(parent) = path.parent() {
                        if let Err(e) = fs::create_dir_all(parent).await {
                            return CommandResult::error(format!(
                                "Failed to create directories: {e}"
                            ));
                        }
                    }
                }

                match fs::write(&path, content.as_bytes()).await {
                    Ok(_) => CommandResult::success(json!({
                        "path": path.display().to_string(),
                        "size": content.len(),
                        "operation": "write",
                    })),
                    Err(e) => CommandResult::error(format!("Failed to write file: {e}")),
                }
            }
            "append" => {
                let content = match attributes.get("content").and_then(|v| v.as_string()) {
                    Some(c) => c.clone(),
                    None => {
                        return CommandResult::error(
                            "Append operation requires 'content' attribute".to_string(),
                        )
                    }
                };

                let create_dirs = attributes
                    .get("create_dirs")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                // Create parent directories if needed
                if create_dirs {
                    if let Some(parent) = path.parent() {
                        if let Err(e) = fs::create_dir_all(parent).await {
                            return CommandResult::error(format!(
                                "Failed to create directories: {e}"
                            ));
                        }
                    }
                }

                match fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .await
                {
                    Ok(mut file) => match file.write_all(content.as_bytes()).await {
                        Ok(_) => CommandResult::success(json!({
                            "path": path.display().to_string(),
                            "appended_size": content.len(),
                            "operation": "append",
                        })),
                        Err(e) => CommandResult::error(format!("Failed to append to file: {e}")),
                    },
                    Err(e) => CommandResult::error(format!("Failed to open file for append: {e}")),
                }
            }
            "delete" => match fs::remove_file(&path).await {
                Ok(_) => CommandResult::success(json!({
                    "path": path.display().to_string(),
                    "operation": "delete",
                })),
                Err(e) => CommandResult::error(format!("Failed to delete file: {e}")),
            },
            "copy" => {
                let destination = match attributes.get("destination").and_then(|v| v.as_string()) {
                    Some(d) => context.resolve_path(d.as_ref()),
                    None => {
                        return CommandResult::error(
                            "Copy operation requires 'destination' attribute".to_string(),
                        )
                    }
                };

                let overwrite = attributes
                    .get("overwrite")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !overwrite && destination.exists() {
                    return CommandResult::error(
                        "Destination already exists and overwrite is false".to_string(),
                    );
                }

                match fs::copy(&path, &destination).await {
                    Ok(bytes) => CommandResult::success(json!({
                        "source": path.display().to_string(),
                        "destination": destination.display().to_string(),
                        "size": bytes,
                        "operation": "copy",
                    })),
                    Err(e) => CommandResult::error(format!("Failed to copy file: {e}")),
                }
            }
            "move" => {
                let destination = match attributes.get("destination").and_then(|v| v.as_string()) {
                    Some(d) => context.resolve_path(d.as_ref()),
                    None => {
                        return CommandResult::error(
                            "Move operation requires 'destination' attribute".to_string(),
                        )
                    }
                };

                let overwrite = attributes
                    .get("overwrite")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !overwrite && destination.exists() {
                    return CommandResult::error(
                        "Destination already exists and overwrite is false".to_string(),
                    );
                }

                match fs::rename(&path, &destination).await {
                    Ok(_) => CommandResult::success(json!({
                        "source": path.display().to_string(),
                        "destination": destination.display().to_string(),
                        "operation": "move",
                    })),
                    Err(e) => CommandResult::error(format!("Failed to move file: {e}")),
                }
            }
            "exists" => {
                let exists = path.exists();
                let metadata = if exists {
                    match fs::metadata(&path).await {
                        Ok(meta) => Some(json!({
                            "is_file": meta.is_file(),
                            "is_dir": meta.is_dir(),
                            "size": meta.len(),
                        })),
                        Err(_) => None,
                    }
                } else {
                    None
                };

                CommandResult::success(json!({
                    "path": path.display().to_string(),
                    "exists": exists,
                    "metadata": metadata,
                }))
            }
            _ => CommandResult::error(format!("Unknown file operation: {operation}")),
        };

        let duration = start.elapsed().as_millis() as u64;
        result.with_duration(duration)
    }

    fn description(&self) -> &str {
        "Handles file system operations like read, write, copy, move, and delete"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            r#"{"operation": "read", "path": "config.json"}"#.to_string(),
            r#"{"operation": "write", "path": "output.txt", "content": "Hello, World!", "overwrite": true}"#.to_string(),
            r#"{"operation": "copy", "path": "source.txt", "destination": "backup.txt"}"#.to_string(),
            r#"{"operation": "exists", "path": "test.md"}"#.to_string(),
        ]
    }
}

impl Default for FileHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_handler_schema() {
        let handler = FileHandler::new();
        let schema = handler.schema();

        assert!(schema.required().contains_key("operation"));
        assert!(schema.required().contains_key("path"));
        assert!(schema.optional().contains_key("content"));
    }

    #[tokio::test]
    async fn test_file_write_and_read() {
        let handler = FileHandler::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let context = ExecutionContext::new(temp_dir.path().to_path_buf());

        // Write file
        let mut write_attrs = HashMap::new();
        write_attrs.insert(
            "operation".to_string(),
            AttributeValue::String("write".to_string()),
        );
        write_attrs.insert(
            "path".to_string(),
            AttributeValue::String(file_path.display().to_string()),
        );
        write_attrs.insert(
            "content".to_string(),
            AttributeValue::String("Test content".to_string()),
        );

        let write_result = handler.execute(&context, write_attrs).await;
        assert!(write_result.is_success());

        // Read file
        let mut read_attrs = HashMap::new();
        read_attrs.insert(
            "operation".to_string(),
            AttributeValue::String("read".to_string()),
        );
        read_attrs.insert(
            "path".to_string(),
            AttributeValue::String(file_path.display().to_string()),
        );

        let read_result = handler.execute(&context, read_attrs).await;
        assert!(read_result.is_success());

        let data = read_result.data.unwrap();
        assert_eq!(data.get("content"), Some(&json!("Test content")));
    }

    #[tokio::test]
    async fn test_file_exists() {
        let handler = FileHandler::new();
        let temp_dir = TempDir::new().unwrap();
        let context = ExecutionContext::new(temp_dir.path().to_path_buf());

        let mut attrs = HashMap::new();
        attrs.insert(
            "operation".to_string(),
            AttributeValue::String("exists".to_string()),
        );
        attrs.insert(
            "path".to_string(),
            AttributeValue::String("nonexistent.txt".to_string()),
        );

        let result = handler.execute(&context, attrs).await;
        assert!(result.is_success());

        let data = result.data.unwrap();
        assert_eq!(data.get("exists"), Some(&json!(false)));
    }

    #[tokio::test]
    async fn test_file_dry_run() {
        let handler = FileHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test")).with_dry_run(true);

        let mut attrs = HashMap::new();
        attrs.insert(
            "operation".to_string(),
            AttributeValue::String("delete".to_string()),
        );
        attrs.insert(
            "path".to_string(),
            AttributeValue::String("important.txt".to_string()),
        );

        let result = handler.execute(&context, attrs).await;
        assert!(result.is_success());

        let data = result.data.unwrap();
        assert_eq!(data.get("dry_run"), Some(&json!(true)));
    }
}
