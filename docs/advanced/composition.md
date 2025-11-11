# Workflow Composition

Prodigy enables building complex workflows from reusable components through imports, inheritance, templates, and parameters.

## Overview

Workflow composition features:
- **Imports**: Include external workflow definitions
- **Inheritance**: Extend base workflows with `extends`
- **Templates**: Reusable workflow templates with parameters
- **Parameters**: Type-safe workflow parameterization
- **Sub-workflows**: Nested workflow execution with result passing

## Imports

Import workflow definitions from external files:

```yaml
# main-workflow.yml
name: main-workflow

imports:
  - path: "common/ci-steps.yml"
    alias: ci
  - path: "common/deploy-steps.yml"
    alias: deploy

- shell: "cargo build"
- workflow: ci.test-suite
- workflow: deploy.to-staging
```

### Import Syntax

```yaml
imports:
  - path: "relative/path/to/workflow.yml"
    alias: workflow_name  # Optional alias for referencing
```

## Inheritance

Extend base workflows to customize behavior:

```yaml
# base-workflow.yml
name: base-ci
env:
  RUST_BACKTRACE: "1"

- shell: "cargo build"
- shell: "cargo test"

---
# specialized-workflow.yml
name: specialized-ci
extends: "base-workflow.yml"

env:
  EXTRA_FLAG: "true"

# Adds additional steps to base workflow
- shell: "cargo clippy"
- shell: "cargo doc"
```

### Override Behavior

- Environment variables are merged (child overrides parent)
- Steps from parent executed first
- Child can override specific fields

## Templates

Create reusable workflow templates:

```yaml
# template: rust-ci-template
name: rust-ci
description: "Standard Rust CI workflow"

parameters:
  target:
    type: string
    required: true
    description: "Build target"
  coverage:
    type: boolean
    default: false
    description: "Run coverage analysis"

- shell: "cargo build --target ${target}"
- shell: "cargo test --target ${target}"

- shell: "cargo tarpaulin"
  when: "${coverage} == true"
```

### Using Templates

```yaml
name: my-project-ci
template: "rust-ci-template"

parameters:
  target: "x86_64-unknown-linux-gnu"
  coverage: true
```

### Template Storage

Templates can be stored in:
- **Local**: `.prodigy/templates/`
- **Registry**: `~/.prodigy/template-registry/`
- **Remote**: Git repositories or URLs

## Parameters

Define workflow parameters with type checking:

```yaml
parameters:
  environment:
    type: string
    required: true
    description: "Deployment environment"
    validation: "environment in ['dev', 'staging', 'prod']"

  replicas:
    type: number
    default: 3
    description: "Number of replicas"

  features:
    type: array
    default: []
    description: "Feature flags to enable"

  config:
    type: object
    required: false
    description: "Additional configuration"
```

### Parameter Types

- **string**: Text value
- **number**: Numeric value (integer or float)
- **boolean**: true/false
- **array**: List of values
- **object**: Nested key-value pairs

### Parameter Validation

```yaml
parameters:
  port:
    type: number
    validation: "port >= 1024 && port <= 65535"

  environment:
    type: string
    validation: "environment in ['dev', 'staging', 'prod']"
```

### Using Parameters

```yaml
parameters:
  environment:
    type: string
    required: true

- shell: "deploy.sh --env ${environment}"
- shell: "kubectl apply -f k8s/${environment}/"
```

## Sub-Workflows

Execute workflows within workflows:

```yaml
name: main-workflow

sub_workflows:
  build:
    name: build-subworkflow
    - shell: "cargo build --release"
    - shell: "cargo test --release"

  deploy:
    name: deploy-subworkflow
    parameters:
      environment: string
    - shell: "kubectl apply -f k8s/${environment}/"

# Execute sub-workflows
- workflow: build

- workflow: deploy
  parameters:
    environment: "staging"

# Access sub-workflow results
- shell: "echo Build completed: ${build.success}"
```

### Sub-Workflow Features

- Independent execution contexts
- Parameter passing between workflows
- Result capture and access
- Conditional execution

## Template Registry

Manage reusable templates centrally:

```bash
# Add template to registry
prodigy template add rust-ci ./templates/rust-ci.yml

# List available templates
prodigy template list

# Use template from registry
prodigy run --template rust-ci --param target=x86_64
```

### Registry Structure

```
~/.prodigy/template-registry/
├── rust-ci/
│   ├── template.yml
│   └── metadata.json
├── python-ci/
│   └── template.yml
└── deploy/
    └── template.yml
```

## Template Metadata

Templates can include metadata:

```yaml
name: rust-ci-template
version: "1.0.0"
description: "Standard Rust CI workflow with testing and linting"
author: "team@example.com"
tags: ["rust", "ci", "testing"]

parameters:
  # ... parameter definitions
```

## Examples

### Modular CI Pipeline

```yaml
# .prodigy/templates/common-ci.yml
name: common-ci-steps

- shell: "git fetch origin"
- shell: "git diff --name-only origin/main"
  capture_output: changed_files

---
# rust-ci.yml
name: rust-project-ci
extends: ".prodigy/templates/common-ci.yml"

imports:
  - path: ".prodigy/templates/rust-lint.yml"
    alias: lint

- shell: "cargo build"
- workflow: lint.clippy
- shell: "cargo test"
```

### Parameterized Deployment

```yaml
name: deploy-workflow

parameters:
  environment:
    type: string
    required: true
    validation: "environment in ['dev', 'staging', 'prod']"

  image_tag:
    type: string
    required: true
    description: "Docker image tag to deploy"

  replicas:
    type: number
    default: 3
    validation: "replicas >= 1 && replicas <= 20"

env:
  DEPLOY_ENV: "${environment}"
  IMAGE_TAG: "${image_tag}"
  REPLICAS: "${replicas}"

- shell: "kubectl set image deployment/app app=${IMAGE_TAG}"
- shell: "kubectl scale deployment/app --replicas=${REPLICAS}"
- shell: "kubectl rollout status deployment/app"
```

### Template with Sub-Workflows

```yaml
name: comprehensive-ci
description: "Full CI/CD pipeline with build, test, and deploy"

parameters:
  deploy_enabled:
    type: boolean
    default: false

sub_workflows:
  build:
    - shell: "cargo build --release"
    - shell: "docker build -t app:latest ."

  test:
    - shell: "cargo test"
    - shell: "cargo clippy"

  deploy:
    parameters:
      environment: string
    - shell: "kubectl apply -f k8s/${environment}/"

# Main workflow
- workflow: build
- workflow: test

- workflow: deploy
  when: "${deploy_enabled} == true"
  parameters:
    environment: "staging"
```

## See Also

- [Workflow Structure](../workflow-basics/workflow-structure.md) - Basic workflow syntax
- [Variables](../workflow-basics/variables.md) - Variable system for parameters
- [Templates Guide](../guides/workflow-templates.md) - Creating and using templates
