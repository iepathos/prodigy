# Workflow Composition

Prodigy enables building complex workflows from reusable components using imports, inheritance, templates, and sub-workflows. Compose workflows to reduce duplication and improve maintainability.

## Overview

Workflow composition provides:
- **Imports** - Import external workflow definitions
- **Inheritance** - Extend base workflows with overrides
- **Templates** - Reusable workflow templates with parameters
- **Parameters** - Type-safe workflow parameterization
- **Sub-workflows** - Nested workflow execution

## Imports

Import external workflow definitions:

```yaml
name: my-workflow
imports:
  - path: "common/setup.yml"
    alias: setup
  - path: "common/deploy.yml"
    alias: deploy

steps:
  - use: setup.prepare
  - shell: "cargo build --release"
  - use: deploy.production
```

## Inheritance

Extend base workflows and override specific values:

```yaml
name: production-deploy
extends: "base-deploy.yml"

env:
  PROFILE: prod
  API_URL: "https://api.production.com"

steps:
  - shell: "cargo build --release"  # Override base steps
```

Base workflow (`base-deploy.yml`):

```yaml
name: base-deploy
env:
  PROFILE: dev
  API_URL: "http://localhost:3000"

steps:
  - shell: "cargo build"
  - shell: "deploy.sh"
```

## Templates

Create reusable workflow templates with parameters:

```yaml
# templates/test-workflow.yml
name: test-workflow
parameters:
  - name: test_suite
    type: string
    required: true
  - name: parallel
    type: number
    default: 4

steps:
  - shell: "cargo test ${test_suite} -j ${parallel}"
```

Use template:

```yaml
name: run-tests
steps:
  - use_template: "templates/test-workflow.yml"
    parameters:
      test_suite: "integration"
      parallel: 8
```

## Parameters

Define typed parameters for workflows:

```yaml
parameters:
  - name: environment
    type: string
    required: true
    validation: "${environment} in ['dev', 'staging', 'prod']"

  - name: version
    type: string
    default: "latest"

  - name: enable_tests
    type: boolean
    default: true

steps:
  - shell: "deploy.sh --env ${environment} --version ${version}"
  - shell: "cargo test"
    when: "${enable_tests} == true"
```

### Parameter Types

- `string` - Text values
- `number` - Numeric values
- `boolean` - True/false flags
- `array` - Lists of values
- `object` - Structured data

## Sub-Workflows

Execute nested workflows independently:

```yaml
name: main-workflow
sub_workflows:
  prepare:
    steps:
      - shell: "npm install"
      - shell: "cargo build"

  test:
    steps:
      - shell: "cargo test"
      - shell: "cargo clippy"

steps:
  - run: prepare
  - run: test
  - shell: "deploy.sh"
```

## Template Registry

Store templates in a central registry:

```bash
# Add template to registry
prodigy template add my-template ./templates/my-template.yml

# Use from registry
prodigy run --template my-template --params environment=prod
```

## Template Sources

Templates can be loaded from:
- **Local files** - File system paths
- **Registry** - Template registry storage
- **Remote URLs** - Git repositories or HTTP endpoints

Example:

```yaml
steps:
  - use_template: "github.com/org/repo/workflow.yml"
    parameters:
      config: "production"
```

## Override Mechanisms

Override template values:

```yaml
steps:
  - use_template: "templates/deploy.yml"
    parameters:
      environment: prod
    overrides:
      env:
        API_KEY: "${PROD_API_KEY}"
      steps:
        - shell: "custom-deploy.sh"  # Override template steps
```

## See Also

- [Workflow Structure](../workflow-basics/workflow-structure.md) - Basic workflow syntax
- [Parameters](../workflow-basics/variables.md) - Variable system
- [Examples](../reference/examples.md) - Composition examples
