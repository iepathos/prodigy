---
number: 59
title: Simplified CLI Interface for Common Operations
category: foundation
priority: critical
status: draft
dependencies: [58]
created: 2025-01-14
---

# Specification 59: Simplified CLI Interface for Common Operations

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [58 - MapReduce Command Output]

## Context

The whitepaper shows simple, intuitive CLI commands that don't exist in the current implementation:
- `prodigy run workflow.yml` - Run a workflow
- `prodigy exec "/refactor user.py" --retry 3` - Single command with retries
- `prodigy batch "*.py" --command "/add-types" --parallel 5` - Batch operations

Currently, users must use `prodigy cook` with complex workflow files for even simple operations. This high barrier to entry contradicts the whitepaper's vision of Prodigy as a practical, easy-to-use orchestration tool.

## Objective

Implement intuitive CLI commands that enable users to leverage Prodigy's power without writing YAML workflows for common operations.

## Requirements

### Functional Requirements
- `prodigy run <workflow.yml>` - Alias for cook with better semantics
- `prodigy exec <command>` - Execute single command with retry support
- `prodigy batch <pattern>` - Process multiple files in parallel
- `prodigy resume <workflow-id>` - Resume interrupted workflow
- Support common flags: `--retry`, `--parallel`, `--timeout`
- Generate temporary workflow files for exec/batch commands
- Provide meaningful progress output and error messages

### Non-Functional Requirements
- Commands must feel native and intuitive
- Minimal cognitive overhead for simple operations
- Performance equivalent to full workflow execution
- Clear, actionable error messages

## Acceptance Criteria

- [ ] `prodigy run workflow.yml` executes the workflow
- [ ] `prodigy exec "claude: /refactor app.py" --retry 3` works with retries
- [ ] `prodigy batch "*.py" --command "claude: /add-types" --parallel 5` processes files
- [ ] `prodigy resume workflow-123` resumes from checkpoint
- [ ] Shell commands work: `prodigy exec "shell: npm test"`
- [ ] Progress bars show for parallel operations
- [ ] Interrupted batch operations can be resumed
- [ ] Generated workflow files are cleaned up after execution
- [ ] Help text is clear and includes examples

## Technical Details

### Implementation Approach

1. **CLI Command Structure**:
   ```rust
   #[derive(Parser)]
   enum Commands {
       Run {
           workflow: PathBuf,
           #[arg(long)]
           verbose: bool,
       },
       Exec {
           command: String,
           #[arg(long, default_value = "1")]
           retry: u32,
           #[arg(long)]
           timeout: Option<u64>,
       },
       Batch {
           pattern: String,
           #[arg(long)]
           command: String,
           #[arg(long, default_value = "5")]
           parallel: usize,
           #[arg(long)]
           retry: Option<u32>,
       },
       Resume {
           workflow_id: String,
           #[arg(long)]
           force: bool,
       }
   }
   ```

2. **Workflow Generation for Simple Commands**:
   ```rust
   fn generate_exec_workflow(cmd: &str, retry: u32) -> WorkflowConfig {
       WorkflowConfig {
           name: format!("exec-{}", Uuid::new_v4()),
           tasks: vec![
               WorkflowStep {
                   name: Some("Execute command".into()),
                   command: parse_command(cmd),
                   retry: Some(retry),
                   ..Default::default()
               }
           ],
           ..Default::default()
       }
   }

   fn generate_batch_workflow(
       pattern: &str,
       command: &str,
       parallel: usize
   ) -> WorkflowConfig {
       WorkflowConfig {
           name: format!("batch-{}", Uuid::new_v4()),
           mode: "mapreduce".into(),
           map: Some(MapPhase {
               input: format!("find . -name '{}'", pattern),
               max_parallel: parallel,
               agent_template: vec![
                   WorkflowStep {
                       command: parse_command(command),
                       ..Default::default()
                   }
               ],
               ..Default::default()
           }),
           ..Default::default()
       }
   }
   ```

3. **Command Parsing**:
   ```rust
   fn parse_command(cmd: &str) -> CommandType {
       if cmd.starts_with("claude:") {
           CommandType::Claude(cmd[7..].trim().into())
       } else if cmd.starts_with("shell:") {
           CommandType::Shell(cmd[6..].trim().into())
       } else if cmd.starts_with("/") {
           CommandType::Claude(cmd.into())
       } else {
           CommandType::Shell(cmd.into())
       }
   }
   ```

### Architecture Changes
- Add new CLI command handlers in `src/cli/mod.rs`
- Create workflow generation module
- Implement command parsing logic
- Add progress tracking for batch operations

### Data Structures
```rust
// Temporary workflow storage
struct TemporaryWorkflow {
    path: PathBuf,
    config: WorkflowConfig,
}

impl Drop for TemporaryWorkflow {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
```

## Dependencies

- **Prerequisites**: [58 - MapReduce Command Output]
- **Affected Components**:
  - `src/cli/mod.rs` - New command handlers
  - `src/cook/workflow/` - Workflow generation
  - `src/main.rs` - CLI argument parsing
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Command parsing logic
  - Workflow generation from commands
  - Argument validation
- **Integration Tests**:
  - End-to-end exec command with retries
  - Batch processing with pattern matching
  - Resume functionality
- **User Acceptance Tests**:
  - Common workflows executable via simple commands
  - Error messages are helpful
  - Progress indication works correctly

## Documentation Requirements

- **Code Documentation**: Document command parsing and workflow generation
- **User Documentation**:
  - Quick start guide with simple examples
  - Command reference with all options
  - Migration from `cook` to new commands
- **Architecture Updates**: Document temporary workflow lifecycle

## Implementation Notes

- Consider aliasing `cook` to `run` for backwards compatibility
- Support both `claude:` and `/` prefix for Claude commands
- Implement smart defaults (retry=3, parallel=5, timeout=300s)
- Show generated workflow in verbose mode for learning

## Migration and Compatibility

- `prodigy cook` continues to work as before
- `prodigy run` is preferred alias going forward
- Documentation updated to use new commands
- Examples repository updated with new syntax