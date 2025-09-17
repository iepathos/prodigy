---
number: 57
title: Claude Streaming Output Support
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-01-03
updated: 2025-01-16
---

# Specification 57: Claude Streaming Output Support

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently executes Claude commands using `claude --print` which captures the complete output after execution finishes. This provides no visibility into Claude's execution progress, tool usage, or intermediate results. The Claude CLI supports `--output-format stream-json` with `--verbose` flags that provide real-time JSONL event streams during execution.

With Prodigy's existing comprehensive event system, web dashboard (port 8080), and progress tracking infrastructure already in place, adding Claude streaming support would provide immediate visibility into agent operations without requiring significant architectural changes. The existing `EventLogger`, `ProgressTracker`, and WebSocket dashboard can be leveraged to deliver real-time Claude observability.

## Objective

Extend the existing `ClaudeExecutorImpl` to support streaming JSON output from Claude CLI, parsing events in real-time and integrating them with Prodigy's existing event and progress tracking systems. This focused enhancement will provide immediate visibility into Claude agent operations while maintaining full backward compatibility.

## Requirements

### Functional Requirements

#### Stream Processing
- Add `--output-format stream-json --verbose` flags to Claude CLI execution
- Parse JSONL event stream line-by-line as it arrives
- Emit events to existing `EventLogger` infrastructure
- Maintain backward compatibility with `--print` mode
- Support configuration flag to enable/disable streaming

#### Event Integration
- Extend existing `MapReduceEvent` enum with Claude-specific events:
  - `ClaudeToolInvoked`: Tool name, parameters, timestamp
  - `ClaudeTokenUsage`: Input/output/cache token counts
  - `ClaudeSessionStarted`: Session ID, model, configuration
  - `ClaudeMessage`: Assistant responses and content
- Emit events through existing event pipeline
- Preserve correlation IDs for tracking

#### Output Handling
- Support both streaming and non-streaming modes via configuration
- Parse structured JSON output when available
- Fall back to text parsing for compatibility
- Buffer partial lines for complete JSON parsing
- Handle malformed JSON gracefully

### Non-Functional Requirements

#### Performance
- Minimal overhead on agent execution (< 2% CPU)
- Line-by-line processing without blocking
- Efficient buffer management for partial JSON
- Reuse existing event infrastructure

#### Reliability
- Graceful fallback to non-streaming mode on error
- Handle partial JSON lines correctly
- Continue execution even if streaming fails
- Log parsing errors without failing commands

#### Compatibility
- Full backward compatibility with existing workflows
- Opt-in streaming via configuration flag
- No changes required to existing workflow files
- Preserve existing `ExecutionResult` interface

## Acceptance Criteria

- [ ] Claude executor supports optional `--output-format stream-json` mode
- [ ] JSONL events are parsed line-by-line as they arrive
- [ ] Claude events are emitted to existing `EventLogger`
- [ ] Streaming mode is configurable via environment variable or config
- [ ] Backward compatibility maintained with `--print` mode
- [ ] Partial JSON lines are buffered correctly
- [ ] Parsing errors don't interrupt command execution
- [ ] Events appear in existing dashboard at port 8080
- [ ] Integration tests cover both streaming and non-streaming modes
- [ ] Performance overhead is less than 2% CPU

## Technical Details

### Implementation Approach

#### Extend ClaudeExecutorImpl
```rust
// src/cook/execution/claude.rs
impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    pub async fn execute_claude_command(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        // Check for streaming mode via env var
        let streaming = env_vars.get("PRODIGY_CLAUDE_STREAMING")
            .map_or(false, |v| v == "true");

        if streaming {
            self.execute_with_streaming(command, project_path, env_vars).await
        } else {
            // Existing --print mode execution
            self.execute_with_print(command, project_path, env_vars).await
        }
    }

    async fn execute_with_streaming(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        let args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            command.to_string(),
        ];

        // Use existing runner with streaming output capture
        let mut context = ExecutionContext::default();
        context.working_directory = project_path.to_path_buf();
        context.env_vars = env_vars;
        context.capture_streaming = true; // New flag

        self.runner.run_with_context("claude", &args, &context).await
    }
}
```

#### Extend MapReduceEvent Enum
```rust
// src/cook/execution/events/types.rs
pub enum MapReduceEvent {
    // Existing events...

    // New Claude-specific events
    ClaudeToolInvoked {
        agent_id: String,
        tool_name: String,
        tool_id: String,
        parameters: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    ClaudeTokenUsage {
        agent_id: String,
        input_tokens: u64,
        output_tokens: u64,
        cache_tokens: u64,
    },
    ClaudeSessionStarted {
        agent_id: String,
        session_id: String,
        model: String,
        tools: Vec<String>,
    },
    ClaudeMessage {
        agent_id: String,
        content: String,
        message_type: String,
    },
}
```

#### Enhanced CommandRunner for Streaming
```rust
// src/cook/execution/runner.rs
impl RealCommandRunner {
    async fn run_with_context(
        &self,
        cmd: &str,
        args: &[String],
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        if context.capture_streaming {
            // Spawn process and capture streaming output
            let mut child = self.spawn_process(cmd, args, context)?;
            let stdout_handle = self.process_stream(child.stdout.take());

            let status = child.wait().await?;
            let output = stdout_handle.await?;

            Ok(ExecutionResult {
                success: status.success(),
                stdout: output,
                stderr: String::new(),
                exit_code: status.code(),
            })
        } else {
            // Existing batch output capture
            self.run_batch(cmd, args, context).await
        }
    }

    async fn process_stream(&self, stdout: ChildStdout) -> JoinHandle<String> {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut output = String::new();

            for line in reader.lines() {
                if let Ok(line) = line {
                    // Parse and emit Claude events if JSON
                    if let Ok(event) = serde_json::from_str::<Value>(&line) {
                        Self::emit_claude_event(event).await;
                    }
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            output
        })
    }
}
```

## Dependencies

- **Prerequisites**: None (leverages existing infrastructure)
- **Affected Components**:
  - `ClaudeExecutor`: Add streaming mode support
  - `CommandRunner`: Enhanced to capture streaming output
  - `MapReduceEvent`: Extended with Claude-specific events
- **External Dependencies**: None (uses existing Tokio, serde_json)

## Testing Strategy

### Unit Tests
- Streaming mode flag detection and configuration
- JSONL parsing with partial lines
- Event emission to existing logger
- Fallback to non-streaming mode

### Integration Tests
- End-to-end streaming with real Claude CLI
- Both streaming and non-streaming modes
- Dashboard event visibility
- Performance overhead measurement

## Documentation Requirements

### User Documentation
- Configuration guide for enabling streaming mode
- Environment variable documentation
- Dashboard viewing instructions for Claude events

### Code Documentation
- New ExecutionContext.capture_streaming flag
- Claude event types in MapReduceEvent enum
- Streaming vs non-streaming mode selection

## Implementation Notes

### Configuration
```yaml
# Enable via environment variable
PRODIGY_CLAUDE_STREAMING=true

# Or via configuration file
claude:
  streaming_enabled: true
```

### Streaming Considerations
- `--verbose` flag required with `--output-format stream-json`
- Buffer partial JSON lines until complete
- Continue execution even if parsing fails
- Preserve complete output for ExecutionResult

## Migration and Compatibility

### Backward Compatibility
- Default to non-streaming mode (no changes required)
- Opt-in via configuration
- No changes to workflow files
- Existing ExecutionResult interface preserved

### Migration Path
1. Deploy with streaming disabled by default
2. Test with individual workflows using env var
3. Enable globally when validated
4. Monitor performance and reliability