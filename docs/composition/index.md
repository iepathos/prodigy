# Workflow Composition

Prodigy provides powerful composition features that enable building complex workflows from reusable components. This chapter covers importing workflows, using templates, defining parameters, and composing workflows through inheritance.

> **⚠️ Implementation Status**
>
> Workflow composition is currently in **phased implementation**. The core composition engine and template system are fully implemented and tested, but integration with workflow execution varies by feature:
>
> **✅ What Works Today:**
> - Template management via `prodigy template` CLI commands (register, list, show, delete, etc.)
> - Programmatic workflow composition using `WorkflowComposer` API
> - Parameter validation with type checking
> - Template registry storage and retrieval (`~/.prodigy/templates/`)
>
> **⏳ Limited Integration:**
> - Using imports, extends, and templates in `prodigy run` workflows (detection works, execution integration is limited)
> - Composable workflow detection and parsing (functional but not extensively tested end-to-end)
>
> **❌ Not Yet Implemented:**
> - Sub-workflow execution (types defined, executor is placeholder)
> - MapReduce workflow composition
> - `prodigy compose` command
>
> See the [Implementation Roadmap](#implementation-roadmap) section below for details.

## Overview

Workflow composition allows you to:
- **Import** shared workflow configurations from other files
- **Extend** base workflows to inherit common configurations
- **Use templates** from a registry for standardized patterns
- **Define parameters** with type validation for flexible workflows
- **Execute sub-workflows** in parallel or sequentially (planned)
- **Set defaults** for common parameter values

These features promote code reuse, maintainability, and consistency across your automation workflows.

### When to Use Composition

Composition features are most valuable when:

1. **Multiple projects share common workflow patterns** - Standardize CI/CD, deployment, or testing workflows across teams
2. **Workflows need environment-specific parameterization** - Same workflow logic with different configurations for dev/staging/prod
3. **Building a library of reusable components** - Create organizational workflow templates for consistent practices

For simple, project-specific workflows, direct YAML without composition is often clearer and easier to maintain.

## Workflow Imports

Import external workflow files to reuse configurations and share common patterns across multiple workflows. Imports allow you to reference workflows from other files and incorporate them into your current workflow with optional aliasing and selective field imports.

> **📝 Note on Usage**
>
> The examples below show composition syntax in workflow YAML files. While the core composition logic is fully implemented, integration with `prodigy run` is limited. For production use today, the recommended approach is using the [Template System](#template-system-cli) via `prodigy template` commands.
>
> The syntax shown is validated and supported by the composition engine but may have limited end-to-end testing in workflow execution. See [Implementation Roadmap](#implementation-roadmap) for current status.

### Basic Import Syntax

```yaml
name: my-workflow
mode: standard

imports:
  # Simple import - loads entire workflow
  - path: "workflows/common-setup.yml"

  # Import with alias for namespacing
  - path: "workflows/deployment.yml"
    alias: "prod-deploy"

  # Selective import - only import specific workflows
  - path: "workflows/utilities.yml"
    selective:
      - "test-runner"
      - "linter"
```

### Import Fields

Each import can specify (defined in `WorkflowImport` struct, src/cook/workflow/composition/mod.rs:52-65):
- **path** (required): Relative or absolute path to workflow file
- **alias** (optional): Namespace alias for imported workflows
- **selective** (optional): List of specific workflow names to import

**Source**: `WorkflowImport` struct in src/cook/workflow/composition/mod.rs:52-65
**Test example**: tests/workflow_composition_test.rs:95-106 shows import usage with both alias and selective fields

### How Imports Work

When a workflow is imported:
1. The external file is loaded and parsed
2. If an alias is specified, imported content is namespaced under that alias
3. If selective is specified, only named workflows are included
4. Imported workflows are merged into the current workflow's configuration
5. Circular dependencies are detected and prevented

**Implementation**: Import processing in src/cook/workflow/composition/composer.rs:98-133 (`process_imports` function)
**Circular dependency detection**: src/cook/workflow/composition/composer.rs:56 and validation in `validate_composition` (lines 259-273)

### Use Cases

**Shared Setup Steps:**
```yaml
# common-setup.yml
setup:
  - shell: "npm install"
  - shell: "cargo build"

# main-workflow.yml
imports:
  - path: "common-setup.yml"

name: integration-tests
mode: standard
# Inherits setup steps from common-setup.yml
```

**Namespace Isolation:**
```yaml
imports:
  - path: "prod-workflows.yml"
    alias: "production"
  - path: "staging-workflows.yml"
    alias: "staging"

# Reference as ${production.deploy} vs ${staging.deploy}
```

**Selective Imports:**
```yaml
# Only import specific utilities, not entire file
imports:
  - path: "workflows/all-utilities.yml"
    selective:
      - "lint"
      - "format"
      - "test"
```

For more advanced composition patterns, see the [Template System](template-system.md) and [Workflow Extension](workflow-extension-inheritance.md) sections.

## Template System CLI

While full workflow composition integration is in progress, the template management system is **fully functional** and ready for production use. Templates provide a practical way to reuse workflow patterns today.

### Managing Templates

The `prodigy template` commands provide complete template lifecycle management:

**Register a template:**
```bash
prodigy template register workflow.yml --name my-template \
  --description "CI pipeline for Rust projects" \
  --version 1.0.0 \
  --tags rust,ci,testing \
  --author "team@example.com"
```

**List available templates:**
```bash
# List all templates
prodigy template list

# Long format with details
prodigy template list --long

# Filter by tag
prodigy template list --tag rust
```

**Show template details:**
```bash
prodigy template show my-template
```

**Search templates:**
```bash
prodigy template search "rust ci"
```

**Delete a template:**
```bash
prodigy template delete my-template
```

**Validate template syntax:**
```bash
prodigy template validate workflow.yml
```

**Initialize from template:**
```bash
prodigy template init my-template --output new-workflow.yml
```

### Template Storage

Templates are stored in `~/.prodigy/templates/` with the following structure:

```
~/.prodigy/templates/
├── my-template.yml
├── ci-pipeline.yml
├── deployment.yml
└── metadata/
    ├── my-template.json
    ├── ci-pipeline.json
    └── deployment.json
```

**Implementation**: See [Template System](template-system.md) section for detailed template syntax and usage patterns.

**Source**: Template CLI implementation in `src/cli/template.rs` (388 lines), wired in `src/cli/router.rs:199-234`

## Implementation Roadmap

This section clarifies what's implemented, what's in progress, and what's planned for workflow composition features.

### Phase 1: Core Composition Engine (✅ Complete)

The foundational composition system is fully implemented and tested:

**Core Types and Logic:**
- `WorkflowComposer` - Main composition orchestration (`src/cook/workflow/composition/composer.rs`, 986 lines)
- `TemplateRegistry` - Template storage and retrieval (`src/cook/workflow/composition/registry.rs`, 779 lines)
- `ComposableWorkflow` - Type system with validation (`src/cook/workflow/composition/mod.rs`, 334 lines)
- Parameter validation with type checking
- Circular dependency detection
- Template parameter interpolation

**Quality Metrics:**
- 2,300+ lines of core composition code
- 100+ unit tests
- Zero `unwrap()` calls in production code
- Full async/await support with tokio
- Comprehensive error handling with `Result<T>`

**Test Coverage:**
- `tests/workflow_composition_test.rs` - Integration tests with real workflows
- Unit tests in each composition module
- Parameter validation edge cases
- Import circular dependency scenarios

### Phase 2: CLI and Template Management (✅ Complete)

Template management commands are fully functional and production-ready:

**Template CLI Commands** (`src/cli/template.rs`, 388 lines):
- ✅ `prodigy template register` - Register templates with metadata
- ✅ `prodigy template list` - List templates with filtering
- ✅ `prodigy template show` - Display template details
- ✅ `prodigy template delete` - Remove templates
- ✅ `prodigy template search` - Search by name/tags
- ✅ `prodigy template validate` - Validate template syntax
- ✅ `prodigy template init` - Initialize from template

**Template Storage:**
- File-based storage in `~/.prodigy/templates/`
- Metadata tracking (version, author, tags, timestamps)
- Template caching for performance

**CLI Integration:**
- Commands wired in `src/cli/router.rs:199-234`
- Argument parsing in `src/cli/args.rs:333-907`
- Proper error handling and user feedback

### Phase 3: Workflow Execution Integration (⏳ Partial)

Integration with workflow execution has limited implementation:

**What's Implemented:**
- Composable workflow detection (`src/cook/workflow/composer_integration.rs:43-90`)
- Workflow parsing and conversion to `WorkflowConfig`
- Integration point in workflow loading (`src/cook/mod.rs:438-442`)
- Parameter passing via `--param` and `--param-file` flags

**What's Limited:**
- End-to-end testing of composition in `prodigy run` workflows
- MapReduce workflow composition (not implemented)
- Sub-workflow execution (executor is placeholder, `src/cook/workflow/composition/sub_workflow.rs:233-239`)

**Detection Logic:**
```rust
// From src/cook/workflow/composer_integration.rs
pub fn is_composable_workflow(yaml: &str) -> bool {
    // Detects: imports, template, extends, parameters
    yaml.contains("imports:")
        || yaml.contains("template:")
        || yaml.contains("extends:")
        || yaml.contains("parameters:")
}
```

**Integration Point:**
```rust
// From src/cook/mod.rs:438-442
if composer_integration::is_composable_workflow(&content) {
    let composable = composer_integration::parse_composable_workflow(&content)?;
    return Ok(composable.into());  // Converts to WorkflowConfig
}
```

### Phase 4: Advanced Features (❌ Not Implemented)

Features planned but not yet started:

- **Sub-Workflow Execution**: Types defined, executor needs implementation
- **MapReduce Composition**: Composition in MapReduce agent templates
- **`prodigy compose` Command**: Dedicated composition command for testing
- **URL-based Templates**: Load templates from remote URLs
- **Template Inheritance**: Templates extending other templates
- **Template Override Application**: Override fields during composition (structure exists, application logic TODO)

### Current Recommendations

**For Production Use Today:**

1. **Use `prodigy template` commands** for managing reusable workflows
2. **Register templates** in `~/.prodigy/templates/` for your team
3. **Use template parameters** for environment-specific configuration
4. **Keep workflows simple** unless you need heavy parameterization

**For Experimentation:**

1. **Try composable workflow syntax** in YAML files - detection and parsing work
2. **Report issues** if composition doesn't work as expected
3. **Contribute tests** for end-to-end composition scenarios
4. **Review Specs 131-133** for implementation progress tracking

**What to Avoid:**

1. ❌ Don't rely on sub-workflow execution (not implemented)
2. ❌ Don't use composition in MapReduce workflows (not supported)
3. ❌ Don't expect URL-based template loading (returns error)
4. ❌ Don't assume template override fields are applied (TODO)

### Contributing

The composition system has excellent code quality and test coverage, making it approachable for contributions:

**Good First Issues:**
- Implement sub-workflow executor (placeholder exists at `src/cook/workflow/composition/sub_workflow.rs:233-239`)
- Add end-to-end integration tests for composition in workflows
- Implement template override application (`apply_overrides` function)
- Add support for URL-based template loading

**Code Quality Standards:**
- No `unwrap()` in production code (use `?` operator with `Result`)
- Comprehensive error messages with context
- Unit tests for all new functionality
- Integration tests for user-facing features

**Source References:**
- **Specs**: Look for Spec 131-133 in project documentation
- **Core Implementation**: `src/cook/workflow/composition/`
- **CLI Integration**: `src/cli/template.rs`, `src/cli/router.rs`
- **Tests**: `tests/workflow_composition_test.rs`


## Additional Topics

See also:
- [Workflow Extension (Inheritance)](workflow-extension-inheritance.md)
- [Template System](template-system.md)
- [Parameter Definitions](parameter-definitions.md)
- [Default Values](default-values.md)
- [Sub-Workflows](sub-workflows.md)
- [Composition Metadata](composition-metadata.md)
- [Complete Examples](complete-examples.md)
- [Troubleshooting](troubleshooting.md)
