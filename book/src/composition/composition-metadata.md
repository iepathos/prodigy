## Composition Metadata

Prodigy tracks metadata about workflow composition for debugging and dependency analysis. This metadata provides visibility into how workflows are composed, what dependencies exist, and when composition occurred.

### CompositionMetadata Structure

Every composed workflow includes metadata tracking all composition operations:

```rust
CompositionMetadata {
    sources: Vec<String>,          // File paths of all loaded workflows
    templates: Vec<String>,        // Template names/sources used
    parameters: HashMap<String, Value>,  // Final parameter values
    composed_at: DateTime,         // Composition timestamp
    dependencies: Vec<DependencyInfo>,   // Dependency graph
}
```

### Dependency Tracking

Each dependency includes detailed information:

```rust
DependencyInfo {
    dependency_type: DependencyType,  // Import, Extends, Template, SubWorkflow
    source: String,                    // Source file or template name
    target: Option<String>,            // Target workflow name
    resolved_path: Option<String>,     // Resolved file path
}
```

### Dependency Types

Prodigy tracks four types of dependencies:

**Import Dependencies:**
```yaml
imports:
  - path: "shared/utilities.yml"

# Creates DependencyInfo:
# type: Import
# source: "shared/utilities.yml"
# resolved_path: "/full/path/to/shared/utilities.yml"
```

**Extends Dependencies:**
```yaml
extends: "base-config.yml"

# Creates DependencyInfo:
# type: Extends
# source: "base-config.yml"
# resolved_path: "/full/path/to/base-config.yml"
```

**Template Dependencies:**
```yaml
template:
  source:
    registry: "ci-pipeline"

# Creates DependencyInfo:
# type: Template
# source: "registry:ci-pipeline"
# resolved_path: "~/.prodigy/templates/ci-pipeline.yml"
```

**SubWorkflow Dependencies:**
```yaml
sub_workflows:
  - name: "tests"
    source: "workflows/test.yml"

# Creates DependencyInfo:
# type: SubWorkflow
# source: "workflows/test.yml"
# target: "tests"
# resolved_path: "/full/path/to/workflows/test.yml"
```

### Viewing Composition Metadata

**During Dry-Run:**
```bash
# Show composition metadata
prodigy run workflow.yml --dry-run --show-composition
```

**Output Example:**
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

Access metadata in code:

```rust
use prodigy::workflow::composition::ComposerBuilder;

let composer = ComposerBuilder::new()
    .with_registry(registry)
    .build();

let composed = composer.compose_workflow(&workflow).await?;
let metadata = composed.metadata();

// Inspect dependencies
for dep in &metadata.dependencies {
    println!("{:?}: {} -> {:?}",
        dep.dependency_type,
        dep.source,
        dep.resolved_path
    );
}

// Check composition timestamp
println!("Composed at: {}", metadata.composed_at);

// View final parameters
for (name, value) in &metadata.parameters {
    println!("Parameter {}: {:?}", name, value);
}
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

# View composition metadata to see full dependency chain
prodigy run each-dependent.yml --dry-run --show-composition
```

**Compliance and Auditing:**
- Track template versions used
- Record composition timestamps
- Document parameter sources
- Verify configuration origins

### Metadata in Composed Workflows

Composed workflows carry metadata through execution:

```rust
let workflow = composer.compose_workflow(&input).await?;

// Metadata is preserved in composed workflow
assert!(workflow.composition_metadata.is_some());

let metadata = workflow.composition_metadata.unwrap();
println!("This workflow was composed from {} sources", metadata.sources.len());
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

### Parameter Source Tracking

Metadata tracks parameter origins:

```yaml
# base.yml
defaults:
  timeout: 300

# workflow.yml
extends: "base.yml"
parameters:
  definitions:
    timeout:
      default: 600
```

**Metadata shows:**
```
Parameters:
  timeout: 600
    Source: parameter default (overrides workflow default 300)
    Origin: workflow.yml:4
```

### Caching and Performance

Composition metadata supports caching:

- Workflows with identical metadata can reuse composition
- Timestamp enables cache invalidation
- Dependency hashes detect source changes
- Registry templates cache by version

### Thread-Safety

Metadata operations are thread-safe:

```rust
// Composition metadata uses Arc<RwLock<>>
let metadata = workflow.composition_metadata.clone();

// Safe to share across threads
tokio::spawn(async move {
    let deps = metadata.read().await;
    println!("Dependencies: {:?}", deps.dependencies);
});
```

### Related Topics

- [Template System](template-system.md) - Template caching and loading
- [Workflow Extension](workflow-extension-inheritance.md) - Inheritance tracking
- [Best Practices](best-practices.md) - Using metadata for debugging
