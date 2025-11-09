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

Import external workflow files to reuse configurations and share common patterns across multiple workflows. Imports allow you to reference workflows from other files and incorporate them into your current workflow with optional aliasing and selective field imports.

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

Each import can specify:
- **path** (required): Relative or absolute path to workflow file
- **alias** (optional): Namespace alias for imported workflows
- **selective** (optional): List of specific workflow names to import

### How Imports Work

When a workflow is imported:
1. The external file is loaded and parsed
2. If an alias is specified, imported content is namespaced under that alias
3. If selective is specified, only named workflows are included
4. Imported workflows are merged into the current workflow's configuration
5. Circular dependencies are detected and prevented

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


## Additional Topics

See also:
- [Workflow Extension (Inheritance)](workflow-extension-inheritance.md)
- [Template System](template-system.md)
- [Parameter Definitions](parameter-definitions.md)
- [Default Values](default-values.md)
- [Sub-Workflows](sub-workflows.md)
- [Composition Metadata](composition-metadata.md)
- [Complete Examples](complete-examples.md)
- [Best Practices](best-practices.md)
- [Troubleshooting](troubleshooting.md)
- [Related Chapters](related-chapters.md)
- [Further Reading](further-reading.md)
