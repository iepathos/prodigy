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

