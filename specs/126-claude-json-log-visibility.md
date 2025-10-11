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

Prodigy captures Claude's JSON log files for every command execution (per Spec 121), storing complete conversation history, tool invocations, and token usage in `~/.local/state/claude/logs/session-{id}.json`. However, when workflow failures occur (especially in MapReduce agents), users must manually search for these logs or enable verbose mode (`-v`) to see the log path.

Currently, the JSON log location is:
- Shown in verbose output (`-v` flag)
- Stored in `AgentResult.json_log_location`
- Included in `MapReduceEvent::AgentCompleted`
- Preserved in DLQ `FailureDetail.json_log_location`

But when a workflow fails with an error like:
```
❌ Session failed: Setup phase failed: Step 'claude: /prodigy-analyze-features-for-book'
   has commit_required=true but no commits were created
```

The user has no easy way to find the Claude JSON log to understand what Claude actually did during execution.

## Objective

Make Claude JSON log file paths immediately visible and accessible when workflows fail or complete, without requiring verbose mode, enabling faster debugging of Claude command failures.

## Requirements

### Functional Requirements

1. **Error Output Enhancement**
   - When a Claude command fails, automatically display the JSON log path in the error message
   - Include the log path even without `-v` flag
   - Show the path prominently so it's easy to copy

2. **Success Output Enhancement**
   - For successful Claude commands in workflows, optionally show log path (configurable)
   - Always show log path for MapReduce agent failures in DLQ output

3. **Command-Line Helper**
   - Add `prodigy logs` command to easily access recent Claude JSON logs
   - Support filtering by session ID, workflow name, or time range
   - Provide options to view, tail, or analyze logs

4. **Integration with Existing Tools**
   - `prodigy dlq show` should display JSON log paths for failed items
   - `prodigy events` should include JSON log references
   - Error messages should include the log path inline

### Non-Functional Requirements

- **Performance**: Log path lookup and display should add <10ms to error reporting
- **Usability**: Path should be easily selectable and copy-paste friendly
- **Consistency**: Use same format across all output types (errors, DLQ, events)
- **Compatibility**: Work with existing `-v` flag behavior (no regression)

## Acceptance Criteria

- [ ] When a Claude command fails in setup/map/reduce phase, error message includes JSON log path
- [ ] Error output format is consistent: `📋 Claude log: /path/to/session-xyz.json`
- [ ] `prodigy dlq show <job_id>` displays JSON log path for each failed item
- [ ] New command `prodigy logs [session_id]` lists and opens Claude JSON logs
- [ ] `prodigy logs --latest` shows the most recent Claude JSON log
- [ ] JSON log path is shown even without `-v` flag (default verbosity = 0)
- [ ] Existing `-v` behavior unchanged (streaming output still shows log path)
- [ ] Integration tests verify log path visibility in error scenarios
- [ ] Documentation updated with examples of using log paths for debugging

## Technical Details

### Implementation Approach

**1. Error Message Enhancement**

Modify error formatting in workflow executor to include JSON log path:

```rust
// In workflow executor error handling
if let Some(log_path) = result.json_log_location() {
    eprintln!("❌ Session failed: {}", error_message);
    eprintln!("📋 Claude log: {}", log_path);
    eprintln!("   View full conversation: cat {}", log_path);
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
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
        .collect();

    entries.sort_by_key(|e| e.metadata().unwrap().modified().unwrap());
    entries.last()
        .map(|e| e.path())
        .ok_or_else(|| anyhow!("No Claude logs found"))
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
📋 Claude log: /Users/user/.local/state/claude/logs/session-abc123.json
   View: cat /Users/user/.local/state/claude/logs/session-abc123.json
   Messages: 15 | Tokens: 12,450 | Duration: 45s
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

When a Claude command fails, Prodigy automatically shows the JSON log location:

```
❌ Session failed: Setup phase failed
📋 Claude log: /Users/user/.local/state/claude/logs/session-abc123.json
   View: cat /Users/user/.local/state/claude/logs/session-abc123.json
```

You can view the complete conversation, tool invocations, and token usage:

```bash
# View the log file
cat /Users/user/.local/state/claude/logs/session-abc123.json | jq

# View just the messages
cat /Users/user/.local/state/claude/logs/session-abc123.json | jq '.messages'

# View token usage
cat /Users/user/.local/state/claude/logs/session-abc123.json | jq '.usage'
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
