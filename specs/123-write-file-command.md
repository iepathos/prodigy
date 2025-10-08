---
number: 123
title: Built-in File Writing Command
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-10-07
---

# Specification 123: Built-in File Writing Command

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Currently, Prodigy workflows use shell commands with output redirection to write data to files, particularly in MapReduce reduce phases. For example:

```yaml
reduce:
  - shell: "echo '${map.results}' > .prodigy/map-results.json"
```

This approach has several problems:

1. **Verbose Logging**: The entire content of `${map.results}` (which can be thousands of characters of JSON) is logged at INFO level when executing the shell command, cluttering logs
2. **Platform Dependency**: Relies on shell redirection, which may behave differently across platforms (Unix vs Windows)
3. **No Validation**: No JSON/YAML validation before writing
4. **Poor Error Messages**: Shell errors are cryptic and hard to debug
5. **Inconsistent with Design**: Using shell commands for simple file I/O is less declarative than Prodigy's workflow design philosophy

**Example Verbose Output**:
```
2025-10-08T00:16:46.770027Z  INFO Executing shell command: echo '[{"item_id":"item_0","status":"Success","output":"Perfect! Now let me provide the summary output:\n\nCreated implementation plan at .prodigy/plan-item_0.md\n\n**Target**: ./src/cook/orchestrator/core.rs:DefaultCookOrchestrator:105\n**Priority**: 148.67\n**Phases**: 7\n\n**Plan Overview**:\n- Phase 1: Extract construction module (~120 lines)\n[... thousands more characters ...]
```

This verbose output makes it difficult to read logs and understand workflow progress.

## Objective

Add a built-in `write_file` command type to Prodigy workflows that provides clean, declarative file writing with proper validation, minimal logging, and cross-platform compatibility.

## Requirements

### Functional Requirements

1. **New Command Type**: Add `write_file` as a first-class workflow command alongside `shell`, `claude`, and `goal_seek`
2. **Content Interpolation**: Support variable interpolation in both path and content fields
3. **Format Support**: Support writing as plain text, JSON (with validation and pretty-printing), and YAML
4. **Directory Creation**: Optional automatic creation of parent directories
5. **File Permissions**: Optional file permission specification (Unix systems)
6. **Clean Logging**: Log file write operations concisely without including file content
7. **Validation**: Validate JSON/YAML syntax before writing
8. **Error Handling**: Clear, actionable error messages for write failures

### Non-Functional Requirements

1. **Performance**: File writing should not significantly impact workflow execution time
2. **Security**: Prevent path traversal attacks (no `..` in paths)
3. **Usability**: Simple, intuitive syntax that matches Prodigy's declarative design
4. **Maintainability**: Clean implementation that follows existing command patterns
5. **Backward Compatibility**: Existing `shell` commands continue to work

## Acceptance Criteria

- [ ] `write_file` command type is supported in workflow YAML files
- [ ] Content can be written as plain text, JSON, or YAML format
- [ ] Variable interpolation works in both `path` and `content` fields
- [ ] JSON format validates syntax and pretty-prints output
- [ ] YAML format validates syntax and formats output
- [ ] Parent directories are created when `create_dirs: true`
- [ ] File permissions can be set on Unix systems via `mode` field
- [ ] Path traversal (paths containing `..`) is rejected with clear error
- [ ] Logging shows file path, size, and format but NOT content
- [ ] Error messages clearly indicate write failures with actionable information
- [ ] All existing tests pass
- [ ] No clippy warnings
- [ ] Documentation includes usage examples
- [ ] Integration test demonstrates MapReduce use case

## Technical Details

### Implementation Approach

Add a new command execution path in the workflow executor that handles file writing operations with proper validation and formatting.

### Architecture Changes

**Files to Modify**:

1. **src/config/workflow.rs**
   - Add `WriteFileConfig` struct
   - Add `WriteFileFormat` enum
   - Add `write_file` field to `WorkflowStep`

2. **src/cook/workflow/executor/commands.rs**
   - Add `execute_write_file_command()` function
   - Implement JSON validation and pretty-printing
   - Implement YAML validation and formatting
   - Add path validation (no `..` traversal)
   - Add directory creation logic
   - Add Unix file permission setting

3. **src/cook/workflow/executor/step_executor.rs**
   - Add handling for `write_file` command type
   - Wire up to `execute_write_file_command()`

### Data Structures

```rust
/// Configuration for write_file command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WriteFileConfig {
    /// Path to write file (supports variable interpolation)
    pub path: String,

    /// Content to write (supports variable interpolation)
    pub content: String,

    /// Format to use when writing (default: text)
    #[serde(default)]
    pub format: WriteFileFormat,

    /// File permissions in octal format (default: "0644")
    #[serde(default = "default_file_mode")]
    pub mode: String,

    /// Create parent directories if they don't exist (default: false)
    #[serde(default)]
    pub create_dirs: bool,
}

/// File format for write_file command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WriteFileFormat {
    /// Plain text (no processing)
    #[default]
    Text,

    /// JSON with validation and pretty-printing
    Json,

    /// YAML with validation and formatting
    Yaml,
}

fn default_file_mode() -> String {
    "0644".to_string()
}
```

### APIs and Interfaces

**Workflow Syntax**:

```yaml
# Basic text file writing
- write_file:
    path: ".prodigy/output.txt"
    content: "${some.variable}"

# JSON with validation and pretty-printing
- write_file:
    path: ".prodigy/map-results.json"
    content: "${map.results}"
    format: json
    create_dirs: true

# YAML with custom permissions
- write_file:
    path: "config/settings.yml"
    content: "${settings}"
    format: yaml
    mode: "0600"
    create_dirs: true
```

**Executor Function Signature**:

```rust
pub async fn execute_write_file_command(
    config: &WriteFileConfig,
    working_dir: &Path,
) -> Result<StepResult>
```

**Logging Output**:

```
🔄 Reduce phase: Executing step 5/6
ℹ️ Wrote 15234 bytes to .prodigy/map-results.json (format: Json)
✅ Step completed successfully
```

### Implementation Details

**Path Validation**:
```rust
// Reject path traversal
if config.path.contains("..") {
    return Err(anyhow!("Invalid path: parent directory traversal not allowed"));
}
```

**JSON Formatting**:
```rust
// Validate and pretty-print JSON
let value: serde_json::Value = serde_json::from_str(&config.content)
    .context("Invalid JSON content")?;
let formatted = serde_json::to_string_pretty(&value)?;
```

**Directory Creation**:
```rust
if config.create_dirs {
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
}
```

**Unix Permissions**:
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let mode = u32::from_str_radix(&config.mode, 8)
        .context("Invalid file mode")?;
    let permissions = fs::Permissions::from_mode(mode);
    fs::set_permissions(&file_path, permissions)?;
}
```

## Dependencies

**Prerequisites**: None

**Affected Components**:
- Workflow configuration parser
- Workflow executor
- Step executor
- Command executor

**External Dependencies**: None (uses std::fs)

## Testing Strategy

### Unit Tests

**File**: `src/cook/workflow/executor/commands.rs`

```rust
#[cfg(test)]
mod write_file_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_text_file() {
        // Test basic text file writing
    }

    #[tokio::test]
    async fn test_write_json_file() {
        // Test JSON validation and pretty-printing
    }

    #[tokio::test]
    async fn test_write_yaml_file() {
        // Test YAML formatting
    }

    #[tokio::test]
    async fn test_create_directories() {
        // Test automatic directory creation
    }

    #[tokio::test]
    async fn test_reject_path_traversal() {
        // Test that "../foo" is rejected
    }

    #[tokio::test]
    async fn test_invalid_json_content() {
        // Test that invalid JSON returns clear error
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_file_permissions() {
        // Test file permission setting
    }
}
```

### Integration Tests

**File**: `tests/write_file_integration_test.rs`

Test complete workflow with `write_file` command:

```rust
#[tokio::test]
async fn test_mapreduce_write_results_file() {
    // Test MapReduce workflow that uses write_file in reduce phase
    // Verify that map.results is written to file correctly
    // Verify that Claude command can read the written file
}
```

### Manual Testing

1. Run `debtmap-reduce.yml` with updated `write_file` command
2. Verify logs are clean (no JSON dumped to console)
3. Verify `.prodigy/map-results.json` is created with valid JSON
4. Verify reduce phase Claude command can read the file
5. Test on Unix and Windows (if available)

## Documentation Requirements

### Code Documentation

- Add comprehensive doc comments to `WriteFileConfig`
- Add doc comments to `execute_write_file_command()`
- Document format validation behavior
- Document security restrictions (path traversal)

### User Documentation

**File**: `README.md` - Add to "Workflow Commands" section:

```markdown
#### write_file

Write content to a file with validation and formatting.

**Basic Usage**:
```yaml
- write_file:
    path: "output.txt"
    content: "${variable}"
```

**JSON Output**:
```yaml
- write_file:
    path: ".prodigy/results.json"
    content: "${map.results}"
    format: json
    create_dirs: true
```

**Options**:
- `path`: File path (required, supports variables)
- `content`: Content to write (required, supports variables)
- `format`: `text`, `json`, or `yaml` (default: `text`)
- `create_dirs`: Create parent directories (default: `false`)
- `mode`: Unix file permissions in octal (default: `"0644"`)
```

**File**: `workflows/debtmap-reduce.yml` - Update reduce phase:

```yaml
reduce:
  # Write map results to file (replaces verbose shell echo)
  - write_file:
      path: ".prodigy/map-results.json"
      content: "${map.results}"
      format: json
      create_dirs: true

  - claude: |
      /prodigy-compare-debt-results \
        --map-results-file .prodigy/map-results.json \
        ...
```

### Architecture Updates

No ARCHITECTURE.md updates required - this is a new command type following existing patterns.

## Implementation Notes

### Design Decisions

1. **Separate Config Struct**: Use dedicated `WriteFileConfig` instead of inline fields for clarity and reusability
2. **Default to Text**: Plain text format is default for simplicity
3. **Explicit create_dirs**: Require explicit opt-in for directory creation to avoid surprising behavior
4. **Unix-only Permissions**: File permissions only work on Unix; Windows has different permission model
5. **No Append Mode**: Initial implementation only supports overwrite; append can be added later if needed

### Gotchas

1. **Variable Interpolation Timing**: Interpolation happens before write_file execution, so all variables must be available
2. **Large Files**: No size limits initially; consider adding warnings for very large files in future
3. **Binary Data**: Only supports text content; binary file writing not supported
4. **Concurrent Writes**: No file locking; concurrent writes to same file may cause issues

### Best Practices

1. **Use JSON Format**: Always use `format: json` when writing structured data
2. **Create Directories**: Use `create_dirs: true` for files in subdirectories
3. **Secure Permissions**: Use `mode: "0600"` for sensitive data files
4. **Descriptive Paths**: Use clear file paths that indicate content (e.g., `.prodigy/map-results.json`)

## Migration and Compatibility

### Backward Compatibility

- All existing workflows continue to work unchanged
- `shell` commands with file redirection still supported
- No breaking changes to any APIs

### Migration Path

Users can gradually migrate from shell redirection to `write_file`:

**Before**:
```yaml
- shell: "echo '${map.results}' > .prodigy/map-results.json"
```

**After**:
```yaml
- write_file:
    path: ".prodigy/map-results.json"
    content: "${map.results}"
    format: json
```

### Deprecation Plan

No deprecation needed - both patterns can coexist indefinitely.

## Example Usage

### MapReduce Results Writing

```yaml
reduce:
  - write_file:
      path: ".prodigy/map-results.json"
      content: "${map.results}"
      format: json
      create_dirs: true
```

### Configuration File Generation

```yaml
setup:
  - write_file:
      path: "config/workflow-config.yml"
      content: |
        version: 2
        features:
          - ${feature1}
          - ${feature2}
      format: yaml
      create_dirs: true
```

### Report Generation

```yaml
reduce:
  - write_file:
      path: "reports/summary-${timestamp}.txt"
      content: |
        Workflow: ${workflow.name}
        Completed: ${workflow.completed}
        Results: ${map.successful}/${map.total} successful
      create_dirs: true
```

## Success Metrics

1. **Log Cleanliness**: Reduce phase logs in `debtmap-reduce.yml` should be <50 lines (vs current >2000 lines)
2. **User Adoption**: 50% of workflows using shell redirection migrate within 3 months
3. **Error Reduction**: File writing errors decrease due to better validation
4. **Developer Satisfaction**: Positive feedback from workflow authors on improved syntax

## Future Enhancements

Potential future extensions (not in scope for this spec):

1. **Append Mode**: `mode: append` to add to existing files
2. **Binary Support**: Write binary data (base64 encoded)
3. **Size Limits**: Warnings or errors for very large files
4. **Compression**: Automatic gzip compression for large outputs
5. **Templates**: Jinja2-style templates for complex file generation
6. **Atomic Writes**: Write to temp file then rename for atomicity
