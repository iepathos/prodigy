## Environment Variables

Prodigy supports two types of environment variables:

1. **System Environment Variables**: Standard Unix environment variables that control Prodigy's behavior globally
2. **Workflow Environment Variables**: Variables defined in workflow YAML files that are available during workflow execution

This page documents both types. For details on how environment variables interact with other configuration sources, see [Configuration Precedence Rules](configuration-precedence-rules.md).

---

## Workflow Environment Variables

Workflows can define custom environment variables using the `env:` block. These variables are available to all commands within the workflow and support advanced features like secrets, profiles, and interpolation.

### Basic Syntax

```yaml
name: my-workflow

env:
  # Plain variables
  PROJECT_NAME: "prodigy"
  VERSION: "1.0.0"
  BUILD_DIR: "target/release"

commands:
  - shell: "echo Building $PROJECT_NAME version $VERSION"
  - shell: "cargo build --release --target-dir $BUILD_DIR"
```

### Variable Interpolation

Workflow environment variables can be referenced using two syntaxes:

- **`$VAR`** - Simple variable reference (shell-style)
- **`${VAR}`** - Bracketed reference (recommended for clarity and complex expressions)

```yaml
env:
  API_URL: "https://api.example.com"
  API_VERSION: "v2"
  ENDPOINT: "${API_URL}/${API_VERSION}"

commands:
  - shell: "curl ${ENDPOINT}/status"
  - claude: "/deploy --url $API_URL --version $API_VERSION"
```

### Secrets and Sensitive Data

Mark sensitive values as secrets to automatically mask them in logs and output:

```yaml
env:
  # Public configuration
  DATABASE_HOST: "db.example.com"

  # Secret configuration (masked in logs)
  DATABASE_PASSWORD:
    secret: true
    value: "super-secret-password"

  API_KEY:
    secret: true
    value: "sk-abc123..."

commands:
  - shell: "psql -h $DATABASE_HOST -p $DATABASE_PASSWORD"
  # Output: psql -h db.example.com -p ***
```

**Security Best Practices**:
- Always mark API keys, passwords, and tokens as secrets
- Never commit secret values to version control
- Use environment variable references for secrets: `value: "${PROD_API_KEY}"`
- Rotate secrets regularly

### Profiles for Multiple Environments

Profiles allow different values for different environments (dev, staging, prod):

```yaml
env:
  # API endpoints vary by environment
  API_URL:
    default: "http://localhost:3000"
    staging: "https://staging.api.com"
    prod: "https://api.com"

  # Credentials vary by environment
  API_KEY:
    secret: true
    default: "dev-key-123"
    staging:
      secret: true
      value: "${STAGING_API_KEY}"  # From system env
    prod:
      secret: true
      value: "${PROD_API_KEY}"

commands:
  - shell: "curl -H 'Authorization: Bearer ${API_KEY}' ${API_URL}/health"
```

**Activate a profile**:

```bash
# Use staging profile
prodigy run workflow.yml --profile staging

# Use prod profile via environment variable
export PRODIGY_PROFILE=prod
prodigy run workflow.yml
```

### Step-Level Environment Overrides

Individual commands can override workflow environment variables:

```yaml
env:
  NODE_ENV: "development"
  LOG_LEVEL: "info"

commands:
  # Uses workflow-level NODE_ENV
  - shell: "npm test"

  # Override for this command only
  - shell: "npm run build"
    env:
      NODE_ENV: "production"
      LOG_LEVEL: "warn"

  # Back to workflow-level NODE_ENV
  - shell: "npm start"
```

**Precedence**: Step env > Profile env > Workflow env > System env

### MapReduce Environment Variables

Environment variables work across all MapReduce phases (setup, map, reduce, merge):

```yaml
name: parallel-processing
mode: mapreduce

env:
  MAX_PARALLEL: "10"
  TIMEOUT: "300"
  OUTPUT_DIR: "/tmp/results"

setup:
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "generate-work-items.sh > items.json"

map:
  input: "items.json"
  json_path: "$[*]"
  max_parallel: ${MAX_PARALLEL}  # Use env var for parallelism

  agent_template:
    - claude: "/process ${item.file} --timeout $TIMEOUT"
    - shell: "cp result.json ${OUTPUT_DIR}/${item.name}.json"

reduce:
  - shell: "echo Processed ${map.total} items to $OUTPUT_DIR"
```

**Advanced MapReduce Usage**:
- Use env vars for `max_parallel`, `timeout`, `agent_timeout_secs`
- Reference in `filter` and `sort_by` expressions
- Pass to validation and gap-filling commands

### Environment Files (`.env`)

Load variables from dotenv-format files (not yet implemented in Prodigy, but planned):

```yaml
env:
  env_files:
    - ".env"
    - ".env.${PRODIGY_PROFILE}"

# .env file format:
# PROJECT_NAME=prodigy
# VERSION=1.0.0
# API_KEY=sk-abc123
```

**Note**: This feature is planned but not yet available. Use system environment variables as a workaround.

### Complete Workflow Example

```yaml
name: deployment-workflow

env:
  # Project configuration
  PROJECT_NAME: "my-app"
  VERSION: "2.1.0"

  # Environment-specific settings
  DEPLOY_TARGET:
    default: "dev-server"
    staging: "staging-cluster"
    prod: "prod-cluster"

  # Secrets (masked in logs)
  DEPLOY_TOKEN:
    secret: true
    default: "${DEV_TOKEN}"
    prod:
      secret: true
      value: "${PROD_TOKEN}"

commands:
  - shell: "echo Deploying $PROJECT_NAME v$VERSION to $DEPLOY_TARGET"
  - shell: "docker build -t ${PROJECT_NAME}:${VERSION} ."
  - shell: "deploy --target $DEPLOY_TARGET --token $DEPLOY_TOKEN"
  # Output: deploy --target prod-cluster --token ***
```

Run with:

```bash
# Development deployment
prodigy run deploy.yml

# Production deployment
export PROD_TOKEN="secret-prod-token"
prodigy run deploy.yml --profile prod
```

---

## System Environment Variables

System environment variables control Prodigy's global behavior and configuration.

### Claude API Configuration

#### `PRODIGY_CLAUDE_API_KEY`

**Purpose**: Claude API key for AI-powered commands
**Default**: None
**Overrides**: Global and project `claude_api_key` settings

```bash
export PRODIGY_CLAUDE_API_KEY="sk-ant-api03-..."
```

This is the **recommended** way to provide API keys (more secure than storing in config files).

#### `PRODIGY_CLAUDE_STREAMING`

**Purpose**: Control Claude JSON streaming output
**Default**: `true` (streaming enabled by default)
**Valid values**: `true`, `false`

```bash
export PRODIGY_CLAUDE_STREAMING=false  # Disable streaming
```

When `false`, uses legacy print mode instead of JSON streaming.

### General Configuration

#### `PRODIGY_LOG_LEVEL`

**Purpose**: Logging verbosity
**Default**: `info`
**Valid values**: `trace`, `debug`, `info`, `warn`, `error`
**Overrides**: Global and project `log_level` settings

```bash
export PRODIGY_LOG_LEVEL=debug
```

#### `PRODIGY_EDITOR`

**Purpose**: Default text editor for interactive operations
**Default**: None
**Overrides**: Global `default_editor` setting
**Fallback**: `EDITOR` environment variable

```bash
export PRODIGY_EDITOR=vim
```

If neither `PRODIGY_EDITOR` nor `EDITOR` is set, Prodigy uses system defaults.

#### `EDITOR`

**Purpose**: Standard Unix editor variable (fallback)
**Default**: None
**Fallback for**: `PRODIGY_EDITOR`

```bash
export EDITOR=nano
```

**Precedence**: `PRODIGY_EDITOR` takes precedence over `EDITOR` if both are set.

#### `PRODIGY_AUTO_COMMIT`

**Purpose**: Automatic commit after successful commands
**Default**: `true`
**Valid values**: `true`, `false`
**Overrides**: Global and project `auto_commit` settings

```bash
export PRODIGY_AUTO_COMMIT=false
```

### Storage Configuration

#### `PRODIGY_STORAGE_TYPE`

**Purpose**: Storage backend type
**Default**: `file`
**Valid values**: `file`, `memory`
**Overrides**: Storage `backend` setting

```bash
export PRODIGY_STORAGE_TYPE=file
```

#### `PRODIGY_STORAGE_BASE_PATH`

**Purpose**: Base directory for file storage
**Default**: `~/.prodigy`
**Overrides**: Storage `backend_config.base_dir` setting

```bash
export PRODIGY_STORAGE_BASE_PATH=/custom/storage/path
```

**Alternative names** (deprecated, use `PRODIGY_STORAGE_BASE_PATH`):
- `PRODIGY_STORAGE_DIR`
- `PRODIGY_STORAGE_PATH`

### Workflow Execution

#### `PRODIGY_AUTOMATION`

**Purpose**: Signal automated execution mode
**Default**: Not set
**Set by**: Prodigy when executing workflows

```bash
export PRODIGY_AUTOMATION=true
```

This variable is **set automatically** by Prodigy during workflow execution. It signals to Claude and other tools that execution is automated (not interactive).

#### `PRODIGY_CLAUDE_CONSOLE_OUTPUT`

**Purpose**: Force Claude streaming output regardless of verbosity
**Default**: Not set
**Valid values**: `true`, `false`

```bash
export PRODIGY_CLAUDE_CONSOLE_OUTPUT=true
```

When set to `true`, forces JSON streaming output even when verbosity is 0. Useful for debugging specific runs without changing command flags.

### Complete Example

Set up a complete Prodigy environment:

```bash
# API key (recommended method)
export PRODIGY_CLAUDE_API_KEY="sk-ant-api03-..."

# Logging
export PRODIGY_LOG_LEVEL=info

# Editor
export PRODIGY_EDITOR=code

# Behavior
export PRODIGY_AUTO_COMMIT=true

# Storage
export PRODIGY_STORAGE_TYPE=file
export PRODIGY_STORAGE_BASE_PATH=/Users/username/.prodigy

# Development/debugging
export PRODIGY_CLAUDE_STREAMING=true
export PRODIGY_CLAUDE_CONSOLE_OUTPUT=false
```

### Environment Files

You can use `.env` files (not committed to version control) to manage environment variables:

```bash
# .env (add to .gitignore)
PRODIGY_CLAUDE_API_KEY=sk-ant-api03-...
PRODIGY_LOG_LEVEL=debug
PRODIGY_AUTO_COMMIT=false
```

Load with:

```bash
# Using direnv
eval "$(cat .env)"

# Using dotenv tool
dotenv run prodigy run workflow.yml

# Manually
export $(cat .env | xargs)
```

### Security Best Practices

1. **Never commit API keys** to version control
2. **Use environment variables** for secrets (not config files)
3. **Use `.env` files** (gitignored) for local development
4. **Use secret managers** (AWS Secrets Manager, Vault) in production
5. **Rotate keys regularly** and use project-specific keys when possible

**Example `.gitignore`**:

```
.env
.env.*
!.env.example
.prodigy/config.local.yml
```

### Precedence Summary

For any given setting, the effective value comes from (highest to lowest):

1. **CLI flags** (if applicable)
2. **Environment variables** ← This level
3. **Project config** (`.prodigy/config.yml`)
4. **Global config** (`~/.prodigy/config.yml`)
5. **Defaults** (built-in values)

Example:

```yaml
# ~/.prodigy/config.yml
log_level: info
```

```yaml
# .prodigy/config.yml
log_level: warn
```

```bash
export PRODIGY_LOG_LEVEL=debug  # This wins
```

**Result**: `log_level: debug`

### Checking Environment Variables

To see which environment variables are active:

```bash
# List all PRODIGY_* variables
env | grep PRODIGY_

# Check effective configuration (merges all sources)
prodigy config show
```
