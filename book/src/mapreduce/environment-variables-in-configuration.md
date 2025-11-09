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

**2. Secrets (`secrets`)**

Secret values that are automatically masked in logs, output, events, and checkpoints:

```yaml
secrets:
  # Simple environment variable reference
  API_KEY:
    secret: true
    value: "sk-abc123"

  # Provider-based secrets
  DB_PASSWORD:
    secret: true
    provider: vault
    key: "database/prod/password"
    version: "v2"

  AWS_SECRET:
    secret: true
    provider: aws
    key: "prod/api-credentials"
```

**Supported Secret Providers:**
- `env` - Environment variable reference
- `file` - File-based secrets
- `vault` - HashiCorp Vault integration
- `aws` - AWS Secrets Manager

**3. Environment Files (`env_files`)**

Load environment variables from `.env` files:

```yaml
env_files:
  - ".env"
  - ".env.local"
```

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

Activate a profile with:
```bash
prodigy run workflow.yml --profile prod
```

### Variable Interpolation

Environment variables can be referenced using two syntaxes:
- `$VAR` - Simple variable reference (shell-style)
- `${VAR}` - Bracketed reference for clarity and complex expressions

Use `${VAR}` when:
- Variable name is followed by alphanumeric characters
- Embedding in strings or paths
- Preference for explicit syntax

**Supported Fields:**
- `max_parallel` - Control parallelism dynamically
- `agent_timeout_secs` - Adjust timeouts per environment
- `setup.timeout` - Configure setup phase timeouts
- `merge.timeout` - Control merge operation timeouts
- Any string field in your workflow (commands, paths, etc.)

### Complete Example

```yaml
name: configurable-mapreduce
mode: mapreduce

# Basic environment variables
env:
  PROJECT_NAME: "data-pipeline"
  VERSION: "1.0.0"

# Secret management
secrets:
  API_KEY:
    secret: true
    value: "sk-prod-abc123"

# Environment files
env_files:
  - ".env"

# Multi-environment profiles
profiles:
  dev:
    MAX_WORKERS: "5"
    API_URL: "http://localhost:3000"

  prod:
    MAX_WORKERS: "20"
    API_URL: "https://api.prod.com"

setup:
  timeout: "300"
  - shell: "echo Processing $PROJECT_NAME v$VERSION"
  - shell: "curl -H 'Authorization: Bearer ${API_KEY}' $API_URL/init"

map:
  input: "items.json"
  json_path: "$[*]"
  max_parallel: "$MAX_WORKERS"
  agent_template:
    - claude: "/process ${item} --project $PROJECT_NAME"
    - shell: "curl -H 'Authorization: Bearer ${API_KEY}' $API_URL/items"

reduce:
  - claude: "/summarize ${map.results} --project $PROJECT_NAME"
```

### Running with Different Configurations

```bash
# Development environment
prodigy run workflow.yml --profile dev

# Production environment
prodigy run workflow.yml --profile prod

# Override specific variables
MAX_WORKERS=10 prodigy run workflow.yml --profile prod

# Local development with .env file
echo "MAX_WORKERS=3" > .env
prodigy run workflow.yml
```

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

Step-level variables inherit from global `env` and active `profiles`, with step-level values taking precedence.

### Best Practices

**Secrets vs Environment Variables:**
- Use `secrets` for sensitive data (API keys, passwords, tokens)
- Use `env` for non-sensitive configuration (timeouts, URLs, feature flags)
- Secrets are automatically masked in all logs and outputs

**Security Considerations:**
- Never commit secrets to version control
- Use secret providers (Vault, AWS Secrets Manager) for production
- Leverage `.env.local` files (git-ignored) for local development
- Rotate secrets regularly and use versioned providers

**Profile Usage:**
- Use profiles for multi-environment deployment (dev, staging, prod)
- Define sensible defaults in the base `env` block
- Override only environment-specific values in profiles
- Activate profiles explicitly to avoid accidental production deployments

**Environment Files:**
- Use `env_files` for local development configuration
- Order matters: later files override earlier ones
- Combine with profiles for flexible local/remote workflows
- Add `.env.local` to `.gitignore`

### Cross-References

For comprehensive environment variable documentation, see:
- [Variables Chapter](./variables.md) - Complete guide to environment variables, profiles, secrets, and advanced usage
- [Setup Phase](./setup-phase.md) - Using environment variables in setup commands
- [Map Phase Configuration](./map-phase-configuration.md) - Parameterizing map operations
- [Reduce Phase](./reduce-phase.md) - Environment variables in reduce aggregation

