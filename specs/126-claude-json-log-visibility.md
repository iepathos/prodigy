---
number: 126
title: Claude JSON Log Path Visibility in Workflow Output
category: observability
priority: high
status: draft
dependencies: [121]
created: 2025-10-11
---

# Specification 126: Claude JSON Log Path Visibility in Workflow Output

**Category**: observability
**Priority**: high
**Status**: draft
**Dependencies**: Spec 121 (Claude Command Observability)

## Context

Prodigy captures Claude's JSON log files for every command execution in streaming JSONL format (one JSON object per line) at:
```
~/.claude/projects/{worktree-path}/{session-uuid}.jsonl
```

Each Claude command gets its own log file containing complete conversation history, tool invocations, and token usage.

### Current Implementation Issues

**Issue 1: Log Path Buried in INFO Logs**
```
2025-10-11T15:34:15.796803Z  INFO Claude JSON log saved to: /Users/glen/.claude/projects/...
```
The log path is shown at INFO level and easily missed, especially when multiple commands run.

**Issue 2: Only First Log Shown**
When multiple Claude commands run (e.g., `/prodigy-implement-spec` then `/prodigy-validate-spec` then `/prodigy-complete-spec`), only the first log path is displayed, making it hard to find logs for later commands.

**Issue 3: Not Shown on Error**
When a Claude command fails with:
```
❌ Session failed: Setup phase failed: Step 'claude: /prodigy-analyze-features-for-book'
   has commit_required=true but no commits were created
```
The user has NO indication of where the Claude JSON log is located.

**Issue 4: Path Confusion**
The actual path is `~/.claude/projects/{worktree-path}/{uuid}.jsonl` but users might expect it in `~/.local/state/claude/logs/` based on other tools.

**Issue 5: Streaming Not Obvious**
The `.jsonl` format supports streaming (append-only, one JSON per line) but it's not clear to users that they can `tail -f` the file during execution.

## Objective

Make Claude JSON log file paths immediately visible and accessible when workflows fail or complete, without requiring verbose mode, enabling faster debugging of Claude command failures.

## Requirements

### Functional Requirements

1. **Per-Command Log Display**
   - Display Claude JSON log path for EVERY Claude command execution
   - Show path at user-facing level (not buried in INFO logs)
   - Format: `📋 Claude log: /path/to/{uuid}.jsonl`
   - Display immediately when command starts (so users can tail it)

2. **Error Output Enhancement**
   - When a Claude command fails, prominently display the JSON log path in the error message
   - Include the log path even without `-v` flag
   - Show the path above the error message so it's easy to find
   - Include hint about streaming: "tail -f" for live viewing

3. **Success Output Enhancement**
   - After each Claude command completes, show summary with log path
   - Format: `✅ Command completed | Log: /path/to/{uuid}.jsonl`
   - Always show log path for MapReduce agent failures in DLQ output

4. **Streaming Support**
   - Make it obvious that logs are streaming (append-only JSONL)
   - Suggest `tail -f` command for live viewing during long-running commands
   - Show when log file is being written vs. completed

5. **Command-Line Helper**
   - Add `prodigy logs` command to easily access recent Claude JSON logs
   - Support filtering by session ID, workflow name, or time range
   - Provide options to view, tail, or analyze logs
   - Handle both `.jsonl` (streaming) and `.json` (legacy) formats

6. **Integration with Existing Tools**
   - `prodigy dlq show` should display JSON log paths for failed items
   - `prodigy events` should include JSON log references
   - Error messages should include the log path inline
   - Store log path in `ExecutionResult.json_log_location` for tracking

### Non-Functional Requirements

- **Performance**: Log path lookup and display should add <10ms to error reporting
- **Usability**: Path should be easily selectable and copy-paste friendly
- **Consistency**: Use same format across all output types (errors, DLQ, events)
- **Compatibility**: Work with existing `-v` flag behavior (no regression)

## Acceptance Criteria

- [ ] EVERY Claude command displays its log path when it starts: `🔄 Executing: claude /command`
      followed by `📋 Claude log: /path/to/{uuid}.jsonl (streaming - use 'tail -f' to watch)`
- [ ] When a Claude command fails, error includes log path above the error message
- [ ] When a Claude command succeeds, completion shows log path: `✅ Completed | Log: /path/to/{uuid}.jsonl`
- [ ] Multiple Claude commands in same workflow each show their own unique log paths
- [ ] `prodigy dlq show <job_id>` displays JSON log path for each failed item
- [ ] New command `prodigy logs [session_id]` lists and opens Claude JSON logs
- [ ] `prodigy logs --latest` shows the most recent Claude JSON log
- [ ] `prodigy logs` command works with both `.jsonl` and `.json` formats
- [ ] JSON log paths shown even without `-v` flag (default verbosity = 0)
- [ ] Streaming hint (`tail -f`) shown for in-progress commands
- [ ] Log path stored in `ExecutionResult.json_log_location` for all commands
- [ ] Integration tests verify each command gets its own log and path is displayed
- [ ] Documentation updated with examples of using log paths and streaming

## Technical Details

### Implementation Approach

**1. Per-Command Log Path Display**

Display log path for EVERY Claude command execution:

```rust
// In Claude command executor, RIGHT BEFORE executing
pub async fn execute_claude_command(
    command: &str,
    context: &ExecutionContext,
) -> Result<ExecutionResult> {
    // Generate or get log file path
    let log_path = get_claude_log_path(context)?;

    // Show log path IMMEDIATELY (so user can tail it)
    println!("📋 Claude log: {} (streaming - use 'tail -f' to watch)",
             log_path.display());

    // Execute command
    let result = execute_command_internal(command, &log_path).await?;

    // Show completion with log path
    if result.success {
        println!("✅ Completed | Log: {}", log_path.display());
    } else {
        eprintln!("❌ Failed | Log: {}", log_path.display());
    }

    // Store log path in result for later reference
    Ok(ExecutionResult {
        json_log_location: Some(log_path.to_string_lossy().to_string()),
        ..result
    })
}
```

**2. Error Message Enhancement**

Modify error formatting to include JSON log path ABOVE the error:

```rust
// In workflow executor error handling
if let Some(log_path) = result.json_log_location() {
    eprintln!("\n📋 Claude log: {}", log_path);
    eprintln!("   View: tail -f {} (or cat for completed log)\n", log_path);
    eprintln!("❌ Session failed: {}", error_message);
}
```

**2. Claude Command Result Tracking**

Ensure `ExecutionResult` always captures JSON log location:

```rust
pub struct ExecutionResult {
    pub success: bool,
    pub output: Option<String>,
    pub exit_code: Option<i32>,
    pub json_log_location: Option<String>, // Already exists per Spec 121
    // ... other fields
}
```

**3. DLQ Output Enhancement**

Update `prodigy dlq show` to display JSON log paths:

```rust
// In DLQ display code
for item in dlq_items {
    println!("Item ID: {}", item.item_id);
    println!("Failures: {}", item.failure_count);

    for (i, failure) in item.failure_history.iter().enumerate() {
        println!("  Attempt {}: {}", i + 1, failure.error);
        if let Some(log_path) = &failure.json_log_location {
            println!("  📋 Claude log: {}", log_path);
        }
    }
}
```

**4. New `prodigy logs` Command**

Add new CLI command for log management:

```rust
#[derive(clap::Subcommand)]
enum Commands {
    // ... existing commands

    /// Manage and view Claude JSON logs
    Logs {
        /// Session ID to view logs for
        session_id: Option<String>,

        /// Show only the latest log
        #[arg(long)]
        latest: bool,

        /// Open log in editor
        #[arg(long)]
        open: bool,

        /// Tail the log file
        #[arg(long)]
        tail: bool,
    },
}
```

Implementation:

```rust
async fn handle_logs_command(
    session_id: Option<String>,
    latest: bool,
    open: bool,
    tail: bool,
) -> Result<()> {
    let log_dir = dirs::state_dir()
        .ok_or_else(|| anyhow!("Could not determine state directory"))?
        .join("claude/logs");

    if latest {
        // Find most recent log file
        let latest_log = find_latest_log(&log_dir)?;
        if open {
            open_in_editor(&latest_log)?;
        } else if tail {
            tail_file(&latest_log)?;
        } else {
            println!("Latest Claude log: {}", latest_log.display());
            display_log_summary(&latest_log)?;
        }
        return Ok(());
    }

    if let Some(sid) = session_id {
        // Find log for specific session
        let log_file = log_dir.join(format!("session-{}.json", sid));
        if log_file.exists() {
            display_or_open_log(&log_file, open, tail)?;
        } else {
            eprintln!("❌ No log found for session: {}", sid);
        }
    } else {
        // List all recent logs
        list_recent_logs(&log_dir)?;
    }

    Ok(())
}

fn find_latest_log(log_dir: &Path) -> Result<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(log_dir)?
        .filter_map(Result::ok)
        .filter(|e| {
            let path = e.path();
            path.extension().map_or(false, |ext| ext == "json" || ext == "jsonl")
        })
        .collect();

    entries.sort_by_key(|e| e.metadata().unwrap().modified().unwrap());
    entries.last()
        .map(|e| e.path())
        .ok_or_else(|| anyhow!("No Claude logs found"))
}

fn display_log_summary(log_path: &Path) -> Result<()> {
    let is_jsonl = log_path.extension().map_or(false, |ext| ext == "jsonl");

    if is_jsonl {
        // JSONL format: one JSON object per line
        let file = File::open(log_path)?;
        let reader = BufReader::new(file);

        let mut message_count = 0;
        let mut tool_use_count = 0;
        let mut total_tokens = None;

        for line in reader.lines() {
            let line = line?;
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(msg_type) = obj.get("type").and_then(|v| v.as_str()) {
                    if msg_type == "user" || msg_type == "assistant" {
                        message_count += 1;
                    }
                }

                if let Some(usage) = obj.get("usage") {
                    if let Some(total) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
                        total_tokens = Some(total);
                    }
                }

                // Count tool uses
                if let Some(content) = obj.get("content").and_then(|v| v.as_array()) {
                    for item in content {
                        if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            tool_use_count += 1;
                        }
                    }
                }
            }
        }

        println!("Format: JSONL (streaming)");
        println!("Messages: {}", message_count);
        println!("Tool uses: {}", tool_use_count);
        if let Some(tokens) = total_tokens {
            println!("Tokens: {}", tokens);
        }
    } else {
        // JSON format (legacy): single JSON object
        let content = fs::read_to_string(log_path)?;
        let log: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(messages) = log.get("messages").and_then(|v| v.as_array()) {
            println!("Format: JSON (legacy)");
            println!("Messages: {}", messages.len());
        }

        if let Some(usage) = log.get("usage").and_then(|v| v.as_object()) {
            if let Some(total) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
                println!("Tokens: {}", total);
            }
        }
    }

    Ok(())
}
```

### Architecture Changes

**Error Reporting Flow**:
```
Claude Command Execution
    ↓
ExecutionResult (with json_log_location)
    ↓
Error/Success Handler
    ↓
Format Output (include log path)
    ↓
Display to User
```

**Log Discovery Flow**:
```
User runs: prodigy logs --latest
    ↓
Scan ~/.local/state/claude/logs/
    ↓
Find most recent session-*.json
    ↓
Display path + summary
```

### Data Structures

```rust
/// Enhanced error context with log reference
pub struct WorkflowError {
    pub message: String,
    pub phase: WorkflowPhase, // Setup, Map, Reduce, Merge
    pub command: String,
    pub json_log_location: Option<String>,
    pub session_id: String,
}

impl Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "❌ {} failed: {}", self.phase, self.message)?;
        if let Some(log_path) = &self.json_log_location {
            write!(f, "\n📋 Claude log: {}", log_path)?;
            write!(f, "\n   View: cat {}", log_path)?;
        }
        Ok(())
    }
}
```

### APIs and Interfaces

**New CLI Commands**:
```bash
# View latest Claude log
prodigy logs --latest

# View log for specific session
prodigy logs session-abc123

# Open log in editor
prodigy logs --latest --open

# Tail log file
prodigy logs session-abc123 --tail

# List recent logs
prodigy logs
```

**Output Format**:

```
# During execution (immediately when command starts)
🔄 Executing step 1/3: claude: /prodigy-implement-spec $ARG
📋 Claude log: ~/.claude/projects/.../6ded63ac-dd89-4259-b275-a42174ac58f4.jsonl
   (streaming - use 'tail -f' to watch live)

# On successful completion
✅ Completed | Log: ~/.claude/projects/.../6ded63ac-dd89-4259-b275-a42174ac58f4.jsonl

# On failure
❌ Failed | Log: ~/.claude/projects/.../6ded63ac-dd89-4259-b275-a42174ac58f4.jsonl

📋 Claude log: ~/.claude/projects/.../6ded63ac-dd89-4259-b275-a42174ac58f4.jsonl
   View: tail -f ~/.claude/projects/.../6ded63ac-dd89-4259-b275-a42174ac58f4.jsonl
   Or:   cat ~/.claude/projects/.../6ded63ac-dd89-4259-b275-a42174ac58f4.jsonl | jq

❌ Session failed: Setup phase failed: Step 'claude: /prodigy-analyze-features-for-book'
   has commit_required=true but no commits were created

# Example with multiple commands
🔄 Executing: claude: /prodigy-implement-spec 127
📋 Claude log: ~/.claude/projects/.../6ded63ac.jsonl (streaming)
✅ Completed | Log: ~/.claude/projects/.../6ded63ac.jsonl

🔄 Executing: claude: /prodigy-validate-spec 127
📋 Claude log: ~/.claude/projects/.../d24bfe47.jsonl (streaming)
✅ Completed | Log: ~/.claude/projects/.../d24bfe47.jsonl

🔄 Executing: claude: /prodigy-complete-spec 127
📋 Claude log: ~/.claude/projects/.../f7899b21.jsonl (streaming)
✅ Completed | Log: ~/.claude/projects/.../f7899b21.jsonl
```

## Dependencies

- **Prerequisites**: Spec 121 (Claude Command Observability) - provides `json_log_location` field
- **Affected Components**:
  - Workflow executor (error handling)
  - DLQ display code
  - CLI command parser
  - Event logging system
- **External Dependencies**:
  - `dirs` crate for finding state directory
  - Existing `clap` for CLI parsing

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_error_message_includes_log_path() {
    let error = WorkflowError {
        message: "Command failed".to_string(),
        phase: WorkflowPhase::Setup,
        command: "claude: /test".to_string(),
        json_log_location: Some("/path/to/log.json".to_string()),
        session_id: "test-session".to_string(),
    };

    let output = format!("{}", error);
    assert!(output.contains("📋 Claude log: /path/to/log.json"));
    assert!(output.contains("View: cat /path/to/log.json"));
}

#[test]
fn test_find_latest_log() {
    let temp_dir = create_temp_log_dir_with_files();
    let latest = find_latest_log(&temp_dir).unwrap();
    assert!(latest.to_string_lossy().contains("session-"));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_workflow_error_shows_log_path() {
    let workflow = create_failing_workflow();
    let result = execute_workflow(&workflow).await;

    assert!(result.is_err());
    let error_output = capture_stderr();
    assert!(error_output.contains("📋 Claude log:"));
    assert!(error_output.contains("session-"));
}

#[tokio::test]
async fn test_dlq_show_includes_log_paths() {
    let job_id = create_failed_job_with_logs().await;
    let output = run_command(&["dlq", "show", &job_id]).await;

    assert!(output.contains("📋 Claude log:"));
    assert!(output.contains(".local/state/claude/logs/"));
}
```

### Performance Tests

```rust
#[test]
fn test_log_path_lookup_performance() {
    let start = Instant::now();
    let _ = find_latest_log(&log_dir);
    let duration = start.elapsed();

    assert!(duration < Duration::from_millis(10));
}
```

### User Acceptance

- **Scenario 1**: User runs workflow that fails, immediately sees Claude log path in error
- **Scenario 2**: User runs `prodigy logs --latest` and can quickly access most recent Claude conversation
- **Scenario 3**: User checks DLQ and sees JSON log paths for each failure
- **Scenario 4**: User can copy-paste log path from terminal to view full Claude conversation

## Documentation Requirements

### Code Documentation

```rust
/// Display Claude JSON log location in user-friendly format
///
/// Formats the log path with an emoji indicator and a helpful
/// `cat` command for easy viewing. This is shown even without
/// verbose mode to aid debugging.
///
/// # Example Output
/// ```text
/// 📋 Claude log: /Users/user/.local/state/claude/logs/session-abc.json
///    View: cat /Users/user/.local/state/claude/logs/session-abc.json
/// ```
pub fn display_claude_log_location(log_path: &str) {
    eprintln!("📋 Claude log: {}", log_path);
    eprintln!("   View: cat {}", log_path);
}
```

### User Documentation

**Update CLAUDE.md**:

```markdown
## Troubleshooting

### Viewing Claude Execution Logs

**Every Claude command creates a streaming JSONL log file:**

```
🔄 Executing: claude /prodigy-implement-spec 127
📋 Claude log: ~/.claude/projects/.../6ded63ac.jsonl (streaming)
```

The log file is written in real-time (append-only JSONL format, one JSON object per line).

### Watching Logs Live

For long-running Claude commands, you can watch the log live:

```bash
# Watch live as Claude executes
tail -f ~/.claude/projects/.../6ded63ac.jsonl

# Pretty-print each line as it's added
tail -f ~/.claude/projects/.../6ded63ac.jsonl | jq -c '.'
```

### Analyzing Completed Logs

After a command completes, analyze the full conversation:

```bash
# View all events (one JSON object per line)
cat ~/.claude/projects/.../6ded63ac.jsonl

# Count messages by type
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -r '.type' | sort | uniq -c

# View only user messages
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -c 'select(.type == "user")'

# View only assistant responses
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -c 'select(.type == "assistant")'

# Extract all tool uses
cat ~/.claude/projects/.../6ded63ac.jsonl | \
  jq -c 'select(.type == "assistant") | .content[]? | select(.type == "tool_use")'

# View token usage (usually at end of file)
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -c 'select(.usage)'
```

### When Workflows Fail

When a Claude command fails, the log path is shown prominently:

```
📋 Claude log: ~/.claude/projects/.../6ded63ac.jsonl
   View: tail -f ~/.claude/projects/.../6ded63ac.jsonl
   Or:   cat ~/.claude/projects/.../6ded63ac.jsonl | jq

❌ Session failed: Setup phase failed: Step 'claude: /prodigy-analyze-features-for-book'
   has commit_required=true but no commits were created
```

### Using the `prodigy logs` Command

```bash
# View most recent Claude log
prodigy logs --latest

# View log for specific session
prodigy logs session-abc123

# Open in editor
prodigy logs --latest --open

# List recent logs
prodigy logs
```

### Debugging Failed MapReduce Agents

When map agents fail, check the DLQ for log references:

```bash
prodigy dlq show job-123
```

Output will include:
```
Item ID: item-5
Failures: 2
  Attempt 1: Command failed with exit code 1
  📋 Claude log: /Users/user/.local/state/claude/logs/session-agent-5.json
```
```

### Architecture Documentation

Update ARCHITECTURE.md with observability section:

```markdown
## Observability

### Claude JSON Logs

Every Claude command execution creates a JSON log file in:
```
~/.local/state/claude/logs/session-{id}.json
```

These logs contain:
- Complete conversation history
- All tool invocations and results
- Token usage statistics
- Error details and stack traces

**Log Path Visibility** (Spec 126):
- Shown in error messages by default
- Included in DLQ failure details
- Accessible via `prodigy logs` command
- Available in event logs and agent results
```

## Implementation Notes

### Error Message Formatting

- Use `📋` emoji for visual distinction
- Keep path on separate line for easy selection
- Include `cat` command suggestion for quick access
- Ensure proper line wrapping for long paths

### Log File Rotation

Consider adding log rotation to prevent unlimited growth:
```rust
fn rotate_old_logs(log_dir: &Path, keep_days: u64) -> Result<()> {
    let cutoff = SystemTime::now() - Duration::from_secs(keep_days * 86400);

    for entry in fs::read_dir(log_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.modified()? < cutoff {
            fs::remove_file(entry.path())?;
        }
    }

    Ok(())
}
```

### Cross-Platform Considerations

- Use `dirs` crate for portable path resolution
- Test on macOS, Linux, and Windows
- Handle path separators correctly
- Ensure log directory creation is cross-platform

## Migration and Compatibility

### Backward Compatibility

- No breaking changes to existing APIs
- `-v` flag behavior unchanged
- Existing log storage location unchanged
- DLQ format extended (backward compatible)

### New Features

- New `prodigy logs` command (opt-in)
- Enhanced error output (automatic)
- DLQ output enhancement (automatic)

### Migration Steps

None required - this is purely additive functionality. Existing workflows will automatically benefit from enhanced error messages.
