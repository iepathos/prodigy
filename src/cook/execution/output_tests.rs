//! Unit tests for the output processing module

#[cfg(test)]
mod tests {
    use super::super::output::*;
    use super::super::command::{CommandType, OutputFormat};
    use serde_json::json;
    use std::time::Duration;

    #[tokio::test]
    async fn test_process_output_empty() {
        let output = ProcessOutput::empty();
        assert!(output.stdout.is_none());
        assert!(output.stderr.is_none());
        assert!(output.structured_data.is_none());
        assert!(output.error_summary.is_none());
    }

    #[tokio::test]
    async fn test_process_output_builder() {
        let output = ProcessOutput::new()
            .with_stdout("test stdout".to_string())
            .with_stderr("test stderr".to_string())
            .with_structured_data(Some(json!({"key": "value"})));

        assert_eq!(output.stdout, Some("test stdout".to_string()));
        assert_eq!(output.stderr, Some("test stderr".to_string()));
        assert!(output.structured_data.is_some());
    }

    #[tokio::test]
    async fn test_output_metadata_default() {
        let metadata = OutputMetadata::default();
        assert_eq!(metadata.bytes_stdout, 0);
        assert_eq!(metadata.bytes_stderr, 0);
        assert_eq!(metadata.lines_stdout, 0);
        assert_eq!(metadata.lines_stderr, 0);
        assert!(!metadata.truncated);
    }

    #[tokio::test]
    async fn test_processed_output_creation() {
        let content = ProcessOutput::empty();
        let processed = ProcessedOutput::new(content.clone(), Some(OutputFormat::Json));
        
        assert_eq!(processed.format, OutputFormat::Json);
        assert_eq!(processed.processing_duration, Duration::from_secs(0));
        assert!(processed.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_processed_output_default_format() {
        let content = ProcessOutput::empty();
        let processed = ProcessedOutput::new(content, None);
        
        assert_eq!(processed.format, OutputFormat::PlainText);
    }

    #[tokio::test]
    async fn test_output_processor_creation() {
        let processor = OutputProcessor::new();
        
        // Test that default formatters are registered
        assert_eq!(processor.formatters.len(), 4);
        assert_eq!(processor.parsers.len(), 3);
    }

    #[tokio::test]
    async fn test_claude_output_formatter_remove_artifacts() {
        let formatter = ClaudeOutputFormatter;
        let input = "Claude: Starting\n[Claude] Processing\nActual output\nClaude: Done";
        let result = formatter.remove_claude_artifacts(input);
        
        assert_eq!(result, "Actual output");
    }

    #[tokio::test]
    async fn test_claude_output_formatter_extract_json() {
        let formatter = ClaudeOutputFormatter;
        
        // Test JSON in code block
        let input = "Here is the result:\n```json\n{\"status\": \"success\"}\n```\nDone";
        let result = formatter.extract_structured_data(input).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), json!({"status": "success"}));
        
        // Test direct JSON
        let input = r#"{"status": "success"}"#;
        let result = formatter.extract_structured_data(input).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), json!({"status": "success"}));
        
        // Test non-JSON
        let input = "Plain text output";
        let result = formatter.extract_structured_data(input).unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_claude_output_formatter_extract_error() {
        let formatter = ClaudeOutputFormatter;
        
        let stderr = "Warning: something\nError: Command failed\nDebug info";
        let result = formatter.extract_claude_error(stderr);
        assert_eq!(result, Some("Error: Command failed".to_string()));
        
        let stderr = "Just some warnings";
        let result = formatter.extract_claude_error(stderr);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_claude_output_formatter_format() {
        let formatter = ClaudeOutputFormatter;
        let mut output = ProcessOutput::empty();
        output.stdout = Some("Claude: Info\nActual output".to_string());
        output.stderr = Some("Error: Something failed".to_string());
        
        let result = formatter.format(&output).await.unwrap();
        assert_eq!(result.stdout, Some("Actual output".to_string()));
        assert_eq!(result.error_summary, Some("Error: Something failed".to_string()));
    }

    #[tokio::test]
    async fn test_shell_output_formatter_extract_error() {
        let formatter = ShellOutputFormatter;
        
        // Test command not found
        let stderr = "bash: foo: command not found";
        let result = formatter.extract_error_summary(stderr);
        assert_eq!(result, Some("bash: foo: command not found".to_string()));
        
        // Test permission denied
        let stderr = "Permission denied";
        let result = formatter.extract_error_summary(stderr);
        assert_eq!(result, Some("Permission denied".to_string()));
        
        // Test no specific error
        let stderr = "Some output\nAnother line";
        let result = formatter.extract_error_summary(stderr);
        assert_eq!(result, Some("Some output".to_string()));
        
        // Test empty
        let stderr = "";
        let result = formatter.extract_error_summary(stderr);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_shell_output_formatter_format() {
        let formatter = ShellOutputFormatter;
        let mut output = ProcessOutput::empty();
        output.stderr = Some("error: Something went wrong".to_string());
        
        let result = formatter.format(&output).await.unwrap();
        assert_eq!(result.error_summary, Some("error: Something went wrong".to_string()));
    }

    #[tokio::test]
    async fn test_test_output_formatter_extract_results() {
        let formatter = TestOutputFormatter;
        
        // Test passed result
        let stdout = "Running tests...\n10 tests passed\nAll tests PASSED";
        let result = formatter.extract_test_results(stdout);
        assert!(result.is_some());
        
        let json = result.unwrap();
        assert!(json.is_object());
        assert_eq!(json["status"], json!("passed"));
        
        // Test failed result
        let stdout = "Running tests...\n2 tests FAILED";
        let result = formatter.extract_test_results(stdout);
        assert!(result.is_some());
        
        let json = result.unwrap();
        assert_eq!(json["status"], json!("failed"));
        
        // Test no results
        let stdout = "Just some output";
        let result = formatter.extract_test_results(stdout);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_test_output_formatter_format() {
        let formatter = TestOutputFormatter;
        let mut output = ProcessOutput::empty();
        output.stdout = Some("test result: 5 tests passed".to_string());
        
        let result = formatter.format(&output).await.unwrap();
        assert!(result.structured_data.is_some());
    }

    #[tokio::test]
    async fn test_handler_output_formatter() {
        let formatter = HandlerOutputFormatter;
        let output = ProcessOutput::new()
            .with_stdout("handler output".to_string());
        
        let result = formatter.format(&output).await.unwrap();
        assert_eq!(result.stdout, Some("handler output".to_string()));
    }

    #[tokio::test]
    async fn test_json_output_parser() {
        let parser = JsonOutputParser;
        
        // Valid JSON
        let mut output = ProcessOutput::empty();
        output.stdout = Some(r#"{"key": "value", "number": 42}"#.to_string());
        
        let result = parser.parse(&output).await.unwrap();
        assert!(result.structured_data.is_some());
        assert_eq!(result.structured_data.unwrap()["key"], json!("value"));
        
        // Invalid JSON
        let mut output = ProcessOutput::empty();
        output.stdout = Some("not json".to_string());
        
        let result = parser.parse(&output).await.unwrap();
        assert!(result.structured_data.is_none());
    }

    #[tokio::test]
    async fn test_yaml_output_parser() {
        let parser = YamlOutputParser;
        
        // Valid YAML
        let mut output = ProcessOutput::empty();
        output.stdout = Some("key: value\nnumber: 42".to_string());
        
        let result = parser.parse(&output).await.unwrap();
        assert!(result.structured_data.is_some());
        
        // Invalid YAML
        let mut output = ProcessOutput::empty();
        output.stdout = Some("[invalid yaml".to_string());
        
        let result = parser.parse(&output).await.unwrap();
        // Should still return output, just without structured data
        assert_eq!(result.stdout, Some("[invalid yaml".to_string()));
    }

    #[tokio::test]
    async fn test_plain_text_parser() {
        let parser = PlainTextOutputParser;
        
        let mut output = ProcessOutput::empty();
        output.stdout = Some("plain text".to_string());
        
        let result = parser.parse(&output).await.unwrap();
        assert_eq!(result.stdout, Some("plain text".to_string()));
        assert!(result.structured_data.is_none());
    }

    #[tokio::test]
    async fn test_output_processor_process_output() {
        let processor = OutputProcessor::new();
        
        let raw_output = ProcessOutput::new()
            .with_stdout("test output".to_string());
        
        // Test with Shell command type and JSON format
        let result = processor
            .process_output(raw_output.clone(), CommandType::Shell, Some(OutputFormat::Json))
            .await
            .unwrap();
        
        assert_eq!(result.format, OutputFormat::Json);
        assert_eq!(result.content.stdout, Some("test output".to_string()));
        
        // Test with Claude command type
        let raw_output = ProcessOutput::new()
            .with_stdout("Claude: Info\nActual output".to_string());
        
        let result = processor
            .process_output(raw_output, CommandType::Claude, None)
            .await
            .unwrap();
        
        assert_eq!(result.content.stdout, Some("Actual output".to_string()));
    }

    #[tokio::test]
    async fn test_output_processor_default() {
        let processor = OutputProcessor::default();
        assert_eq!(processor.formatters.len(), 4);
        assert_eq!(processor.parsers.len(), 3);
    }

    #[tokio::test]
    async fn test_process_output_with_all_fields() {
        let output = ProcessOutput {
            stdout: Some("stdout content".to_string()),
            stderr: Some("stderr content".to_string()),
            structured_data: Some(json!({"test": true})),
            error_summary: Some("Error occurred".to_string()),
            metadata: OutputMetadata {
                bytes_stdout: 14,
                bytes_stderr: 14,
                lines_stdout: 1,
                lines_stderr: 1,
                truncated: false,
            },
        };

        assert_eq!(output.stdout.as_ref().unwrap().len(), 14);
        assert_eq!(output.stderr.as_ref().unwrap().len(), 14);
        assert!(output.structured_data.is_some());
        assert_eq!(output.error_summary, Some("Error occurred".to_string()));
        assert!(!output.metadata.truncated);
    }
}