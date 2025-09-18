---
number: 97
title: Real-time Claude Streaming Integration
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-09-18
---

# Specification 97: Real-time Claude Streaming Integration

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, Prodigy has partially implemented Claude streaming support but lacks true real-time output visibility. Users experience "hanging" behavior during long Claude operations because:

1. **Streaming flags are set correctly** (`--output-format stream-json --verbose`) but output is buffered
2. **CommandRunner interface doesn't support streaming** - waits for complete execution before processing output
3. **Existing StreamingCommandRunner** exists but isn't integrated with ClaudeExecutor
4. **No real-time feedback** for users during lengthy Claude operations (spec implementation, debugging, etc.)

This creates poor user experience, especially for workflows like `/prodigy-implement-spec` that can take several minutes with no visible progress.

## Objective

Integrate real-time streaming output from Claude commands into both MapReduce and regular workflows, providing immediate visual feedback and enabling real-time event processing without breaking existing functionality.

## Requirements

### Functional Requirements

- **Real-time Output Display**: Claude streaming JSON appears in console as it's generated
- **Backward Compatibility**: Existing CommandRunner interface continues to work unchanged
- **Event Processing**: Streaming events (tool invocations, token usage) are processed in real-time
- **User Feedback**: Progress indicators and tool invocations visible during execution
- **Error Handling**: Streaming failures gracefully fall back to buffered execution
- **Integration**: Works with both MapReduce agents and regular workflow execution

### Non-Functional Requirements

- **Performance**: Minimal overhead compared to current buffered execution
- **Memory Efficiency**: Stream processing doesn't accumulate unbounded buffers
- **Reliability**: Network interruptions don't break streaming, fallback to buffered mode
- **Testability**: Streaming behavior is mockable and testable
- **Configuration**: Streaming can be disabled via environment variable

## Acceptance Criteria

- [ ] Claude commands show real-time output during execution (not after completion)
- [ ] Tool invocations appear immediately as Claude uses them
- [ ] Token usage metrics update in real-time during execution
- [ ] Progress indicators show Claude's current activity
- [ ] Streaming works for both MapReduce and regular workflows
- [ ] Fallback to buffered mode when streaming fails
- [ ] All existing tests continue to pass
- [ ] `PRODIGY_CLAUDE_STREAMING=false` disables real-time streaming
- [ ] Mock testing infrastructure supports streaming scenarios
- [ ] Memory usage remains bounded during long-running operations
- [ ] Error handling preserves original error context when streaming fails

## Technical Details

### Implementation Approach

1. **Extend CommandRunner Interface**
   - Add optional streaming support to existing interface
   - Maintain backward compatibility with current implementations
   - Enable real-time output processing through callback mechanisms

2. **Integrate StreamingCommandRunner**
   - Connect existing streaming infrastructure to ClaudeExecutor
   - Map ExecutionContext to StreamingConfig
   - Process Claude JSON events in real-time

3. **Event Processing Pipeline**
   - Parse Claude streaming JSON line-by-line
   - Emit real-time events to console and event logger
   - Maintain complete output buffer for backward compatibility

### Architecture Changes

#### CommandRunner Interface Extension

```rust
#[async_trait]
pub trait CommandRunner: Send + Sync {
    // Existing methods remain unchanged
    async fn run_command(&self, cmd: &str, args: &[String]) -> Result<std::process::Output>;
    async fn run_with_context(&self, cmd: &str, args: &[String], context: &ExecutionContext) -> Result<ExecutionResult>;

    // New streaming method (optional implementation)
    async fn run_with_streaming(
        &self,
        cmd: &str,
        args: &[String],
        context: &ExecutionContext,
        output_handler: Box<dyn StreamOutputHandler>,
    ) -> Result<ExecutionResult> {
        // Default implementation falls back to buffered execution
        self.run_with_context(cmd, args, context).await
    }
}

#[async_trait]
pub trait StreamOutputHandler: Send + Sync {
    async fn handle_stdout_line(&mut self, line: &str) -> Result<()>;
    async fn handle_stderr_line(&mut self, line: &str) -> Result<()>;
    async fn handle_completion(&mut self, result: &ExecutionResult) -> Result<()>;
}
```

#### ClaudeExecutor Integration

```rust
impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    async fn execute_with_streaming(&self, /* params */) -> Result<ExecutionResult> {
        if context.capture_streaming {
            // Use new streaming interface if available
            let handler = ClaudeStreamHandler::new(self.event_logger.clone());
            self.runner.run_with_streaming(cmd, args, context, Box::new(handler)).await
        } else {
            // Fall back to current implementation
            self.runner.run_with_context(cmd, args, context).await
        }
    }
}
```

#### Real-time Event Handler

```rust
pub struct ClaudeStreamHandler {
    event_logger: Option<Arc<EventLogger>>,
    output_buffer: String,
    user_interface: Arc<dyn UserInterface>,
}

impl StreamOutputHandler for ClaudeStreamHandler {
    async fn handle_stdout_line(&mut self, line: &str) -> Result<()> {
        // Buffer for compatibility
        self.output_buffer.push_str(line);
        self.output_buffer.push('\n');

        // Real-time processing
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            self.process_claude_event(json).await?;
        }

        // Display to user
        self.user_interface.display_streaming_line(line).await?;
        Ok(())
    }

    async fn process_claude_event(&mut self, event: Value) -> Result<()> {
        match event.get("event").and_then(|v| v.as_str()) {
            Some("tool_use") => self.display_tool_invocation(&event).await,
            Some("token_usage") => self.display_token_metrics(&event).await,
            Some("message") => self.display_claude_message(&event).await,
            _ => Ok(())
        }
    }
}
```

### Data Structures

#### Streaming Configuration

```rust
pub struct StreamingConfig {
    pub enabled: bool,
    pub buffer_size: usize,
    pub timeout_ms: u64,
    pub event_handler: Option<Box<dyn StreamOutputHandler>>,
}

impl From<&ExecutionContext> for StreamingConfig {
    fn from(context: &ExecutionContext) -> Self {
        StreamingConfig {
            enabled: context.capture_streaming,
            buffer_size: 8192,
            timeout_ms: 30000,
            event_handler: None, // Set by caller
        }
    }
}
```

#### User Interface Extensions

```rust
#[async_trait]
pub trait UserInterface: Send + Sync {
    async fn display_streaming_line(&self, line: &str) -> Result<()>;
    async fn display_tool_invocation(&self, tool_name: &str, params: &Value) -> Result<()>;
    async fn display_token_metrics(&self, input: u64, output: u64, cache: u64) -> Result<()>;
    async fn display_progress_update(&self, message: &str) -> Result<()>;
}
```

### APIs and Interfaces

#### Enhanced RealCommandRunner

```rust
impl CommandRunner for RealCommandRunner {
    async fn run_with_streaming(
        &self,
        cmd: &str,
        args: &[String],
        context: &ExecutionContext,
        output_handler: Box<dyn StreamOutputHandler>,
    ) -> Result<ExecutionResult> {
        if context.capture_streaming {
            let streaming_runner = StreamingCommandRunner::new(
                Box::new(self.subprocess.clone())
            );

            let processors = vec![
                Box::new(CallbackProcessor::new(output_handler)),
            ];

            let command = ProcessCommand {
                program: cmd.to_string(),
                args: args.to_vec(),
                env: context.env_vars.clone(),
                working_dir: Some(context.working_directory.clone()),
                suppress_stderr: false,
            };

            let streaming_result = streaming_runner.run_streaming(command, processors).await?;

            // Convert to ExecutionResult for compatibility
            Ok(ExecutionResult {
                success: streaming_result.exit_code == 0,
                stdout: streaming_result.stdout,
                stderr: streaming_result.stderr,
                exit_code: Some(streaming_result.exit_code),
            })
        } else {
            // Use existing buffered implementation
            self.run_with_context(cmd, args, context).await
        }
    }
}
```

## Dependencies

- **Prerequisites**: None (builds on existing streaming infrastructure)
- **Affected Components**:
  - `ClaudeExecutorImpl` - Modified to use streaming interface
  - `CommandRunner` trait - Extended with optional streaming method
  - `RealCommandRunner` - Implements streaming support
  - `UserInteraction` trait - Extended for real-time display
  - `StreamingCommandRunner` - Integration point for existing streaming
- **External Dependencies**: None (uses existing tokio, serde_json)

## Testing Strategy

### Unit Tests

- **CommandRunner Interface**: Test streaming method fallback behavior
- **ClaudeStreamHandler**: Test JSON parsing and event emission
- **Event Processing**: Test tool invocation, token usage, message parsing
- **Fallback Logic**: Test graceful degradation when streaming fails
- **Buffer Management**: Test output accumulation for compatibility

### Integration Tests

- **End-to-End Streaming**: Test complete workflow with real Claude commands
- **MapReduce Integration**: Test streaming in agent execution context
- **Error Scenarios**: Test network failures, invalid JSON, process crashes
- **Performance**: Test memory usage during long-running operations
- **Configuration**: Test enabling/disabling via environment variables

### Mock Testing

- **StreamOutputHandler Mocking**: Enable unit tests for streaming behavior
- **MockCommandRunner Extension**: Add streaming support to test infrastructure
- **Event Verification**: Test that expected events are emitted at correct times

## Documentation Requirements

### Code Documentation

- **StreamOutputHandler Trait**: Comprehensive documentation with examples
- **Integration Patterns**: Document how to add streaming to new command types
- **Error Handling**: Document fallback behavior and error scenarios
- **Performance Considerations**: Document memory and CPU implications

### User Documentation

- **Streaming Output Guide**: How to interpret real-time Claude output
- **Troubleshooting**: Common streaming issues and solutions
- **Configuration**: How to enable/disable streaming features
- **Performance Tuning**: Optimizing for different use cases

### Architecture Updates

- **CommandRunner Evolution**: Document interface extensions and compatibility
- **Streaming Architecture**: Update ARCHITECTURE.md with streaming flow diagrams
- **Event System**: Document real-time event processing pipeline

## Implementation Notes

### Phase 1: Interface Extensions
1. Extend CommandRunner trait with optional streaming method
2. Add StreamOutputHandler trait and basic implementation
3. Ensure all existing code continues to work unchanged

### Phase 2: ClaudeExecutor Integration
1. Modify ClaudeExecutorImpl to detect streaming context
2. Integrate with StreamingCommandRunner infrastructure
3. Implement ClaudeStreamHandler for JSON processing

### Phase 3: User Interface Enhancements
1. Add real-time display methods to UserInteraction trait
2. Implement console output formatting for streaming events
3. Add progress indicators and status updates

### Phase 4: Testing and Validation
1. Add comprehensive unit and integration tests
2. Test with real Claude workflows (implement-spec, debug-failure)
3. Validate memory usage and performance characteristics
4. Ensure backward compatibility with existing workflows

### Performance Considerations

- **Buffering Strategy**: Balance real-time display with output buffering
- **Memory Management**: Prevent unbounded growth during long operations
- **CPU Usage**: Minimize JSON parsing overhead for high-frequency events
- **Network Resilience**: Handle transient network issues gracefully

### Error Scenarios

- **Streaming Failure**: Graceful fallback to buffered execution
- **Invalid JSON**: Skip malformed lines, continue processing
- **Handler Errors**: Log errors but don't interrupt command execution
- **Resource Exhaustion**: Implement backpressure and rate limiting

## Migration and Compatibility

### Backward Compatibility

- **Existing CommandRunner Implementations**: Continue to work unchanged
- **Current ClaudeExecutor Usage**: Maintains identical behavior by default
- **Test Infrastructure**: MockCommandRunner supports both modes
- **Configuration**: Streaming disabled maintains exact current behavior

### Migration Path

1. **Immediate**: New streaming interface available but optional
2. **Gradual Adoption**: Individual command types can opt into streaming
3. **Full Integration**: All streaming-capable commands use real-time output
4. **Performance Tuning**: Optimize based on real-world usage patterns

### Breaking Changes

- **None**: All changes are additive and backward compatible
- **Future**: May deprecate old buffered-only interfaces in major version

## Success Metrics

### User Experience
- **Feedback Responsiveness**: Users see Claude activity within 1 second of generation
- **Progress Clarity**: Users understand what Claude is doing during long operations
- **Error Transparency**: Real-time error messages improve debugging experience

### Technical Performance
- **Memory Usage**: No more than 10% increase in peak memory usage
- **CPU Overhead**: Less than 5% additional CPU usage for streaming processing
- **Latency**: Real-time events appear within 100ms of generation
- **Reliability**: 99.9% success rate for streaming mode, graceful fallback otherwise

### Development Experience
- **Test Coverage**: 100% coverage for new streaming interfaces
- **Documentation Quality**: Clear examples and troubleshooting guides
- **Integration Ease**: Other command types can add streaming in <50 lines of code