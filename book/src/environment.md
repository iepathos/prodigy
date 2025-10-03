# Environment Configuration

Prodigy provides flexible environment configuration for workflows, allowing you to manage environment variables, secrets, profiles, and step-specific settings. This chapter explains the user-facing configuration options available in workflow YAML files.

## Architecture Overview

Prodigy uses a two-layer architecture for environment management:

1. **WorkflowConfig**: User-facing YAML configuration with `env`, `secrets`, `profiles`, and `env_files` fields
2. **EnvironmentConfig**: Internal runtime configuration that extends workflow config with additional features

This chapter documents the user-facing WorkflowConfig layer - what you write in your workflow YAML files.

---

## Global Environment Variables

Define static environment variables that apply to all commands in your workflow:

```yaml
# Global environment variables (static strings only)
env:
  NODE_ENV: production
  PORT: "3000"
  API_URL: https://api.example.com
  DEBUG: "false"

commands:
  - shell: "echo $NODE_ENV"  # Uses global environment
```

**Important:** The `env` field at the workflow level only supports static string values. Dynamic or conditional environment variables are handled internally by the runtime but are not directly exposed in workflow YAML.

**Environment Inheritance:** Parent process environment variables are always inherited by default. All global environment variables are merged with the parent environment.

---

## Environment Files

Load environment variables from `.env` files:

```yaml
# Environment files to load
env_files:
  - .env
  - .env.local
  - config/.env.production

commands:
  - shell: "echo $DATABASE_URL"
```

**Environment File Format:**

Environment files use the standard `.env` format with `KEY=value` pairs:

```bash
# .env file example
DATABASE_URL=postgresql://localhost:5432/mydb
REDIS_HOST=localhost
REDIS_PORT=6379

# Comments are supported
API_KEY=secret-key-here

# Multi-line values use quotes
PRIVATE_KEY="-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBg...
-----END PRIVATE KEY-----"
```

**Loading Order and Precedence:**

1. Files are loaded in the order specified in `env_files`
2. Later files override earlier files
3. Step-level `env` overrides environment files
4. Global `env` overrides environment files

---

## Secrets Management

Store sensitive values securely using secret providers:

```yaml
secrets:
  # Provider-based secrets (recommended)
  AWS_SECRET:
    provider: aws
    key: "my-app/api-key"

  VAULT_SECRET:
    provider: vault
    key: "secret/data/myapp"
    version: "v2"  # Optional version

  # Environment variable reference
  API_KEY:
    provider: env
    key: "SECRET_API_KEY"

  # File-based secret
  DB_PASSWORD:
    provider: file
    key: "~/.secrets/db.pass"

  # Custom provider (extensible)
  CUSTOM_SECRET:
    provider:
      custom: "my-custom-provider"
    key: "secret-id"

commands:
  - shell: "echo $API_KEY"  # Secrets are available as environment variables
```

**Supported Secret Providers:**

- `env` - Reference another environment variable
- `file` - Read secret from a file
- `vault` - HashiCorp Vault integration (requires Vault setup)
- `aws` - AWS Secrets Manager (requires AWS credentials)
- `custom` - Custom provider (extensible for your own secret backends)

**Security Notes:**

- Secrets are masked in logs and output
- Secret values are only resolved at runtime
- Use secrets for API keys, passwords, tokens, and other sensitive data

---

## Environment Profiles

Define named environment configurations for different contexts:

```yaml
# Define profiles with environment variables
profiles:
  development:
    description: "Development environment with debug enabled"
    NODE_ENV: development
    DEBUG: "true"
    API_URL: http://localhost:3000

  production:
    description: "Production environment configuration"
    NODE_ENV: production
    DEBUG: "false"
    API_URL: https://api.example.com

# Global environment still applies
env:
  APP_NAME: "my-app"

commands:
  - shell: "npm run build"
```

**Profile Structure:**

Profiles use a flat structure where environment variables are defined directly at the profile level (not nested under an `env:` key). The `description` field is optional and helps document the profile's purpose.

```yaml
profiles:
  staging:
    description: "Staging environment"  # Optional
    NODE_ENV: staging                   # Direct key-value pairs
    API_URL: https://staging.api.com
    DEBUG: "true"
```

**Note:** Profile activation is managed internally by the runtime environment manager. The selection mechanism is not currently exposed in WorkflowConfig YAML. Profiles are defined for future use and internal runtime configuration.

---

## Step-Level Environment

Commands can specify their own environment variables and working directory:

```yaml
commands:
  # Basic step-level environment variables
  - shell: "echo $API_URL"
    env:
      API_URL: "https://api.staging.com"
      DEBUG: "true"

  # Step with custom working directory
  - shell: "pwd && ls -la"
    working_dir: "/tmp/sandbox"
    env:
      TEMP_VAR: "value"

  # Step overrides global environment
  - shell: "echo $NODE_ENV"
    env:
      NODE_ENV: "test"  # Overrides global NODE_ENV
```

**Available Step-Level Fields:**

- `env` - Step-specific environment variables (HashMap<String, String>)
  - Merged with global environment
  - Step env overrides global env for conflicting keys
  - Supports variable interpolation with `${variable}` syntax

- `working_dir` - Working directory for command execution
  - Can be an absolute or relative path
  - Relative paths are resolved from the workflow root
  - Default: workflow root directory

**Variable Interpolation:**

Step environment variables support interpolation from the workflow variable context:

```yaml
commands:
  - shell: "echo 'Version: $APP_VERSION'"
    env:
      APP_VERSION: "${VERSION}"  # Interpolate from workflow variables
      BUILD_TAG: "build-${BUILD_NUMBER}"
```

---

## Environment Precedence

Environment variables are resolved in the following order (highest to lowest precedence):

1. **Step-level `env`** - Defined on individual commands
2. **Active profile** - If a profile is activated (internal)
3. **Global `env`** - Defined at workflow level
4. **Environment files** - Loaded from `env_files` (in order)
5. **Parent environment** - Inherited from the parent process

Example demonstrating precedence:

```yaml
# Parent environment: NODE_ENV=local

env_files:
  - .env  # Contains: NODE_ENV=development

env:
  NODE_ENV: production  # Overrides .env file

profiles:
  test:
    NODE_ENV: test  # Overrides global env when profile is active

commands:
  - shell: "echo $NODE_ENV"  # Prints: production (global env)

  - shell: "echo $NODE_ENV"  # Prints: staging (step env overrides global)
    env:
      NODE_ENV: staging
```

---

## Best Practices

### 1. Use Environment Files for Configuration

Store configuration in `.env` files instead of hardcoding in YAML:

```yaml
# Good: Load from files
env_files:
  - .env
  - .env.${ENVIRONMENT}

# Avoid: Hardcoding sensitive values
env:
  API_KEY: "hardcoded-key-here"  # Don't do this!
```

### 2. Use Secrets for Sensitive Data

Always use the `secrets` field for sensitive information:

```yaml
# Good: Use secrets provider
secrets:
  DATABASE_PASSWORD:
    provider: vault
    key: "db/password"

# Bad: Sensitive data in plain env
env:
  DATABASE_PASSWORD: "my-password"  # Don't do this!
```

### 3. Leverage Profiles for Environments

Define profiles for different deployment environments:

```yaml
profiles:
  development:
    NODE_ENV: development
    LOG_LEVEL: debug
    API_URL: http://localhost:3000

  production:
    NODE_ENV: production
    LOG_LEVEL: error
    API_URL: https://api.example.com
```

### 4. Use Step Environment for Overrides

Override global settings for specific commands:

```yaml
env:
  RUST_LOG: info

commands:
  # Most commands use info level
  - shell: "cargo run"

  # But this command needs debug level
  - shell: "cargo run --verbose"
    env:
      RUST_LOG: debug
```

### 5. Document Your Environment Variables

Add comments to explain environment variables:

```yaml
env:
  # Number of worker threads (adjust based on CPU cores)
  WORKER_COUNT: "4"

  # API rate limit (requests per minute)
  RATE_LIMIT: "1000"

  # Feature flags
  ENABLE_BETA_FEATURES: "false"
```

---

## Common Patterns

### Multi-Environment Workflows

```yaml
# Load environment-specific configuration
env_files:
  - .env.${ENVIRONMENT}

env:
  APP_NAME: "my-app"

commands:
  - shell: "npm run deploy"
```

### Secrets with Fallbacks

```yaml
secrets:
  # Try Vault first, fall back to environment variable
  API_KEY:
    provider: vault
    key: "api/key"

env:
  # Fallback for local development
  API_KEY: "${API_KEY:-default-key}"
```

### Build Matrix with Profiles

```yaml
profiles:
  debug:
    CARGO_PROFILE: debug
    RUST_BACKTRACE: "1"

  release:
    CARGO_PROFILE: release
    RUST_BACKTRACE: "0"

commands:
  - shell: "cargo build --profile ${CARGO_PROFILE}"
```

### Temporary Environment Changes

```yaml
commands:
  # Set PATH for this command only
  - shell: "./node_modules/.bin/webpack"
    working_dir: "frontend"
    env:
      PATH: "${PWD}/node_modules/.bin:${PATH}"

  # PATH is back to normal for subsequent commands
  - shell: "echo $PATH"
```
