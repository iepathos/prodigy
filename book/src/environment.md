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

## MapReduce Environment Variables

In MapReduce workflows, environment variables provide powerful parameterization across all phases (setup, map, reduce, and merge). This enables workflows to be reusable across different projects and configurations.

### Defining Environment Variables in MapReduce

Environment variables for MapReduce workflows follow the same global `env` field structure:

```yaml
name: mapreduce-workflow
mode: mapreduce

env:
  # Plain variables for parameterization
  PROJECT_NAME: "prodigy"
  PROJECT_CONFIG: "prodigy.yml"
  FEATURES_PATH: "specs/"
  OUTPUT_DIR: "results"

  # Workflow-specific settings
  MAX_RETRIES: "3"
  TIMEOUT_SECONDS: "300"

setup:
  - shell: "echo Starting $PROJECT_NAME workflow"
  - shell: "mkdir -p $OUTPUT_DIR"

map:
  input: "${FEATURES_PATH}/items.json"
  agent_template:
    - claude: "/process '${item.name}' --config $PROJECT_CONFIG"
    - shell: "test -f $PROJECT_NAME/${item.path}"

reduce:
  - shell: "echo Processed ${map.total} items for $PROJECT_NAME"
  - shell: "cp results.json $OUTPUT_DIR/"

merge:
  commands:
    - shell: "echo Merging $PROJECT_NAME changes"
```

### Variable Interpolation in MapReduce

MapReduce workflows support two interpolation syntaxes:

1. **`$VAR`** - Shell-style variable reference
2. **`${VAR}`** - Bracketed reference (recommended for clarity)

Both syntaxes work throughout all workflow phases:

```yaml
env:
  PROJECT_NAME: "myproject"
  CONFIG_FILE: "config.yml"

setup:
  - shell: "echo $PROJECT_NAME"           # Shell-style
  - shell: "echo ${PROJECT_NAME}"         # Bracketed
  - shell: "test -f ${CONFIG_FILE}"       # Recommended in paths

map:
  agent_template:
    - claude: "/analyze '${item}' --project $PROJECT_NAME"
```

### Environment Variables in All MapReduce Phases

#### Setup Phase

Environment variables are available for initialization:

```yaml
env:
  WORKSPACE_DIR: "/tmp/workspace"
  INPUT_SOURCE: "https://api.example.com/data"

setup:
  - shell: "mkdir -p $WORKSPACE_DIR"
  - shell: "curl $INPUT_SOURCE -o items.json"
  - shell: "echo Setup complete for ${WORKSPACE_DIR}"
```

#### Map Phase

Variables are available in agent templates:

```yaml
env:
  PROJECT_ROOT: "/path/to/project"
  CONFIG_PATH: "config/settings.yml"

map:
  agent_template:
    - claude: "/analyze ${item.file} --config $CONFIG_PATH"
    - shell: "test -f $PROJECT_ROOT/${item.file}"
    - shell: "cp ${item.file} $OUTPUT_DIR/"
```

#### Reduce Phase

Variables work in aggregation commands:

```yaml
env:
  PROJECT_NAME: "myproject"
  REPORT_DIR: "reports"

reduce:
  - claude: "/summarize ${map.results} --project $PROJECT_NAME"
  - shell: "mkdir -p $REPORT_DIR"
  - shell: "cp summary.json $REPORT_DIR/${PROJECT_NAME}-summary.json"
```

#### Merge Phase

Variables are available during merge operations:

```yaml
env:
  PROJECT_NAME: "myproject"
  NOTIFY_WEBHOOK: "https://hooks.example.com/notify"

merge:
  commands:
    - shell: "echo Merging $PROJECT_NAME changes"
    - claude: "/validate-merge --branch ${merge.source_branch}"
    - shell: "curl -X POST $NOTIFY_WEBHOOK -d 'project=$PROJECT_NAME'"
```

### Secrets in MapReduce

Sensitive data can be marked as secrets to enable automatic masking:

```yaml
env:
  PROJECT_NAME: "myproject"

secrets:
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"

  WEBHOOK_SECRET:
    provider: file
    key: "~/.secrets/webhook.key"

setup:
  - shell: "curl -H 'Authorization: Bearer $API_TOKEN' https://api.github.com/repos"

map:
  agent_template:
    - shell: "curl -X POST $WEBHOOK_URL -H 'X-Secret: $WEBHOOK_SECRET'"
```

Secrets are automatically masked in:
- Command output logs
- Error messages
- Event logs
- Checkpoint files

### Profile Support in MapReduce

Profiles enable different configurations for different environments:

```yaml
env:
  PROJECT_NAME: "myproject"

profiles:
  development:
    description: "Development environment"
    API_URL: "http://localhost:3000"
    DEBUG: "true"
    TIMEOUT_SECONDS: "60"

  production:
    description: "Production environment"
    API_URL: "https://api.example.com"
    DEBUG: "false"
    TIMEOUT_SECONDS: "300"

map:
  agent_template:
    - shell: "curl $API_URL/data"
    - shell: "timeout ${TIMEOUT_SECONDS}s ./process.sh"
```

Activate a profile:
```bash
prodigy run workflow.yml --profile production
```

### Reusable Workflows with Environment Variables

Environment variables enable the same workflow to work for different projects:

```yaml
# This workflow works for both Prodigy and Debtmap
name: check-book-docs-drift
mode: mapreduce

env:
  # Override these when running the workflow
  PROJECT_NAME: "prodigy"              # or "debtmap"
  PROJECT_CONFIG: "prodigy.yml"        # or "debtmap.yml"
  FEATURES_PATH: "specs/"              # or "features/"

setup:
  - shell: "echo Checking $PROJECT_NAME documentation"
  - shell: "./${PROJECT_NAME} generate-book-items --output items.json"

map:
  input: "items.json"
  agent_template:
    - claude: "/check-drift '${item}' --config $PROJECT_CONFIG"
    - shell: "git diff --exit-code ${item.doc_path}"

reduce:
  - claude: "/summarize-drift ${map.results} --project $PROJECT_NAME"
```

Run for different projects:
```bash
# For Prodigy
prodigy run workflow.yml

# For Debtmap
env PROJECT_NAME=debtmap PROJECT_CONFIG=debtmap.yml FEATURES_PATH=features/ \
  prodigy run workflow.yml
```

### Best Practices for MapReduce Environment Variables

1. **Parameterize project-specific values**:
   ```yaml
   env:
     PROJECT_NAME: "myproject"
     PROJECT_ROOT: "/workspace"
     CONFIG_FILE: "config.yml"
   ```

2. **Use consistent naming**:
   - Use UPPER_CASE for environment variables
   - Use descriptive names (PROJECT_NAME not PN)
   - Group related variables with prefixes (AWS_*, DB_*)

3. **Document variables**:
   ```yaml
   env:
     # Project identifier used in logs and reports
     PROJECT_NAME: "prodigy"

     # Path to project configuration file
     PROJECT_CONFIG: "prodigy.yml"

     # Maximum concurrent agents (tune based on resources)
     MAX_PARALLEL: "10"
   ```

4. **Use secrets for sensitive data**:
   ```yaml
   secrets:
     GITHUB_TOKEN:
       provider: env
       key: "GH_TOKEN"
   ```

5. **Prefer `${VAR}` syntax**:
   ```yaml
   # Good - explicit and safe
   - shell: "test -f ${CONFIG_PATH}"

   # Risky - may fail with special characters
   - shell: "test -f $CONFIG_PATH"
   ```

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
