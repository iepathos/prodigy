# Environment Variables

Prodigy supports comprehensive environment variable management, enabling you to parameterize workflows, manage secrets securely, and use different configurations for different environments.

## Overview

Environment variables in Prodigy provide:
- **Global variables** - Workflow-wide configuration values
- **Secrets** - Secure credential management with automatic masking
- **Profiles** - Context-specific configurations (dev, staging, prod)
- **Environment files** - Load variables from .env files

## Global Environment Variables

Define workflow-wide environment variables in the `env` block:

```yaml
# Source: workflows/environment-example.yml
name: my-workflow
env:
  PROJECT_NAME: "prodigy"
  VERSION: "1.0.0"
  BUILD_DIR: "target/release"

steps:
  - shell: "echo Building $PROJECT_NAME version $VERSION"
  - shell: "cp binary $BUILD_DIR/"
```

## Secrets Management

Mark sensitive variables as secrets to automatically mask them in logs:

```yaml
env:
  API_KEY:
    secret: true
    value: "sk-abc123..."

  DATABASE_PASSWORD:
    secret: true
    value: "super-secret"
```

Secret values are masked in:
- Command output logs
- Error messages
- Event logs
- Checkpoint files

Example output:
```
$ curl -H 'Authorization: Bearer ***' https://api.example.com
```

## Profile Support

Use profiles to maintain different configurations for different environments:

```yaml
# Source: workflows/mapreduce-env-example.yml
env:
  API_URL:
    default: "http://localhost:3000"
    staging: "https://staging.api.com"
    prod: "https://api.com"

  DATABASE_URL:
    default: "postgres://localhost/dev"
    prod: "postgres://prod-server/db"
```

Activate a profile at runtime:

```bash
prodigy run workflow.yml --profile prod
```

### Profile-Specific Secrets

Combine profiles with secrets for environment-specific credentials:

```yaml
env:
  API_KEY:
    secret: true
    default: "sk-dev-abc123"
    staging: "sk-staging-def456"
    prod: "sk-prod-xyz789"

  DATABASE_PASSWORD:
    secret: true
    default: "dev-password"
    prod: "prod-secure-password"
```

This ensures sensitive credentials are:
- Automatically masked in all logs and output
- Environment-specific (different keys for dev vs prod)
- Managed securely without hardcoding in workflow files

## Environment Files

Load variables from .env files:

```yaml
env_files:
  - .env
  - .env.local
```

Variables in .env files follow standard format:

```
PROJECT_NAME=prodigy
VERSION=1.0.0
API_KEY=sk-abc123
```

## Variable Interpolation

Use environment variables in commands with two supported syntaxes:

- **`${VAR}`** - Recommended for clarity and complex expressions
- **`$VAR`** - Concise shell-style syntax for simple references

```yaml
# Source: workflows/environment-example.yml
- shell: "npm install --prefix $PROJECT_DIR"
- claude: "/analyze ${item.file} --config $CONFIG_PATH"
```

Both syntaxes work in all contexts, but `${VAR}` is preferred when:
- Embedding variables in complex strings
- Using with item field access (e.g., `${item.name}`)
- Avoiding ambiguity in variable names

## Usage in Workflow Phases

Environment variables are available throughout all workflow execution phases.

### Standard Workflows

```yaml
# Source: workflows/environment-example.yml
env:
  NODE_ENV: production
  API_URL: https://api.example.com

commands:
  - shell: "echo Building in $NODE_ENV mode"
  - shell: "curl ${API_URL}/health"
```

### MapReduce Workflows

Environment variables work across setup, map, and reduce phases:

```yaml
# Source: workflows/mapreduce-env-example.yml
name: mapreduce-example
mode: mapreduce

env:
  PROJECT_NAME: "my-project"
  OUTPUT_DIR: "results"
  MAX_RETRIES: "3"

# Setup phase: Initialize with env vars
setup:
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "echo Starting $PROJECT_NAME workflow"

# Map phase: Process items with env vars
map:
  input: "items.json"
  json_path: "$.items[*]"

  agent_template:
    - claude: "/process '${item.name}' --project $PROJECT_NAME"
    - shell: "echo Saving to $OUTPUT_DIR/${item.name}"
      on_failure:
        - claude: "/fix --max-retries $MAX_RETRIES"

# Reduce phase: Aggregate with env vars
reduce:
  - claude: "/summarize ${map.results} --project $PROJECT_NAME"
  - shell: "cp summary.json $OUTPUT_DIR/${PROJECT_NAME}-summary.json"
```

Environment variables are inherited by all map agents and available in reduce phase for result aggregation.

## Precedence Rules

When the same variable is defined in multiple places, precedence is:

1. Command-line arguments (highest priority)
2. Profile-specific values
3. Environment files
4. Default values
5. System environment variables (lowest priority)

## See Also

- [Variables and Interpolation](variables.md) - Variable syntax and advanced features
- [Workflow Structure](workflow-structure.md) - Workflow configuration basics
- [MapReduce Work Distribution](../mapreduce/work-distribution.md) - Using environment variables in parallel workflows
