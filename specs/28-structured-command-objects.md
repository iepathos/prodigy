# Specification 28: Structured Command Objects

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: 21 (configurable-workflow)

## Context

Currently, MMM's workflow system uses simple string-based command representations in the WorkflowConfig:

```rust
pub struct WorkflowConfig {
    pub commands: Vec<String>,
    pub max_iterations: u32,
}
```

This approach, while simple, has several limitations:
- No type safety for command parameters
- Limited ability to pass configuration to individual commands
- String parsing for special cases (e.g., extracting spec IDs for mmm-implement-spec)
- No structured way to handle command-specific options or behaviors
- Difficult to extend with new command features without breaking existing configs

## Objective

Transform the workflow command system from string-based to structured command objects, providing:
- Type-safe command definitions with validated parameters
- First-class support for command arguments and options
- Extensible command metadata (retries, timeouts, error handling)
- Better integration with the Claude CLI interface
- Backward compatibility with existing string-based configurations

## Requirements

### Functional Requirements

1. **Command Object Structure**
   - Define a `Command` struct with fields for name, arguments, and options
   - Support both positional arguments and named options
   - Allow commands to specify their required and optional parameters
   - Enable command-specific configuration (retries, timeouts, etc.)

2. **Type Safety**
   - Compile-time validation of command structure
   - Runtime validation of command parameters
   - Clear error messages for invalid command configurations

3. **Backward Compatibility**
   - Support existing string-based command configurations
   - Automatically convert simple strings to command objects
   - Maintain existing workflow behavior for legacy configs

4. **Command Registry**
   - Central registry of available commands with metadata
   - Command validation against registry
   - Extensible for custom commands

### Non-Functional Requirements

1. **Performance**
   - Minimal overhead compared to string-based approach
   - Efficient serialization/deserialization

2. **Usability**
   - Clear, intuitive command configuration syntax
   - Helpful error messages for misconfiguration
   - Good documentation and examples

3. **Maintainability**
   - Clean separation between command definition and execution
   - Easy to add new commands or modify existing ones
   - Testable command logic

## Acceptance Criteria

- [ ] Command struct defined with name, args, options, and metadata fields
- [ ] WorkflowConfig updated to use Vec<Command> while maintaining backward compatibility
- [ ] Command registry implemented with built-in MMM commands
- [ ] String-to-Command conversion for legacy configurations
- [ ] Updated workflow executor to use structured commands
- [ ] Validation logic for commands against registry
- [ ] Tests for command parsing, validation, and execution
- [ ] Documentation updated with new command syntax examples
- [ ] No breaking changes to existing workflow configurations

## Technical Details

### Implementation Approach

1. **Command Structure**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// The command name (e.g., "mmm-code-review")
    pub name: String,
    
    /// Positional arguments for the command
    #[serde(default)]
    pub args: Vec<String>,
    
    /// Named options/flags for the command
    #[serde(default)]
    pub options: HashMap<String, Value>,
    
    /// Command-specific metadata
    #[serde(default)]
    pub metadata: CommandMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandMetadata {
    /// Number of retry attempts (overrides global setting)
    pub retries: Option<u32>,
    
    /// Timeout in seconds
    pub timeout: Option<u64>,
    
    /// Continue workflow on command failure
    pub continue_on_error: Option<bool>,
    
    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,
}
```

2. **Backward Compatibility**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowCommand {
    /// Legacy string format
    Simple(String),
    /// New structured format
    Structured(Command),
}

impl WorkflowCommand {
    pub fn to_command(&self) -> Command {
        match self {
            WorkflowCommand::Simple(s) => Command::from_string(s),
            WorkflowCommand::Structured(c) => c.clone(),
        }
    }
}
```

3. **Command Registry**
```rust
pub struct CommandRegistry {
    commands: HashMap<String, CommandDefinition>,
}

pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    pub required_args: Vec<ArgumentDef>,
    pub optional_args: Vec<ArgumentDef>,
    pub options: Vec<OptionDef>,
    pub defaults: CommandMetadata,
}
```

### Architecture Changes

1. **New Modules**
   - `src/config/command.rs` - Command structures and registry
   - `src/config/command_parser.rs` - String to Command conversion
   - `src/config/command_validator.rs` - Command validation logic

2. **Modified Modules**
   - `src/config/workflow.rs` - Update to use WorkflowCommand enum
   - `src/improve/workflow.rs` - Update executor for structured commands
   - `src/config/loader.rs` - Handle new configuration format

### Data Structures

1. **Updated WorkflowConfig**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub commands: Vec<WorkflowCommand>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
}
```

2. **Example Configurations**
```toml
# Legacy format (still supported)
[workflow]
commands = [
    "mmm-code-review",
    "mmm-implement-spec",
    "mmm-lint"
]

# New structured format
[workflow]
[[workflow.commands]]
name = "mmm-code-review"
options = { focus = "security" }
metadata = { retries = 3 }

[[workflow.commands]]
name = "mmm-implement-spec"
args = ["${SPEC_ID}"]

[[workflow.commands]]
name = "mmm-lint"
metadata = { continue_on_error = true }
```

### APIs and Interfaces

1. **Command Execution Interface**
```rust
impl WorkflowExecutor {
    async fn execute_command(&self, command: &Command) -> Result<bool> {
        // Validate command against registry
        self.validate_command(command)?;
        
        // Build subprocess command with structured data
        let mut cmd = self.build_subprocess_command(command)?;
        
        // Apply command metadata (retries, timeout, env)
        self.apply_command_metadata(&mut cmd, &command.metadata);
        
        // Execute with appropriate error handling
        self.execute_with_metadata(cmd, command).await
    }
}
```

## Dependencies

- **Prerequisites**: Spec 21 (configurable workflows) must be implemented
- **Affected Components**: 
  - Config loader and validator
  - Workflow executor
  - Command execution logic
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Command parsing from strings
  - Command validation against registry
  - Backward compatibility with string configs
  - Command metadata application

- **Integration Tests**:
  - Full workflow execution with structured commands
  - Mixed legacy and structured command workflows
  - Error handling for invalid commands
  - Command registry validation

- **Performance Tests**:
  - Command parsing performance
  - Workflow execution overhead

- **User Acceptance**:
  - Existing workflows continue to work
  - New structured format is intuitive
  - Error messages are helpful

## Documentation Requirements

- **Code Documentation**:
  - Document all new structures and enums
  - Add examples for command construction
  - Document registry format

- **User Documentation**:
  - Update workflow configuration examples
  - Add migration guide from string to structured format
  - Document available command options

- **Architecture Updates**:
  - Update ARCHITECTURE.md with new command system
  - Add command registry documentation

## Implementation Notes

1. **Phased Rollout**
   - Phase 1: Implement command structures with backward compatibility
   - Phase 2: Add command registry and validation
   - Phase 3: Enhance with advanced features (conditional execution, etc.)

2. **Error Handling**
   - Validate commands early in workflow loading
   - Provide clear error messages for invalid configurations
   - Fall back gracefully for unrecognized commands

3. **Extensibility**
   - Design with future command types in mind
   - Allow for custom command handlers
   - Support command plugins in future

## Migration and Compatibility

1. **Automatic Migration**
   - String commands are automatically converted to structured format
   - No changes required to existing workflows
   - Deprecation warnings for legacy format (in future release)

2. **Migration Path**
   - Users can gradually migrate commands one at a time
   - Mix of string and structured commands supported
   - Migration tool to convert existing configs (future)

3. **Version Compatibility**
   - Full backward compatibility in initial release
   - Legacy format supported for at least 3 major versions
   - Clear deprecation timeline when announced