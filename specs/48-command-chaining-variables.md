# Specification 48: Command Chaining with Variables

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: Spec 21 (Configurable Workflow), Spec 28 (Structured Command Objects)

## Context

MMM currently has hardcoded behavior for extracting specs from git commits created by specific commands (mmm-code-review, mmm-cleanup-tech-debt) and passing them to mmm-implement-spec. This hardcoded approach limits flexibility and makes it difficult to create custom workflows with arbitrary commands that produce and consume specs or other outputs.

As the system evolves, users need the ability to:
- Create custom Claude commands that generate specs
- Chain commands together with data passing between them
- Configure which commands produce specs and which consume them
- Pass different types of outputs between commands (not just specs)

## Objective

Implement a flexible command chaining system with variables that allows workflow configurations to define how data flows between commands, making spec generation and consumption configurable rather than hardcoded.

## Requirements

### Functional Requirements

1. **Command Output Declaration**
   - Commands can declare named outputs they produce
   - Support different output types: git:commit:spec, file:path, stdout, variable
   - Output extraction methods are configurable

2. **Command Input Declaration**
   - Commands can declare inputs they expect
   - Inputs can reference outputs from previous commands by ID
   - Support passing inputs as arguments, environment variables, or stdin

3. **Variable Resolution System**
   - Resolve ${command_id.output_name} references
   - Support fallback values for missing variables
   - Validate variable references at workflow start

4. **Backward Compatibility**
   - Existing workflows without IDs/variables continue to work
   - Default behavior for known commands (mmm-code-review → mmm-implement-spec)
   - Gradual migration path for existing configurations

### Non-Functional Requirements

- Performance: Variable resolution should add minimal overhead
- Clarity: Variable syntax should be intuitive and readable
- Flexibility: Support various data passing patterns
- Reliability: Clear error messages for missing/invalid references

## Acceptance Criteria

- [ ] Commands can be assigned unique IDs in workflow configs
- [ ] Commands can declare outputs with extraction methods
- [ ] Commands can reference outputs from previous commands
- [ ] Variable references are resolved before command execution
- [ ] Git commit spec extraction works via output declaration
- [ ] File path outputs can be passed between commands
- [ ] Command stdout can be captured and passed as input
- [ ] Existing workflows continue functioning without changes
- [ ] Clear error messages for unresolved variables
- [ ] Documentation updated with examples

## Technical Details

### Implementation Approach

1. **Extend WorkflowCommand Structure**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCommand {
    // Existing fields...
    
    /// Unique identifier for this command in the workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    
    /// Outputs this command produces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<HashMap<String, OutputDeclaration>>,
    
    /// Inputs this command expects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, InputReference>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDeclaration {
    /// Type of output extraction
    pub extract_from: OutputSource,
    
    /// Optional pattern for extraction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputSource {
    /// Extract from git commit (e.g., spec files)
    GitCommit { file_pattern: String },
    
    /// Capture command stdout
    Stdout,
    
    /// Read from file path
    File { path: String },
    
    /// Set directly as variable
    Variable { value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputReference {
    /// Reference to output: "${command_id.output_name}"
    pub from: String,
    
    /// How to pass the input to the command
    pub pass_as: InputMethod,
    
    /// Fallback value if reference not found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMethod {
    /// Pass as positional argument
    Argument { position: usize },
    
    /// Set as environment variable
    Environment { name: String },
    
    /// Pass via stdin
    Stdin,
}
```

2. **Variable Resolution in WorkflowExecutor**
```rust
impl WorkflowExecutor {
    /// Track command outputs during execution
    command_outputs: HashMap<String, HashMap<String, String>>,
    
    /// Resolve variable references before command execution
    fn resolve_inputs(&self, command: &Command) -> Result<Command> {
        let mut resolved_command = command.clone();
        
        if let Some(inputs) = &command.inputs {
            for (input_name, input_ref) in inputs {
                let value = self.resolve_variable(&input_ref.from)
                    .or_else(|_| input_ref.default.ok_or("No default value"))
                    .context(format!("Failed to resolve input '{}'", input_name))?;
                
                match &input_ref.pass_as {
                    InputMethod::Argument { position } => {
                        // Insert at specified position
                        resolved_command.args.insert(*position, CommandArg::Literal(value));
                    }
                    InputMethod::Environment { name } => {
                        resolved_command.metadata.env.insert(name.clone(), value);
                    }
                    InputMethod::Stdin => {
                        // Store for stdin passing
                        resolved_command.metadata.stdin = Some(value);
                    }
                }
            }
        }
        
        Ok(resolved_command)
    }
    
    /// Extract outputs after command execution
    async fn extract_outputs(&mut self, command_id: &str, outputs: &HashMap<String, OutputDeclaration>) -> Result<()> {
        let mut extracted = HashMap::new();
        
        for (output_name, output_decl) in outputs {
            let value = match &output_decl.extract_from {
                OutputSource::GitCommit { file_pattern } => {
                    // Extract spec from git commit (existing logic)
                    self.extract_spec_from_git_pattern(file_pattern).await?
                }
                OutputSource::Stdout => {
                    // Capture from last command stdout
                    self.last_command_stdout.clone()
                }
                OutputSource::File { path } => {
                    // Read file content
                    tokio::fs::read_to_string(path).await?
                }
                OutputSource::Variable { value } => {
                    // Direct value
                    value.clone()
                }
            };
            
            extracted.insert(output_name.clone(), value);
        }
        
        self.command_outputs.insert(command_id.clone(), extracted);
        Ok(())
    }
}
```

### Architecture Changes

1. **WorkflowExecutor Enhancement**
   - Add command output tracking HashMap
   - Implement variable resolution before command execution
   - Extract outputs after command execution
   - Validate variable references at workflow start

2. **Command Execution Flow**
   - Resolve input variables
   - Execute command with resolved inputs
   - Extract declared outputs
   - Store outputs for future references

3. **Backward Compatibility Layer**
   - Auto-generate IDs for commands without explicit IDs
   - Default output extraction for known commands
   - Implicit spec passing for mmm-implement-spec

### Configuration Examples

```yaml
# Example 1: Explicit variable chaining
workflow:
  commands:
    - name: mmm-code-review
      id: review
      outputs:
        spec: 
          extract_from: 
            git_commit: 
              file_pattern: "specs/temp/*.md"
    
    - name: mmm-implement-spec
      inputs:
        spec: 
          from: "${review.spec}"
          pass_as:
            argument:
              position: 0
    
    - name: mmm-lint

# Example 2: Custom command chaining
workflow:
  commands:
    - name: custom-analyzer
      id: analyze
      outputs:
        issues:
          extract_from: stdout
        spec:
          extract_from:
            git_commit:
              file_pattern: "analysis/*.md"
    
    - name: custom-fixer
      inputs:
        spec:
          from: "${analyze.spec}"
          pass_as:
            argument:
              position: 0
        context:
          from: "${analyze.issues}"
          pass_as:
            environment:
              name: "ANALYSIS_CONTEXT"

# Example 3: Backward compatible (works as before)
workflow:
  commands:
    - mmm-code-review
    - mmm-implement-spec
    - mmm-lint
```

## Dependencies

- **Prerequisites**: 
  - Spec 21: Configurable Workflow (provides workflow structure)
  - Spec 28: Structured Command Objects (provides command metadata)
- **Affected Components**: 
  - workflow.rs: Extended command structure
  - cook/workflow.rs: Variable resolution logic
  - config/command.rs: New input/output types
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Variable resolution with valid/invalid references
  - Output extraction for different source types
  - Input passing via different methods
  - Backward compatibility scenarios
- **Integration Tests**: 
  - Full workflow execution with variable chaining
  - Git commit spec extraction
  - File and stdout capture
  - Error handling for missing variables
- **Performance Tests**: 
  - Variable resolution overhead measurement
  - Large workflow performance
- **User Acceptance**: 
  - Existing workflows continue working
  - New variable syntax is intuitive
  - Clear error messages

## Documentation Requirements

- **Code Documentation**: 
  - Document new struct fields and enums
  - Variable resolution algorithm
  - Output extraction methods
- **User Documentation**: 
  - Update workflow configuration guide
  - Add variable chaining examples
  - Migration guide from hardcoded behavior
- **Architecture Updates**: 
  - Document data flow between commands
  - Variable resolution lifecycle

## Implementation Notes

1. **Variable Syntax**
   - Use ${command_id.output_name} for clarity
   - Support nested resolution for complex scenarios
   - Consider ${command_id.output_name:-default} syntax

2. **Error Handling**
   - Validate all variable references before execution
   - Provide helpful error messages with command context
   - Support --dry-run to validate workflows

3. **Performance Considerations**
   - Cache resolved variables to avoid re-computation
   - Lazy evaluation where possible
   - Minimal overhead for non-variable workflows

4. **Future Extensions**
   - Array outputs for batch processing
   - Conditional execution based on variables
   - Variable transformations (uppercase, regex, etc.)

## Migration and Compatibility

1. **Existing Workflows**
   - Continue working without modification
   - Implicit behavior for known command pairs
   - Deprecation warnings for legacy patterns

2. **Migration Path**
   - Document how to convert implicit to explicit
   - Provide migration tool/script if needed
   - Phase out hardcoded behavior over time

3. **Breaking Changes**
   - None - fully backward compatible
   - New features are opt-in via configuration
   - Existing behavior preserved by default