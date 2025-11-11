## Template System

Templates provide reusable workflow patterns that can be instantiated with different parameters. Templates can be stored in a registry or loaded from files, enabling standardized workflows across teams and projects.

**Source**: Implemented in src/cook/workflow/composition/mod.rs and src/cook/workflow/composition/registry.rs

### Template Basics

A template is a reusable workflow definition that can be parameterized and instantiated multiple times with different values. Templates support:
- Registry-based or file-based storage
- Parameter substitution
- Field overrides
- Metadata and versioning

### Template Configuration

Templates are defined using the `WorkflowTemplate` struct (src/cook/workflow/composition/mod.rs:67-83):

```yaml
template:
  # Template name (for identification)
  name: "standard-ci"

  # Template source (see Template Sources below)
  source: "template-name"  # or { file: "path.yml" }

  # Parameter values to pass to template
  with:
    param1: "value1"
    param2: "value2"

  # Override specific template fields
  override:
    timeout: 600
    max_parallel: 5
```

**Source**: Field definitions from `WorkflowTemplate` struct in src/cook/workflow/composition/mod.rs:67-83

### Template Sources

Prodigy uses an untagged enum for `TemplateSource` (src/cook/workflow/composition/mod.rs:85-95), which means the YAML format varies based on the source type:

#### Registry Lookup (String)

Load a template by name from the template registry:

```yaml
template:
  name: "ci-pipeline"
  source: "standard-ci"  # Simple string = registry lookup
  with:
    project_name: "my-project"
```

**Source**: `TemplateSource::Registry(String)` variant in src/cook/workflow/composition/mod.rs:92

**How it works**: When the source is a plain string, Prodigy looks up the template in the registry at `~/.prodigy/templates/` or `.prodigy/templates/`. Example from test: tests/workflow_composition_test.rs:125-133

#### File Path

Load a template from a file:

```yaml
template:
  name: "deployment"
  source:
    file: "templates/k8s-deploy.yml"  # File path in struct format
  with:
    environment: "production"
```

**Source**: `TemplateSource::File(PathBuf)` variant in src/cook/workflow/composition/mod.rs:90

**How it works**: When the source uses a `file` field, Prodigy loads the template from the specified file path. Paths can be relative (to workflow file) or absolute.

#### URL (Planned)

Load a template from a remote URL:

```yaml
template:
  name: "remote-template"
  source: "https://templates.example.com/ci.yml"  # String starting with https://
  with:
    config: "production"
```

**Source**: `TemplateSource::Url(String)` variant in src/cook/workflow/composition/mod.rs:94

**Status**: Currently returns an error. Planned for future implementation. See src/cook/workflow/composition/composer.rs for URL handling code.

### Template Registry

The template registry stores reusable workflow templates. Templates can be stored in two locations (similar to git worktrees):

**Registry Locations:**
```
# Local (project-specific)
.prodigy/templates/
├── project-ci.yml
├── custom-deployment.yml
└── ...

# Global (user-wide)
~/.prodigy/templates/
├── standard-ci.yml
├── deployment-pipeline.yml
├── test-suite.yml
└── ...
```

**Implementation**: `FileTemplateStorage` in src/cook/workflow/composition/registry.rs:26-29 uses a configurable base directory (defaults to "templates").

#### Programmatic Registration

Register templates using the API (src/cook/workflow/composition/registry.rs:41-73):

```rust
use prodigy::cook::workflow::composition::registry::TemplateRegistry;

let registry = TemplateRegistry::new();

// Basic registration
registry
    .register_template("ci-pipeline".to_string(), template)
    .await?;

// Registration with metadata
registry
    .register_template_with_metadata(
        "deployment".to_string(),
        template,
        metadata
    )
    .await?;
```

**Source**: Example from tests/workflow_composition_test.rs:177-199

#### Manual Registry Management

```bash
# Copy template to local registry
cp my-template.yml .prodigy/templates/my-template.yml

# Copy to global registry (user-wide)
cp my-template.yml ~/.prodigy/templates/my-template.yml

# Templates are automatically discovered by filename
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

### Template Metadata

Templates can include metadata for better organization and discovery (src/cook/workflow/composition/registry.rs:475-508):

**Metadata Fields:**
- `description`: Human-readable description
- `author`: Template author
- `version`: Semantic version string
- `tags`: List of categorization tags
- `created_at`: Creation timestamp
- `updated_at`: Last modification timestamp

**Metadata Storage:**
Templates registered with metadata store an additional `.meta.json` file alongside the template YAML. For example, `standard-ci.yml` has metadata in `standard-ci.yml.meta.json`.

**Source**: `TemplateMetadata` struct in src/cook/workflow/composition/registry.rs:475-495

### Template Discovery

The registry provides search and listing capabilities (src/cook/workflow/composition/registry.rs:156-168):

```rust
// List all templates
let templates = registry.list().await?;

// Search by tags
let ci_templates = registry
    .search_by_tags(&["ci".to_string(), "testing".to_string()])
    .await?;

// Get specific template
let template = registry.get("standard-ci").await?;

// Delete template
registry.delete("old-template").await?;
```

**Source**: Methods in `TemplateRegistry` implementation

### Template Caching

Prodigy caches loaded templates to improve performance:
- Templates are loaded once and reused across workflow executions
- Registry templates are cached until the registry is updated
- In-memory cache stored in `Arc<RwLock<HashMap>>` (src/cook/workflow/composition/registry.rs:14)

**File Change Detection**: The documentation previously claimed file-based templates are re-read on file changes, but this is not currently implemented in the caching layer. Templates are cached for the lifetime of the registry instance.

### Template Override

The `override` field allows you to override specific template fields without modifying the template file:

```yaml
template:
  source: "standard-workflow"
  override:
    timeout: 1200  # Override default timeout
    max_parallel: 10  # Override concurrency limit
```

**Implementation Status**: The `override_field` is defined in the `WorkflowTemplate` struct (src/cook/workflow/composition/mod.rs:80-83) and is properly deserialized, but the application logic in `apply_overrides()` is not yet implemented. The field is validated and stored but not applied during workflow composition.

**Source**: See `override_field` in src/cook/workflow/composition/mod.rs:81-82

### Use Cases

**Standardized CI/CD Pipelines:**
```yaml
# Use company-wide CI template
template:
  source: "company-ci-pipeline"
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

### Implementation Status

- ✅ File-based template loading
- ✅ Registry template storage and retrieval
- ✅ Template parameter validation
- ✅ Template caching (in-memory)
- ✅ Template metadata and versioning
- ✅ Template search and discovery
- ✅ Programmatic registration API
- ⏳ URL-based template loading (returns error, planned for future)
- ⏳ Template override application (field exists but not applied in compose())
- ⏳ File modification detection for cache invalidation

### Related Topics

- [Parameter Definitions](parameter-definitions.md) - Define and validate template parameters
- [Workflow Extension](workflow-extension-inheritance.md) - Inherit from base workflows
- [Default Values](default-values.md) - Set default parameter values
