---
number: 71
title: Real-time Streaming Infrastructure
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-01-16
---

# Specification 71: Real-time Streaming Infrastructure

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Prodigy's current command execution model captures output in batch mode - commands run to completion before their output is available. This limits visibility into long-running processes, prevents real-time monitoring, and makes debugging difficult when commands hang or fail partway through execution.

Many command-line tools support streaming output that could provide valuable real-time feedback: build tools show compilation progress, test runners display results as they execute, deployment scripts report status updates, and AI agents like Claude emit events during processing. A generic streaming infrastructure would benefit all command executors in Prodigy, not just Claude.

## Objective

Implement a foundational streaming infrastructure in the `CommandRunner` that enables line-by-line output capture, real-time event processing, and multiplexed stream handling. This infrastructure will support any command executor that needs real-time output visibility while maintaining full backward compatibility with batch execution.

## Requirements

### Functional Requirements

#### Stream Capture
- Spawn processes with separate stdout/stderr streams
- Capture output line-by-line as it arrives
- Support both text and JSON line formats
- Handle partial lines and buffering correctly
- Multiplex stdout/stderr while preserving order

#### Stream Processing
- Process lines through configurable handlers
- Support multiple concurrent stream processors
- Enable filtering and transformation of output
- Parse structured formats (JSON, YAML) on the fly
- Emit events for matching patterns

#### Backpressure Management
- Buffer output when processors can't keep up
- Configurable buffer sizes and overflow behavior
- Drop strategies for excessive output
- Flow control between producer and consumers

#### Process Control
- Support graceful shutdown of streaming processes
- Handle process termination during streaming
- Timeout management for long-running streams
- Signal forwarding and handling

### Non-Functional Requirements

#### Performance
- Minimal overhead for non-streaming commands
- Efficient line buffering and parsing
- Zero-copy processing where possible
- Concurrent stream processing

#### Reliability
- Handle process crashes during streaming
- Recover from stream processing errors
- Preserve output completeness for audit
- Support reconnection for network streams

#### Compatibility
- Full backward compatibility with batch execution
- Opt-in streaming via ExecutionContext
- No API changes for existing callers
- Graceful degradation when streaming unavailable

## Acceptance Criteria

- [ ] CommandRunner supports streaming output capture
- [ ] Line-by-line processing works for stdout and stderr
- [ ] JSON lines are parsed and emitted as events
- [ ] Backpressure handling prevents memory issues
- [ ] Process control allows clean shutdown
- [ ] Performance overhead less than 5% for batch mode
- [ ] Streaming can be enabled per-command via context
- [ ] Multiple stream processors can run concurrently
- [ ] Integration tests cover streaming scenarios
- [ ] Documentation includes streaming usage examples

## Technical Details

### Implementation Approach

#### Enhanced ExecutionContext
```rust
// src/cook/execution/context.rs
pub struct ExecutionContext {
    // Existing fields...

    /// Enable streaming output capture
    pub streaming_mode: StreamingMode,
    /// Stream processors to apply
    pub stream_processors: Vec<Box<dyn StreamProcessor>>,
    /// Buffer configuration
    pub buffer_config: BufferConfig,
}

pub enum StreamingMode {
    /// Traditional batch capture (default)
    Batch,
    /// Line-by-line streaming
    Streaming,
    /// Streaming with structured parsing
    StructuredStreaming { format: OutputFormat },
}

pub enum OutputFormat {
    PlainText,
    JsonLines,
    YamlStream,
}
```

#### Stream Processor Trait
```rust
// src/subprocess/streaming.rs
#[async_trait]
pub trait StreamProcessor: Send + Sync {
    /// Process a line from the stream
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()>;

    /// Handle stream completion
    async fn on_complete(&self, exit_code: Option<i32>) -> Result<()>;

    /// Handle stream errors
    async fn on_error(&self, error: &Error) -> Result<()>;
}

pub enum StreamSource {
    Stdout,
    Stderr,
}

/// Example processor for JSON lines
pub struct JsonLineProcessor {
    event_sender: mpsc::Sender<serde_json::Value>,
}

#[async_trait]
impl StreamProcessor for JsonLineProcessor {
    async fn process_line(&self, line: &str, _source: StreamSource) -> Result<()> {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            self.event_sender.send(json).await?;
        }
        Ok(())
    }
}
```

#### Streaming Command Runner
```rust
// src/subprocess/runner.rs
impl SubprocessRunner {
    pub async fn run_streaming(
        &self,
        command: ProcessCommand,
        processors: Vec<Box<dyn StreamProcessor>>,
    ) -> Result<StreamingOutput> {
        let mut child = self.spawn_command(command)?;

        // Take ownership of streams
        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");

        // Create stream handlers
        let stdout_handle = self.process_stream(stdout, StreamSource::Stdout, &processors);
        let stderr_handle = self.process_stream(stderr, StreamSource::Stderr, &processors);

        // Wait for completion
        let status = child.wait().await?;
        let (stdout_lines, stderr_lines) = tokio::try_join!(stdout_handle, stderr_handle)?;

        Ok(StreamingOutput {
            status,
            stdout: stdout_lines,
            stderr: stderr_lines,
        })
    }

    async fn process_stream(
        &self,
        stream: impl AsyncRead + Unpin,
        source: StreamSource,
        processors: &[Box<dyn StreamProcessor>],
    ) -> JoinHandle<Result<Vec<String>>> {
        let processors = processors.to_vec();

        tokio::spawn(async move {
            let reader = BufReader::new(stream);
            let mut lines = reader.lines();
            let mut output = Vec::new();

            while let Some(line) = lines.next_line().await? {
                // Store for final output
                output.push(line.clone());

                // Process through handlers
                for processor in &processors {
                    processor.process_line(&line, source).await?;
                }
            }

            Ok(output)
        })
    }
}
```

#### Backpressure Management
```rust
// src/subprocess/backpressure.rs
pub struct BufferedStreamProcessor {
    inner: Box<dyn StreamProcessor>,
    buffer: Arc<Mutex<VecDeque<String>>>,
    max_buffer_size: usize,
    overflow_strategy: OverflowStrategy,
}

pub enum OverflowStrategy {
    /// Drop oldest lines
    DropOldest,
    /// Drop newest lines
    DropNewest,
    /// Block until space available
    Block,
    /// Fail with error
    Fail,
}

impl BufferedStreamProcessor {
    pub async fn process_with_backpressure(&self, line: String) -> Result<()> {
        let mut buffer = self.buffer.lock().await;

        if buffer.len() >= self.max_buffer_size {
            match self.overflow_strategy {
                OverflowStrategy::DropOldest => {
                    buffer.pop_front();
                    buffer.push_back(line);
                }
                OverflowStrategy::DropNewest => {
                    // Simply don't add the new line
                }
                OverflowStrategy::Block => {
                    // Wait for consumer to process
                    while buffer.len() >= self.max_buffer_size {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    buffer.push_back(line);
                }
                OverflowStrategy::Fail => {
                    return Err(anyhow!("Buffer overflow"));
                }
            }
        } else {
            buffer.push_back(line);
        }

        Ok(())
    }
}
```

### Architecture Integration

#### Layered Processing
```
Command Execution
       ↓
Process Spawning
       ↓
Stream Capture (stdout/stderr)
       ↓
Line Buffering
       ↓
Stream Processors (parallel)
    ├─ JSON Parser
    ├─ Event Emitter
    ├─ Pattern Matcher
    └─ Custom Handlers
       ↓
Output Aggregation
       ↓
ExecutionResult
```

### APIs and Interfaces

#### Streaming Configuration
```rust
pub struct StreamingConfig {
    /// Enable streaming mode
    pub enabled: bool,
    /// Line buffer size
    pub line_buffer_size: usize,
    /// Maximum lines to keep in memory
    pub max_lines: Option<usize>,
    /// Processors to apply
    pub processors: Vec<ProcessorConfig>,
}

pub enum ProcessorConfig {
    JsonLines { emit_events: bool },
    PatternMatcher { patterns: Vec<Regex> },
    EventEmitter { event_type: String },
    Custom { handler: Box<dyn StreamProcessor> },
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - All command executors benefit from streaming
  - Event system receives streamed events
  - Dashboard can display real-time output
- **External Dependencies**:
  - `tokio`: Async streams and spawning
  - `futures`: Stream utilities

## Testing Strategy

### Unit Tests
- Line buffering with partial lines
- JSON parsing from streams
- Backpressure handling strategies
- Process termination during streaming

### Integration Tests
- Real process streaming with various tools
- Concurrent stream processing
- Large output handling
- Error recovery scenarios

### Performance Tests
- Overhead measurement for batch mode
- Streaming throughput limits
- Memory usage under load
- Concurrent process limits

## Documentation Requirements

### Code Documentation
- StreamProcessor trait and implementations
- Streaming configuration options
- Backpressure strategies
- Usage examples

### User Documentation
- Enabling streaming for commands
- Writing custom stream processors
- Performance considerations
- Troubleshooting guide

## Implementation Notes

### Design Considerations
- Use async streams for efficiency
- Leverage tokio's channel primitives
- Consider zero-copy where possible
- Plan for future WebSocket streaming

### Error Handling
- Never fail the command due to streaming errors
- Log processor errors without interrupting
- Graceful degradation to batch mode
- Preserve output for debugging

### Future Extensions
- WebSocket streaming to remote clients
- Compression for large streams
- Record and replay functionality
- Stream filtering and routing

## Migration and Compatibility

### Backward Compatibility
- Default to batch mode (no changes)
- Streaming is opt-in per command
- Existing APIs unchanged
- No performance impact when disabled

### Adoption Path
1. Implement core streaming infrastructure
2. Add to Claude executor (spec 57)
3. Enable for build and test commands
4. Extend to all appropriate executors
5. Add dashboard integration