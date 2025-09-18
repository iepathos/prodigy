//! Tests for streaming infrastructure

#[cfg(test)]
use crate::subprocess::streaming::{
    JsonLineProcessor, LoggingProcessor, PatternMatchProcessor, StreamProcessor,
    StreamingCommandRunner,
};
#[cfg(test)]
use crate::subprocess::ProcessCommand;
#[cfg(test)]
use tokio::sync::mpsc;

#[tokio::test]
async fn test_streaming_basic_echo() {
    let runner =
        StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

    let processors: Vec<Box<dyn StreamProcessor>> = vec![Box::new(LoggingProcessor::new("test"))];

    let command = ProcessCommand {
        program: "echo".to_string(),
        args: vec!["hello", "world"]
            .into_iter()
            .map(String::from)
            .collect(),
        env: Default::default(),
        working_dir: None,
        timeout: None,
        stdin: None,
        suppress_stderr: false,
    };

    let result = runner.run_streaming(command, processors).await.unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout.len(), 1);
    assert_eq!(result.stdout[0], "hello world");
}

#[tokio::test]
async fn test_streaming_multiline_output() {
    let runner =
        StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

    let processors: Vec<Box<dyn StreamProcessor>> = vec![];

    let command = ProcessCommand {
        program: "sh".to_string(),
        args: vec!["-c", "echo 'line1'; echo 'line2'; echo 'line3'"]
            .into_iter()
            .map(String::from)
            .collect(),
        env: Default::default(),
        working_dir: None,
        timeout: None,
        stdin: None,
        suppress_stderr: false,
    };

    let result = runner.run_streaming(command, processors).await.unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout.len(), 3);
    assert_eq!(result.stdout[0], "line1");
    assert_eq!(result.stdout[1], "line2");
    assert_eq!(result.stdout[2], "line3");
}

#[tokio::test]
async fn test_json_line_processor() {
    let runner =
        StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

    let (sender, mut receiver) = mpsc::channel(10);
    let processors: Vec<Box<dyn StreamProcessor>> =
        vec![Box::new(JsonLineProcessor::new(sender, true))];

    let json_output = r#"{"key": "value1"}
{"key": "value2"}
not json
{"key": "value3"}"#;

    let command = ProcessCommand {
        program: "echo".to_string(),
        args: vec![json_output].into_iter().map(String::from).collect(),
        env: Default::default(),
        working_dir: None,
        timeout: None,
        stdin: None,
        suppress_stderr: false,
    };

    let _result = runner.run_streaming(command, processors).await.unwrap();

    // Collect JSON events
    let mut json_events = Vec::new();
    receiver.close();
    while let Some(value) = receiver.recv().await {
        json_events.push(value);
    }

    // Should have captured 3 JSON objects (skipping the non-JSON line)
    assert_eq!(json_events.len(), 3);
    assert_eq!(json_events[0]["key"], "value1");
    assert_eq!(json_events[1]["key"], "value2");
    assert_eq!(json_events[2]["key"], "value3");
}

#[tokio::test]
async fn test_pattern_match_processor() {
    use regex::Regex;

    let runner =
        StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

    let (sender, mut receiver) = mpsc::channel(10);
    let patterns = vec![
        Regex::new(r"ERROR: (.+)").unwrap(),
        Regex::new(r"WARNING: (.+)").unwrap(),
    ];

    let processors: Vec<Box<dyn StreamProcessor>> =
        vec![Box::new(PatternMatchProcessor::new(patterns, sender))];

    let output = r#"INFO: Starting process
ERROR: Failed to connect
WARNING: Retry in 5 seconds
INFO: Process complete"#;

    let command = ProcessCommand {
        program: "echo".to_string(),
        args: vec![output].into_iter().map(String::from).collect(),
        env: Default::default(),
        working_dir: None,
        timeout: None,
        stdin: None,
        suppress_stderr: false,
    };

    let _result = runner.run_streaming(command, processors).await.unwrap();

    // Collect pattern matches
    let mut matches = Vec::new();
    receiver.close();
    while let Some(pattern_match) = receiver.recv().await {
        matches.push(pattern_match);
    }

    // Should have captured 2 matches (ERROR and WARNING)
    assert_eq!(matches.len(), 2);
    assert!(matches[0].line.contains("ERROR"));
    assert_eq!(matches[0].captures[0], "Failed to connect");
    assert!(matches[1].line.contains("WARNING"));
    assert_eq!(matches[1].captures[0], "Retry in 5 seconds");
}

#[tokio::test]
async fn test_streaming_with_stdin() {
    let runner =
        StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

    let processors: Vec<Box<dyn StreamProcessor>> = vec![];

    let command = ProcessCommand {
        program: "cat".to_string(),
        args: vec![],
        env: Default::default(),
        working_dir: None,
        timeout: None,
        stdin: Some("input data\nmore data".to_string()),
        suppress_stderr: false,
    };

    let result = runner.run_streaming(command, processors).await.unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout.len(), 2);
    assert_eq!(result.stdout[0], "input data");
    assert_eq!(result.stdout[1], "more data");
}

#[tokio::test]
async fn test_streaming_stderr_capture() {
    let runner =
        StreamingCommandRunner::new(Box::new(crate::subprocess::runner::TokioProcessRunner));

    let processors: Vec<Box<dyn StreamProcessor>> = vec![];

    let command = ProcessCommand {
        program: "sh".to_string(),
        args: vec!["-c", "echo 'stdout'; echo 'stderr' >&2"]
            .into_iter()
            .map(String::from)
            .collect(),
        env: Default::default(),
        working_dir: None,
        timeout: None,
        stdin: None,
        suppress_stderr: false,
    };

    let result = runner.run_streaming(command, processors).await.unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout.len(), 1);
    assert_eq!(result.stdout[0], "stdout");
    assert_eq!(result.stderr.len(), 1);
    assert_eq!(result.stderr[0], "stderr");
}

#[tokio::test]
async fn test_backpressure_drop_oldest() {
    use crate::subprocess::streaming::{BufferedStreamProcessor, OverflowStrategy};
    use std::time::Duration;

    let inner = Box::new(LoggingProcessor::new("test"));
    let processor = BufferedStreamProcessor::new(
        inner,
        3, // Max buffer size of 3
        OverflowStrategy::DropOldest,
        Duration::from_secs(1),
    );

    // Add 5 lines to a buffer that can only hold 3
    for i in 1..=5 {
        processor
            .process_with_backpressure(
                format!("line{}", i),
                crate::subprocess::streaming::StreamSource::Stdout,
            )
            .await
            .unwrap();
    }

    // Buffer should have kept the 3 most recent lines
    assert_eq!(processor.buffer_size().await, 3);
}

#[tokio::test]
async fn test_rate_limited_processor() {
    use crate::subprocess::streaming::backpressure::RateLimitedProcessor;
    use std::time::{Duration, Instant};

    let inner = Box::new(LoggingProcessor::new("test"));
    let processor = RateLimitedProcessor::new(inner, 2); // Max 2 lines per second

    let start = Instant::now();

    // Try to process 4 lines - should take about 1 second due to rate limiting
    for i in 1..=4 {
        processor
            .process_line(
                &format!("line{}", i),
                crate::subprocess::streaming::StreamSource::Stdout,
            )
            .await
            .unwrap();
    }

    let elapsed = start.elapsed();
    // Should have taken at least 1 second to process 4 lines at 2 lines/sec
    assert!(elapsed >= Duration::from_millis(900)); // Allow some tolerance
}

#[tokio::test]
async fn test_execution_context_with_streaming() {
    use crate::cook::execution::{CommandRunner, ExecutionContext, RealCommandRunner};
    use crate::subprocess::streaming::{ProcessorConfig, StreamingConfig, StreamingMode};

    let runner = RealCommandRunner::new();

    let context = ExecutionContext {
        streaming_config: Some(StreamingConfig {
            enabled: true,
            mode: StreamingMode::Streaming,
            buffer_config: Default::default(),
            processors: vec![ProcessorConfig::JsonLines { emit_events: false }],
        }),
        ..Default::default()
    };

    let result = runner
        .run_with_context("echo", &["test".to_string()], &context)
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.stdout.trim(), "test");
}
