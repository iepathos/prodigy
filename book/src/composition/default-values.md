## Default Values

Set default parameter values and environment variables at the workflow level. Defaults reduce required parameters and simplify workflow usage by providing sensible fallback values.

### Basic Syntax

```yaml
name: my-workflow
mode: standard

defaults:
  timeout: 300
  retry_count: 3
  verbose: false
  environment: "development"
  log_level: "info"
```

### Defaults Field Structure

The `defaults` field is a HashMap<String, Value> that accepts any JSON-compatible values:

```yaml
defaults:
  # String values
  environment: "staging"
  log_level: "debug"

  # Number values
  timeout: 600
  max_retries: 5

  # Boolean values
  dry_run: false
  enable_cache: true

  # Array values
  allowed_regions: ["us-west-2", "us-east-1"]

  # Object values
  database_config:
    host: "localhost"
    port: 5432
    pool_size: 10
```

### Parameter Precedence

Default values have the lowest precedence in parameter resolution:

1. **CLI `--param` flags** (highest priority)
2. **`--param-file` JSON values**
3. **Workflow `defaults` values**
4. **Parameter `default` values** (lowest priority)

### Example with Precedence

```yaml
# workflow.yml
defaults:
  environment: "development"
  timeout: 300

parameters:
  definitions:
    environment:
      type: String

    timeout:
      type: Number
      default: 600  # Parameter default overrides workflow default
```

**CLI override:**
```bash
# Final values: environment="production", timeout=900
prodigy run workflow.yml --param environment=production --param timeout=900
```

### Defaults with Parameters

Defaults simplify parameter requirements:

**Without defaults:**
```yaml
parameters:
  required:
    - environment
    - timeout
    - log_level
    - retry_count

# Users must provide all 4 parameters
prodigy run workflow.yml \
  --param environment=staging \
  --param timeout=600 \
  --param log_level=info \
  --param retry_count=3
```

**With defaults:**
```yaml
defaults:
  environment: "development"
  timeout: 300
  log_level: "info"
  retry_count: 3

parameters:
  required:
    - environment  # Still required but has default

# Users can run without parameters (uses defaults)
prodigy run workflow.yml

# Or override specific values
prodigy run workflow.yml --param environment=production
```

### Defaults for Environment Variables

Use defaults to set common environment variables:

```yaml
defaults:
  RUST_BACKTRACE: "1"
  CARGO_INCREMENTAL: "0"
  DATABASE_URL: "postgres://localhost/dev"
  REDIS_URL: "redis://localhost:6379"

commands:
  # Commands use default environment variables
  - shell: "cargo test"
  - shell: "redis-cli -u $REDIS_URL ping"
```

### Template Integration

Templates can use defaults for parameterization:

```yaml
# template.yml
name: deployment-template

defaults:
  replicas: "3"
  environment: "staging"

parameters:
  required:
    - app_name

commands:
  - shell: "kubectl scale deployment ${app_name} --replicas=${replicas}"
  - shell: "kubectl set env deployment/${app_name} ENV=${environment}"
```

**Using template:**
```yaml
template:
  source:
    file: "template.yml"
  with:
    app_name: "my-service"
    # Uses defaults: replicas=3, environment=staging
```

### Implementation Status

- ✅ Defaults field parsing and storage
- ✅ Defaults validation
- ✅ Integration into composition flow
- ⏳ Merge logic with parameters (TODO in apply_defaults at composer.rs:217-257)

*Note: The `apply_defaults` function is called during composition and defaults are validated/stored, but the actual merge logic to apply defaults to parameters is pending implementation. The infrastructure is complete and defaults are tracked in CompositionMetadata.*

### Best Practices

1. **Provide sensible defaults** - Choose values that work for most use cases
2. **Document defaults** - Add comments explaining default choices
3. **Use defaults for non-sensitive data** - Secrets should be passed via CLI/env
4. **Keep defaults simple** - Complex objects can be hard to override partially

### Related Topics

- [Parameter Definitions](parameter-definitions.md) - Define parameters with types
- [Template System](template-system.md) - Use defaults in templates
- [Workflow Extension](workflow-extension-inheritance.md) - Inherit defaults from base workflows

