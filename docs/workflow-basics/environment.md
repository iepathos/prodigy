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

Use environment variables in commands with `$VAR` or `${VAR}` syntax:

```yaml
- shell: "npm install --prefix $PROJECT_DIR"
- claude: "/analyze ${item.file} --config $CONFIG_PATH"
```

## Precedence Rules

When the same variable is defined in multiple places, precedence is:

1. Command-line arguments (highest priority)
2. Profile-specific values
3. Environment files
4. Default values
5. System environment variables (lowest priority)

## See Also

- [Variables and Interpolation](variables.md) - Variable syntax and usage
- [Workflow Structure](workflow-structure.md) - Workflow configuration basics
- [Global Configuration](../configuration/global-config.md) - System-wide settings
