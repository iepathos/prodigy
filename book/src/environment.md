# Environment Configuration

## Global Environment Configuration

```yaml
# Inherit parent process environment (default: true)
inherit: true

# Global environment variables
env:
  # Static variables (EnvValue::Static)
  NODE_ENV: production
  PORT: "3000"

  # Dynamic variables (EnvValue::Dynamic - computed from command)
  WORKER_COUNT:
    command: "nproc || echo 4"
    cache: true  # Cache result for reuse

  # Conditional variables (EnvValue::Conditional)
  DEPLOY_TARGET:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"
```

**Environment Control:**
- `inherit: false` - Start with clean environment instead of inheriting from parent process (default: true)

---

## Secrets Management

```yaml
secrets:
  # Simple format (syntactic sugar - parsed into structured format)
  API_KEY: "${env:SECRET_API_KEY}"
  DB_PASSWORD: "${file:~/.secrets/db.pass}"

  # Structured format (Provider variant)
  AWS_SECRET:
    provider: aws
    key: "my-app/api-key"

  VAULT_SECRET:
    provider: vault
    key: "secret/data/myapp"
    version: "v2"  # Optional version

  # Custom provider
  CUSTOM_SECRET:
    provider: custom-provider
    key: "secret-id"
```

**Supported Secret Providers:**
- `env` - Environment variable reference
- `file` - Read from file
- `vault` - HashiCorp Vault integration
- `aws` - AWS Secrets Manager
- `custom` - Custom provider (extensible)

---

## Environment Profiles

```yaml
profiles:
  development:
    description: "Development environment with debug enabled"
    env:
      NODE_ENV: development
      DEBUG: "true"
      API_URL: http://localhost:3000

  production:
    description: "Production environment configuration"
    env:
      NODE_ENV: production
      DEBUG: "false"
      API_URL: https://api.example.com

# Activate profile globally
active_profile: "development"
# OR use dynamic profile selection
active_profile: "${DEPLOY_ENV}"

commands:
  - shell: "npm run build"
```

**Note:** Profile activation uses the `active_profile` field at the root WorkflowConfig level, not at the command level.

---

## Step-Level Environment Configuration

Commands support step-specific environment configuration with advanced control:

```yaml
# Basic step-level environment variables
- shell: "echo $API_URL"
  env:
    API_URL: "https://api.staging.com"
    DEBUG: "true"

# Advanced step environment features
- shell: "isolated-command.sh"
  working_dir: "/tmp/sandbox"  # Change working directory
  clear_env: true              # Clear all parent environment variables
  temporary: true              # Restore previous environment after step
  env:
    ISOLATED_VAR: "value"
```

**Step Environment Fields:**
- `env` - Step-specific environment variables (HashMap<String, String>)
- `working_dir` - Working directory for command execution
- `clear_env` - Start with clean environment (default: false)
- `temporary` - Restore previous environment after step completes (default: false)
