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

/// Execute read operation
async fn execute_read(path: &std::path::Path) -> Result<serde_json::Value, String> {
    match fs::read_to_string(path).await {
        Ok(content) => Ok(json!({
            "content": content,
            "path": path.display().to_string(),
            "size": content.len(),
        })),
        Err(e) => Err(format!("Failed to read file: {e}")),
    }
}

/// Ensure parent directories exist
async fn ensure_parent_dirs(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directories: {e}"))
    } else {
        Ok(())
    }
}

/// Execute write operation
async fn execute_write(
    path: &std::path::Path,
    content: &str,
    overwrite: bool,
    create_dirs: bool,
) -> Result<serde_json::Value, String> {
    // Check if file exists and overwrite is false
    if !overwrite && path.exists() {
        return Err("File already exists and overwrite is false".to_string());
    }

    // Create parent directories if needed
    if create_dirs {
        ensure_parent_dirs(path).await?;
    }

    match fs::write(path, content.as_bytes()).await {
        Ok(_) => Ok(json!({
            "path": path.display().to_string(),
            "size": content.len(),
            "operation": "write",
        })),
        Err(e) => Err(format!("Failed to write file: {e}")),
    }
}

/// Execute append operation
async fn execute_append(
    path: &std::path::Path,
    content: &str,
    create_dirs: bool,
) -> Result<serde_json::Value, String> {
    // Create parent directories if needed
    if create_dirs {
        ensure_parent_dirs(path).await?;
    }

    match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        Ok(mut file) => match file.write_all(content.as_bytes()).await {
            Ok(_) => Ok(json!({
                "path": path.display().to_string(),
                "appended_size": content.len(),
                "operation": "append",
            })),
            Err(e) => Err(format!("Failed to append to file: {e}")),
        },
        Err(e) => Err(format!("Failed to open file for append: {e}")),
    }
}

/// Execute delete operation
async fn execute_delete(path: &std::path::Path) -> Result<serde_json::Value, String> {
    match fs::remove_file(path).await {
        Ok(_) => Ok(json!({
            "path": path.display().to_string(),
            "operation": "delete",
        })),
        Err(e) => Err(format!("Failed to delete file: {e}")),
    }
}

/// Execute exists operation
async fn execute_exists(path: &std::path::Path) -> Result<serde_json::Value, String> {
    let exists = path.exists();
    let metadata = if exists {
        match fs::metadata(path).await {
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

    Ok(json!({
        "path": path.display().to_string(),
        "exists": exists,
        "metadata": metadata,
    }))
}

/// Check if destination exists when overwrite is false
fn check_overwrite(destination: &std::path::Path, overwrite: bool) -> Result<(), String> {
    if !overwrite && destination.exists() {
        Err("Destination already exists and overwrite is false".to_string())
    } else {
        Ok(())
    }
}

/// Execute copy operation
async fn execute_copy(
    source: &std::path::Path,
    destination: &std::path::Path,
    overwrite: bool,
) -> Result<serde_json::Value, String> {
    check_overwrite(destination, overwrite)?;

    match fs::copy(source, destination).await {
        Ok(bytes) => Ok(json!({
            "source": source.display().to_string(),
            "destination": destination.display().to_string(),
            "size": bytes,
            "operation": "copy",
        })),
        Err(e) => Err(format!("Failed to copy file: {e}")),
    }
}

/// Execute move operation
async fn execute_move(
    source: &std::path::Path,
    destination: &std::path::Path,
    overwrite: bool,
) -> Result<serde_json::Value, String> {
    check_overwrite(destination, overwrite)?;

    match fs::rename(source, destination).await {
        Ok(_) => Ok(json!({
            "source": source.display().to_string(),
            "destination": destination.display().to_string(),
            "operation": "move",
        })),
        Err(e) => Err(format!("Failed to move file: {e}")),
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
            "read" => match execute_read(&path).await {
                Ok(data) => CommandResult::success(data),
                Err(e) => CommandResult::error(e),
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

                match execute_write(&path, &content, overwrite, create_dirs).await {
                    Ok(data) => CommandResult::success(data),
                    Err(e) => CommandResult::error(e),
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

                match execute_append(&path, &content, create_dirs).await {
                    Ok(data) => CommandResult::success(data),
                    Err(e) => CommandResult::error(e),
                }
            }
            "delete" => match execute_delete(&path).await {
                Ok(data) => CommandResult::success(data),
                Err(e) => CommandResult::error(e),
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

                match execute_copy(&path, &destination, overwrite).await {
                    Ok(data) => CommandResult::success(data),
                    Err(e) => CommandResult::error(e),
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

                match execute_move(&path, &destination, overwrite).await {
                    Ok(data) => CommandResult::success(data),
                    Err(e) => CommandResult::error(e),
                }
            }
            "exists" => match execute_exists(&path).await {
                Ok(data) => CommandResult::success(data),
                Err(e) => CommandResult::error(e),
            },
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
