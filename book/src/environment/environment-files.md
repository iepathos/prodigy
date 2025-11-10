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

Environment files are loaded in order, with later files overriding earlier files. This enables layered configuration:

```yaml
env_files:
  - .env                # Base configuration
  - .env.local          # Local overrides (gitignored)
  - .env.production     # Environment-specific settings
```

Example override behavior:

```bash
# .env (base)
DATABASE_URL=postgresql://localhost:5432/dev
API_TIMEOUT=30

# .env.production (overrides)
DATABASE_URL=postgresql://prod-server:5432/app
# API_TIMEOUT remains 30 from base file
```

Precedence order (highest to lowest):
1. Global `env` field in workflow YAML
2. Later files in `env_files` list
3. Earlier files in `env_files` list
4. Parent process environment

**Error Handling:**

If an env file specified in `env_files` does not exist or contains invalid syntax, Prodigy will report an error and halt workflow execution. Use absolute paths or paths relative to the workflow file location.

---

### Integration with Profiles and Secrets

Environment files work seamlessly with other environment features like profiles and secrets management.

**Combining env_files with profiles:**

```yaml
# Base configuration in .env file
env_files:
  - .env

# Profile-specific overrides
profiles:
  dev:
    API_URL: http://localhost:3000
    DEBUG: "true"
  prod:
    API_URL: https://api.production.com
    DEBUG: "false"

commands:
  - shell: "echo $API_URL"  # Uses profile value if active, otherwise .env value
```

The precedence order is:
1. Profile-specific values (if profile active)
2. Global `env` field values
3. Environment file values (later files override earlier)
4. Parent process environment

**Loading secrets from env_files:**

Environment files can contain secrets, but you must explicitly mark them as secrets in the workflow configuration:

```yaml
# .env.secrets file
API_KEY=sk-abc123xyz
DATABASE_PASSWORD=secret-password

# Workflow configuration
env_files:
  - .env.secrets

secrets:
  # Retrieve from environment variable (loaded from .env.secrets)
  API_KEY: "${env:API_KEY}"
  DATABASE_PASSWORD: "${env:DATABASE_PASSWORD}"
```

**Note:** Variables loaded from env_files are NOT automatically masked. You must explicitly mark them as secrets in the `secrets` section for masking in logs.

**Complete integration example:**

```yaml
# Layered configuration strategy
env_files:
  - .env                # Base configuration
  - .env.local          # Local overrides (gitignored)
  - .env.${ENVIRONMENT} # Environment-specific (e.g., .env.production)

env:
  PROJECT_NAME: my-project
  VERSION: "1.0.0"

secrets:
  # Secrets loaded from env files, masked in logs
  API_KEY: "${env:API_KEY}"
  DATABASE_URL: "${env:DATABASE_URL}"

profiles:
  dev:
    MAX_WORKERS: "2"
    TIMEOUT: "60"
  prod:
    MAX_WORKERS: "20"
    TIMEOUT: "30"

commands:
  - shell: "echo 'Project: $PROJECT_NAME v$VERSION'"
  - shell: "echo 'Workers: $MAX_WORKERS, Timeout: $TIMEOUT'"
  - shell: "curl -H 'Authorization: Bearer ***' $API_URL"  # API_KEY masked
```

**Best practices for organizing env files:**

1. **.env**: Base configuration, safe to commit (no secrets)
2. **.env.local**: Personal overrides, add to .gitignore
3. **.env.production / .env.staging / .env.dev**: Environment-specific, may contain encrypted secrets
4. **.env.secrets**: Sensitive values, NEVER commit, always in .gitignore

**Precedence example:**

```bash
# .env (base)
API_URL=http://localhost:3000
MAX_WORKERS=5
TIMEOUT=30

# .env.production (overrides)
API_URL=https://api.production.com
MAX_WORKERS=20
```

```yaml
env_files:
  - .env
  - .env.production  # Overrides API_URL and MAX_WORKERS

env:
  TIMEOUT: "60"      # Overrides TIMEOUT from both files

profiles:
  prod:
    MAX_WORKERS: "50"  # Overrides MAX_WORKERS when --profile prod used
```

Final values when running with `--profile prod`:
- `API_URL`: `https://api.production.com` (from .env.production)
- `MAX_WORKERS`: `50` (from prod profile - highest precedence)
- `TIMEOUT`: `60` (from global env field - overrides files)

---

