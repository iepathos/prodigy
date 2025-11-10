## Environment Variables in Configuration

MapReduce workflows support comprehensive environment variable configuration, enabling parameterized workflows with secrets management, multi-environment deployment, and secure credential handling.

### Configuration Fields

MapReduce workflows support four types of environment configuration:

**1. Basic Environment Variables (`env`)**

Define static environment variables available throughout the workflow:

```yaml
env:
  MAX_WORKERS: "10"
  AGENT_TIMEOUT: "300"
  PROJECT_NAME: "my-project"
```

**Source**: `src/config/workflow.rs:11-39` - WorkflowConfig struct with `env: HashMap<String, String>` field

**2. Secrets Configuration**

Secret values are defined within the `env` block and marked with the `secret: true` flag. They are automatically masked in logs, output, events, and checkpoints:

```yaml
env:
  # Basic environment variable
  PROJECT_NAME: "my-project"

  # Secret environment variable (masked in logs)
  API_KEY:
    secret: true
    value: "${env:SECRET_API_KEY}"

  # Another secret
  DB_PASSWORD:
    secret: true
    value: "my-secret-password"
```

**Source**: `src/cook/environment/config.rs:84-96` - SecretValue enum

**Secret Provider Support**:

The codebase defines a `SecretValue` enum supporting both simple string secrets and provider-based secrets:

```rust
pub enum SecretValue {
    Simple(String),
    Provider {
        provider: SecretProvider,
        key: String,
        version: Option<String>,
    },
}
```

**Supported Secret Providers** (defined in `src/cook/environment/config.rs:99-112`):
- `env` - Environment variable reference
- `file` - File-based secrets
- `vault` - HashiCorp Vault integration (planned)
- `aws` - AWS Secrets Manager (planned)
- `custom` - Custom provider support

**Note**: Provider-based secrets (vault, aws) are defined in the type system but not yet fully implemented. The current working syntax uses the simple format with `secret: true` flag and environment variable references like `${env:VAR_NAME}`.

**3. Environment Files (`env_files`)**

Load environment variables from `.env` files:

```yaml
env_files:
  - ".env"
  - ".env.local"
```

**Source**: `src/config/workflow.rs:11-39` - WorkflowConfig with `env_files: Vec<PathBuf>` field

Files are loaded in order, with later files overriding earlier ones. Standard `.env` format: `KEY=value`

**4. Profiles (`profiles`)**

Environment-specific configurations for different deployment targets:

```yaml
profiles:
  dev:
    API_URL: "http://localhost:3000"
    TIMEOUT: "60"
    MAX_WORKERS: "5"

  prod:
    API_URL: "https://api.prod.com"
    TIMEOUT: "30"
    MAX_WORKERS: "20"
```

**Source**: `src/cook/environment/config.rs:115-124` - EnvProfile struct

Activate a profile with:
```bash
prodigy run workflow.yml --profile prod
```

**Profile Structure**: Each profile contains a HashMap of environment variables and an optional description field for documentation.

### Variable Interpolation

Environment variables can be referenced using two syntaxes:
- `$VAR` - Simple variable reference (shell-style)
- `${VAR}` - Bracketed reference for clarity and complex expressions

**Source**: Verified in `tests/mapreduce_env_execution_test.rs:157-189` - both syntaxes are fully supported

Use `${VAR}` when:
- Variable name is followed by alphanumeric characters
- Embedding in strings or paths
- Preference for explicit syntax

**Interpolation in Environment Values**:

Environment variables can reference other environment variables or system environment variables within their values:

```yaml
env:
  BASE_URL: "https://api.example.com"
  API_ENDPOINT: "${BASE_URL}/v1/data"
  FULL_PATH: "${PROJECT_DIR}/output/${REPORT_FORMAT}"
```

**Source**: `src/cook/environment/config.rs:39-60` - EnvValue supports Static, Dynamic, and Conditional variants

**Supported Fields:**
- `max_parallel` - Control parallelism dynamically
- `agent_timeout_secs` - Adjust timeouts per environment
- `setup.timeout` - Configure setup phase timeouts
- `merge.timeout` - Control merge operation timeouts
- Any string field in your workflow (commands, paths, etc.)

**Example from tests** (`tests/mapreduce_env_execution_test.rs:10-52`):
```yaml
env:
  MAX_PARALLEL: "5"
  TIMEOUT_SECONDS: "900"

map:
  max_parallel: ${MAX_PARALLEL}
  agent_timeout_secs: ${TIMEOUT_SECONDS}
```

### Environment Precedence

When the same variable is defined in multiple places, Prodigy uses the following precedence order (highest to lowest):

1. **Command-line environment variables** - Set when invoking prodigy
2. **Step-level environment** - Variables defined in individual command `env:` blocks
3. **Active profile values** - Variables from the selected profile
4. **Global env block** - Base workflow environment variables
5. **Environment files** - Variables loaded from .env files (later files override earlier)
6. **System environment** - Parent process environment (if `inherit: true`)

**Source**: `src/cook/environment/config.rs:11-36` - EnvironmentConfig structure and `tests/environment_workflow_test.rs:225-256` - precedence verification

**Example**:
```bash
# Command-line overrides profile, which overrides base env
MAX_WORKERS=10 prodigy run workflow.yml --profile prod
```

### Complete Example

Here's a complete MapReduce workflow demonstrating all environment features:

```yaml
name: configurable-mapreduce
mode: mapreduce

# Basic environment variables
env:
  PROJECT_NAME: "data-pipeline"
  VERSION: "1.0.0"
  OUTPUT_DIR: "output"

  # Secret with environment variable reference
  API_KEY:
    secret: true
    value: "${env:SECRET_API_KEY}"

# Environment files
env_files:
  - ".env"

# Multi-environment profiles
profiles:
  dev:
    MAX_WORKERS: "5"
    API_URL: "http://localhost:3000"
    DEBUG_MODE: "true"

  prod:
    MAX_WORKERS: "20"
    API_URL: "https://api.prod.com"
    DEBUG_MODE: "false"

setup:
  timeout: 300
  commands:
    - shell: "echo Processing $PROJECT_NAME v$VERSION"
    - shell: "mkdir -p $OUTPUT_DIR"
    - shell: "curl -H 'Authorization: Bearer ${API_KEY}' $API_URL/init"

map:
  input: "items.json"
  json_path: "$[*]"
  max_parallel: ${MAX_WORKERS}
  agent_template:
    - claude: "/process ${item} --project $PROJECT_NAME"
    - shell: "curl -H 'Authorization: Bearer ${API_KEY}' $API_URL/items"

reduce:
  - claude: "/summarize ${map.results} --project $PROJECT_NAME"
  - shell: "cp summary.json $OUTPUT_DIR/${PROJECT_NAME}-summary.json"
```

**Source**: Based on `workflows/mapreduce-env-example.yml:1-94` with structure validated against `src/config/mapreduce.rs:15-82`

### Running with Different Configurations

```bash
# Development environment
prodigy run workflow.yml --profile dev

# Production environment
prodigy run workflow.yml --profile prod

# Override specific variables (command-line takes precedence)
MAX_WORKERS=10 prodigy run workflow.yml --profile prod

# Local development with .env file
echo "MAX_WORKERS=3" > .env
prodigy run workflow.yml
```

**Precedence Note**: Command-line environment variables override profile settings, which override the base env block.

**Source**: Precedence behavior verified in `src/cook/environment/manager.rs:43-52` and `tests/mapreduce_env_execution_test.rs:225-256`

### Step-Level Environment Overrides

Individual commands can override environment variables:

```yaml
map:
  agent_template:
    - shell: "process-item.sh"
      env:
        CUSTOM_VAR: "value"
        PATH: "/custom/bin:${PATH}"
```

**Source**: `src/cook/environment/config.rs:127-144` - StepEnvironment struct with `env`, `working_dir`, `clear_env`, and `temporary` fields

Step-level variables inherit from global `env` and active `profiles`, with step-level values taking precedence.

**Advanced Step Environment Features** (`tests/environment_workflow_test.rs:42-70`):
- `clear_env: true` - Start with clean environment except step-specific vars
- `temporary: true` - Restore environment after step execution
- `working_dir` - Set working directory for the step

### Best Practices

**Secrets vs Environment Variables:**
- Use secrets with `secret: true` flag for sensitive data (API keys, passwords, tokens)
- Use plain `env` for non-sensitive configuration (timeouts, URLs, feature flags)
- Secrets are automatically masked in all logs and outputs

**Security Considerations:**
- Never commit secrets to version control
- Use environment variable references like `${env:SECRET_VAR}` to inject secrets at runtime
- Leverage `.env.local` files (git-ignored) for local development
- For production, provider-based secrets (Vault, AWS Secrets Manager) are planned but not yet fully implemented

**Profile Usage:**
- Use profiles for multi-environment deployment (dev, staging, prod)
- Define sensible defaults in the base `env` block
- Override only environment-specific values in profiles
- Activate profiles explicitly to avoid accidental production deployments
- Include `description` field in profiles for documentation

**Environment Files:**
- Use `env_files` for local development configuration
- Order matters: later files override earlier ones
- Combine with profiles for flexible local/remote workflows
- Add `.env.local` to `.gitignore`

### Debugging Environment Variables

When running workflows with verbose output (`-vv` flag), Prodigy shows environment variable values with sensitive data masked:

**Source**: `examples/test-sensitive-masking.yml:1-22` - demonstrates automatic masking

```bash
# Run with debug output to see environment resolution
prodigy run workflow.yml -vv --profile dev

# Output will show:
# - PASSWORD as ***REDACTED***
# - API_KEY as ***REDACTED***
# - Normal variables displayed in full
```

### Cross-References

For comprehensive environment variable documentation, see:
- [Variables Chapter](../variables/index.md) - Complete guide to environment variables and interpolation
- [Environment Configuration](../environment/index.md) - Detailed environment management patterns
- [MapReduce Environment Variables](../environment/mapreduce-environment-variables.md) - MapReduce-specific environment features
- [Secrets Management](../environment/secrets-management.md) - Security best practices for secrets
- [Environment Profiles](../environment/environment-profiles.md) - Multi-environment deployment patterns
- [Setup Phase](./setup-phase-advanced.md) - Using environment variables in setup commands
- [MapReduce Index](./index.md) - Main MapReduce workflow documentation
