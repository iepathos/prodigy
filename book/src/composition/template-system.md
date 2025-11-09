## Template System

Templates provide reusable workflow patterns that can be instantiated with different parameters. Templates can be stored in a registry or loaded from files, enabling standardized workflows across teams and projects.

### Template Sources

Prodigy supports multiple template sources:

1. **File-based templates** - Load templates from local or shared file paths
2. **Registry templates** - Use templates from the local template registry (~/.prodigy/templates/)
3. **URL templates** - *(Planned)* Load templates from remote URLs

### Basic Template Usage

```yaml
name: my-deployment
mode: standard

template:
  # Load template from file
  source:
    file: "templates/ci-pipeline.yml"

  # Override template parameters
  with:
    environment: "production"
    version: "1.2.3"
```

### Template Structure

Templates are defined using the `WorkflowTemplate` struct:

```yaml
template:
  # Template name (for registry lookups)
  name: "standard-ci"

  # Template source (one of: file, registry, url)
  source:
    registry: "standard-ci"  # Load from registry
    # OR
    file: "path/to/template.yml"  # Load from file
    # OR (planned)
    url: "https://templates.example.com/ci.yml"  # Load from URL

  # Parameter values to pass to template
  with:
    param1: "value1"
    param2: "value2"

  # Override specific template fields
  override:
    timeout: 600
    max_parallel: 5
```

### Template Registry

The template registry stores reusable workflow templates in `~/.prodigy/templates/`. Templates are organized by name and can be referenced from any workflow.

**Registry Structure:**
```
~/.prodigy/templates/
├── standard-ci.yml
├── deployment-pipeline.yml
├── test-suite.yml
└── ...
```

**Using Registry Templates:**
```yaml
template:
  source:
    registry: "standard-ci"
  with:
    project_name: "my-project"
    test_command: "cargo test"
```

**Publishing to Registry:**
```bash
# Copy template to registry (manual approach)
cp my-template.yml ~/.prodigy/templates/my-template.yml

# Templates are automatically discovered by name
```

### Template Parameters

Templates can define parameters that are substituted when the template is instantiated. See [Parameter Definitions](parameter-definitions.md) for detailed parameter syntax and validation.

**Template with Parameters:**
```yaml
# templates/deployment.yml
name: deployment-template

parameters:
  required:
    - environment
    - version
  optional:
    - timeout

commands:
  - shell: "deploy --env ${environment} --version ${version}"
  - shell: "verify-deployment ${environment}"
```

**Using Parameterized Template:**
```yaml
template:
  source:
    file: "templates/deployment.yml"
  with:
    environment: "staging"
    version: "2.0.0"
    timeout: "300"
```

### Template Caching

Prodigy caches loaded templates to improve performance:
- Templates are loaded once and reused across workflow executions
- File-based templates are re-read if the file changes
- Registry templates are cached until registry is updated

### Use Cases

**Standardized CI/CD Pipelines:**
```yaml
# Use company-wide CI template
template:
  source:
    registry: "company-ci-pipeline"
  with:
    project_type: "rust"
    test_coverage: "80"
    deploy_targets: ["staging", "production"]
```

**Environment-Specific Deployments:**
```yaml
# Reuse deployment template with different params
template:
  source:
    file: "templates/k8s-deploy.yml"
  with:
    cluster: "us-west-2"
    namespace: "production"
    replicas: "5"
```

**Testing Workflow Variations:**
```yaml
# Test different configurations using same template
template:
  source:
    file: "templates/integration-tests.yml"
  with:
    database: "postgres"
    cache: "redis"
    message_queue: "rabbitmq"
```

### Template Override

The `override` field allows you to override specific template fields without modifying the template file:

```yaml
template:
  source:
    registry: "standard-workflow"
  override:
    timeout: 1200  # Override default timeout
    max_parallel: 10  # Override concurrency limit
```

*Note: Template override application is currently in development. The field is validated and stored but not yet applied during workflow composition.*

### Implementation Status

- ✅ File-based template loading
- ✅ Registry template storage and retrieval
- ✅ Template parameter validation
- ✅ Template caching
- ⏳ URL-based template loading (returns error, planned for future)
- ⏳ Template override application (TODO in apply_overrides)

### Related Topics

- [Parameter Definitions](parameter-definitions.md) - Define and validate template parameters
- [Workflow Extension](workflow-extension-inheritance.md) - Inherit from base workflows
- [Default Values](default-values.md) - Set default parameter values

