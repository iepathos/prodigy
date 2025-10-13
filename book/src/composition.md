# Workflow Composition

Prodigy provides powerful composition features that enable building complex workflows from reusable components. This chapter covers importing workflows, using templates, defining parameters, and composing workflows through inheritance.

## Overview

Workflow composition allows you to:
- **Import** shared workflow configurations from other files
- **Extend** base workflows to inherit common configurations
- **Use templates** from a registry for standardized patterns
- **Define parameters** with type validation for flexible workflows
- **Execute sub-workflows** in parallel or sequentially
- **Set defaults** for common parameter values

These features promote code reuse, maintainability, and consistency across your automation workflows.

## Workflow Imports

Import external workflow files to reuse configurations and share common patterns across multiple workflows.

### Basic Import

Import an entire workflow file:

```yaml
name: my-workflow
imports:
  - path: ./common/utilities.yml
```

This imports all content from `utilities.yml` into your workflow.

### Import with Alias

Use an alias to reference imported workflows:

```yaml
imports:
  - path: ./common/utilities.yml
    alias: utils
```

The alias provides a namespace for the imported content, preventing naming conflicts.

### Selective Import

Import only specific items from a workflow file:

```yaml
imports:
  - path: ./common/validators.yml
    selective:
      - validate_output
      - validate_schema
```

Selective import configuration is validated and stored. The actual filtering of specific items during composition is planned for a future release (see `import_selective` in `composer.rs`).

### Multiple Imports

You can import from multiple files:

```yaml
imports:
  - path: ./common/setup.yml
    alias: setup
  - path: ./common/cleanup.yml
    alias: cleanup
  - path: ./shared/validators.yml
    selective:
      - validate_config
```

Imports are processed in order, and later imports can override earlier ones.

## Workflow Extension (Inheritance)

Extend a base workflow to inherit its configuration. Child workflows override parent values, allowing you to customize specific aspects while maintaining common configuration.

### Basic Extension

```yaml
name: production-workflow
extends: base-workflow
```

This workflow inherits all configuration from `base-workflow`.

### Base Resolution Paths

Prodigy searches for base workflows in the following locations (in order):
1. `./bases/<name>.yml`
2. `./templates/<name>.yml`
3. `./workflows/<name>.yml`
4. `./<name>.yml`

For example, `extends: base-workflow` will look for:
- `./bases/base-workflow.yml`
- `./templates/base-workflow.yml`
- `./workflows/base-workflow.yml`
- `./base-workflow.yml`

### Override Behavior

Child workflows override parent configuration:

```yaml
# bases/base-workflow.yml
name: base
commands:
  - shell: "echo 'Setup'"
  - shell: "echo 'Base task'"
defaults:
  timeout: 300
  verbose: false

# production.yml
name: production
extends: base-workflow
commands:
  - shell: "echo 'Production task'"  # Overrides base commands
defaults:
  verbose: true  # Overrides base default
```

In inheritance, child values completely replace parent values at the field level. If the child has a `commands` array, it replaces the entire parent `commands` array (commands are not merged). Individual fields in `defaults` are merged, with child values overriding parent values only for those specific keys. In this example, the `production` workflow completely replaces the `commands` array and overrides the `verbose` default, while inheriting the `timeout` default.

## Template System

Templates provide reusable workflow patterns that can be instantiated with different parameters. Templates can be stored in a registry or loaded from files.

### Template Sources

Templates can come from three sources:

#### 1. Registry Templates

Store templates in a central registry (fully implemented with `FileTemplateStorage`):

```yaml
name: my-workflow
template:
  name: refactor-base
  source: refactor-base  # Registry name
  with:
    style: modular
    target: src/
```

Templates are loaded from the registry using `TemplateRegistry` with `FileTemplateStorage` backend. Templates are cached in memory after first load for performance.

#### 2. File Templates

Load templates from local files (fully implemented via `TemplateSource::File` enum):

```yaml
template:
  name: ci-pipeline
  source: ./templates/ci-pipeline.yml
  with:
    environment: staging
```

#### 3. URL Templates (Planned)

Future support for remote templates:

```yaml
template:
  name: shared-workflow
  source: https://example.com/templates/workflow.yml
```

*Note: URL template sources are not yet implemented. The system returns an explicit error message if a URL source is attempted.*

### Template Parameters

Pass parameters to templates using the `with` field:

```yaml
template:
  name: refactor-base
  source: refactor-base
  with:
    style: functional
    max_complexity: 5
    target_dir: src/
```

Parameter validation and storage is fully implemented. The automatic substitution of parameters throughout template commands is planned for a future release (see `apply_template_params` in `composer.rs`).

### Template Overrides

Override specific template values:

```yaml
template:
  name: test-workflow
  source: test-base
  with:
    parallel: true
  override:
    timeout: 600
    retry_count: 5
```

The `override` field allows you to modify template values after parameter substitution.

### Template Registry Management

#### Registering Templates

Register a template for reuse:

```rust
use prodigy::cook::workflow::{ComposableWorkflow, TemplateRegistry};

let registry = TemplateRegistry::new();
let template = ComposableWorkflow::from_config(/* ... */);

registry.register_template("my-template".to_string(), template).await?;
```

#### Listing Templates

View all available templates:

```rust
let templates = registry.list().await?;
for template in templates {
    println!("{}: {} (v{})", template.name,
             template.description.unwrap_or_default(),
             template.version);
}
```

#### Searching by Tags

Find templates by tags:

```rust
let refactor_templates = registry.search_by_tags(&["refactor".to_string()]).await?;
```

#### Deleting Templates

Remove a template from the registry:

```rust
registry.delete("old-template").await?;
```

### Template Metadata

Templates can include metadata for better organization:

```rust
use prodigy::cook::workflow::composition::registry::TemplateMetadata;

let metadata = TemplateMetadata {
    description: Some("Refactoring workflow for Rust code".to_string()),
    author: Some("DevOps Team".to_string()),
    version: "2.0.0".to_string(),
    tags: vec!["refactor".to_string(), "rust".to_string()],
    created_at: chrono::Utc::now(),
    updated_at: chrono::Utc::now(),
};

registry.register_template_with_metadata(
    "refactor-rust".to_string(),
    template,
    metadata
).await?;
```

## Parameter Definitions

Define parameters with type validation to create flexible, reusable workflows.

### Parameter Types

Prodigy supports the following parameter types:
- `string` - Text values
- `number` - Numeric values
- `boolean` - True/false values
- `array` - List of values
- `object` - Structured data
- `any` - Any JSON value

### Required Parameters

Define parameters that must be provided:

```yaml
parameters:
  required:
    - name: target_file
      type: string
      description: File to process
    - name: iteration_count
      type: number
      description: Number of iterations to run
    - name: enabled
      type: boolean
      description: Enable feature flag
```

### Optional Parameters

Define optional parameters with default values:

```yaml
parameters:
  optional:
    - name: style
      type: string
      description: Processing style
      default: "functional"
    - name: timeout
      type: number
      description: Timeout in seconds
      default: 300
    - name: verbose
      type: boolean
      description: Enable verbose output
      default: false
```

### Parameter Validation

Prodigy supports two types of parameter validation:

#### Type Validation (Fully Implemented)

All parameter types are validated automatically:

```yaml
parameters:
  required:
    - name: count
      type: number
      description: Item count
    - name: enabled
      type: boolean
      description: Feature flag
    - name: files
      type: array
      description: File list
```

Type validation is fully implemented for all six parameter types (string, number, boolean, array, object, any) in the `validate_parameter_value` method.

#### Custom Validation Expressions (Planned)

Custom validation expressions can be defined but evaluation is not yet implemented:

```yaml
parameters:
  required:
    - name: email
      type: string
      description: Email address
      validation: "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"
    - name: count
      type: number
      description: Item count
      validation: "value >= 1 && value <= 100"
```

*Note: The validation field is stored but custom expression evaluation is not yet implemented. Type validation IS working for all parameter types.*

### Array and Object Parameters

Define complex parameter types:

```yaml
parameters:
  required:
    - name: files
      type: array
      description: List of files to process
    - name: config
      type: object
      description: Configuration object
  optional:
    - name: tags
      type: array
      description: Tags to apply
      default: []
```

### Type Validation

Prodigy automatically validates parameter types:

```yaml
# Valid
parameters:
  target_file: "src/main.rs"
  iteration_count: 5
  enabled: true

# Invalid - type mismatch
parameters:
  target_file: "src/main.rs"
  iteration_count: "five"  # Error: expected number, got string
  enabled: true
```

### Parameter Substitution

Use parameters in commands with `${parameter_name}` syntax:

```yaml
parameters:
  required:
    - name: target_dir
      type: string
      description: Target directory
commands:
  - shell: "cd ${target_dir} && cargo build"
  - claude: "/analyze ${target_dir}"
```

## Default Values

Set default parameter values at the workflow level:

```yaml
defaults:
  timeout: 300
  retry_count: 3
  verbose: false
  environment: development
```

Default values are validated and stored in the workflow configuration. The automatic application of defaults to missing parameters is planned but not yet fully implemented (see `apply_defaults` in `composer.rs`).

When implemented, defaults will be applied before parameter validation and can be overridden by:
1. Values in the `parameters` section
2. Values passed at workflow invocation time
3. Template `override` fields

Defaults interact with parameters as follows:
- If a required parameter has a default, it's not strictly required
- Optional parameter defaults take precedence over workflow defaults
- Workflow defaults provide fallback values for any parameter

## Sub-Workflows

Execute child workflows as part of a parent workflow. Sub-workflows can run in parallel and have their own parameters and outputs.

*Implementation Status: Sub-workflow configuration, composition, and context management are fully implemented. Integration with the main workflow executor is in progress (see `execute_composed` in `sub_workflow.rs`).*

### Basic Sub-Workflow

```yaml
workflows:
  process_files:
    source: ./workflows/process.yml
    parameters:
      parallel: true
```

### Sub-Workflow Parameters

Pass parameters to sub-workflows:

```yaml
workflows:
  validate_code:
    source: ./workflows/validate.yml
    parameters:
      strict_mode: true
      max_warnings: 5
```

### Input/Output Mapping

Map inputs and outputs between parent and sub-workflows:

```yaml
workflows:
  transform_data:
    source: ./workflows/transform.yml
    inputs:
      source_files: "file_list"  # Map parent variable to sub-workflow input
      config: "transform_config"
    outputs:
      - processed_count  # Expose sub-workflow output to parent
      - error_log
```

### Parallel Execution

Run multiple sub-workflows in parallel:

```yaml
workflows:
  run_tests:
    source: ./workflows/test.yml
    parallel: true
    parameters:
      suite: unit

  run_linting:
    source: ./workflows/lint.yml
    parallel: true
```

Both workflows execute concurrently.

### Error Handling

Control sub-workflow error behavior:

```yaml
workflows:
  optional_task:
    source: ./workflows/optional.yml
    continue_on_error: true  # Parent continues if this fails

  critical_task:
    source: ./workflows/critical.yml
    continue_on_error: false  # Parent fails if this fails (default)
```

### Timeouts and Working Directory

Configure execution constraints:

```yaml
workflows:
  long_running:
    source: ./workflows/process.yml
    timeout: 600  # 10 minutes in seconds
    working_dir: ./build/  # Execute from this directory
```

## Composition Metadata

Prodigy tracks metadata about workflow composition for debugging and dependency analysis.

### Tracked Information

The composition system tracks:
- **Sources**: All workflow files involved in composition
- **Templates**: Templates used and their sources
- **Parameters**: Parameters applied during composition
- **Composed At**: Timestamp of composition
- **Dependencies**: Full dependency graph with types

### Dependency Types

The system tracks four types of dependencies:
- `Import` - Files imported with `imports:`
- `Extends` - Base workflows referenced with `extends:`
- `Template` - Templates used with `template:`
- `SubWorkflow` - Sub-workflows defined in `workflows:`

### Dependency Graph

Prodigy builds a complete dependency graph during composition:

```rust
use prodigy::cook::workflow::{WorkflowComposer, ComposedWorkflow};

let composed: ComposedWorkflow = composer.compose(&source, params).await?;

// Access composition metadata
println!("Sources: {:?}", composed.metadata.sources);
println!("Templates: {:?}", composed.metadata.templates);
println!("Dependencies: {:?}", composed.metadata.dependencies);
```

### Circular Dependency Detection

The composer automatically detects circular dependencies:

```yaml
# workflow-a.yml
name: workflow-a
extends: workflow-b

# workflow-b.yml
name: workflow-b
extends: workflow-a  # Error: circular dependency detected
```

This prevents infinite loops during composition.

## Complete Examples

### Example 1: Basic Import with Alias

```yaml
name: integration-test
imports:
  - path: ./common/setup.yml
    alias: setup
  - path: ./common/assertions.yml
    alias: assert

commands:
  - shell: "npm install"
  - shell: "npm test"
```

### Example 2: Selective Import

```yaml
name: validation-workflow
imports:
  - path: ./validators/all.yml
    selective:
      - validate_yaml
      - validate_json
      - validate_toml

commands:
  - shell: "find . -name '*.yml' -exec validate {}"
```

### Example 3: Template from Registry

```yaml
name: refactor-project
template:
  name: rust-refactor
  source: rust-refactor  # From registry
  with:
    max_complexity: 5
    style: functional
    target: src/

commands:
  - claude: "/analyze-debt"
```

### Example 4: Template with Override

```yaml
name: custom-ci
template:
  name: ci-base
  source: ./templates/ci.yml
  with:
    environment: production
  override:
    timeout: 1800
    retry_count: 3
```

### Example 5: Workflow with Parameters

```yaml
name: deploy-service
parameters:
  required:
    - name: environment
      type: string
      description: Deployment environment
      validation: "^(dev|staging|prod)$"
    - name: version
      type: string
      description: Version to deploy
  optional:
    - name: rollback_on_error
      type: boolean
      description: Auto-rollback on deployment failure
      default: true

commands:
  - shell: "deploy --env ${environment} --version ${version}"
  - shell: "verify-deployment ${environment}"
```

### Example 6: Complex Composition

```yaml
name: full-pipeline
extends: base-pipeline

imports:
  - path: ./common/docker.yml
    alias: docker
  - path: ./common/kubernetes.yml
    alias: k8s

template:
  name: ci-cd-base
  source: ci-cd-base
  with:
    registry: docker.io
    namespace: my-app

parameters:
  required:
    - name: branch
      type: string
      description: Git branch
    - name: tag
      type: string
      description: Docker tag
  optional:
    - name: skip_tests
      type: boolean
      description: Skip test execution
      default: false

defaults:
  timeout: 600
  retry_count: 2

workflows:
  run_tests:
    source: ./workflows/test.yml
    parallel: true
    parameters:
      suite: all
    continue_on_error: false

  build_image:
    source: ./workflows/docker-build.yml
    parameters:
      tag: "${tag}"
    timeout: 900

commands:
  - shell: "echo 'Pipeline completed for ${branch}'"
```

### Example 7: Sub-Workflow Execution

```yaml
name: multi-stage-pipeline
workflows:
  stage1_build:
    source: ./workflows/build.yml
    parameters:
      target: release
    outputs:
      - artifact_path

  stage2_test:
    source: ./workflows/test.yml
    parallel: true
    inputs:
      artifact: "artifact_path"
    continue_on_error: false

  stage3_deploy:
    source: ./workflows/deploy.yml
    inputs:
      artifact: "artifact_path"
    timeout: 600
    working_dir: ./deployment/

commands:
  - shell: "echo 'All stages completed'"
```

### Example 8: Parameterized Template

```yaml
name: analyze-codebase
template:
  name: code-analysis
  source: code-analysis
  with:
    language: rust
    strictness: high
    output_format: json
  override:
    max_parallel: 4

parameters:
  required:
    - name: target_dir
      type: string
      description: Directory to analyze
  optional:
    - name: exclude_patterns
      type: array
      description: Patterns to exclude
      default: ["target/", "node_modules/"]

commands:
  - claude: "/analyze ${target_dir} --exclude ${exclude_patterns}"
```

## Best Practices

### When to Use Each Feature

- **Imports**: Share common configurations (environment variables, setup steps)
- **Extends**: Create workflow hierarchies with base configurations
- **Templates**: Standardize workflows across projects and teams
- **Parameters**: Make workflows flexible and reusable
- **Sub-workflows**: Break complex workflows into manageable pieces

### Organizing Reusable Workflows

Create a clear directory structure:

```
project/
├── workflows/          # Main workflows
├── bases/             # Base workflows for inheritance
├── templates/         # Template workflows
└── common/            # Shared configurations for import
    ├── setup.yml
    ├── cleanup.yml
    └── validators.yml
```

### Template Registry Management

- Use descriptive template names
- Version templates semantically (1.0.0, 2.0.0)
- Tag templates for discoverability (e.g., "refactor", "ci", "test")
- Document required parameters in template descriptions
- Test templates before registering them

#### Template Registry Setup

Place templates in a central `templates/` directory in your project. Use `FileTemplateStorage` with your project's template directory:

```rust
use prodigy::cook::workflow::composition::registry::{TemplateRegistry, FileTemplateStorage};

let storage = FileTemplateStorage::new("./templates");
let registry = TemplateRegistry::new_with_storage(storage);
```

Templates are cached in memory after first load for performance. The registry automatically scans the template directory and makes all templates available by name.

### Avoiding Circular Dependencies

- Keep inheritance chains shallow (max 2-3 levels)
- Use imports for shared utilities, not mutual dependencies
- Design base workflows to be self-contained
- Document dependency relationships

### Testing Composed Workflows

Test workflows at each composition level:

1. **Unit level**: Test individual workflows
2. **Integration level**: Test workflows with imports
3. **Composition level**: Test fully composed workflows with all features

### Naming Conventions

Use consistent naming:
- Base workflows: `base-{purpose}.yml`
- Templates: `{category}-{purpose}.yml`
- Shared utilities: `{function}-utils.yml`
- Parameters: Use snake_case (`target_file`, not `targetFile`)

## Troubleshooting

### Circular Dependency Errors

**Error**: `Circular dependency detected in workflow composition`

**Cause**: Workflows reference each other in a loop

**Solution**: Review your `extends` and `imports` to break the cycle:
```yaml
# Bad: A extends B, B extends A
# Good: A and B both extend base-workflow
```

### Template Not Found Errors

**Error**: `Template 'my-template' not found in registry`

**Cause**: Template doesn't exist or hasn't been registered

**Solution**:
1. List available templates: `registry.list().await?`
2. Register the template before using it
3. Check template name spelling

### Parameter Validation Failures

**Error**: `Required parameter 'target' not provided`

**Cause**: Missing required parameter

**Solution**: Provide all required parameters:
```yaml
parameters:
  target: "src/"
  iterations: 5
```

**Error**: `Type mismatch: expected Number, got String`

**Cause**: Parameter value doesn't match declared type

**Solution**: Ensure parameter types match:
```yaml
# Wrong: count: "5"
# Right: count: 5
```

### Import Path Resolution

**Error**: `Failed to load import from "./common/utils.yml"`

**Cause**: Import path doesn't exist

**Solution**:
1. Use paths relative to workflow file
2. Verify file exists at the specified location
3. Check file permissions

### Base Workflow Not Found

**Error**: `Base workflow 'base' not found`

**Cause**: Base workflow not in standard locations

**Solution**: Place base workflow in one of:
- `./bases/base.yml`
- `./templates/base.yml`
- `./workflows/base.yml`
- `./base.yml`

Or use a full path in `extends`.

## Related Chapters

- [Workflow Basics](workflow-basics.md) - Fundamental workflow concepts
- [Commands](commands.md) - Command types and execution
- [Variables](variables.md) - Variable interpolation and substitution
- [Examples](examples.md) - Additional workflow examples

## Further Reading

- Source code: `src/cook/workflow/composition/`
- Integration tests: `tests/workflow_composition_test.rs`
- Template registry: `src/cook/workflow/composition/registry.rs`
- Dependency resolution: `src/cook/workflow/composition/composer.rs`
