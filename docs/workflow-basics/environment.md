# Environment Variables

Prodigy provides comprehensive environment variable management for workflows, enabling parameterization, secrets management, and environment-specific configurations.

## Overview

Environment variables in Prodigy allow you to:

- Define workflow-wide variables accessible in all commands
- Securely manage sensitive credentials with automatic masking
- Configure environment-specific settings using profiles
- Load variables from `.env` files
- Use dynamic and conditional variables
- Reference variables across all workflow phases

## Defining Environment Variables

Environment variables are defined in the `env` block at the workflow root:

```yaml title="Basic environment variables"
# Source: workflows/environment-example.yml
env:
  # Static variables
  NODE_ENV: production
  API_URL: https://api.example.com
  PROJECT_NAME: "my-project"
  VERSION: "1.0.0"

commands:
  - shell: "echo Building $PROJECT_NAME version $VERSION"
  - shell: "curl $API_URL/health"
```

### Variable Types

#### Static Variables

Simple key-value pairs for constant values:

```yaml
# Source: workflows/mapreduce-env-example.yml:8-11
env:
  PROJECT_NAME: "example-project"
  PROJECT_CONFIG: "config.yml"
  FEATURES_PATH: "features"
```

#### Dynamic Variables

Computed from command output at workflow start:

```yaml
# Source: workflows/environment-example.yml:10-12
env:
  WORKERS:
    command: "nproc 2>/dev/null || echo 4"
    cache: true
```

Dynamic variables are evaluated once and cached for workflow duration when `cache: true`.

#### Conditional Variables

Values that depend on expressions:

```yaml
# Source: workflows/environment-example.yml:14-18
env:
  DEPLOY_ENV:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"
```

## Variable Interpolation

Prodigy supports two interpolation syntaxes for flexibility:

```yaml
# Source: workflows/mapreduce-env-example.yml:43-46
commands:
  # Simple syntax
  - shell: "echo Starting $PROJECT_NAME workflow"

  # Bracketed syntax (more explicit)
  - shell: "echo Created output directory: ${OUTPUT_DIR}"

  # In Claude commands
  - claude: "/analyze --project $PROJECT_NAME --config ${PROJECT_CONFIG}"
```

**When to use bracketed syntax:**

- When variable name is followed by alphanumeric characters: `${VAR}_suffix`
- For clarity in complex expressions: `${map.results}`
- Inside quoted strings: `"Path: ${OUTPUT_DIR}/file"`

## Secrets Management

Secrets are automatically masked in all output, logs, and error messages to prevent credential leaks.

### Defining Secrets

```yaml
# Source: workflows/mapreduce-env-example.yml:22-25
env:
  API_TOKEN:
    secret: true
    value: "${GITHUB_TOKEN}"
```

Secrets can reference environment variables from the parent process using `${ENV_VAR}` syntax.

### Alternative Secrets Syntax

```yaml
# Source: workflows/environment-example.yml:21-23
secrets:
  API_KEY: "${env:SECRET_API_KEY}"
```

The `secrets` block is an alternative to inline `secret: true` definitions.

### Automatic Masking

Secrets are masked in:

- Command output (stdout/stderr)
- Error messages and stack traces
- Event logs and checkpoints
- Workflow summaries
- MapReduce agent logs

**Example output with masking:**

```bash
$ curl -H 'Authorization: Bearer ***' https://api.example.com
```

!!! warning "Secret Security"
    Always mark sensitive values as secrets. Without the `secret: true` flag, values will appear in logs and may be exposed.

## Profiles

Profiles enable environment-specific configurations for development, staging, and production environments.

### Defining Profiles

```yaml
# Source: workflows/mapreduce-env-example.yml:28-39
env:
  DEBUG_MODE: "false"
  TIMEOUT_SECONDS: "300"
  OUTPUT_DIR: "output"

profiles:
  development:
    description: "Development environment with debug enabled"
    DEBUG_MODE: "true"
    TIMEOUT_SECONDS: "60"
    OUTPUT_DIR: "dev-output"

  production:
    description: "Production environment"
    DEBUG_MODE: "false"
    TIMEOUT_SECONDS: "300"
    OUTPUT_DIR: "prod-output"
```

### Activating Profiles

```bash
# Use default values (no profile)
prodigy run workflow.yml

# Activate development profile
prodigy run workflow.yml --profile development

# Activate production profile
prodigy run workflow.yml --profile production
```

Profile variables override default `env` values. Variables not defined in the profile inherit default values.

## Environment Files

Load variables from `.env` format files for external configuration.

### Defining Environment Files

```yaml
# Source: workflows/environment-example.yml:26-27
env_files:
  - .env.production
  - .env.local
```

### .env File Format

```bash title=".env.production"
# Database configuration
DATABASE_URL=postgres://localhost/mydb
DATABASE_POOL_SIZE=10

# API settings
API_KEY=sk-abc123xyz
API_TIMEOUT=30

# Feature flags
ENABLE_CACHING=true
```

### Variable Precedence

When variables are defined in multiple locations, Prodigy uses this precedence (highest to lowest):

1. Profile variables (`--profile` flag)
2. Workflow `env` block
3. Environment files (later files override earlier)
4. Parent process environment

## Usage in Workflow Phases

Environment variables are available in all workflow phases:

### Standard Workflows

```yaml
# Source: workflows/environment-example.yml:42-52
commands:
  - name: "Show environment"
    shell: "echo NODE_ENV=$NODE_ENV API_URL=$API_URL"

  - name: "Build frontend"
    shell: "echo 'Building with NODE_ENV='$NODE_ENV"
    env:
      BUILD_TARGET: production
      OPTIMIZE: "true"
    working_dir: ./frontend
```

### MapReduce Setup Phase

```yaml
# Source: workflows/mapreduce-env-example.yml:42-49
setup:
  - shell: "echo Starting $PROJECT_NAME workflow"
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "echo Created output directory: ${OUTPUT_DIR}"
  - shell: "echo Debug mode: $DEBUG_MODE"
```

### MapReduce Map Phase

Environment variables are available in agent templates:

```yaml
# Source: workflows/mapreduce-env-example.yml:56-68
map:
  agent_template:
    # In Claude commands
    - claude: "/process-item '${item.name}' --project $PROJECT_NAME"

    # In shell commands
    - shell: "echo Processing ${item.name} for $PROJECT_NAME"
    - shell: "echo Output: $OUTPUT_DIR"

    # In failure handlers
    - shell: "timeout ${TIMEOUT_SECONDS}s ./process.sh"
      on_failure:
        - claude: "/fix-issue --max-retries $MAX_RETRIES"
```

### MapReduce Reduce Phase

```yaml
# Source: workflows/mapreduce-env-example.yml:72-79
reduce:
  - shell: "echo Aggregating results for $PROJECT_NAME"
  - claude: "/summarize ${map.results} --format $REPORT_FORMAT"
  - shell: "cp summary.$REPORT_FORMAT $OUTPUT_DIR/${PROJECT_NAME}-summary.$REPORT_FORMAT"
  - shell: "echo Processed ${map.successful}/${map.total} items"
```

### Merge Phase

```yaml
# Source: workflows/mapreduce-env-example.yml:82-93
merge:
  commands:
    - shell: "echo Merging changes for $PROJECT_NAME"
    - claude: "/validate-merge --branch ${merge.source_branch} --project $PROJECT_NAME"
    - shell: "echo Merge completed for ${PROJECT_NAME}"
```

## Per-Step Environment

Override or add variables for specific commands:

```yaml
# Source: workflows/environment-example.yml:54-60
commands:
  - name: "Run tests"
    shell: "pytest tests/"
    env:
      PYTHONPATH: "./src:./tests"
      TEST_ENV: "true"
    working_dir: ./backend
    temporary: true  # Environment restored after this step
```

**Options:**

- `temporary: true` - Restore environment after step completes
- `clear_env: true` - Clear all inherited variables, use only step-specific ones

## Best Practices

!!! tip "Use Secrets for Sensitive Data"
    Always mark API keys, tokens, passwords, and credentials as secrets to enable automatic masking.

!!! tip "Parameterize Project-Specific Values"
    Use environment variables instead of hardcoding paths, URLs, and configuration values. This improves portability and maintainability.

!!! tip "Document Required Variables"
    Add comments in workflow files documenting expected variables and their purposes.

!!! tip "Use Profiles for Environments"
    Separate development, staging, and production configurations using profiles rather than maintaining separate workflow files.

!!! tip "Prefer Bracketed Syntax"
    Use `${VAR}` instead of `$VAR` for explicitness and to avoid ambiguity in complex expressions.

## Common Patterns

### Project Configuration

```yaml
env:
  PROJECT_NAME: "my-app"
  VERSION: "1.2.3"
  BUILD_DIR: "dist"
  RELEASE_CHANNEL: "stable"
```

### API Integration

```yaml
env:
  API_URL: "https://api.example.com"
  API_TIMEOUT: "30"

secrets:
  API_KEY: "${env:EXTERNAL_API_KEY}"

commands:
  - shell: "curl -H 'Authorization: Bearer $API_KEY' $API_URL/data"
```

### Multi-Environment Configuration

```yaml
env:
  APP_ENV: "development"
  LOG_LEVEL: "debug"

profiles:
  staging:
    APP_ENV: "staging"
    LOG_LEVEL: "info"

  production:
    APP_ENV: "production"
    LOG_LEVEL: "warn"
```

### Feature Flags

```yaml
env:
  ENABLE_CACHING: "true"
  ENABLE_ANALYTICS: "false"
  MAX_WORKERS: "4"

commands:
  - shell: |
      if [ "$ENABLE_CACHING" = "true" ]; then
        echo "Caching enabled"
      fi
```

## Troubleshooting

### Variable Not Found

**Symptom:** `$VAR` appears literally in output or command fails with "command not found"

**Cause:** Variable not defined or incorrect interpolation syntax

**Solution:**

1. Verify variable is defined in `env` block or environment file
2. Check spelling and case (variable names are case-sensitive)
3. Ensure proper interpolation syntax (`$VAR` or `${VAR}`)
4. Use `--profile` flag if variable is profile-specific

### Secret Not Masked

**Symptom:** Sensitive value appears in logs or output

**Cause:** Variable not marked as secret

**Solution:**

```yaml
# Before (not masked)
env:
  API_KEY: "sk-abc123"

# After (masked)
env:
  API_KEY:
    secret: true
    value: "sk-abc123"
```

### Profile Variables Not Applied

**Symptom:** Default values used instead of profile values

**Cause:** Profile not activated with `--profile` flag

**Solution:**

```bash
# Activate profile
prodigy run workflow.yml --profile production
```

### Environment File Not Loaded

**Symptom:** Variables from `.env` file not available

**Cause:** File path incorrect or file doesn't exist

**Solution:**

1. Verify file path is relative to workflow file location
2. Check file exists: `ls .env.production`
3. Verify file syntax (KEY=VALUE format, no spaces around `=`)

## See Also

- [Workflow Structure](workflow-structure.md) - Overall workflow configuration
- [Variables and Interpolation](../variables/index.md) - Advanced variable interpolation
- [Command Types](../commands.md) - Using variables in different command types
