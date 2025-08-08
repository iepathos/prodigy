//! Claude command handler for AI-powered operations

use crate::commands::{
    AttributeSchema, AttributeValue, CommandHandler, CommandResult, ExecutionContext,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

/// Handler for Claude CLI integration
pub struct ClaudeHandler;

impl ClaudeHandler {
    /// Creates a new Claude handler
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ClaudeHandler {
    fn name(&self) -> &str {
        "claude"
    }

    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("claude");
        schema.add_required("prompt", "The prompt to send to Claude");
        schema.add_optional("model", "The model to use (default: claude-3-sonnet)");
        schema.add_optional("temperature", "Temperature for generation (0.0-1.0)");
        schema.add_optional("max_tokens", "Maximum tokens to generate");
        schema.add_optional("system", "System prompt to use");
        schema.add_optional("context_files", "Files to include as context");
        schema.add_optional_with_default(
            "timeout",
            "Request timeout in seconds",
            AttributeValue::Number(60.0),
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

        // Extract prompt
        let prompt = match attributes.get("prompt").and_then(|v| v.as_string()) {
            Some(p) => p.clone(),
            None => return CommandResult::error("Missing required attribute: prompt".to_string()),
        };

        // Extract optional parameters
        let model = attributes
            .get("model")
            .and_then(|v| v.as_string())
            .map(|s| s.clone())
            .unwrap_or_else(|| "claude-3-sonnet".to_string());

        let temperature = attributes
            .get("temperature")
            .and_then(|v| v.as_number())
            .unwrap_or(0.7);

        let max_tokens = attributes
            .get("max_tokens")
            .and_then(|v| v.as_number())
            .map(|n| n as u32)
            .unwrap_or(4096);

        let system = attributes
            .get("system")
            .and_then(|v| v.as_string())
            .map(|s| s.clone());

        let timeout = attributes
            .get("timeout")
            .and_then(|v| v.as_number())
            .unwrap_or(60.0) as u64;

        // Build context from files if specified
        let mut full_prompt = prompt.clone();
        if let Some(context_files) = attributes.get("context_files").and_then(|v| v.as_array()) {
            let mut file_contents = Vec::new();
            for file_val in context_files {
                if let Some(file_path) = file_val.as_string() {
                    let abs_path = context.resolve_path(file_path.as_ref());
                    match tokio::fs::read_to_string(&abs_path).await {
                        Ok(content) => {
                            file_contents.push(format!("=== {} ===\n{}", file_path, content));
                        }
                        Err(e) => {
                            return CommandResult::error(format!(
                                "Failed to read context file {}: {}",
                                file_path, e
                            ));
                        }
                    }
                }
            }
            if !file_contents.is_empty() {
                full_prompt = format!(
                    "Context files:\n{}\n\nTask:\n{}",
                    file_contents.join("\n\n"),
                    prompt
                );
            }
        }

        let start = Instant::now();

        if context.dry_run {
            let duration = start.elapsed().as_millis() as u64;
            return CommandResult::success(json!({
                "dry_run": true,
                "model": model,
                "prompt": full_prompt,
                "temperature": temperature,
                "max_tokens": max_tokens,
                "system": system,
            }))
            .with_duration(duration);
        }

        // Build Claude CLI command
        let mut cmd_args = vec![
            "--model".to_string(),
            model.clone(),
            "--max-tokens".to_string(),
            max_tokens.to_string(),
            "--temperature".to_string(),
            temperature.to_string(),
        ];

        if let Some(sys) = system.clone() {
            cmd_args.push("--system".to_string());
            cmd_args.push(sys);
        }

        cmd_args.push(full_prompt);

        // Execute Claude CLI
        let result = context
            .executor
            .execute(
                "claude",
                &cmd_args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                Some(&context.working_dir),
                Some(context.full_env()),
                Some(std::time::Duration::from_secs(timeout)),
            )
            .await;

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    CommandResult::success(json!({
                        "response": stdout,
                        "metadata": {
                            "model": model,
                            "temperature": temperature,
                            "max_tokens": max_tokens,
                        }
                    }))
                    .with_duration(duration)
                } else {
                    CommandResult::error(format!("Claude CLI failed: {}", stderr))
                        .with_duration(duration)
                }
            }
            Err(e) => CommandResult::error(format!("Failed to execute Claude CLI: {}", e))
                .with_duration(duration),
        }
    }

    fn description(&self) -> &str {
        "Integrates with Claude CLI for AI-powered code operations"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            r#"{"prompt": "Review this code for improvements"}"#.to_string(),
            r#"{"prompt": "Generate unit tests", "context_files": ["src/main.rs"], "temperature": 0.5}"#.to_string(),
            r#"{"prompt": "Explain this error", "system": "You are a helpful debugging assistant"}"#.to_string(),
        ]
    }
}

impl Default for ClaudeHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::MockSubprocessExecutor;
    use std::path::PathBuf;
    use std::process::Output;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_claude_handler_schema() {
        let handler = ClaudeHandler::new();
        let schema = handler.schema();

        assert!(schema.required().contains_key("prompt"));
        assert!(schema.optional().contains_key("model"));
        assert!(schema.optional().contains_key("temperature"));
    }

    #[tokio::test]
    async fn test_claude_handler_execute() {
        let handler = ClaudeHandler::new();
        let mut mock_executor = MockSubprocessExecutor::new();

        mock_executor.expect_execute(
            "claude",
            vec![
                "--model",
                "claude-3-sonnet",
                "--max-tokens",
                "4096",
                "--temperature",
                "0.7",
                "Test prompt",
            ],
            Some(PathBuf::from("/test")),
            None,
            Some(std::time::Duration::from_secs(60)),
            Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"Claude response".to_vec(),
                stderr: Vec::new(),
            },
        );

        let context =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(Arc::new(mock_executor));

        let mut attributes = HashMap::new();
        attributes.insert(
            "prompt".to_string(),
            AttributeValue::String("Test prompt".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());

        let data = result.data.unwrap();
        assert!(data.get("response").is_some());
    }

    #[tokio::test]
    async fn test_claude_handler_dry_run() {
        let handler = ClaudeHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/test")).with_dry_run(true);

        let mut attributes = HashMap::new();
        attributes.insert(
            "prompt".to_string(),
            AttributeValue::String("Test".to_string()),
        );
        attributes.insert("temperature".to_string(), AttributeValue::Number(0.5));

        let result = handler.execute(&context, attributes).await;
        assert!(result.is_success());

        let data = result.data.unwrap();
        assert_eq!(data.get("dry_run"), Some(&json!(true)));
        assert_eq!(data.get("temperature"), Some(&json!(0.5)));
    }
}
