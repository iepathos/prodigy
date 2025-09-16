---
number: 61
title: Enhanced Variable Interpolation System
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-15
---

# Specification 61: Enhanced Variable Interpolation System

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current variable interpolation system only supports basic `${item}` substitution in MapReduce workflows. Critical aggregate variables like `${map.successful}`, `${map.total}`, `${map.results}` are missing, and there's no mechanism for passing variables between workflow phases. This severely limits the ability to create sophisticated workflows that adapt based on execution results or share context between phases.

## Objective

Implement a comprehensive variable interpolation system that supports aggregate MapReduce variables, cross-phase variable passing, computed variables, and environment variable integration, enabling dynamic and context-aware workflow execution.

## Requirements

### Functional Requirements

1. **MapReduce Aggregate Variables**
   - `${map.total}` - Total number of items to process
   - `${map.successful}` - Count of successfully processed items
   - `${map.failed}` - Count of failed items
   - `${map.skipped}` - Count of skipped items
   - `${map.results}` - Array of all map phase results
   - `${map.duration}` - Total execution time
   - `${map.success_rate}` - Percentage of successful items

2. **Phase Variables**
   - `${setup.output}` - Captured output from setup phase
   - `${setup.variables.*}` - Variables defined in setup
   - `${reduce.output}` - Output from reduce phase
   - `${workflow.name}` - Current workflow name
   - `${workflow.id}` - Unique workflow execution ID
   - `${workflow.start_time}` - Workflow start timestamp

3. **Item Context Variables**
   - `${item}` - Current work item (existing)
   - `${item.index}` - Zero-based index in work queue
   - `${item.attempt}` - Current retry attempt number
   - `${item.previous_error}` - Error from last attempt
   - `${agent.id}` - Current agent identifier
   - `${agent.worktree}` - Agent's worktree path

4. **Computed Variables**
   - `${env.VAR_NAME}` - Environment variables
   - `${file:path/to/file}` - File contents
   - `${cmd:command}` - Command output
   - `${json:path.to.field}` - JSON field extraction
   - `${date:format}` - Formatted timestamps
   - `${uuid}` - Generate unique identifier

5. **Variable Scoping**
   - Global variables accessible across all phases
   - Phase-local variables with explicit exports
   - Item-scoped variables within map operations
   - Variable shadowing rules and precedence

6. **Variable Persistence**
   - Save variables to checkpoint for resume
   - Pass variables between workflow stages
   - Export variables to subsequent workflows
   - Variable history for debugging

### Non-Functional Requirements

1. **Performance**
   - Lazy evaluation of expensive variables
   - Caching of computed values
   - Minimal overhead for interpolation

2. **Safety**
   - Prevent circular variable references
   - Limit recursive expansion depth
   - Sanitize command execution variables

3. **Debuggability**
   - Variable resolution tracing
   - Clear error messages for undefined variables
   - Dry-run mode to preview interpolations

## Acceptance Criteria

- [ ] All specified MapReduce aggregate variables are available in reduce phase
- [ ] Variables can be passed from setup to map/reduce phases
- [ ] Environment variables are accessible via `${env.NAME}` syntax
- [ ] File contents can be interpolated with `${file:path}`
- [ ] Command output can be captured with `${cmd:command}`
- [ ] JSON extraction works with `${json:path.to.field}`
- [ ] Computed variables are evaluated lazily and cached
- [ ] Variable scoping rules are enforced correctly
- [ ] Undefined variable references produce clear errors
- [ ] Variable resolution can be traced for debugging
- [ ] Variables persist across workflow resume operations
- [ ] Performance overhead is less than 5% for typical workflows

## Technical Details

### Implementation Approach

```rust
pub struct VariableContext {
    global: HashMap<String, Value>,
    phase: HashMap<String, Value>,
    computed: HashMap<String, Box<dyn ComputedVariable>>,
    cache: LruCache<String, Value>,
}

impl VariableContext {
    pub fn interpolate(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();
        let variables = self.extract_variables(template)?;

        for var in variables {
            let value = self.resolve_variable(&var)?;
            result = result.replace(&format!("${{{}}}", var), &value.to_string());
        }

        Ok(result)
    }

    fn resolve_variable(&self, path: &str) -> Result<Value> {
        // Check cache first
        if let Some(cached) = self.cache.get(path) {
            return Ok(cached.clone());
        }

        // Parse variable path
        let parts: Vec<&str> = path.split('.').collect();

        let value = match parts[0] {
            "map" => self.resolve_map_variable(&parts[1..])?,
            "env" => self.resolve_env_variable(&parts[1..])?,
            "file" => self.resolve_file_variable(&parts[1..])?,
            "cmd" => self.resolve_command_variable(&parts[1..])?,
            _ => self.resolve_standard_variable(path)?,
        };

        self.cache.put(path.to_string(), value.clone());
        Ok(value)
    }
}
```

### Architecture Changes

1. **Variable Resolution Module**
   - New `variables` module in `cook::execution`
   - Variable parser and resolver
   - Computed variable trait system

2. **Context Management**
   - Enhanced InterpolationContext
   - Variable scope management
   - Cross-phase variable passing

3. **Integration Points**
   - Hook into all command execution points
   - Variable capture from command outputs
   - State persistence for variables

### Data Structures

```rust
pub enum Variable {
    Static(Value),
    Computed(Box<dyn ComputedVariable>),
    Reference(String), // Reference to another variable
    Aggregate(AggregateType),
}

pub enum AggregateType {
    Count { filter: Option<String> },
    Sum { field: String },
    Average { field: String },
    Min { field: String },
    Max { field: String },
    Collect { field: String },
}

pub trait ComputedVariable: Send + Sync {
    fn evaluate(&self, context: &VariableContext) -> Result<Value>;
    fn cache_key(&self) -> String;
    fn is_expensive(&self) -> bool;
}

pub struct VariableScope {
    pub global: HashMap<String, Variable>,
    pub phase: HashMap<String, Variable>,
    pub local: HashMap<String, Variable>,
    pub precedence: Vec<ScopeLevel>,
}
```

### APIs and Interfaces

```rust
pub trait VariableProvider {
    fn provide_variables(&self) -> HashMap<String, Value>;
    fn update_variables(&mut self, updates: HashMap<String, Value>);
}

pub trait VariableInterpolator {
    fn interpolate(&self, template: &str, context: &VariableContext) -> Result<String>;
    fn extract_variables(&self, template: &str) -> Vec<String>;
    fn validate_variables(&self, template: &str, context: &VariableContext) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/execution/interpolation.rs`
  - `src/cook/execution/mapreduce.rs`
  - `src/cook/workflow/variables.rs`
  - All command executors
- **External Dependencies**:
  - `regex` for variable extraction
  - `lru` for value caching

## Testing Strategy

- **Unit Tests**:
  - Variable extraction from templates
  - Resolution of all variable types
  - Scope precedence rules
  - Cache behavior

- **Integration Tests**:
  - Cross-phase variable passing
  - Aggregate variable calculation
  - Complex nested interpolations
  - Performance with many variables

- **Edge Cases**:
  - Circular variable references
  - Deep nesting limits
  - Missing variables
  - Malformed templates

- **User Acceptance**:
  - Real workflow variable usage
  - Debug output clarity
  - Performance impact measurement

## Documentation Requirements

- **Code Documentation**:
  - Complete variable reference
  - Interpolation syntax guide
  - Computed variable examples

- **User Documentation**:
  - Variable cookbook with patterns
  - Scoping rules explanation
  - Performance considerations

- **Architecture Updates**:
  - Variable resolution flow
  - Caching strategy
  - Extension points

## Implementation Notes

1. **Lazy Evaluation**: Only compute expensive variables when needed
2. **Security**: Sanitize command execution to prevent injection
3. **Debugging**: Add `--trace-variables` flag for resolution details
4. **Performance**: Use string builder for efficient interpolation
5. **Extensibility**: Plugin system for custom computed variables

## Migration and Compatibility

- Backward compatible with existing `${item}` syntax
- Automatic migration of old variable formats
- Deprecation warnings for obsolete patterns
- Gradual rollout with feature flags