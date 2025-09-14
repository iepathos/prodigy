---
number: 58
title: MapReduce Command Output as Input Source
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 58: MapReduce Command Output as Input Source

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The whitepaper specifies two critical input modes for MapReduce that are not fully implemented:
1. Command output: `input: "find . -name '*.py'"` - Execute a command and use its output as work items
2. JSON file: `input: "work-items.json"` with `json_path: "$.items[*]"` - Parse structured JSON data

Currently, Prodigy only supports JSON file input. The lack of command output support severely limits the tool's utility for common use cases like processing all files matching a pattern.

## Objective

Enable MapReduce workflows to accept both command output and JSON files as input sources, with automatic detection of input type and proper parsing for work item distribution.

## Requirements

### Functional Requirements
- Support command execution with output captured as line-separated work items
- Maintain existing JSON file input with JSONPath extraction
- Auto-detect input type (command vs file path)
- Support shell pipeline commands (e.g., `find . -name '*.py' | grep -v test`)
- Provide clear error messages for command failures
- Support both absolute and relative paths for file inputs

### Non-Functional Requirements
- Command execution must be secure (no arbitrary code injection)
- Support large command outputs (>10,000 items) efficiently
- Maintain backwards compatibility with existing JSON-only workflows
- Command timeout configuration to prevent hanging

## Acceptance Criteria

- [ ] MapReduce accepts `input: "find . -name '*.py'"` and processes each line as work item
- [ ] MapReduce accepts `input: "ls *.js"` and processes matching files
- [ ] Complex pipelines work: `input: "grep -r 'TODO' . | cut -d: -f1 | sort -u"`
- [ ] JSON file input continues to work with JSONPath
- [ ] Command failures provide clear error messages
- [ ] Empty command output handled gracefully
- [ ] Command timeout prevents infinite hangs
- [ ] Variable interpolation works in commands: `input: "find ${dir} -name '*.py'"`
- [ ] Integration tests cover both input modes

## Technical Details

### Implementation Approach

1. **Input Type Detection**:
   ```rust
   enum InputSource {
       Command(String),      // Shell command to execute
       JsonFile(PathBuf),   // Path to JSON file
   }

   fn detect_input_type(input: &str) -> InputSource {
       let path = Path::new(input);
       if path.exists() && path.extension() == Some("json") {
           InputSource::JsonFile(path.to_path_buf())
       } else {
           InputSource::Command(input.to_string())
       }
   }
   ```

2. **Command Execution**:
   ```rust
   async fn execute_command_input(cmd: &str, timeout: Duration) -> Result<Vec<Value>> {
       let output = tokio::time::timeout(
           timeout,
           Command::new("sh")
               .arg("-c")
               .arg(cmd)
               .output()
       ).await??;

       let items = String::from_utf8(output.stdout)?
           .lines()
           .filter(|line| !line.is_empty())
           .map(|line| json!({"item": line.trim()}))
           .collect();

       Ok(items)
   }
   ```

3. **Unified Work Item Processing**:
   ```rust
   async fn load_work_items(config: &MapPhase) -> Result<Vec<Value>> {
       match detect_input_type(&config.input) {
           InputSource::Command(cmd) => {
               execute_command_input(&cmd, config.timeout).await
           }
           InputSource::JsonFile(path) => {
               load_json_work_items(&path, &config.json_path).await
           }
       }
   }
   ```

### Architecture Changes
- Modify `MapPhase` to handle both input types
- Add command execution with timeout support
- Implement secure command execution wrapper
- Update configuration parser to accept string input

### Data Structures
```yaml
# Command input example
map:
  input: "find src -name '*.rs' -type f"
  agent_template:
    commands:
      - claude: "/analyze ${item}"

# JSON input example (existing)
map:
  input: "analysis.json"
  json_path: "$.files[*]"
  agent_template:
    commands:
      - claude: "/process ${item.path}"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/config/mapreduce.rs` - Configuration parsing
  - `src/cook/execution/mapreduce.rs` - Execution logic
  - `src/cook/execution/data_pipeline.rs` - Data loading
- **External Dependencies**: None (uses existing subprocess module)

## Testing Strategy

- **Unit Tests**:
  - Input type detection logic
  - Command parsing and execution
  - Error handling for failed commands
- **Integration Tests**:
  - End-to-end MapReduce with command input
  - Pipeline commands with multiple stages
  - Large output handling (10,000+ items)
- **Performance Tests**:
  - Command execution timeout behavior
  - Memory usage with large outputs
- **Security Tests**:
  - Command injection prevention
  - Path traversal protection

## Documentation Requirements

- **Code Documentation**: Document input format detection algorithm
- **User Documentation**:
  - Examples of both input modes in README
  - Migration guide from JSON-only to command input
- **Architecture Updates**: Update ARCHITECTURE.md with input processing flow

## Implementation Notes

- Use existing `SubprocessManager` for secure command execution
- Consider caching command output for resume scenarios
- Implement progress tracking for command execution phase
- Support dry-run mode to preview work items before execution

## Migration and Compatibility

- All existing JSON-based workflows continue to work unchanged
- New workflows can use either input mode
- Future: Support stdin input for piping from external tools