# Environment Variables

Prodigy provides comprehensive environment variable management with secrets masking and profile-based configuration for different deployment contexts.

## Overview

Environment variables in Prodigy enable:
- Global configuration shared across all workflow steps
- Secure secrets management with automatic masking
- Profile-based values for dev/staging/prod environments
- Loading variables from .env files
- Variable interpolation in all command fields

## Global Environment Variables

Define variables at the workflow level:

```yaml
name: my-workflow

env:
  PROJECT_NAME: "prodigy"
  VERSION: "1.0.0"
  BUILD_DIR: "target/release"

- shell: "cargo build --manifest-path ${PROJECT_NAME}/Cargo.toml"
- shell: "cp ${BUILD_DIR}/prodigy /usr/local/bin/"
```

## Secrets Management

Mark sensitive values as secrets to enable automatic masking:

```yaml
env:
  # Plain variable
  API_URL: "https://api.example.com"

  # Secret variable (masked in logs)
  API_KEY:
    secret: true
    value: "sk-abc123xyz789"

  DATABASE_PASSWORD:
    secret: true
    value: "${DB_PASS}"  # Can reference environment
```

### Secret Masking

Secrets are automatically masked in:
- Command output logs
- Error messages
- Event logs
- Checkpoint files
- Console output

Example output:
```
$ curl -H 'Authorization: Bearer ***' https://api.example.com
```

## Profile Support

Define different values for different environments:

```yaml
env:
  API_URL:
    default: "http://localhost:3000"
    staging: "https://staging.api.example.com"
    prod: "https://api.example.com"

  MAX_WORKERS:
    default: 5
    staging: 10
    prod: 20

  DATABASE_URL:
    default: "postgres://localhost/dev"
    staging: "postgres://staging-server/db"
    prod: "postgres://prod-server/db"
```

Activate a profile:
```bash
prodigy run workflow.yml --profile prod
```

## Environment Files

Load variables from .env files:

```yaml
env_files:
  - ".env"
  - ".env.local"
  - ".env.${PROFILE}"
```

**File format (.env)**:
```bash
PROJECT_NAME=prodigy
VERSION=1.0.0
API_KEY=sk-abc123
DATABASE_URL=postgres://localhost/db
```

### Precedence

Variable resolution follows this order (highest to lowest):
1. Command-line arguments
2. Workflow `env` block
3. Environment files (in order listed)
4. System environment variables

## Usage in Workflows

### All Workflow Phases

Environment variables work in all phases:

**Setup Phase:**
```yaml
setup:
  - shell: "npm install --prefix $PROJECT_DIR"
  - shell: "cargo build --manifest-path ${PROJECT_DIR}/Cargo.toml"
```

**Map Phase:**
```yaml
map:
  agent_template:
    - claude: "/analyze ${item.file} --config $CONFIG_PATH"
    - shell: "test -f $PROJECT_DIR/${item.file}"
```

**Reduce Phase:**
```yaml
reduce:
  - claude: "/summarize ${map.results} --project $PROJECT_NAME"
  - shell: "cp results.json $OUTPUT_DIR/"
```

**Merge Phase:**
```yaml
merge:
  commands:
    - shell: "echo Merging $PROJECT_NAME changes"
    - claude: "/validate-merge --env $ENVIRONMENT"
```

## Variable Interpolation

Two syntaxes are supported:

```yaml
# Simple variable reference
- shell: "echo $PROJECT_NAME"

# Bracketed reference (recommended for clarity)
- shell: "echo ${PROJECT_NAME}"

# With nested field access
- shell: "curl ${API_CONFIG.url}/endpoint"
```

## Combining with Capture Variables

Environment variables work alongside capture variables:

```yaml
env:
  OUTPUT_DIR: "results"

- shell: "cargo test"
  capture_output: test_results
  capture_format: json

- shell: "cp test-report.json ${OUTPUT_DIR}/"
- claude: "/analyze-tests ${test_results} --threshold ${MIN_COVERAGE}"
```

## Best Practices

1. **Use secrets for sensitive data**: Mark API keys, tokens, and passwords as secrets
2. **Parameterize project paths**: Use env vars instead of hardcoding paths
3. **Document required variables**: Include comments explaining what each variable does
4. **Use profiles for environments**: Separate dev, staging, and prod configurations
5. **Prefer ${VAR} syntax**: More explicit and works in all contexts
6. **Load from .env files**: Keep secrets out of version control

## Examples

### API Integration

```yaml
env:
  API_URL:
    default: "http://localhost:8000"
    prod: "https://api.production.com"
  API_KEY:
    secret: true
    value: "${API_KEY_SECRET}"

- shell: "curl -H 'Authorization: Bearer ${API_KEY}' ${API_URL}/endpoint"
```

### Multi-Environment Deployment

```yaml
env:
  DEPLOY_TARGET:
    dev: "dev-cluster"
    staging: "staging-cluster"
    prod: "prod-cluster"

  REPLICAS:
    dev: 1
    staging: 3
    prod: 10

- shell: "kubectl apply -f deployment.yml --replicas ${REPLICAS}"
- shell: "kubectl rollout status deployment/${PROJECT_NAME} -n ${DEPLOY_TARGET}"
```

## See Also

- [Variables and Interpolation](variables.md) - Variable system overview
- [Workflow Structure](workflow-structure.md) - Complete workflow configuration
- [MapReduce Workflows](../mapreduce/index.md) - Environment variables in parallel workflows
