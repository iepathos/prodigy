---
number: 121
title: Capture and Log Claude JSON Log Location in Workflows
category: observability
priority: medium
status: draft
dependencies: []
created: 2025-10-04
---

# Specification 121: Capture and Log Claude JSON Log Location in Workflows

**Category**: observability
**Priority**: medium
**Status**: draft
**Dependencies**: []

## Context

Prodigy executes Claude commands in workflows using streaming JSON mode (`--output-format stream-json`). Claude automatically saves these streaming JSON logs to `~/.claude/projects/` as `.jsonl` files. These logs contain valuable debugging information including:

- Full conversation history and context
- Tool invocations and parameters
- Token usage metrics
- Error messages and stack traces
- Model responses and reasoning

Currently, when Claude commands execute in workflows (especially with `-v` or higher verbosity), we stream the JSON output to console but **do not capture or log the location** where Claude saved the complete JSON log file. This makes it difficult for users to:

1. Find the full JSON log for debugging after execution
2. Review complete interaction history for failed commands
3. Access detailed tool invocation data for analysis
4. Debug issues that occurred during workflow execution

## Objective

Capture the Claude JSON log file location during workflow execution and log it for user reference, especially when streaming output is enabled (`-v` or higher verbosity).

This provides users with a direct path to the complete JSON log for debugging and analysis, improving observability and troubleshooting capabilities.

## Requirements

### Functional Requirements

**FR1**: **Detect Claude JSON Log Location**
- After executing a Claude command with `--output-format stream-json`
- Determine the `.jsonl` file path where Claude saved the streaming log
- Support detection via:
  - Claude CLI output (if it provides the path)
  - Path inference based on project directory and session ID
  - File system search in `~/.claude/projects/` as fallback

**FR2**: **Log Location at Appropriate Verbosity**
- When verbosity >= 1 (`-v` flag), display the JSON log location to console
- Include clear formatting to distinguish from other output
- Display location immediately after Claude command completes
- Example: `📝 Claude JSON log: /Users/glen/.claude/projects/-Users-glen-prodigy/abc123.jsonl`

**FR3**: **Include Location in Execution Result**
- Store JSON log location in `ExecutionResult` metadata
- Make location accessible to workflows and error handlers
- Preserve location in checkpoint data for resume functionality
- Enable programmatic access to log location

**FR4**: **MapReduce Integration**
- Capture JSON log location for each MapReduce agent execution
- Store agent log locations in MapReduce events
- Include log location in DLQ entries for failed items
- Display log locations in reduce phase summary

### Non-Functional Requirements

**NFR1**: **Performance**: Log location detection should add <100ms to command execution time

**NFR2**: **Reliability**: Fall back gracefully if log location cannot be determined

**NFR3**: **Compatibility**: Work with both streaming and non-streaming Claude execution modes

**NFR4**: **Portability**: Support different home directory paths and project locations

## Acceptance Criteria

- [ ] Claude JSON log location is captured during streaming execution
- [ ] Log location is displayed to console when verbosity >= 1
- [ ] Log location is stored in `ExecutionResult` metadata
- [ ] MapReduce agent executions include JSON log location in events
- [ ] DLQ entries include JSON log location for failed items
- [ ] Log location detection has appropriate fallback mechanisms
- [ ] Documentation updated to explain JSON log location feature
- [ ] Tests verify log location capture for various scenarios
- [ ] Error messages reference JSON log location when available

## Technical Details

### Implementation Approach

#### 1. Log Location Detection Strategy

**Option A: Parse Claude CLI Output** (Preferred if available)
```rust
// Check if Claude CLI provides log location in its output
// Look for patterns like:
// "Session log: /Users/glen/.claude/projects/.../session.jsonl"
// "Log saved to: /path/to/log.jsonl"
```

**Option B: Infer from Project Path and Session ID**
```rust
// Claude creates project directories based on working directory
// Format: ~/.claude/projects/{sanitized-project-path}/{session-id}.jsonl
fn infer_log_location(project_path: &Path, session_id: &str) -> PathBuf {
    let home = env::var("HOME").unwrap_or_default();
    let sanitized = sanitize_project_path(project_path);
    PathBuf::from(home)
        .join(".claude/projects")
        .join(sanitized)
        .join(format!("{}.jsonl", session_id))
}

fn sanitize_project_path(path: &Path) -> String {
    // Claude sanitizes paths by replacing '/' with '-'
    path.to_string_lossy()
        .replace('/', "-")
        .trim_start_matches('-')
        .to_string()
}
```

**Option C: Search for Recent Log Files** (Fallback)
```rust
// Search ~/.claude/projects/ for recently created .jsonl files
// Match by modification time (within last N seconds)
async fn find_recent_log(since: SystemTime) -> Option<PathBuf> {
    let projects_dir = PathBuf::from(env::var("HOME")?).join(".claude/projects");

    WalkDir::new(projects_dir)
        .max_depth(2) // Project dir + log files
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension() == Some(OsStr::new("jsonl")))
        .filter(|e| e.metadata().ok()?.modified().ok()? > since)
        .map(|e| e.path().to_path_buf())
        .next()
}
```

#### 2. Execution Result Enhancement

**Update `ExecutionResult` Structure**:
```rust
pub struct ExecutionResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,

    // NEW: Metadata map for extensibility
    pub metadata: HashMap<String, String>,
}

impl ExecutionResult {
    pub fn with_json_log_location(mut self, location: PathBuf) -> Self {
        self.metadata.insert(
            "claude_json_log".to_string(),
            location.to_string_lossy().to_string()
        );
        self
    }

    pub fn json_log_location(&self) -> Option<&str> {
        self.metadata.get("claude_json_log").map(String::as_str)
    }
}
```

#### 3. Claude Executor Integration

**Update `claude.rs` execution logic**:
```rust
async fn execute_with_streaming(
    &self,
    command: &str,
    project_path: &Path,
    env_vars: HashMap<String, String>,
) -> Result<ExecutionResult> {
    // Record start time for log file search
    let execution_start = SystemTime::now();

    // ... existing streaming execution code ...

    let mut result = self.runner
        .run_with_streaming("claude", &args, &context, processor)
        .await?;

    // Detect JSON log location
    if let Some(log_location) = detect_json_log_location(
        project_path,
        &result.stdout,
        execution_start
    ).await {
        // Store in metadata
        result = result.with_json_log_location(log_location.clone());

        // Log to console if verbose
        if self.verbosity >= 1 {
            println!("📝 Claude JSON log: {}", log_location.display());
        }

        // Log to tracing for debugging
        tracing::info!(
            "Claude JSON log saved to: {}",
            log_location.display()
        );
    } else {
        tracing::warn!("Could not detect Claude JSON log location");
    }

    Ok(result)
}

async fn detect_json_log_location(
    project_path: &Path,
    cli_output: &str,
    execution_start: SystemTime,
) -> Option<PathBuf> {
    // Try parsing CLI output first
    if let Some(path) = parse_log_location_from_output(cli_output) {
        if path.exists() {
            return Some(path);
        }
    }

    // Try searching for recent files
    if let Some(path) = find_recent_log(execution_start).await {
        return Some(path);
    }

    // Log detection failure
    tracing::debug!("Could not detect Claude JSON log location");
    None
}
```

#### 4. MapReduce Event Integration

**Update MapReduce events**:
```rust
pub enum MapReduceEvent {
    // ... existing variants ...

    ClaudeCommandCompleted {
        agent_id: String,
        command: String,
        success: bool,
        json_log_location: Option<PathBuf>, // NEW
        timestamp: DateTime<Utc>,
    },
}
```

**Update DLQ entries**:
```rust
pub struct DlqEntry {
    pub work_item: Value,
    pub error: String,
    pub agent_id: String,
    pub correlation_id: String,
    pub attempts: usize,
    pub json_log_location: Option<PathBuf>, // NEW - for debugging
    pub failed_at: DateTime<Utc>,
}
```

#### 5. Display Formatting

**Console Output Format** (verbosity >= 1):
```
🔧 Tool invoked: Edit
📊 Tokens - Input: 1234, Output: 567, Cache: 89
✅ Command completed successfully
📝 Claude JSON log: /Users/glen/.claude/projects/-Users-glen-prodigy/abc123.jsonl
```

**Error Messages** (include log location when available):
```
❌ Claude command failed: /prodigy-fix-errors
   Exit code: 1
   Error: Test compilation failed
   📝 Full log: /Users/glen/.claude/projects/.../session.jsonl
```

### Architecture Changes

**Modified Components**:
- `src/cook/execution/claude.rs` - Add log location detection
- `src/cook/execution/mod.rs` - Update `ExecutionResult` with metadata
- `src/cook/execution/mapreduce/command/claude.rs` - Capture log location for agents
- `src/cook/execution/events.rs` - Add log location to events
- `src/storage/dlq.rs` - Include log location in DLQ entries

**New Modules**:
- `src/cook/execution/claude_log_detection.rs` - Log location detection logic

### Edge Cases

**Edge Case 1: Multiple Concurrent Claude Commands**
- **Issue**: Multiple commands may create logs simultaneously
- **Solution**: Use execution start time + recent file matching to find correct log

**Edge Case 2: Claude CLI Doesn't Create Log**
- **Issue**: Some error conditions may prevent log creation
- **Solution**: Detection returns `None`, execution continues normally

**Edge Case 3: Insufficient Permissions**
- **Issue**: User may not have read access to `~/.claude/projects/`
- **Solution**: Catch I/O errors gracefully, log warning, continue execution

**Edge Case 4: Custom HOME Directory**
- **Issue**: Non-standard home directory location
- **Solution**: Respect `HOME` environment variable, fall back to parsing output

## Dependencies

**Prerequisites**:
- Claude CLI must be installed and functional
- Workflows must use streaming mode for log capture
- File system access to `~/.claude/projects/` directory

**Affected Components**:
- Workflow execution: Enhanced with log location metadata
- Error reporting: Include log location in error messages
- MapReduce agents: Store log locations in events and DLQ
- Resume functionality: Preserve log locations in checkpoints

## Testing Strategy

### Unit Tests

**Test 1: Log Location Parsing**
```rust
#[test]
fn test_parse_log_location_from_output() {
    let output = "Session log: /Users/test/.claude/projects/test/abc.jsonl";
    let result = parse_log_location_from_output(output);
    assert_eq!(
        result,
        Some(PathBuf::from("/Users/test/.claude/projects/test/abc.jsonl"))
    );
}
```

**Test 2: Path Sanitization**
```rust
#[test]
fn test_sanitize_project_path() {
    assert_eq!(
        sanitize_project_path(&PathBuf::from("/Users/glen/prodigy")),
        "Users-glen-prodigy"
    );
}
```

**Test 3: ExecutionResult Metadata**
```rust
#[test]
fn test_execution_result_with_json_log() {
    let result = ExecutionResult::default()
        .with_json_log_location(PathBuf::from("/test/log.jsonl"));

    assert_eq!(
        result.json_log_location(),
        Some("/test/log.jsonl")
    );
}
```

### Integration Tests

**Test 1: Streaming Execution Captures Log Location**
```rust
#[tokio::test]
async fn test_streaming_execution_captures_log_location() {
    // Execute Claude command with streaming
    let result = execute_claude_streaming("/test-command").await.unwrap();

    // Verify log location is captured
    assert!(result.json_log_location().is_some());

    // Verify file exists
    let log_path = PathBuf::from(result.json_log_location().unwrap());
    assert!(log_path.exists());
}
```

**Test 2: MapReduce Agent Log Locations**
```rust
#[tokio::test]
async fn test_mapreduce_captures_agent_logs() {
    // Run MapReduce workflow
    let result = run_mapreduce_workflow().await.unwrap();

    // Verify each agent has log location in events
    let events = load_events(&result.job_id).await.unwrap();

    for event in events {
        if let MapReduceEvent::ClaudeCommandCompleted { json_log_location, .. } = event {
            assert!(json_log_location.is_some());
        }
    }
}
```

**Test 3: DLQ Entries Include Log Location**
```rust
#[tokio::test]
async fn test_dlq_includes_log_location() {
    // Run workflow that generates DLQ entry
    let _ = run_failing_workflow().await;

    // Load DLQ entry
    let dlq_entries = load_dlq_entries("test-job").await.unwrap();

    // Verify log location is present
    assert!(dlq_entries[0].json_log_location.is_some());
}
```

### User Acceptance

**Acceptance 1: Visible Log Locations**
- User runs workflow with `-v` flag
- After each Claude command, log location is displayed
- User can copy/paste path to view full JSON log
- Log file exists and contains expected content

**Acceptance 2: Error Debugging**
- Claude command fails during workflow
- Error message includes JSON log location
- User opens log file to see detailed error context
- Log contains tool invocations and error messages

**Acceptance 3: MapReduce Debugging**
- MapReduce workflow completes with some failures
- DLQ entries include JSON log locations
- User reviews specific agent logs to debug failures
- Events file contains all agent log locations

## Documentation Requirements

### Code Documentation

**Add to `claude.rs`**:
```rust
/// Detects the location of Claude's JSON log file after command execution.
///
/// Claude automatically saves streaming JSON logs to ~/.claude/projects/.
/// This function attempts to detect the log file location using multiple strategies:
/// 1. Parse log location from Claude CLI output
/// 2. Search for recently created .jsonl files
///
/// # Arguments
/// * `project_path` - Working directory where Claude command was executed
/// * `cli_output` - Standard output from Claude CLI
/// * `execution_start` - Timestamp when command execution started
///
/// # Returns
/// * `Some(PathBuf)` - Location of JSON log file if detected
/// * `None` - If log location could not be determined
```

### User Documentation

**Update `CLAUDE.md`**:
```markdown
## Claude JSON Logs

When executing Claude commands with verbose output (`-v` flag), Prodigy displays
the location of Claude's streaming JSON log:

```
📝 Claude JSON log: /Users/glen/.claude/projects/-Users-glen-prodigy/abc123.jsonl
```

These logs contain:
- Complete conversation history
- All tool invocations and parameters
- Token usage statistics
- Detailed error messages

### Viewing JSON Logs

To view a Claude JSON log:
```bash
cat /path/to/log.jsonl | jq .
```

To find specific tool invocations:
```bash
grep '"type":"tool_use"' /path/to/log.jsonl | jq .
```
```

**Update Troubleshooting Guide**:
```markdown
## Debugging Failed Claude Commands

When a Claude command fails during workflow execution:

1. **Check the error message** - includes JSON log location
2. **Open the JSON log** - contains full conversation and errors
3. **Search for errors** - `grep error /path/to/log.jsonl`
4. **Review tool invocations** - see what tools Claude attempted
5. **Check token usage** - verify not hitting limits
```

## Implementation Notes

### Development Sequence

**Phase 1: Core Detection** (1-2 hours)
1. Implement log location detection strategies
2. Add metadata support to `ExecutionResult`
3. Integrate detection into streaming execution
4. Add unit tests for detection logic

**Phase 2: Display Integration** (1 hour)
5. Add console output formatting
6. Update error messages to include log location
7. Add verbosity-based display logic

**Phase 3: MapReduce Integration** (1-2 hours)
8. Update MapReduce events with log location
9. Add log location to DLQ entries
10. Update event logging and display

**Phase 4: Testing & Documentation** (1-2 hours)
11. Add integration tests
12. Update user documentation
13. Manual testing across workflows

### Performance Considerations

**Log Detection Overhead**:
- Parsing output: ~1ms (negligible)
- File search: 10-50ms depending on directory size
- Total overhead: <100ms per command

**Optimization**:
- Use bounded file search (max depth, recent files only)
- Cache home directory path
- Skip detection for non-streaming executions

### Common Pitfalls

**Pitfall 1**: Race condition with file creation
- **Risk**: Log file may not exist immediately after command completes
- **Mitigation**: Small retry delay (100ms) before file search

**Pitfall 2**: Incorrect path sanitization
- **Risk**: Generated path doesn't match Claude's sanitization
- **Mitigation**: Test with various project paths, document edge cases

**Pitfall 3**: Excessive console output
- **Risk**: Log locations clutter output at default verbosity
- **Mitigation**: Only display with `-v` or higher verbosity

## Migration and Compatibility

### Breaking Changes
None - this is additive functionality

### Backward Compatibility
- Existing workflows continue to function unchanged
- Log location is optional metadata, not required
- Feature activates automatically when verbosity >= 1

### Rollout Plan
1. Implement log detection with graceful fallback
2. Test with existing workflows
3. Enable by default in new releases
4. Document feature in release notes

## Success Metrics

**Functionality**:
- Log location detected in >95% of streaming executions
- Detection overhead <100ms average
- Zero false positives (wrong log file detected)

**Usability**:
- Users can find logs for debugging within 30 seconds
- Reduced time-to-resolution for Claude command failures
- Positive feedback on observability improvement

**Reliability**:
- Graceful fallback when detection fails
- No workflow failures due to log detection
- Works across different environments (macOS, Linux, custom home dirs)
