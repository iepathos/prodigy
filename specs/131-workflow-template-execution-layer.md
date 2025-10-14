---
number: 131
title: Workflow Template Execution Layer
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-10-13
---

# Specification 131: Workflow Template Execution Layer

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The workflow composition system exists with complete backend infrastructure (types, storage, registry, composer) and comprehensive test coverage (25 passing tests). However, the execution layer is completely missing - there's no integration between the template system and the actual workflow runtime. The `cook()` function and orchestrator do not use `WorkflowComposer`, and workflow loading only parses regular YAML without recognizing composable workflow syntax.

This creates a significant documentation drift where the book describes a complete, working template system, but users cannot actually use templates in real workflows.

## Objective

Integrate the existing workflow composition system into Prodigy's workflow execution path, enabling workflows to use templates, imports, inheritance, and parameters at runtime.

## Requirements

### Functional Requirements

1. **Workflow File Detection**
   - Detect composable workflow files (containing `template:`, `imports:`, `extends:`, or `workflows:` fields)
   - Distinguish between regular workflows and composable workflows during parsing
   - Support both formats seamlessly in the same workflow file loader

2. **Template Composition Integration**
   - Create `WorkflowComposer` instance in workflow loading path
   - Initialize `TemplateRegistry` with appropriate storage backend
   - Compose workflows before execution when composition features detected
   - Pass parameters from CLI or workflow file to composer

3. **Workflow Config Conversion**
   - Convert `ComposedWorkflow` to `WorkflowConfig` for execution
   - Preserve environment variables, secrets, and merge configurations
   - Flatten template hierarchies into executable command sequences
   - Maintain original workflow semantics after composition

4. **Parameter Substitution**
   - Implement parameter interpolation in commands (currently TODO)
   - Support `${param}` syntax in command strings
   - Apply template parameters before workflow execution
   - Validate all required parameters are provided

5. **Sub-Workflow Execution**
   - Execute sub-workflows defined in `workflows:` field
   - Support parallel and sequential sub-workflow execution
   - Handle sub-workflow inputs/outputs correctly
   - Propagate errors according to `continue_on_error` settings

### Non-Functional Requirements

1. **Backward Compatibility**
   - Existing non-composable workflows must continue to work unchanged
   - No breaking changes to workflow file format
   - Graceful fallback when templates not found

2. **Performance**
   - Template composition should not significantly impact startup time
   - Registry should cache loaded templates in memory
   - File I/O should be minimized during workflow loading

3. **Error Handling**
   - Clear error messages when templates not found
   - Validate template structure before execution
   - Report parameter validation errors with context
   - Handle circular dependencies gracefully

## Acceptance Criteria

- [ ] Workflow loader detects composable workflows by checking for composition fields
- [ ] `WorkflowComposer` is instantiated when composable workflow detected
- [ ] Templates from registry are successfully loaded and composed
- [ ] File-based templates (via `TemplateSource::File`) work correctly
- [ ] Workflow inheritance (`extends:`) applies correctly
- [ ] Imports (`imports:`) merge workflows as expected
- [ ] Parameters are validated and substituted in commands
- [ ] Sub-workflows execute in correct order (parallel/sequential)
- [ ] Composed workflows convert to `WorkflowConfig` correctly
- [ ] Environment variables and secrets pass through composition
- [ ] Error messages are clear when templates missing or invalid
- [ ] All existing non-composable workflows still work
- [ ] Integration test covering end-to-end template workflow execution
- [ ] Documentation updated with execution flow diagrams

## Technical Details

### Implementation Approach

#### 1. Workflow File Detection

Modify `load_playbook_with_mapreduce()` in `src/cook/mod.rs`:

```rust
async fn load_playbook_with_mapreduce(
    path: &Path,
) -> Result<(WorkflowConfig, Option<MapReduceWorkflowConfig>)> {
    let content = tokio::fs::read_to_string(path).await?;

    // Try to parse as YAML
    if is_yaml_file(path) {
        // Check if it's a MapReduce workflow
        if content.contains("mode: mapreduce") {
            return parse_mapreduce_workflow(path, &content).await;
        }

        // NEW: Check if it's a composable workflow
        if is_composable_workflow(&content) {
            return parse_composable_workflow(path, &content).await;
        }

        // Otherwise parse as regular workflow
        parse_regular_workflow(&content)
    } else {
        parse_json_workflow(&content)
    }
}

fn is_composable_workflow(content: &str) -> bool {
    content.contains("template:")
        || content.contains("imports:")
        || content.contains("extends:")
        || content.contains("workflows:")
        || content.contains("parameters:")
}
```

#### 2. Composable Workflow Parser

Add new function to handle composable workflows:

```rust
async fn parse_composable_workflow(
    path: &Path,
    content: &str,
) -> Result<(WorkflowConfig, Option<MapReduceWorkflowConfig>)> {
    // Parse as ComposableWorkflow
    let composable: ComposableWorkflow = serde_yaml::from_str(content)
        .with_context(|| format!("Failed to parse composable workflow: {}", path.display()))?;

    // Extract parameters from CLI or defaults
    let params = extract_workflow_parameters(&composable)?;

    // Initialize template registry
    let registry = Arc::new(create_template_registry()?);

    // Create composer
    let composer = WorkflowComposer::new(registry);

    // Compose the workflow
    let composed = composer
        .compose(path, params)
        .await
        .with_context(|| "Failed to compose workflow")?;

    // Convert to WorkflowConfig
    let workflow_config = convert_composed_to_config(composed)?;

    Ok((workflow_config, None))
}
```

#### 3. Template Registry Creation

```rust
fn create_template_registry() -> Result<TemplateRegistry> {
    // Look for templates in standard locations
    let template_dirs = vec![
        PathBuf::from("templates"),
        PathBuf::from(".prodigy/templates"),
        directories::ProjectDirs::from("com", "prodigy", "prodigy")
            .map(|dirs| dirs.data_dir().join("templates"))
            .unwrap_or_else(|| PathBuf::from("templates")),
    ];

    // Use first existing directory, or create default
    let template_dir = template_dirs
        .into_iter()
        .find(|dir| dir.exists())
        .unwrap_or_else(|| PathBuf::from("templates"));

    let storage = Box::new(FileTemplateStorage::new(template_dir));
    let registry = TemplateRegistry::with_storage(storage);

    Ok(registry)
}
```

#### 4. Composed Workflow Conversion

```rust
fn convert_composed_to_config(
    composed: ComposedWorkflow,
) -> Result<WorkflowConfig> {
    let workflow = composed.workflow;

    Ok(WorkflowConfig {
        commands: workflow.config.commands,
        env: workflow.config.env,
        secrets: workflow.config.secrets,
        env_files: workflow.config.env_files,
        profiles: workflow.config.profiles,
        merge: workflow.config.merge,
    })
}
```

#### 5. Parameter Extraction

```rust
fn extract_workflow_parameters(
    composable: &ComposableWorkflow,
) -> Result<HashMap<String, Value>> {
    let mut params = HashMap::new();

    // Start with defaults
    if let Some(defaults) = &composable.defaults {
        for (key, value) in defaults {
            params.insert(key.clone(), value.clone());
        }
    }

    // TODO: Override with CLI parameters when that's implemented
    // For now, just use defaults

    // Validate required parameters
    composable.validate_parameters(&params)?;

    Ok(params)
}
```

#### 6. Parameter Substitution in Commands

In `WorkflowComposer::apply_parameters()`:

```rust
fn apply_parameters(
    &self,
    workflow: &mut ComposableWorkflow,
    params: &HashMap<String, Value>,
) -> Result<()> {
    // Substitute parameters in all commands
    for command in &mut workflow.config.commands {
        substitute_parameters_in_step(command, params)?;
    }

    Ok(())
}

fn substitute_parameters_in_step(
    step: &mut WorkflowStep,
    params: &HashMap<String, Value>,
) -> Result<()> {
    // Use regex to find ${param} patterns
    let param_regex = Regex::new(r"\$\{([^}]+)\}").unwrap();

    match step {
        WorkflowStep::Claude(cmd) => {
            *cmd = substitute_params(&param_regex, cmd, params)?;
        }
        WorkflowStep::Shell(cmd) => {
            *cmd = substitute_params(&param_regex, cmd, params)?;
        }
        // Handle other step types
        _ => {}
    }

    Ok(())
}

fn substitute_params(
    regex: &Regex,
    text: &str,
    params: &HashMap<String, Value>,
) -> Result<String> {
    let mut result = text.to_string();

    for cap in regex.captures_iter(text) {
        let param_name = &cap[1];
        let value = params.get(param_name)
            .ok_or_else(|| anyhow!("Parameter '{}' not found", param_name))?;

        let value_str = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => value.to_string(),
        };

        result = result.replace(&format!("${{{}}}", param_name), &value_str);
    }

    Ok(result)
}
```

### Architecture Changes

1. **New Module**: `src/cook/workflow/composer_integration.rs`
   - Houses integration logic between composer and executor
   - Contains conversion functions
   - Manages template registry lifecycle

2. **Modified Module**: `src/cook/mod.rs`
   - Add composable workflow detection
   - Route to composer when needed
   - Handle both workflow types transparently

3. **Modified Module**: `src/cook/workflow/composition/composer.rs`
   - Complete TODO implementations for parameter substitution
   - Implement template parameter application
   - Implement override application logic

### Data Flow

```
User YAML File
    ↓
load_playbook_with_mapreduce()
    ↓
is_composable_workflow()?
    ├─ Yes → parse_composable_workflow()
    │           ↓
    │       WorkflowComposer.compose()
    │           ↓
    │       convert_composed_to_config()
    │           ↓
    │       WorkflowConfig
    └─ No → parse_regular_workflow()
                ↓
            WorkflowConfig
                ↓
            Executor.execute()
```

## Dependencies

### Prerequisites
- None (composition system already exists)

### Affected Components
- `src/cook/mod.rs` - Workflow loading
- `src/cook/workflow/composition/composer.rs` - TODO implementations
- `src/cook/workflow/composition/mod.rs` - Public API exposure

### External Dependencies
- None (uses existing crates)

## Testing Strategy

### Unit Tests

1. **Composable Workflow Detection**
   ```rust
   #[test]
   fn test_is_composable_workflow() {
       assert!(is_composable_workflow("template:\n  name: foo"));
       assert!(is_composable_workflow("imports:\n  - path: bar.yml"));
       assert!(!is_composable_workflow("commands:\n  - shell: test"));
   }
   ```

2. **Parameter Substitution**
   ```rust
   #[test]
   fn test_substitute_parameters() {
       let mut params = HashMap::new();
       params.insert("target".to_string(), Value::String("app.js".to_string()));

       let command = "claude: /refactor ${target}";
       let result = substitute_params(&command, &params).unwrap();

       assert_eq!(result, "claude: /refactor app.js");
   }
   ```

3. **Workflow Conversion**
   ```rust
   #[test]
   fn test_convert_composed_to_config() {
       let composed = create_test_composed_workflow();
       let config = convert_composed_to_config(composed).unwrap();

       assert_eq!(config.commands.len(), 2);
   }
   ```

### Integration Tests

1. **End-to-End Template Workflow**
   ```rust
   #[tokio::test]
   async fn test_execute_template_workflow() {
       // Create template
       let template = create_test_template();
       save_template("refactor-base", template);

       // Create workflow using template
       let workflow_yaml = r#"
       template:
         name: refactor-base
         source: refactor-base
         with:
           target: src/main.rs
           style: functional
       "#;

       // Load and execute
       let (config, _) = load_playbook_with_mapreduce(&workflow_path).await.unwrap();

       // Verify commands were composed correctly
       assert!(config.commands.len() > 0);
   }
   ```

2. **Template Inheritance**
   ```rust
   #[tokio::test]
   async fn test_workflow_inheritance() {
       // Create base and child workflows
       // Execute child
       // Verify base commands were inherited
   }
   ```

3. **Parameter Validation Errors**
   ```rust
   #[tokio::test]
   async fn test_missing_required_parameter() {
       let result = load_playbook_with_mapreduce(&workflow_path).await;

       assert!(result.is_err());
       assert!(result.unwrap_err().to_string().contains("Required parameter"));
   }
   ```

### Performance Tests

1. **Template Loading Performance**
   - Measure time to load and compose 100 templates
   - Verify caching reduces subsequent load times

2. **Workflow Composition Overhead**
   - Compare execution time: regular vs composed workflows
   - Ensure composition adds < 100ms to startup

## Documentation Requirements

### Code Documentation

1. Document composable workflow file format
2. Add rustdoc examples for `WorkflowComposer`
3. Document parameter substitution syntax
4. Add inline comments for complex composition logic

### User Documentation

1. Update workflow syntax guide with composition examples
2. Add template usage tutorial
3. Document template directory structure
4. Add troubleshooting section for template errors

### Architecture Updates

1. Add composition system to architecture diagram
2. Document data flow through composer
3. Explain template resolution process
4. Add sequence diagram for workflow loading

## Implementation Notes

### Gotchas

1. **Parameter Shadowing**: Template parameters may conflict with environment variables
   - Solution: Document precedence order clearly

2. **Circular Dependencies**: Templates can reference each other
   - Solution: Use existing `DependencyResolver` cycle detection

3. **Path Resolution**: Template files may use relative paths
   - Solution: Resolve relative to workflow file location

4. **Error Context**: Composition errors can be cryptic
   - Solution: Add context at each composition stage

### Best Practices

1. Validate workflows early (before composition)
2. Cache template registry throughout execution
3. Use detailed error messages with file/line context
4. Preserve original workflow structure in metadata

### Migration Path

This is purely additive - no migration needed. Existing workflows continue to work unchanged.

## Migration and Compatibility

### Backward Compatibility Guarantees

1. All existing workflow files work unchanged
2. No changes to command-line interface
3. No changes to workflow execution semantics
4. Template features are opt-in

### Forward Compatibility

1. Reserve composition keywords in workflow files
2. Design for future URL-based template sources
3. Allow for template versioning in future

## Success Metrics

1. Integration tests pass for template workflows
2. Existing workflows continue to pass all tests
3. Template composition completes in < 100ms
4. Error messages are clear and actionable
5. Documentation examples work as written
