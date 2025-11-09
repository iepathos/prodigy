## Workflow Extension (Inheritance)

Extend a base workflow to inherit its configuration. Child workflows override parent values, allowing you to customize specific aspects while maintaining common configuration. This enables environment-specific variations and layered configuration management.

### Basic Extension Syntax

```yaml
# production.yml
name: production-deployment
mode: standard

# Inherit from base workflow
extends: "base-deployment.yml"

# Override specific values
env:
  ENVIRONMENT: "production"
  REPLICAS: "5"
```

### How Extension Works

When a workflow extends a base workflow:

1. **Base workflow is loaded** from the specified path
2. **Child values override parent** for matching keys
3. **Parent values are preserved** where child doesn't override
4. **Merging is deep** - nested objects merge recursively
5. **Arrays are replaced** - child arrays replace parent arrays entirely

### Extension vs Imports

| Feature | Extension (`extends`) | Imports |
|---------|----------------------|---------|
| **Purpose** | Inherit and customize base workflow | Reuse workflow components |
| **Relationship** | Parent-child hierarchy | Modular composition |
| **Override behavior** | Child overrides parent | Imports merge with main |
| **Use case** | Environment variations | Shared utilities |

### Multi-Environment Example

**base-deployment.yml** (shared configuration):
```yaml
name: base-deployment
mode: standard

env:
  APP_NAME: "my-service"
  REPLICAS: "1"
  LOG_LEVEL: "info"

commands:
  - shell: "docker build -t ${APP_NAME}:${VERSION} ."
  - shell: "kubectl apply -f k8s/${ENVIRONMENT}/deployment.yml"
  - shell: "kubectl scale deployment ${APP_NAME} --replicas=${REPLICAS}"
```

**dev.yml** (development environment):
```yaml
name: dev-deployment
extends: "base-deployment.yml"

env:
  ENVIRONMENT: "dev"
  REPLICAS: "1"
  LOG_LEVEL: "debug"

# Inherits all commands from base
```

**staging.yml** (staging environment):
```yaml
name: staging-deployment
extends: "base-deployment.yml"

env:
  ENVIRONMENT: "staging"
  REPLICAS: "3"
  LOG_LEVEL: "info"

# Additional staging-specific commands
commands:
  - shell: "run-smoke-tests.sh"
```

**production.yml** (production environment):
```yaml
name: production-deployment
extends: "base-deployment.yml"

env:
  ENVIRONMENT: "production"
  REPLICAS: "5"
  LOG_LEVEL: "warn"
  ENABLE_MONITORING: "true"

# Additional production safeguards
commands:
  - shell: "verify-release-notes.sh"
  - shell: "notify-team 'Deploying to production'"
```

### Merge Behavior

**Scalar Values** - Child replaces parent:
```yaml
# base.yml
timeout: 300

# child.yml
extends: "base.yml"
timeout: 600  # Replaces 300 with 600
```

**Objects** - Deep merge:
```yaml
# base.yml
env:
  APP_NAME: "service"
  LOG_LEVEL: "info"

# child.yml
extends: "base.yml"
env:
  LOG_LEVEL: "debug"  # Overrides
  NEW_VAR: "value"     # Adds

# Result:
env:
  APP_NAME: "service"     # From base
  LOG_LEVEL: "debug"      # Overridden
  NEW_VAR: "value"        # Added
```

**Arrays** - Child replaces parent:
```yaml
# base.yml
commands:
  - shell: "step1"
  - shell: "step2"

# child.yml
extends: "base.yml"
commands:
  - shell: "custom-step"  # Completely replaces base commands

# Result: Only custom-step runs
```

### Layered Extension

Workflows can extend workflows that themselves extend other workflows:

```yaml
# base.yml
name: base-config
timeout: 300

# intermediate.yml
extends: "base.yml"
timeout: 600
max_parallel: 5

# final.yml
extends: "intermediate.yml"
max_parallel: 10

# Result: timeout=600 (from intermediate), max_parallel=10 (from final)
```

### Path Resolution

Extension paths can be:
- **Relative**: Resolved from workflow file's directory
- **Absolute**: Full filesystem path
- **Registry**: Future support for template registry paths

```yaml
# Relative path
extends: "../shared/base.yml"

# Absolute path
extends: "/etc/prodigy/workflows/base.yml"
```

### Use Cases

**Environment-Specific Deployments:**
- Share common deployment steps
- Override environment variables per environment
- Customize resource limits (replicas, memory, CPU)

**Testing Variations:**
```yaml
# base-test.yml
name: base-test
commands:
  - shell: "cargo build"
  - shell: "cargo test"

# integration-test.yml
extends: "base-test.yml"
env:
  DATABASE_URL: "postgres://localhost/test"
commands:
  - shell: "setup-test-db.sh"
  # Runs instead of base commands

# unit-test.yml
extends: "base-test.yml"
env:
  RUST_TEST_THREADS: "1"
```

**Progressive Configuration:**
- Start with minimal base config
- Add complexity in child workflows
- Keep each layer focused on specific concerns

### Circular Dependency Protection

Prodigy detects and prevents circular dependencies:

```yaml
# workflow-a.yml
extends: "workflow-b.yml"

# workflow-b.yml
extends: "workflow-a.yml"

# Error: Circular dependency detected
```

### Complete Example

**base-ci.yml:**
```yaml
name: base-ci
mode: standard

env:
  RUST_BACKTRACE: "1"

commands:
  - shell: "cargo fmt --check"
  - shell: "cargo clippy"
  - shell: "cargo test"
```

**pr-ci.yml** (runs on pull requests):
```yaml
name: pr-ci
extends: "base-ci.yml"

env:
  CARGO_INCREMENTAL: "0"  # Faster CI builds

# Inherits format, clippy, test from base
```

**release-ci.yml** (runs on release):
```yaml
name: release-ci
extends: "base-ci.yml"

env:
  CARGO_INCREMENTAL: "0"

commands:
  - shell: "cargo build --release"
  - shell: "cargo test --release"
  - shell: "cargo publish --dry-run"
```

### Debugging Extensions

View composition metadata to see inheritance chain:

```bash
# Metadata includes DependencyInfo with Extends type
prodigy run workflow.yml --dry-run --show-composition
```

### Implementation Status

- ✅ Base workflow loading
- ✅ Deep merge of child and parent configurations
- ✅ Circular dependency detection
- ✅ Path resolution (relative and absolute)
- ✅ Composition metadata tracking

### Related Topics

- [Workflow Imports](index.md#workflow-imports) - Modular composition
- [Template System](template-system.md) - Parameterized workflows
- [Composition Metadata](composition-metadata.md) - Inspect composition details

