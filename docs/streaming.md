# Streaming Infrastructure Documentation

## Overview

Prodigy's streaming infrastructure enables real-time processing of command output, providing line-by-line visibility into long-running processes. This is particularly useful for:

- Build tools showing compilation progress
- Test runners displaying results as they execute
- Deployment scripts reporting status updates
- AI agents like Claude emitting events during processing
- Any command that produces progressive output

## Features

- **Line-by-line capture**: Process output as it arrives, not after completion
- **Multiple stream processors**: Apply multiple processors to the same stream
- **Backpressure management**: Handle fast producers and slow consumers
- **JSON line parsing**: Automatically parse and emit JSON events
- **Pattern matching**: Extract structured data from unstructured output
- **Full backward compatibility**: Batch mode remains the default

## Usage

### Basic Streaming Configuration

```rust
use prodigy::subprocess::streaming::{StreamingConfig, StreamingMode};
use prodigy::cook::execution::ExecutionContext;

// Create an execution context with streaming enabled
let mut context = ExecutionContext::default();
context.streaming_config = Some(StreamingConfig {
    enabled: true,
    mode: StreamingMode::Streaming,
    buffer_config: Default::default(),
    processors: vec![],
});
```

### JSON Line Processing

Process commands that output JSON lines (like Claude events):

```rust
use prodigy::subprocess::streaming::{ProcessorConfig, StreamingConfig, StreamingMode};

let streaming_config = StreamingConfig {
    enabled: true,
    mode: StreamingMode::StructuredStreaming {
        format: OutputFormat::JsonLines,
    },
    buffer_config: Default::default(),
    processors: vec![
        ProcessorConfig::JsonLines { emit_events: true }
    ],
};
```

### Pattern Matching

Extract specific patterns from output:

```rust
use regex::Regex;
use prodigy::subprocess::streaming::ProcessorConfig;

let processors = vec![
    ProcessorConfig::PatternMatcher {
        patterns: vec![
            Regex::new(r"ERROR: (.+)").unwrap(),
            Regex::new(r"PROGRESS: (\d+)%").unwrap(),
        ],
    },
];
```

### Backpressure Configuration

Configure how to handle buffer overflow:

```rust
use prodigy::subprocess::streaming::{BufferConfig, OverflowStrategy};
use std::time::Duration;

let buffer_config = BufferConfig {
    line_buffer_size: 8192,         // Line buffer size in bytes
    max_lines: Some(10000),          // Maximum lines to keep in memory
    overflow_strategy: OverflowStrategy::DropOldest,  // Strategy when full
    block_timeout: Duration::from_secs(5),  // Timeout for Block strategy
};
```

Overflow strategies:
- `DropOldest`: Remove oldest lines when buffer is full
- `DropNewest`: Discard new lines when buffer is full
- `Block`: Wait for space (up to timeout)
- `Fail`: Return error on overflow

## Custom Stream Processors

Implement the `StreamProcessor` trait for custom processing:

```rust
use async_trait::async_trait;
use prodigy::subprocess::streaming::{StreamProcessor, StreamSource};
use anyhow::Result;

pub struct MyCustomProcessor {
    // Your fields
}

#[async_trait]
impl StreamProcessor for MyCustomProcessor {
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()> {
        // Process each line
        println!("[{:?}] {}", source, line);
        Ok(())
    }

    async fn on_complete(&self, exit_code: Option<i32>) -> Result<()> {
        // Handle completion
        println!("Process completed with code: {:?}", exit_code);
        Ok(())
    }

    async fn on_error(&self, error: &anyhow::Error) -> Result<()> {
        // Handle errors
        eprintln!("Error: {}", error);
        Ok(())
    }
}
```

## Integration with CommandRunner

The streaming infrastructure integrates seamlessly with Prodigy's CommandRunner:

```rust
use prodigy::cook::execution::{CommandRunner, RealCommandRunner, ExecutionContext};
use prodigy::subprocess::streaming::{StreamingConfig, ProcessorConfig};

let runner = RealCommandRunner::new();
let mut context = ExecutionContext::default();

// Enable streaming with JSON processing
context.streaming_config = Some(StreamingConfig {
    enabled: true,
    mode: StreamingMode::StructuredStreaming {
        format: OutputFormat::JsonLines,
    },
    buffer_config: Default::default(),
    processors: vec![
        ProcessorConfig::JsonLines { emit_events: true }
    ],
});

// Run command with streaming
let result = runner
    .run_with_context("my-command", &["arg1", "arg2"], &context)
    .await?;
```

## Performance Considerations

### Overhead

- Streaming mode adds minimal overhead (< 5%) when no processors are active
- Line buffering requires additional memory proportional to line length
- Multiple processors run concurrently, sharing the same stream

### Memory Usage

- Default buffer holds up to 10,000 lines
- Each processor may maintain its own internal state
- Configure `max_lines` to limit memory usage

### Best Practices

1. **Use batch mode by default**: Only enable streaming when needed
2. **Configure appropriate buffers**: Match buffer size to expected output volume
3. **Handle backpressure**: Choose the right overflow strategy for your use case
4. **Process asynchronously**: Don't block in stream processors
5. **Log processor errors**: Continue processing even if one processor fails

## Future Extensions

The streaming infrastructure is designed to support:

- WebSocket streaming to remote clients
- Stream compression for large outputs
- Record and replay functionality
- Stream filtering and routing
- Integration with event systems