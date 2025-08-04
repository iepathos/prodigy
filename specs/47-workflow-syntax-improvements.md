---
number: 47
title: Workflow Syntax Improvements
category: foundation
priority: high
status: draft
dependencies: [46]
created: 2025-08-04
---

# Specification 47: Workflow Syntax Improvements

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [46 - Self-Healing Analysis Errors]

## Context

The current workflow syntax uses a `name:` field that conflates two different concepts:
1. Claude CLI commands (e.g., `mmm-implement-spec`)
2. What appear to be arbitrary step names

This creates confusion about what actually executes. Users can't tell if `name: run-tests` will run a Claude command called `mmm-run-tests` or if it's just a label. Additionally, there's no way to run shell commands directly, limiting workflow flexibility.

The current approach also makes it awkward to pass arguments - we have a separate `args:` field but it's not clear how variables like `$ARG` are interpolated.

## Objective

Redesign the workflow syntax to clearly distinguish between Claude commands and shell commands, simplify argument passing, and enable more flexible workflow automation including test execution and conditional logic.

## Requirements

### Functional Requirements
- Support explicit `claude:` field for Claude CLI commands
- Support explicit `shell:` field for shell commands
- Allow inline variable interpolation in command strings
- Remove ambiguous `name:` field requirement
- Support conditional execution based on exit codes
- Capture command output for use in subsequent steps
- Maintain backward compatibility with existing workflows

### Non-Functional Requirements
- Syntax should be intuitive and follow established patterns (like Ansible)
- Clear error messages when invalid syntax is used
- Minimal performance impact from new parsing logic
- Documentation must clearly explain the new syntax

## Acceptance Criteria

- [ ] Workflows can use `claude: "/mmm-implement-spec $ARG"` syntax
- [ ] Workflows can use `shell: "cargo test --lib"` syntax
- [ ] Variables like `$ARG` and `$CAPTURED_OUTPUT` are properly interpolated
- [ ] Old `name:` syntax still works for backward compatibility
- [ ] Conditional steps (`on_failure`, `on_success`) work with both command types
- [ ] Command output can be captured and passed to subsequent steps
- [ ] Clear error when both `claude:` and `shell:` are specified
- [ ] Existing workflows continue to function without modification

## Technical Details

### Implementation Approach

1. **Extended WorkflowStep Structure**
   ```rust
   pub enum CommandType {
       Claude(String),      // Claude CLI command with args
       Shell(String),       // Shell command to execute
       Legacy(String),      // Old name-based approach
   }
   
   pub struct WorkflowStep {
       // Command specification (one of these)
       pub claude: Option<String>,     // e.g., "/mmm-implement-spec $ARG"
       pub shell: Option<String>,      // e.g., "cargo test --lib"
       pub name: Option<String>,       // Legacy support
       
       // Execution options
       pub capture_output: bool,
       pub timeout: Option<u64>,
       pub working_dir: Option<PathBuf>,
       
       // Environment and variables
       pub env: HashMap<String, String>,
       
       // Conditional execution
       pub on_failure: Option<Box<WorkflowStep>>,
       pub on_success: Option<Box<WorkflowStep>>,
       pub on_exit_code: HashMap<i32, Box<WorkflowStep>>,
       
       // Git requirements
       pub commit_required: bool,
       
       // Analysis configuration
       pub analysis: Option<WorkflowAnalysisConfig>,
   }
   ```

2. **Variable Interpolation**
   ```rust
   pub struct WorkflowContext {
       pub variables: HashMap<String, String>,
       pub captured_outputs: HashMap<String, String>,
       pub iteration_vars: HashMap<String, String>,
   }
   
   impl WorkflowContext {
       pub fn interpolate(&self, template: &str) -> String {
           // Replace $VAR with actual values
           // Support ${VAR} syntax for clarity
           // Handle $CAPTURED_OUTPUT from previous steps
       }
   }
   ```

3. **Command Execution Logic**
   ```rust
   impl WorkflowExecutor {
       async fn execute_step(&self, step: &WorkflowStep, ctx: &mut WorkflowContext) -> Result<StepResult> {
           let command_type = self.determine_command_type(step)?;
           
           let result = match command_type {
               CommandType::Claude(cmd) => {
                   let interpolated = ctx.interpolate(&cmd);
                   self.execute_claude_command(&interpolated).await?
               }
               CommandType::Shell(cmd) => {
                   let interpolated = ctx.interpolate(&cmd);
                   self.execute_shell_command(&interpolated).await?
               }
               CommandType::Legacy(name) => {
                   // Existing behavior for backward compatibility
                   self.execute_legacy_command(&name, &step.args).await?
               }
           };
           
           if step.capture_output {
               ctx.captured_outputs.insert("CAPTURED_OUTPUT".to_string(), result.stdout.clone());
           }
           
           // Handle conditional execution
           self.handle_conditional_steps(step, &result, ctx).await?;
           
           Ok(result)
       }
   }
   ```

### Architecture Changes

1. **Parser Updates**
   - Extend YAML parsing to handle new fields
   - Validate mutual exclusivity of command types
   - Support variable interpolation syntax

2. **Executor Modifications**  
   - Add shell command execution capability
   - Implement variable interpolation engine
   - Extend context passing between steps

3. **Backwards Compatibility Layer**
   - Detect old `name:` syntax
   - Convert to appropriate command type
   - Emit deprecation warnings (optional)

### Data Structures

```yaml
# New workflow syntax examples
commands:
  # Claude command with inline args
  - claude: "/mmm-implement-spec $ARG"
    analysis:
      max_cache_age: 300
  
  # Shell command with output capture
  - shell: "cargo test --lib"
    capture_output: true
    on_failure:
      claude: "/mmm-fix-test-failures '$CAPTURED_OUTPUT'"
      on_success:
        shell: "cargo test --lib"
        on_failure:
          claude: "/mmm-report-persistent-failure '$CAPTURED_OUTPUT'"
  
  # Complex conditional logic
  - shell: "cargo test --workspace"
    on_exit_code:
      0:
        shell: "echo 'All tests passed!'"
      1:
        claude: "/mmm-debug-test-failures '$CAPTURED_OUTPUT'"
      101:
        claude: "/mmm-fix-compilation-errors '$CAPTURED_OUTPUT'"
    
  # Legacy syntax still works
  - name: mmm-lint
    commit_required: false
```

### APIs and Interfaces

1. **Workflow YAML Schema**
   ```yaml
   commands:
     - claude: <string>    # Claude CLI command with args
       shell: <string>     # Shell command to execute  
       name: <string>      # Legacy field (deprecated)
       
       # Execution options
       capture_output: <bool>
       timeout: <int>      # seconds
       working_dir: <string>
       
       # Environment
       env:
         KEY: value
       
       # Conditional execution
       on_failure: <step>
       on_success: <step>
       on_exit_code:
         <int>: <step>
       
       # Git behavior
       commit_required: <bool>
       
       # Analysis config
       analysis:
         max_cache_age: <int>
         auto_recover: <bool>
   ```

2. **Available Variables**
   - `$ARG` - Argument passed via --args or --map
   - `$CAPTURED_OUTPUT` - Output from previous step with capture_output: true
   - `$ITERATION` - Current iteration number
   - `$WORKTREE` - Current worktree name (if applicable)
   - `$PROJECT_ROOT` - Project root directory
   - Custom environment variables from `env:` field

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - Workflow parser (`src/cook/workflow/`)
  - Command executor (`src/cook/execution/`)
  - YAML configuration parsing
- **External Dependencies**: None new

## Testing Strategy

- **Unit Tests**:
  - Test command type detection logic
  - Test variable interpolation
  - Test YAML parsing with new syntax
  - Test backward compatibility

- **Integration Tests**:
  - Test Claude command execution
  - Test shell command execution
  - Test output capture and passing
  - Test conditional execution paths

- **User Acceptance**:
  - Convert example workflows to new syntax
  - Test with real specifications
  - Verify error handling and messages

## Documentation Requirements

- **Code Documentation**:
  - Document new WorkflowStep fields
  - Add examples in code comments
  - Document interpolation syntax

- **User Documentation**:
  - Update workflow documentation
  - Provide migration guide
  - Include comprehensive examples
  - Document available variables

## Implementation Notes

1. **Parsing Priority**: If multiple command types specified, fail with clear error
2. **Shell Security**: Commands run with user's full permissions - document risks
3. **Output Limits**: Captured output should have reasonable size limits
4. **Variable Escaping**: Support escaping $ with \$ for literal dollar signs
5. **Error Context**: Include step details in error messages for debugging

## Migration and Compatibility

- **Deprecation Strategy**: 
  - Phase 1: Support both syntaxes, emit info messages
  - Phase 2: Emit warnings for old syntax
  - Phase 3: Remove support (major version bump)

- **Migration Tool**: Consider providing automated migration script

- **Documentation**: Extensive examples showing before/after syntax

## Example Workflows

### Implementation with Tests
```yaml
commands:
  - claude: "/mmm-implement-spec $ARG"
    analysis:
      max_cache_age: 300
  
  - shell: "cargo test"
    capture_output: true
    on_failure:
      claude: "/mmm-fix-test-failures '$CAPTURED_OUTPUT'"
      on_success:
        shell: "cargo test"
        
  - claude: "/mmm-lint"
    commit_required: false
```

### Complex Build Pipeline
```yaml
commands:
  - shell: "cargo check"
    on_success:
      shell: "cargo build --release"
      on_success:
        shell: "cargo test --release"
        on_failure:
          claude: "/mmm-debug-and-fix '$CAPTURED_OUTPUT'"
```

### Analysis with Recovery
```yaml
commands:
  - claude: "/mmm-analyze-code"
    analysis:
      auto_recover: true
      
  - shell: "cargo tarpaulin"
    capture_output: true
    on_exit_code:
      0:
        claude: "/mmm-improve-coverage '$CAPTURED_OUTPUT'"
      default:
        claude: "/mmm-fix-coverage-errors '$CAPTURED_OUTPUT'"
```