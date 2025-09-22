---
number: 107
title: Implement Complete Streaming Support
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-09-21
---

# Specification 107: Implement Complete Streaming Support

## Context

Multiple TODO comments indicate incomplete streaming support throughout the codebase, particularly in subprocess handling, mock implementations, and progress rendering. The lack of streaming support means that long-running commands must buffer all output in memory before displaying it, leading to poor user experience and potential memory issues for commands with large output.

## Objective

Implement comprehensive streaming support for subprocess execution, mock testing, and progress display to provide real-time feedback and reduce memory usage.

## Requirements

### Functional Requirements

1. Implement streaming output for subprocess execution
2. Add streaming support to mock subprocess runner
3. Complete JsonProgressRenderer for streaming JSON output
4. Enable real-time output display for long-running commands
5. Support both stdout and stderr streaming
6. Maintain output ordering and interleaving

### Non-Functional Requirements

- No buffering of complete output in memory
- Latency under 100ms for output display
- Preserve ANSI color codes and formatting
- Support cancellation during streaming
- Thread-safe output handling

## Acceptance Criteria

- [ ] Subprocess runner supports streaming output
- [ ] Mock implementation supports streaming for tests
- [ ] JsonProgressRenderer fully implemented
- [ ] Real-time output visible during command execution
- [ ] No memory growth for large output streams
- [ ] Tests verify streaming behavior

## Technical Details

### Implementation Areas

1. **Subprocess Streaming (`/src/subprocess/runner.rs:309`)**
   ```rust
   // Current TODO: Implement streaming support

   // Implementation approach:
   pub async fn run_streaming<F>(
       &self,
       command: Command,
       on_stdout: F,
       on_stderr: F,
   ) -> Result<ExitStatus>
   where
       F: Fn(String) + Send + 'static,
   {
       let mut child = Command::new(&command.program)
           .args(&command.args)
           .stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .spawn()?;

       let stdout = child.stdout.take().unwrap();
       let stderr = child.stderr.take().unwrap();

       // Spawn tasks to read streams
       let stdout_task = tokio::spawn(async move {
           let reader = BufReader::new(stdout);
           let mut lines = reader.lines();
           while let Some(line) = lines.next_line().await? {
               on_stdout(line);
           }
           Ok::<(), io::Error>(())
       });

       let stderr_task = tokio::spawn(async move {
           let reader = BufReader::new(stderr);
           let mut lines = reader.lines();
           while let Some(line) = lines.next_line().await? {
               on_stderr(line);
           }
           Ok::<(), io::Error>(())
       });

       // Wait for completion
       let status = child.wait().await?;
       stdout_task.await??;
       stderr_task.await??;

       Ok(status)
   }
   ```

2. **Mock Streaming Support (`/src/subprocess/mock.rs:136`)**
   ```rust
   // Current TODO: Add streaming support to mock

   // Implementation:
   pub struct StreamingMockRunner {
       outputs: Arc<Mutex<Vec<StreamingOutput>>>,
   }

   struct StreamingOutput {
       pattern: String,
       stream: Box<dyn Stream<Item = String> + Send>,
       exit_code: i32,
   }

   impl StreamingMockRunner {
       pub async fn run_streaming<F>(
           &self,
           command: Command,
           on_output: F,
       ) -> Result<ExitStatus>
       where
           F: Fn(String) + Send + 'static,
       {
           // Find matching mock
           let outputs = self.outputs.lock().await;
           let mock = outputs.iter()
               .find(|o| o.pattern.matches(&command))
               .ok_or("No mock found")?;

           // Stream the output
           let mut stream = mock.stream.clone();
           while let Some(line) = stream.next().await {
               on_output(line);
               tokio::time::sleep(Duration::from_millis(10)).await;
           }

           Ok(ExitStatus::from_raw(mock.exit_code))
       }
   }
   ```

3. **JsonProgressRenderer (`/src/cook/execution/progress_display.rs:250`)**
   ```rust
   // Current TODO: Implement JsonProgressRenderer

   pub struct JsonProgressRenderer {
       output: Arc<Mutex<Box<dyn Write + Send>>>,
   }

   impl ProgressRenderer for JsonProgressRenderer {
       fn render(&self, event: ProgressEvent) -> Result<()> {
           let json = serde_json::to_string(&event)?;
           let mut output = self.output.lock().unwrap();
           writeln!(output, "{}", json)?;
           output.flush()?;
           Ok(())
       }

       fn start_step(&self, step: StepInfo) -> Result<()> {
           self.render(ProgressEvent::StepStarted(step))
       }

       fn update_progress(&self, progress: Progress) -> Result<()> {
           self.render(ProgressEvent::ProgressUpdate(progress))
       }

       fn complete_step(&self, result: StepResult) -> Result<()> {
           self.render(ProgressEvent::StepCompleted(result))
       }
   }
   ```

### Integration Points

1. **CLI Integration**
   - Add `--stream` flag to enable streaming output
   - Default to streaming for interactive terminals
   - Buffer for non-interactive environments

2. **Workflow Execution**
   - Stream output during shell command execution
   - Stream Claude command responses
   - Aggregate streaming for parallel execution

3. **Progress Display**
   - Unified interface for different renderers
   - Support switching between renderers
   - Maintain compatibility with existing display

## Dependencies

- Uses tokio for async I/O
- Requires changes to subprocess interface
- May impact existing tests

## Testing Strategy

1. **Unit Tests**
   - Test streaming with various output patterns
   - Verify correct line buffering
   - Test cancellation during streaming

2. **Integration Tests**
   - Test with real subprocess execution
   - Verify mock streaming behavior matches real
   - Test progress rendering with streaming

3. **Performance Tests**
   - Measure latency of output display
   - Verify no memory growth with large outputs
   - Test throughput for high-volume streams

## Documentation Requirements

- Document streaming API for subprocess runner
- Add examples of streaming usage
- Update mock documentation for streaming
- Document JsonProgressRenderer format