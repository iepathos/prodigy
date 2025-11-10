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

**When to use step-level env overrides:**
- Command-specific configuration that overrides global/profile values
- Per-step resource limits or timeouts
- Temporary environment changes for individual commands
- Testing different configurations in the same workflow

See [Per-Command Environment Overrides](per-command-environment-overrides.md) for detailed step-level env documentation.

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

2. **Always use the secrets block for sensitive values**
   ```yaml
   secrets:
     # Simple format - loads from environment variable
     API_KEY: "my-secret-value"  # Masked in all logs

     # Provider format - explicitly specify source
     DATABASE_PASSWORD:
       provider: env
       key: DB_PASSWORD  # Retrieves from $DB_PASSWORD env var
   ```

3. **Use external secret providers**

   **Currently implemented providers** (src/cook/environment/secret_store.rs:35-46):
   - `env` - Load from environment variable (EnvSecretProvider)
   - `file` - Load from file path (FileSecretProvider)

   ```yaml
   secrets:
     # Load from environment variable
     API_KEY:
       provider: env
       key: SECRET_API_KEY

     # Load from file
     DATABASE_PASSWORD:
       provider: file
       key: /etc/secrets/db-password
   ```

   **Planned providers** (not yet implemented):
   - `vault` - HashiCorp Vault integration (declared in config.rs:106)
   - `aws` - AWS Secrets Manager (declared in config.rs:108)
   - `custom` - Custom provider support (declared in config.rs:110)

4. **Minimize secret exposure**
   - Only expose secrets to commands that need them
   - Use short-lived tokens when possible
   - Use `file` provider to avoid secrets in environment variables

5. **Verify secret masking**
   - Check logs to ensure secrets are masked (appear as `***`)
   - Test with `-v` verbose mode to confirm masking works

**Secret organization pattern:**

```yaml
# Good - Organized with file-based secrets
env_files:
  - .env              # Non-sensitive config
  - .env.local        # Local overrides (gitignored)

secrets:
  # Production secrets from files (managed by deployment system)
  DATABASE_PASSWORD:
    provider: file
    key: /run/secrets/db-password

  API_KEY:
    provider: file
    key: /run/secrets/api-key

  # Development secrets from environment (for local testing)
  DEV_API_KEY:
    provider: env
    key: DEV_API_KEY  # Loaded from environment, still masked in logs
```

**Source**: Secret provider implementations in src/cook/environment/secret_store.rs:34-148

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

Remember the order (highest to lowest priority):
1. **Step-level env** - Per-command overrides (see [Per-Command Environment Overrides](per-command-environment-overrides.md))
2. **Profile values** - When profile is active via `--profile` flag
3. **Global `env` field** - Workflow-level environment variables
4. **Environment files** - Later files override earlier ones (from `env_files`)
5. **Parent process environment** - Inherited from shell (if `inherit: true`)

**Source**: Implementation in src/cook/environment/manager.rs:88-156

For detailed explanation of precedence rules, see [Environment Precedence](environment-precedence.md).

**3. Forgetting to mark secrets:**

```yaml
# Problem - Secret exposed in logs
env_files:
  - .env.secrets  # Contains API_KEY=sk-abc123

commands:
  - shell: "curl -H 'Authorization: Bearer $API_KEY' ..."
  # API_KEY appears in logs!

# Solution - Use secrets block to mask in logs
secrets:
  API_KEY:
    provider: env
    key: SECRET_API_KEY  # Retrieves from environment, masked in all output
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

# Layer 5: Step-level overrides (per-command)
commands:
  - shell: "process-data"
    env:
      TIMEOUT: "60"  # Override for this command only

# Layer 6: Secrets (separate management)
secrets:
  API_KEY:
    provider: file
    key: /run/secrets/api-key
```

**Source**: Precedence implementation in src/cook/environment/manager.rs:88-156

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

- [ ] All secrets defined in `secrets` block for masking
- [ ] Secret files in `.gitignore`
- [ ] Production secrets use `file` provider (vault/aws planned for future)
- [ ] Verified secrets masked in logs with `-v` mode
- [ ] No hardcoded credentials in workflow YAML
- [ ] Environment variables documented in README
- [ ] `.env.example` provided for team
- [ ] Tested all profiles work correctly
- [ ] No sensitive data in environment variable names
- [ ] Secrets rotated regularly
- [ ] File-based secrets have restricted permissions (chmod 600)

See also:
- [Environment Precedence](environment-precedence.md) for understanding resolution order
- [Secrets Management](secrets-management.md) for detailed secret handling
- [Environment Profiles](environment-profiles.md) for profile configuration
- [Common Patterns](common-patterns.md) for practical examples
