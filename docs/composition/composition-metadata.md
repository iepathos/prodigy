## Composition Metadata

Prodigy tracks metadata about workflow composition for debugging and dependency analysis. This metadata provides visibility into how workflows are composed, what dependencies exist, and when composition occurred.

> **Implementation Status**: The composition metadata types and dependency tracking are fully implemented in the core composition system. These features are accessible programmatically via the WorkflowComposer API. CLI integration for viewing composition metadata in workflows is under development (Spec 131-133).

### CompositionMetadata Structure

Every composed workflow includes metadata tracking all composition operations:

**Source**: `src/cook/workflow/composition/mod.rs:153-170`

```rust
/// Metadata about workflow composition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionMetadata {
    /// Source files involved in composition
    pub sources: Vec<PathBuf>,

    /// Templates used
    pub templates: Vec<String>,

    /// Parameters applied
    pub parameters: HashMap<String, Value>,

    /// Composition timestamp
    pub composed_at: chrono::DateTime<chrono::Utc>,

    /// Dependency graph
    pub dependencies: Vec<DependencyInfo>,
}
```

**Field Details**:
- `sources`: File paths of all workflow files involved in composition (as `PathBuf` objects)
- `templates`: Template names/sources used during composition
- `parameters`: Final parameter values applied to the workflow (as `serde_json::Value`)
- `composed_at`: ISO 8601 timestamp when composition occurred (UTC timezone)
- `dependencies`: Complete dependency graph with all imports, extends, templates, and sub-workflows

### Dependency Tracking

Each dependency includes detailed information:

**Source**: `src/cook/workflow/composition/mod.rs:172-183`

```rust
/// Information about workflow dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    /// Source of the dependency
    pub source: PathBuf,

    /// Type of dependency
    pub dep_type: DependencyType,

    /// Resolved path or name
    pub resolved: String,
}
```

**Field Details**:
- `source`: Source file path of the dependency (as `PathBuf`)
- `dep_type`: Type of dependency (Import, Extends, Template, or SubWorkflow)
- `resolved`: Resolved file path or template name (as `String`)

### Dependency Types

Prodigy tracks four types of dependencies:

**Source**: `src/cook/workflow/composition/mod.rs:185-193`

```rust
/// Type of workflow dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Import,
    Extends,
    Template,
    SubWorkflow,
}
```

**Import Dependencies:**
```yaml
imports:
  - path: "shared/utilities.yml"

# Creates DependencyInfo:
# dep_type: DependencyType::Import
# source: PathBuf::from("shared/utilities.yml")
# resolved: "/full/path/to/shared/utilities.yml"
```

**Extends Dependencies:**
```yaml
extends: "base-config.yml"

# Creates DependencyInfo:
# dep_type: DependencyType::Extends
# source: PathBuf::from("base-config.yml")
# resolved: "/full/path/to/base-config.yml"
```

**Template Dependencies:**
```yaml
template:
  source:
    registry: "ci-pipeline"

# Creates DependencyInfo:
# dep_type: DependencyType::Template
# source: PathBuf::from("registry:ci-pipeline")
# resolved: "~/.prodigy/templates/ci-pipeline.yml"
```

**SubWorkflow Dependencies:**
```yaml
sub_workflows:
  - name: "tests"
    source: "workflows/test.yml"

# Creates DependencyInfo:
# dep_type: DependencyType::SubWorkflow
# source: PathBuf::from("workflows/test.yml")
# resolved: "/full/path/to/workflows/test.yml"
```

### Viewing Composition Metadata

> **Note**: CLI commands for viewing composition metadata in workflow execution are under development. Currently, metadata can be accessed programmatically via the WorkflowComposer API (see [Programmatic Access](#programmatic-access) below).

**Future CLI Usage** (planned):
```bash
# Show composition metadata (planned feature)
prodigy run workflow.yml --dry-run --show-composition
```

**Expected Output:**
```
Composition Metadata:
  Composed at: 2025-01-11T20:00:00Z

  Sources (3):
    - /path/to/workflow.yml
    - /path/to/base-config.yml
    - /path/to/shared/utilities.yml

  Templates (1):
    - registry:ci-pipeline

  Dependencies (3):
    [Import] shared/utilities.yml -> /path/to/shared/utilities.yml
    [Extends] base-config.yml -> /path/to/base-config.yml
    [Template] registry:ci-pipeline -> ~/.prodigy/templates/ci-pipeline.yml

  Parameters (2):
    environment: "production"
    timeout: 600
```

### Programmatic Access

Access metadata in code using the WorkflowComposer API:

**Source**: `src/cook/workflow/composition/composer.rs:21-37`

```rust
use prodigy::cook::workflow::composition::{WorkflowComposer, TemplateRegistry};
use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;

// Create composer with template registry
let registry = Arc::new(TemplateRegistry::new());
let composer = WorkflowComposer::new(registry);

// Compose workflow with parameters
let params = HashMap::new();
let composed = composer.compose(Path::new("workflow.yml"), params).await?;

// Access metadata
let metadata = &composed.metadata;

// Inspect dependencies
for dep in &metadata.dependencies {
    println!("{:?}: {} -> {}",
        dep.dep_type,
        dep.source.display(),
        dep.resolved
    );
}

// Check composition timestamp
println!("Composed at: {}", metadata.composed_at);

// View final parameters
for (name, value) in &metadata.parameters {
    println!("Parameter {}: {:?}", name, value);
}

// List source files
println!("Sources:");
for source in &metadata.sources {
    println!("  - {}", source.display());
}

// List templates used
println!("Templates:");
for template in &metadata.templates {
    println!("  - {}", template);
}
```

**Real-World Example** (from `src/cook/workflow/composition/composer.rs:45-51`):

```rust
// Metadata is created during composition
let mut metadata = CompositionMetadata {
    sources: vec![source.to_path_buf()],
    templates: Vec::new(),
    parameters: params.clone(),
    composed_at: chrono::Utc::now(),
    dependencies: Vec::new(),
};
```

### Dependency Graph Visualization

Metadata enables dependency visualization:

```
workflow.yml
├─ [Extends] base-config.yml
│  └─ [Import] shared/setup.yml
├─ [Import] shared/utilities.yml
└─ [Template] registry:ci-pipeline
   └─ [SubWorkflow] workflows/test.yml
```

### Use Cases

**Debugging Composition Issues:**
- Verify which files were loaded
- Check parameter resolution order
- Identify circular dependencies
- Trace inheritance chains

**Dependency Analysis:**
- Find all workflows using a template
- Identify shared imports
- Map workflow relationships
- Audit composition complexity

**Change Impact Assessment:**
```bash
# Before changing base-config.yml, find all dependents
grep -r "extends.*base-config" workflows/

# View composition metadata programmatically
# (CLI integration for --show-composition is under development)
```

**Compliance and Auditing:**
- Track template versions used
- Record composition timestamps
- Document parameter sources
- Verify configuration origins

### Metadata in Composed Workflows

Composed workflows carry metadata through the composition process:

**Source**: `src/cook/workflow/composition/mod.rs:143-151`

```rust
// ComposedWorkflow structure
pub struct ComposedWorkflow {
    /// The composed workflow
    pub workflow: ComposableWorkflow,

    /// Metadata about the composition
    pub metadata: CompositionMetadata,
}

// Access metadata from composed workflow
let composed = composer.compose(source, params).await?;
println!("This workflow was composed from {} sources",
    composed.metadata.sources.len());
```

### Circular Dependency Detection

Metadata enables circular dependency detection:

```yaml
# workflow-a.yml
extends: "workflow-b.yml"

# workflow-b.yml
extends: "workflow-a.yml"
```

**Detection:**
```
Error: Circular dependency detected
  workflow-a.yml -> workflow-b.yml -> workflow-a.yml

Dependency chain:
  1. workflow-a.yml (extends workflow-b.yml)
  2. workflow-b.yml (extends workflow-a.yml) <- Circular!
```

### Parameter Tracking

Metadata tracks final parameter values applied during composition:

```yaml
# workflow.yml
parameters:
  definitions:
    environment:
      type: string
      required: true
    timeout:
      type: integer
      default: 600
```

**Metadata captures:**
```rust
metadata.parameters = {
    "environment": "production",
    "timeout": 600,
}
```

The `parameters` field in `CompositionMetadata` stores the final resolved parameter values as a `HashMap<String, Value>`. This enables reproducibility and debugging of composed workflows.

### Caching and Performance

Composition metadata enables future caching optimizations:

- `composed_at` timestamp can be used for cache invalidation
- `sources` list enables dependency-based cache busting
- `dependencies` graph supports incremental composition
- `parameters` hash can detect identical compositions

> **Note**: Workflow caching is a planned feature. Currently, metadata is generated fresh on each composition.

### Data Structure Properties

CompositionMetadata uses standard Rust types for broad compatibility:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionMetadata {
    // All fields use standard types
    pub sources: Vec<PathBuf>,           // Cloneable
    pub templates: Vec<String>,          // Cloneable
    pub parameters: HashMap<String, Value>,  // Cloneable
    pub composed_at: chrono::DateTime<chrono::Utc>,  // Copy
    pub dependencies: Vec<DependencyInfo>,   // Cloneable
}
```

The struct derives `Clone`, making it easy to share metadata across components without requiring explicit synchronization primitives.

### Related Topics

- [Template System](template-system.md) - Template caching and loading
- [Workflow Extension](workflow-extension-inheritance.md) - Inheritance tracking
- Best Practices (see composition/best-practices.md) - Using metadata for debugging
