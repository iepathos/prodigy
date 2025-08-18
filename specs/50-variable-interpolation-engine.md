---
number: 50
title: Variable Interpolation Engine for MapReduce
category: foundation
priority: critical
status: draft
dependencies: [49]
created: 2025-08-18
---

# Specification 50: Variable Interpolation Engine for MapReduce

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [49 - MapReduce Parallel Execution]

## Context

The MapReduce implementation (spec 49) has been partially completed with the core parallel execution framework in place. However, a critical missing component is the variable interpolation engine that allows dynamic command generation based on work item data. The YAML workflows define templates with placeholders like `${item.description}`, `${shell.output}`, and `${map.results}`, but the actual substitution logic is not implemented.

Currently, the `execute_agent_commands` function in `mapreduce.rs` contains placeholder code that acknowledges the need for variable interpolation but doesn't perform it. This prevents the MapReduce executor from actually executing meaningful commands tailored to each work item.

## Objective

Implement a robust variable interpolation engine that can:
1. Parse template strings containing variable placeholders
2. Resolve variables from multiple contexts (item data, shell output, map results)
3. Handle nested object paths (e.g., `${item.location.file}`)
4. Provide safe fallbacks for missing variables
5. Support different variable scopes for map and reduce phases

## Requirements

### Functional Requirements

1. **Template Parsing**
   - Identify all `${...}` placeholders in strings
   - Support nested property access with dot notation
   - Handle escaped placeholders that shouldn't be interpolated
   - Support array indexing (e.g., `${items[0].name}`)

2. **Variable Resolution**
   - Resolve variables from work item JSON data
   - Access shell command output from previous steps
   - Retrieve map phase results in reduce phase
   - Support environment variables
   - Handle special variables (e.g., `${timestamp}`, `${iteration}`)

3. **Context Management**
   - Maintain separate variable contexts for each agent
   - Pass context between workflow steps
   - Merge contexts for reduce phase
   - Support context inheritance from parent scopes

4. **Type Handling**
   - Convert JSON values to appropriate string representations
   - Support numeric and boolean values
   - Handle null/undefined values gracefully
   - Preserve JSON structure for complex objects when needed

5. **Error Handling**
   - Provide clear error messages for undefined variables
   - Support optional variables with default values (e.g., `${var:-default}`)
   - Allow strict mode that fails on undefined variables
   - Log all interpolation activities for debugging

### Non-Functional Requirements

1. **Performance**
   - Interpolation should add <1ms overhead per command
   - Cache parsed templates for repeated use
   - Minimize memory allocation during substitution

2. **Security**
   - Prevent code injection through variable values
   - Sanitize file paths and shell arguments
   - Limit recursion depth for nested interpolations

3. **Maintainability**
   - Clear separation between parsing and resolution
   - Extensible for new variable sources
   - Well-documented variable naming conventions

## Acceptance Criteria

- [ ] Can parse and interpolate basic variables: `${item.name}`
- [ ] Supports nested property access: `${item.location.file}`
- [ ] Handles array indexing: `${results[0].status}`
- [ ] Provides default values: `${timeout:-600}`
- [ ] Escapes special characters in interpolated values for shell safety
- [ ] Maintains separate contexts for parallel agents
- [ ] Passes shell output between steps via `${shell.output}`
- [ ] Aggregates map results accessible as `${map.results}` in reduce phase
- [ ] Fails gracefully with clear errors for undefined required variables
- [ ] Includes comprehensive unit tests for all interpolation patterns
- [ ] Documentation includes variable reference guide
- [ ] Performance benchmark shows <1ms overhead

## Technical Details

### Implementation Approach

1. Create a new module `src/cook/execution/interpolation.rs`
2. Implement a two-phase approach: parse then resolve
3. Use regex for template parsing with proper escaping
4. Leverage serde_json for JSON path traversal
5. Implement a ContextStack for variable scoping

### Architecture Changes

```rust
// src/cook/execution/interpolation.rs
pub struct InterpolationEngine {
    strict_mode: bool,
    cache: HashMap<String, Template>,
}

pub struct Template {
    raw: String,
    segments: Vec<Segment>,
}

pub enum Segment {
    Literal(String),
    Variable {
        path: Vec<String>,
        default: Option<String>,
    },
}

pub struct InterpolationContext {
    variables: HashMap<String, Value>,
    parent: Option<Box<InterpolationContext>>,
}

impl InterpolationEngine {
    pub fn interpolate(&self, template: &str, context: &InterpolationContext) -> Result<String>;
    pub fn parse_template(&self, template: &str) -> Result<Template>;
    pub fn resolve_variable(&self, path: &[String], context: &InterpolationContext) -> Result<Value>;
}
```

### Data Structures

```rust
// Variable path representation
pub struct VariablePath {
    segments: Vec<PathSegment>,
}

pub enum PathSegment {
    Property(String),
    Index(usize),
}

// Context hierarchy for scoping
pub struct ContextStack {
    contexts: Vec<InterpolationContext>,
}
```

### APIs and Interfaces

```rust
// Integration with MapReduceExecutor
impl MapReduceExecutor {
    fn prepare_agent_context(&self, item: &Value, item_id: &str) -> InterpolationContext;
    fn interpolate_command(&self, cmd: &str, context: &InterpolationContext) -> Result<String>;
    fn update_context_with_output(&mut self, context: &mut InterpolationContext, output: &str);
}

// Workflow step enhancement
impl WorkflowStep {
    fn interpolate_fields(&self, engine: &InterpolationEngine, context: &InterpolationContext) -> Result<WorkflowStep>;
}
```

## Dependencies

- **Prerequisites**: Spec 49 (MapReduce base implementation)
- **Affected Components**:
  - `src/cook/execution/mapreduce.rs` - Integration points
  - `src/cook/workflow/executor.rs` - Command interpolation
  - `src/config/workflow.rs` - Template validation
- **External Dependencies**:
  - `regex` - For template parsing
  - `jsonpath` or similar - For JSON path evaluation (optional)

## Testing Strategy

- **Unit Tests**:
  - Template parsing with various placeholder patterns
  - Variable resolution from nested JSON
  - Default value handling
  - Escape sequence processing
  - Context inheritance and scoping

- **Integration Tests**:
  - Full MapReduce workflow with interpolation
  - Multi-step workflows with output passing
  - Parallel agent context isolation
  - Error propagation for undefined variables

- **Performance Tests**:
  - Benchmark interpolation of 1000 templates
  - Memory usage with large contexts
  - Cache effectiveness measurement

## Documentation Requirements

- **Code Documentation**:
  - Variable syntax reference
  - Supported path expressions
  - Context scoping rules

- **User Documentation**:
  - Variable interpolation guide
  - Common patterns and examples
  - Debugging interpolation issues

- **Architecture Updates**:
  - Add interpolation engine to component diagram
  - Document variable flow in MapReduce

## Implementation Notes

### Phase 1: Core Engine (Day 1-2)
- Template parser with regex
- Basic variable resolution
- Simple context management

### Phase 2: Advanced Features (Day 3-4)
- Nested path resolution
- Default values and options
- Context inheritance

### Phase 3: Integration (Day 5)
- Wire into MapReduceExecutor
- Update WorkflowStep processing
- Add shell output passing

### Key Considerations

1. **Shell Safety**: Always escape special characters when interpolating for shell commands
2. **JSON Paths**: Support both dot notation and bracket notation
3. **Debugging**: Log all interpolations at debug level
4. **Caching**: Cache parsed templates but not resolved values
5. **Extensibility**: Design for future variable sources (env, secrets, etc.)

## Migration and Compatibility

- **Breaking Changes**: None - adds new functionality
- **Migration Path**: Existing workflows continue to work, interpolation is opt-in
- **Compatibility**: Works with all existing workflow commands
- **Rollback**: Can disable interpolation with feature flag if needed