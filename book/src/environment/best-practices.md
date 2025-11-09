## Best Practices

This section provides guidelines for effective environment variable management, secrets handling, and profile organization in Prodigy workflows.

### Environment Variable Usage

**When to use environment variables:**
- Configuration values that change between environments (dev/staging/prod)
- Parameterization for reusable workflows
- Non-sensitive API endpoints and URLs
- Timeouts, limits, and resource constraints
- Project-specific paths and file locations

**When to use profiles instead:**
- Environment-specific configuration sets (dev/staging/prod)
- Multiple related values that change together
- When you need to switch entire configuration contexts

**When to use env_files instead:**
- Loading many variables from external sources
- Sharing configuration across multiple workflows
- Managing .env files in version control (without secrets)
- Local development overrides (.env.local)

### Naming Conventions

**Follow these naming conventions for clarity:**

1. **Use UPPERCASE with underscores**: `API_URL`, `MAX_WORKERS`, `DATABASE_URL`
2. **Be descriptive**: `AGENT_TIMEOUT` not `TIMEOUT`, `API_BASE_URL` not `URL`
3. **Use prefixes for grouping**: `DB_HOST`, `DB_PORT`, `DB_NAME` or `REDIS_HOST`, `REDIS_PORT`
4. **Avoid abbreviations**: `PROJECT_NAME` not `PROJ_NM`, `ENVIRONMENT` not `ENV`

**Examples:**

```yaml
env:
  # Good - Clear and descriptive
  PROJECT_NAME: my-project
  API_BASE_URL: https://api.example.com
  MAX_PARALLEL_WORKERS: "10"
  CLAUDE_COMMAND_TIMEOUT: "600"

  # Avoid - Unclear or abbreviated
  PROJ: my-project
  URL: https://api.example.com
  MAX: "10"
  TIMEOUT: "600"
```

### Secrets Management Guidelines

**Critical security practices:**

1. **NEVER commit secrets to version control**
   - Add `.env.secrets`, `.env.local` to `.gitignore`
   - Use secret providers (vault, aws-secrets) for production
   - Rotate secrets regularly

2. **Always mark secrets with `secret: true`**
   ```yaml
   secrets:
     API_KEY:
       secret: true
       value: sk-abc123...  # Masked in all logs
   ```

3. **Use external secret providers in production**
   ```yaml
   secrets:
     API_KEY:
       secret: true
       provider: vault
       path: secret/data/api-keys
   ```

4. **Minimize secret exposure**
   - Only expose secrets to commands that need them
   - Use short-lived tokens when possible
   - Prefer secret providers over inline values

5. **Verify secret masking**
   - Check logs to ensure secrets are masked (appear as `***`)
   - Test with `-v` verbose mode to confirm masking works

**Secret organization pattern:**

```yaml
# Good - Organized and provider-backed
env_files:
  - .env              # Non-sensitive config
  - .env.local        # Local overrides (gitignored)

secrets:
  # Production secrets from Vault
  DATABASE_PASSWORD:
    secret: true
    provider: vault
    path: secret/data/db/prod

  API_KEY:
    secret: true
    provider: aws
    secret_name: api-key-prod

  # Local dev secrets from env file (for local testing only)
  DEV_API_KEY:
    secret: true
    # Loaded from .env.local, still masked
```

### Profile Organization Strategies

**Multi-environment structure:**

Use profiles to manage dev/staging/prod configurations:

```yaml
env:
  # Common values shared across all environments
  PROJECT_NAME: my-project
  VERSION: "1.0.0"

profiles:
  dev:
    API_URL: http://localhost:3000
    MAX_WORKERS: "2"
    TIMEOUT: "60"
    DEBUG: "true"

  staging:
    API_URL: https://staging-api.example.com
    MAX_WORKERS: "10"
    TIMEOUT: "30"
    DEBUG: "true"

  prod:
    API_URL: https://api.example.com
    MAX_WORKERS: "20"
    TIMEOUT: "30"
    DEBUG: "false"
```

**Activate with:** `prodigy run workflow.yml --profile prod`

**Base + override pattern:**

```yaml
env:
  # Base/default values
  API_URL: http://localhost:3000
  MAX_WORKERS: "5"
  CACHE_ENABLED: "true"

profiles:
  prod:
    # Only override what changes in prod
    API_URL: https://api.production.com
    MAX_WORKERS: "20"
    # CACHE_ENABLED inherits "true" from base
```

### Avoiding Common Pitfalls

**1. Variable name conflicts:**

```yaml
# Problem - Confusing overlap with shell variables
env:
  PATH: /custom/bin  # Conflicts with system PATH
  HOME: /app         # Conflicts with system HOME

# Solution - Use prefixed names
env:
  APP_PATH: /custom/bin
  APP_HOME: /app
```

**2. Precedence confusion:**

Remember the order (highest to lowest):
1. Profile values (when profile active)
2. Global `env` field
3. Later `env_files` entries
4. Earlier `env_files` entries
5. Parent process environment

**3. Forgetting to mark secrets:**

```yaml
# Problem - Secret exposed in logs
env_files:
  - .env.secrets  # Contains API_KEY=sk-abc123

commands:
  - shell: "curl -H 'Authorization: Bearer $API_KEY' ..."
  # API_KEY appears in logs!

# Solution - Explicitly mark as secret
secrets:
  API_KEY:
    secret: true
```

**4. Hardcoding environment-specific values:**

```yaml
# Problem - Not reusable across environments
commands:
  - shell: "curl https://api.production.com/data"
  - shell: "timeout 30 ./process.sh"

# Solution - Use environment variables
env:
  API_URL: https://api.production.com
  TIMEOUT: "30"

commands:
  - shell: "curl $API_URL/data"
  - shell: "timeout $TIMEOUT ./process.sh"
```

### Documentation Practices

**Document your environment configuration:**

1. **List required variables** in README or workflow comments:
   ```yaml
   # Required environment variables:
   # - API_URL: Base URL for API endpoints
   # - MAX_WORKERS: Number of parallel workers (default: 5)
   # - TIMEOUT: Command timeout in seconds (default: 30)

   env:
     API_URL: https://api.example.com
     MAX_WORKERS: "5"
     TIMEOUT: "30"
   ```

2. **Provide example .env file**:
   ```bash
   # .env.example (safe to commit)
   API_URL=https://api.example.com
   MAX_WORKERS=5
   TIMEOUT=30

   # Copy to .env and fill in values:
   # cp .env.example .env
   ```

3. **Document profile usage**:
   ```yaml
   # Profiles available:
   # - dev: Local development (low resource usage)
   # - staging: Staging environment (medium resources)
   # - prod: Production environment (high resources)
   #
   # Usage: prodigy run workflow.yml --profile prod
   ```

### Testing Environment Configurations

**Validate your environment setup:**

1. **Test with dry-run mode** (if available)
2. **Verify variable interpolation**:
   ```yaml
   commands:
     - shell: "echo 'API_URL: $API_URL'"
     - shell: "echo 'MAX_WORKERS: $MAX_WORKERS'"
     - shell: "echo 'TIMEOUT: $TIMEOUT'"
   ```

3. **Test each profile**:
   ```bash
   prodigy run workflow.yml --profile dev
   prodigy run workflow.yml --profile staging
   prodigy run workflow.yml --profile prod
   ```

4. **Verify secret masking**:
   - Run with `-v` verbose mode
   - Check that secrets appear as `***` in output
   - Verify secrets don't leak in error messages

### Environment Variable Composition

**Layered configuration strategy:**

```yaml
# Layer 1: Base configuration (committed)
env_files:
  - .env

# Layer 2: Local overrides (gitignored)
env_files:
  - .env.local

# Layer 3: Global workflow values
env:
  PROJECT_NAME: my-project
  VERSION: "1.0.0"

# Layer 4: Profile-specific overrides
profiles:
  prod:
    MAX_WORKERS: "20"
    TIMEOUT: "30"

# Layer 5: Secrets (separate management)
secrets:
  API_KEY:
    secret: true
    provider: vault
```

**Benefits:**
- Clear separation of concerns
- Easy to understand precedence
- Secure secret handling
- Flexible environment switching
- Local development friendly

### Performance Considerations

**Use environment variables to optimize performance:**

1. **Parameterize resource limits**:
   ```yaml
   env:
     MAX_WORKERS: "10"      # Tune based on CPU cores
     MEMORY_LIMIT: "8G"     # Tune based on available RAM
     TIMEOUT: "300"         # Tune based on expected duration
   ```

2. **Environment-specific optimizations**:
   ```yaml
   profiles:
     dev:
       MAX_WORKERS: "2"     # Low resource usage for local dev
       CACHE_ENABLED: "false"  # Disable caching for faster iteration

     prod:
       MAX_WORKERS: "20"    # High throughput for production
       CACHE_ENABLED: "true"   # Enable caching for performance
   ```

3. **Avoid expensive operations in variable expansion**:
   ```yaml
   # Problem - Runs command on every variable access
   env:
     TIMESTAMP: "$(date +%s)"  # Evaluated once, not per use

   # Better - Capture once if dynamic value needed
   commands:
     - shell: "date +%s"
       capture_output: TIMESTAMP
   ```

### Security Checklist

Before deploying workflows to production:

- [ ] All secrets marked with `secret: true`
- [ ] Secret files in `.gitignore`
- [ ] Production secrets use secret provider (vault/aws/etc)
- [ ] Verified secrets masked in logs with `-v` mode
- [ ] No hardcoded credentials in workflow YAML
- [ ] Environment variables documented in README
- [ ] `.env.example` provided for team
- [ ] Tested all profiles work correctly
- [ ] No sensitive data in environment variable names
- [ ] Secrets rotated regularly

See also:
- [Environment Precedence](environment-precedence.md) for understanding resolution order
- [Secrets Management](secrets-management.md) for detailed secret handling
- [Environment Profiles](environment-profiles.md) for profile configuration
- [Common Patterns](common-patterns.md) for practical examples
