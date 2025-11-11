# Workflow Composition

Prodigy enables building complex workflows from reusable components using templates, inheritance, imports, and parameters. Compose workflows to reduce duplication and improve maintainability.

!!! note "Implementation Status"
    Workflow composition features are implemented in the codebase with varying levels of CLI and runtime integration. This page documents both **available** features (fully integrated) and **planned** features (implemented but not yet exposed through CLI).

## Overview

Workflow composition provides:

- **Templates** ✅ - Reusable workflow templates with parameters (Available)
- **Inheritance** ⚙️ - Extend base workflows with overrides (Planned)
- **Parameters** ✅ - Type-safe workflow parameterization (Available)
- **Imports** ⚙️ - Import external workflow definitions (Planned)
- **Sub-workflows** ⚙️ - Nested workflow execution (Planned)

## How Composition Works

Workflows with composition features are automatically detected and processed during loading:

```rust
// Source: src/cook/workflow/composer_integration.rs:44-50
pub fn is_composable_workflow(content: &str) -> bool {
    content.contains("template:")
        || content.contains("imports:")
        || content.contains("extends:")
        || content.contains("workflows:")
        || content.contains("parameters:")
}
```

When a workflow file contains any of these keywords, it's parsed as a `ComposableWorkflow` and processed by the `WorkflowComposer` before execution. This happens transparently during the `prodigy run` command.

## Templates ✅

Templates are fully available for creating reusable workflow patterns with parameterization.

### Creating a Template

Define parameters and use them throughout your workflow:

```yaml
# Source: tests/workflow_composition_test.rs:48-64
name: test-workflow
parameters:
  required:
    - name: target_file
      type: string
      description: "File to process"

  optional:
    - name: style
      type: string
      description: "Processing style"
      default: "functional"

steps:
  - shell: "cargo test ${target_file}"
  - shell: "cargo clippy --fix ${target_file}"
```

### Template Registry Commands

Manage templates using the CLI:

```bash
# Register a template (add to registry)
# Source: src/cli/template.rs:29-72
prodigy template register my-template.yml --name my-template --version 1.0.0

# List all registered templates
prodigy template list

# Show template details
prodigy template show my-template

# Remove a template
prodigy template remove my-template
```

### Using Templates from Registry

Reference registered templates in your workflows:

```yaml
# Source: src/cook/workflow/composition/mod.rs:88-95
name: my-workflow
template:
  source: my-template  # From registry
  with:
    target_file: "src/main.rs"
    style: "imperative"
```

### Template Sources

Templates can be loaded from multiple sources:

```rust
// Source: src/cook/workflow/composition/mod.rs:88-95
pub enum TemplateSource {
    File(PathBuf),      // ✅ Available: Local file paths
    Registry(String),   // ✅ Available: Template registry
    Url(String),        // ⚙️ Planned: Remote URLs (not implemented)
}
```

**Available Sources:**

=== "Local File"
    ```yaml
    template:
      source: "./templates/my-template.yml"
      with:
        environment: "production"
    ```

=== "Registry"
    ```yaml
    template:
      source: "my-template"  # Looks up in registry
      with:
        environment: "staging"
    ```

!!! warning "Remote URLs Not Yet Supported"
    The `Url` template source returns an error:
    ```rust
    // Source: src/cook/workflow/composition/composer.rs:177-179
    TemplateSource::Url(url) => {
        anyhow::bail!("URL template sources not yet implemented: {}", url);
    }
    ```

### Template Overrides

Override specific workflow values when using templates:

```yaml
name: customized-workflow
template:
  source: "base-template"
  with:
    environment: "prod"
  override:
    env:
      API_KEY: "${PROD_API_KEY}"
    steps:
      - shell: "custom-deploy.sh"
```

## Parameters ✅

Define typed, validated parameters for workflows.

### Parameter Definition

```yaml
# Source: tests/workflow_composition_test.rs:48-64
parameters:
  required:
    - name: environment
      type: string
      description: "Deployment environment"

  optional:
    - name: version
      type: string
      description: "Version to deploy"
      default: "latest"

    - name: enable_tests
      type: boolean
      description: "Run tests before deploy"
      default: true

steps:
  - shell: "deploy.sh --env ${environment} --version ${version}"
  - shell: "cargo test"
    when: "${enable_tests} == true"
```

### Parameter Types

Supported parameter types:

```rust
// Source: src/cook/workflow/composition/mod.rs
pub enum ParameterType {
    String,   // Text values
    Number,   // Numeric values
    Boolean,  // True/false flags
    Array,    // Lists of values
    Object,   // Structured data
}
```

### Parameter Validation

!!! info "Validation Expressions - Planned Feature"
    Parameter validation expressions are defined in the data structure but not yet enforced:
    ```rust
    // Source: src/cook/workflow/composition/mod.rs:269
    // TODO: Implement actual validation expression evaluation
    ```

Current behavior validates:
- Required parameters are provided
- Parameter types match expected types
- Default values are applied

## Inheritance ⚙️

**Status: Planned** - Implementation exists but not exposed through CLI.

The `extends` feature allows workflows to inherit from base workflows:

```yaml
# Base workflow: base-deploy.yml
name: base-deploy
env:
  PROFILE: dev
  API_URL: "http://localhost:3000"

steps:
  - shell: "cargo build"
  - shell: "deploy.sh"
```

```yaml
# Extended workflow
name: production-deploy
extends: "base-deploy.yml"

env:
  PROFILE: prod
  API_URL: "https://api.production.com"

steps:
  - shell: "cargo build --release"  # Overrides base steps
```

!!! warning "Not Yet Available"
    The `extends` keyword is detected by `is_composable_workflow()` but the merge behavior is not yet fully implemented in the CLI workflow execution path.

## Imports ⚙️

**Status: Planned** - Data structures exist but import resolution not integrated.

Import external workflow definitions for reuse:

```yaml
# Source: tests/workflow_composition_test.rs:95-100
name: my-workflow
imports:
  - path: "./common/utilities.yml"
    alias: "utils"
  - path: "./common/deploy.yml"
    alias: "deploy"
```

!!! warning "Step-Level References Not Implemented"
    While imports can be defined, referencing imported workflows in steps using `use:` syntax is not yet supported:
    ```yaml
    # NOT YET SUPPORTED
    steps:
      - use: utils.setup
      - use: deploy.production
    ```
    The `WorkflowStep` structure does not include a `use` field for import references.

## Sub-Workflows ⚙️

**Status: Planned** - Structure defined but execution not implemented.

Define nested workflows for modular execution:

```yaml
# Source: tests/workflow_composition_test.rs (concept)
name: main-workflow
workflows:
  prepare:
    steps:
      - shell: "npm install"
      - shell: "cargo build"

  test:
    steps:
      - shell: "cargo test"
      - shell: "cargo clippy"
```

!!! warning "Invocation Syntax Not Implemented"
    While sub-workflows can be defined in the `ComposableWorkflow` structure, invoking them with `run:` is not yet supported:
    ```yaml
    # NOT YET SUPPORTED
    steps:
      - run: prepare
      - run: test
    ```
    The `WorkflowStep` parser does not recognize the `run:` field.

## Working Example

Here's a complete working example using available features:

```yaml
# my-template.yml - Register this first
name: deploy-template
parameters:
  required:
    - name: environment
      type: string
      description: "Target environment"

  optional:
    - name: skip_tests
      type: boolean
      default: false

steps:
  - shell: "cargo build --release"
  - shell: "cargo test"
    when: "${skip_tests} == false"
  - shell: "deploy.sh --env ${environment}"
```

```bash
# Register the template
prodigy template register my-template.yml --name deploy-template --version 1.0.0

# Create a workflow using the template
cat > deploy-prod.yml <<EOF
name: production-deployment
template:
  source: "deploy-template"
  with:
    environment: "production"
    skip_tests: false
EOF

# Run the composed workflow
prodigy run deploy-prod.yml
```

## Troubleshooting

### Template Not Found

```
Error: Template 'my-template' not found in registry
```

**Solution:** Register the template first with `prodigy template register` or use a file path source.

### Invalid Parameter Type

```
Error: Parameter 'count' expected type Number, got String
```

**Solution:** Ensure parameter values match their declared types in the parameter definitions.

### Composition Not Triggered

If your workflow with composition features isn't being processed:

1. Verify the workflow file contains composition keywords (`template:`, `parameters:`, etc.)
2. Check for YAML syntax errors: `yamllint workflow.yml`
3. Ensure the template source (file or registry name) is correct

## See Also

- [Workflow Structure](../workflow-basics/workflow-structure.md) - Basic workflow syntax
- [Variables](../workflow-basics/variables.md) - Variable interpolation system
- [Templates CLI Reference](../reference/cli.md#template-commands) - Template management commands
