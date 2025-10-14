---
number: 133
title: Workflow Template Integration and Polish
category: foundation
priority: medium
status: draft
dependencies: [131, 132]
created: 2025-10-13
---

# Specification 133: Workflow Template Integration and Polish

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 131 (Execution Layer), Spec 132 (CLI Interface)

## Context

After implementing the execution layer (Spec 131) and CLI interface (Spec 132), the template system will be functional but incomplete. Several advanced features remain unimplemented (URL templates, sub-workflow execution, parameter overrides), and the system needs comprehensive testing, documentation, and examples to be production-ready.

This specification addresses the remaining TODOs, adds production polish, and ensures the template system is fully integrated with Prodigy's existing features.

## Objective

Complete remaining template system features, add comprehensive examples and documentation, ensure seamless integration with existing Prodigy features, and prepare for production use.

## Requirements

### Functional Requirements

1. **Complete TODO Implementations**
   - Implement template parameter substitution in `WorkflowComposer::apply_template_params()`
   - Implement override application in `WorkflowComposer::apply_overrides()`
   - Implement selective imports in `WorkflowComposer::import_selective()`
   - Implement default value application in `WorkflowComposer::apply_defaults()`
   - Add aliased import support in `WorkflowComposer::process_imports()`

2. **Sub-Workflow Execution**
   - Integrate `SubWorkflowExecutor` into workflow execution path
   - Support parallel sub-workflow execution
   - Handle sub-workflow inputs/outputs correctly
   - Propagate errors according to configuration

3. **Advanced Template Features**
   - Support nested templates (templates using templates)
   - Add template validation during composition
   - Support conditional template application
   - Add template debugging/dry-run mode

4. **Integration with Existing Features**
   - Template support in MapReduce workflows
   - Template parameters work with environment variables
   - Template composition respects merge workflows
   - Templates work with session management

5. **Production Features**
   - Template caching for performance
   - Template versioning support (basic)
   - Template dependency resolution
   - Template validation at multiple stages

### Non-Functional Requirements

1. **Performance**
   - Template composition overhead < 100ms for simple templates
   - Caching reduces repeated composition time by 80%
   - Sub-workflow overhead < 50ms per workflow

2. **Reliability**
   - Comprehensive error handling at all levels
   - Graceful fallback when templates unavailable
   - Validation catches errors before execution
   - Clear error messages with context

3. **Usability**
   - Extensive examples covering common use cases
   - Clear documentation with diagrams
   - Debugging tools for template development
   - Migration guide for existing workflows

## Acceptance Criteria

- [ ] All TODO implementations complete and tested
- [ ] Sub-workflow executor integrated into execution path
- [ ] Nested templates work correctly
- [ ] Template validation catches common errors
- [ ] Caching improves performance measurably
- [ ] MapReduce workflows support templates
- [ ] Template debugging mode works
- [ ] 10+ example templates provided
- [ ] Complete user guide published
- [ ] Migration guide for existing workflows
- [ ] Architecture documentation updated
- [ ] API documentation complete
- [ ] Integration tests cover all features
- [ ] Performance benchmarks documented

## Technical Details

### Implementation Approach

#### 1. Complete Template Parameter Application

In `src/cook/workflow/composition/composer.rs`:

```rust
fn apply_template_params(
    &self,
    template: &mut ComposableWorkflow,
    params: &HashMap<String, Value>,
) -> Result<()> {
    // Apply parameters to commands
    for command in &mut template.config.commands {
        self.substitute_params_in_step(command, params)?;
    }

    // Apply parameters to environment variables
    if let Some(env) = &mut template.config.env {
        for (_key, value) in env.iter_mut() {
            *value = self.substitute_params_in_string(value, params)?;
        }
    }

    // Apply parameters to merge workflow
    if let Some(merge) = &mut template.config.merge {
        for command in &mut merge.commands {
            self.substitute_params_in_step(command, params)?;
        }
    }

    Ok(())
}

fn substitute_params_in_step(
    &self,
    step: &mut WorkflowStep,
    params: &HashMap<String, Value>,
) -> Result<()> {
    let param_regex = Regex::new(r"\$\{([^}]+)\}").unwrap();

    match step {
        WorkflowStep::Claude(cmd) => {
            *cmd = self.substitute_params_in_string(cmd, params)?;
        }
        WorkflowStep::Shell(cmd) => {
            *cmd = self.substitute_params_in_string(cmd, params)?;
        }
        WorkflowStep::GoalSeek { goal, validation, .. } => {
            *goal = self.substitute_params_in_string(goal, params)?;
            *validation = self.substitute_params_in_string(validation, params)?;
        }
        _ => {}
    }

    Ok(())
}

fn substitute_params_in_string(
    &self,
    text: &str,
    params: &HashMap<String, Value>,
) -> Result<String> {
    let param_regex = Regex::new(r"\$\{([^}]+)\}").unwrap();
    let mut result = text.to_string();

    for cap in param_regex.captures_iter(text) {
        let param_expr = &cap[1];

        // Support nested property access: ${item.location.file}
        let value = self.resolve_param_expression(param_expr, params)?;

        let placeholder = format!("${{{}}}", param_expr);
        result = result.replace(&placeholder, &value);
    }

    Ok(result)
}

fn resolve_param_expression(
    &self,
    expr: &str,
    params: &HashMap<String, Value>,
) -> Result<String> {
    // Split on dots for nested access
    let parts: Vec<&str> = expr.split('.').collect();

    let mut current = params
        .get(parts[0])
        .ok_or_else(|| anyhow!("Parameter '{}' not found", parts[0]))?;

    // Navigate nested structure
    for part in &parts[1..] {
        current = current
            .get(part)
            .ok_or_else(|| anyhow!("Property '{}' not found in parameter '{}'", part, expr))?;
    }

    // Convert to string
    Ok(match current {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => current.to_string(),
    })
}
```

#### 2. Implement Override Application

```rust
fn apply_overrides(
    &self,
    template: &mut ComposableWorkflow,
    overrides: &HashMap<String, Value>,
) -> Result<()> {
    for (key, value) in overrides {
        match key.as_str() {
            "commands" => {
                // Override commands array
                if let Value::Array(commands) = value {
                    template.config.commands = self.parse_commands(commands)?;
                }
            }

            "env" => {
                // Override environment variables
                if let Value::Object(env) = value {
                    let env_map: HashMap<String, String> = env
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_string()))
                        .collect();
                    template.config.env = Some(env_map);
                }
            }

            "merge" => {
                // Override merge workflow
                if let Value::Object(merge) = value {
                    template.config.merge = Some(self.parse_merge_config(merge)?);
                }
            }

            // Support dot notation for nested overrides
            key if key.contains('.') => {
                self.apply_nested_override(template, key, value)?;
            }

            _ => {
                tracing::warn!("Unknown override key: {}", key);
            }
        }
    }

    Ok(())
}

fn apply_nested_override(
    &self,
    template: &mut ComposableWorkflow,
    path: &str,
    value: &Value,
) -> Result<()> {
    // Support paths like "commands[0].timeout"
    let parts: Vec<&str> = path.split('.').collect();

    // Implementation for nested path navigation
    // This would use similar logic to resolve_param_expression
    // but in reverse - setting values instead of getting them

    Ok(())
}
```

#### 3. Implement Selective Imports

```rust
fn import_selective(
    &self,
    target: &mut ComposableWorkflow,
    source: ComposableWorkflow,
    selective: &[String],
) -> Result<()> {
    for item in selective {
        // Check if it's a command name
        if let Some(command) = source
            .config
            .commands
            .iter()
            .find(|cmd| self.get_command_name(cmd) == item)
        {
            target.config.commands.push(command.clone());
            continue;
        }

        // Check if it's a sub-workflow name
        if let Some(workflows) = &source.workflows {
            if let Some(workflow) = workflows.get(item) {
                if target.workflows.is_none() {
                    target.workflows = Some(HashMap::new());
                }
                target
                    .workflows
                    .as_mut()
                    .unwrap()
                    .insert(item.clone(), workflow.clone());
                continue;
            }
        }

        // Check if it's a parameter
        if let Some(params) = &source.parameters {
            let found = params
                .required
                .iter()
                .chain(params.optional.iter())
                .any(|p| p.name == *item);

            if found {
                self.import_parameter(target, &source, item)?;
                continue;
            }
        }

        anyhow::bail!("Item '{}' not found in source workflow", item);
    }

    Ok(())
}

fn get_command_name(&self, step: &WorkflowStep) -> String {
    // Extract name from step (if it has an id or name field)
    match step {
        WorkflowStep::Claude(cmd) => cmd.clone(),
        WorkflowStep::Shell(cmd) => cmd.clone(),
        _ => String::new(),
    }
}
```

#### 4. Sub-Workflow Integration

In `src/cook/workflow/executor/step_executor.rs`:

```rust
pub async fn execute_step(
    &mut self,
    step: &WorkflowStep,
    context: &mut WorkflowContext,
) -> Result<StepResult> {
    match step {
        WorkflowStep::SubWorkflow { name, config } => {
            self.execute_sub_workflow(name, config, context).await
        }
        // ... existing step types ...
    }
}

async fn execute_sub_workflow(
    &mut self,
    name: &str,
    config: &SubWorkflow,
    context: &mut WorkflowContext,
) -> Result<StepResult> {
    let composer = Arc::new(WorkflowComposer::new(self.template_registry.clone()));
    let executor = SubWorkflowExecutor::new(composer);

    let result = executor
        .execute_sub_workflow(context, name, config)
        .await?;

    // Convert SubWorkflowResult to StepResult
    Ok(StepResult {
        success: result.success,
        output: result.logs.join("\n"),
        duration: result.duration,
        changes: vec![],
    })
}
```

#### 5. Template Caching

Add caching layer in `WorkflowComposer`:

```rust
pub struct WorkflowComposer {
    loader: WorkflowLoader,
    template_registry: Arc<TemplateRegistry>,
    resolver: DependencyResolver,
    cache: Arc<RwLock<HashMap<PathBuf, ComposedWorkflow>>>,
}

impl WorkflowComposer {
    pub async fn compose(
        &self,
        source: &Path,
        params: HashMap<String, Value>,
    ) -> Result<ComposedWorkflow> {
        // Check cache
        let cache_key = self.create_cache_key(source, &params);
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                tracing::debug!("Using cached composition for {:?}", source);
                return Ok(cached.clone());
            }
        }

        // Compose workflow
        let composed = self.compose_uncached(source, params).await?;

        // Cache result
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, composed.clone());
        }

        Ok(composed)
    }

    fn create_cache_key(
        &self,
        source: &Path,
        params: &HashMap<String, Value>,
    ) -> PathBuf {
        // Create unique key based on source path and params
        // For simplicity, just use source path (params could be hashed)
        source.to_path_buf()
    }
}
```

#### 6. Template Debugging Mode

Add to `WorkflowComposer`:

```rust
impl WorkflowComposer {
    pub fn with_debug_mode(mut self, debug: bool) -> Self {
        self.debug_mode = debug;
        self
    }

    pub async fn compose(
        &self,
        source: &Path,
        params: HashMap<String, Value>,
    ) -> Result<ComposedWorkflow> {
        if self.debug_mode {
            println!("🔍 Composing workflow from: {:?}", source);
            println!("   Parameters: {:?}", params);
        }

        // Load base workflow
        let mut workflow = self.loader.load(source).await?;

        if self.debug_mode {
            println!("   Uses composition: {}", workflow.uses_composition());
        }

        // ... composition steps with debug output ...

        if self.debug_mode {
            println!("✅ Composition complete");
            println!("   Commands: {}", workflow.config.commands.len());
            println!("   Sources: {:?}", metadata.sources);
        }

        Ok(ComposedWorkflow { workflow, metadata })
    }
}
```

### Example Templates

Create comprehensive examples in `templates/`:

#### 1. **refactor-workflow.yml**
```yaml
# Standard code refactoring workflow
parameters:
  required:
    - name: target
      type: string
      description: Target file or directory

  optional:
    - name: style
      type: string
      description: Refactoring style
      default: "functional"

commands:
  - claude: "/analyze ${target}"
  - claude: "/refactor ${target} --style ${style}"
  - shell: "cargo test"
  - shell: "cargo fmt && cargo clippy"
```

#### 2. **test-suite.yml**
```yaml
# Comprehensive testing workflow
parameters:
  required:
    - name: coverage_threshold
      type: number
      description: Minimum coverage percentage
      default: 80

commands:
  - shell: "cargo test"
  - shell: "cargo tarpaulin --out Lcov"
  - claude: "/analyze-coverage --threshold ${coverage_threshold}"
```

#### 3. **ci-pipeline.yml**
```yaml
# CI/CD pipeline workflow
imports:
  - path: ./test-suite.yml
    alias: tests

extends: base-ci

parameters:
  required:
    - name: environment
      type: string
      description: Target environment (dev, staging, prod)

commands:
  - shell: "cargo build --release"
  - template:
      name: tests
      source: test-suite
      with:
        coverage_threshold: 90
  - shell: "deploy-${environment}.sh"
```

#### 4. **mapreduce-template.yml**
```yaml
# Template for MapReduce workflows
parameters:
  required:
    - name: item_field
      type: string
      description: Field to process from each item

workflows:
  process_item:
    source: ./item-processor.yml
    parameters:
      field: "${item_field}"
    parallel: true
```

### Integration Tests

```rust
#[tokio::test]
async fn test_nested_templates() {
    // Template A uses Template B
    // Template B uses Template C
    // Verify composition works correctly
}

#[tokio::test]
async fn test_template_with_mapreduce() {
    // MapReduce workflow uses template
    // Verify agents inherit template parameters
}

#[tokio::test]
async fn test_template_caching() {
    // Compose same template twice
    // Verify second composition is faster
}

#[tokio::test]
async fn test_sub_workflow_execution() {
    // Template with sub-workflows
    // Verify execution order and data flow
}

#[tokio::test]
async fn test_parameter_override_precedence() {
    // CLI params > param file > template defaults
    // Verify correct precedence
}
```

### Documentation Structure

#### User Guide (`docs/templates.md`)

```markdown
# Workflow Templates

## Introduction
- What are templates?
- Why use templates?
- When to use templates?

## Getting Started
- Installing templates
- Your first template
- Using a template
- Passing parameters

## Template Features
- Parameters (required/optional)
- Defaults
- Imports
- Inheritance
- Sub-workflows
- Overrides

## Advanced Topics
- Nested templates
- Template composition
- Performance optimization
- Debugging templates

## Examples
- Common workflows
- Best practices
- Anti-patterns

## Reference
- Template file format
- CLI commands
- Error messages
```

### Performance Benchmarks

```rust
#[bench]
fn bench_simple_template_composition(b: &mut Bencher) {
    // Benchmark simple template with no imports
    // Target: < 50ms
}

#[bench]
fn bench_complex_template_composition(b: &mut Bencher) {
    // Benchmark template with imports + inheritance
    // Target: < 100ms
}

#[bench]
fn bench_cached_template_composition(b: &mut Bencher) {
    // Benchmark cached template access
    // Target: < 5ms
}

#[bench]
fn bench_parameter_substitution(b: &mut Bencher) {
    // Benchmark parameter substitution in commands
    // Target: < 1ms per command
}
```

## Dependencies

### Prerequisites
- Spec 131: Workflow Template Execution Layer
- Spec 132: Workflow Template CLI Interface

### Affected Components
- All workflow composition modules
- Workflow executor
- CLI commands
- Documentation

### External Dependencies
- None (uses existing crates)

## Testing Strategy

### Unit Tests
- All TODO implementations
- Parameter substitution edge cases
- Override application logic
- Selective import functionality
- Cache behavior

### Integration Tests
- End-to-end template workflows
- Nested template composition
- Sub-workflow execution
- MapReduce integration
- Error scenarios

### Performance Tests
- Composition benchmarks
- Caching effectiveness
- Sub-workflow overhead
- Parameter substitution cost

### User Acceptance Tests
- Example templates work correctly
- Documentation examples run successfully
- Migration guide accurate
- CLI usability

## Documentation Requirements

### User Documentation
1. Complete template user guide
2. CLI reference
3. Example library
4. Migration guide
5. Troubleshooting guide

### Developer Documentation
1. Template system architecture
2. Composition algorithm
3. API reference
4. Extension points
5. Performance characteristics

### Architecture Documentation
1. Update ARCHITECTURE.md
2. Add composition diagrams
3. Document data flows
4. Explain integration points

## Implementation Notes

### Completion Order

1. **Phase 1: Core Implementations**
   - Complete TODO methods
   - Add comprehensive error handling
   - Implement caching

2. **Phase 2: Integration**
   - Sub-workflow execution
   - MapReduce integration
   - Debugging mode

3. **Phase 3: Polish**
   - Example templates
   - Documentation
   - Performance optimization
   - User testing

### Design Decisions

1. **Caching Strategy**: Cache by file path, not content hash
   - Simpler implementation
   - Good enough for most use cases
   - Can enhance later if needed

2. **Parameter Resolution**: Support dot notation for nested access
   - Enables complex parameter structures
   - Matches common conventions
   - Easy to understand

3. **Override Semantics**: Last override wins
   - Clear, predictable behavior
   - Matches user expectations
   - Easy to document

## Migration and Compatibility

### Migration from Regular Workflows

No migration required - templates are opt-in:

1. Existing workflows continue to work
2. Templates can be adopted incrementally
3. Mix regular and template workflows

### Migration Guide Contents

```markdown
# Migrating to Templates

## When to Migrate
- Repeated workflow patterns
- Complex parameter needs
- Workflow composition requirements

## How to Migrate
1. Identify repeated patterns
2. Extract to template
3. Register template
4. Update workflows to use template

## Example Migration
[Before/after examples]

## Gotchas
- Parameter naming
- Path resolution
- Composition order
```

## Success Metrics

1. All TODO implementations complete
2. Test coverage > 90% for composition module
3. Template composition < 100ms (benchmarked)
4. 10+ example templates provided
5. Complete user guide published
6. Zero reported bugs in first month
7. User satisfaction > 4.5/5 (survey)
8. Documentation clarity rating > 4.0/5

## Future Enhancements

Post-implementation improvements:

1. **Template Marketplace**
   - Share templates publicly
   - Version management
   - Dependency resolution

2. **Template Linting**
   - Static analysis
   - Best practice checks
   - Security scanning

3. **Template Testing**
   - Built-in test framework
   - Mock parameters
   - Validation workflows

4. **Visual Template Editor**
   - GUI for template creation
   - Visual composition
   - Drag-and-drop parameters

5. **Template Analytics**
   - Usage tracking
   - Performance metrics
   - Popular templates
