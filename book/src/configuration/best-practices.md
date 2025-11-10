## Best Practices

This section provides practical guidance for configuring Prodigy workflows effectively, based on the implementation patterns and defaults in the codebase.

## Start with Sensible Defaults

Prodigy provides carefully chosen defaults that work well for most use cases. You typically only need to configure exceptions.

**Default Storage Configuration** (src/storage/config.rs:184-226):
```yaml
# These defaults are already set - no need to configure unless you need different values
storage:
  connection_pool_size: 10
  timeout_secs: 30
  max_retries: 3
  initial_delay_ms: 1000
  max_delay_ms: 30000
  backoff_multiplier: 2.0
  max_file_size_mb: 100
  cache_size: 1000
  cache_ttl_secs: 3600
```

**Default Cleanup Configuration** (src/cook/execution/mapreduce/cleanup/config.rs:39-87):
```yaml
# Default preset: balanced cleanup
cleanup:
  auto_cleanup: true
  cleanup_delay_secs: 30
  max_worktrees_per_job: 50
  max_total_worktrees: 200
  disk_threshold_mb: 1024
```

**When to Override Defaults:**
- **Storage**: Only if you have specific latency/throughput requirements
- **Cleanup**: Use "aggressive" preset for disk-constrained environments
- **Timeouts**: Increase for long-running operations

## Use Configuration Layering Effectively

Prodigy uses a three-tier configuration hierarchy (src/config/mod.rs:102-154):

**Configuration Precedence** (highest to lowest):
1. **Workflow configuration** - Specific to a single workflow YAML file
2. **Project configuration** - `.prodigy/config.toml` in your project
3. **Global configuration** - `~/.prodigy/config.toml` in your home directory
4. **Built-in defaults** - Sensible defaults from the codebase

**Recommended Usage:**
```yaml
# Global config (~/.prodigy/config.toml): User-wide preferences
[global]
log_level = "info"
auto_commit = true

# Project config (.prodigy/config.toml): Project-specific overrides
[project]
claude_api_key = "${CLAUDE_API_KEY}"  # Environment variable reference
auto_commit = false  # Override global setting

# Workflow YAML: Workflow-specific settings
env:
  MAX_PARALLEL: 5
  PROJECT_NAME: "my-project"
```

See [Configuration Precedence Rules](configuration-precedence-rules.md) for detailed examples.

## Parameterize with Environment Variables

Use environment variables for flexible, reusable workflows (src/config/mapreduce.rs:395-449).

**Variable Syntax:**
```yaml
env:
  PROJECT_NAME: "prodigy"
  MAX_PARALLEL: 3
  API_URL: "https://api.example.com"

map:
  max_parallel: $MAX_PARALLEL  # Simple reference
  agent_template:
    - shell: "echo Processing ${PROJECT_NAME}"  # Bracketed reference
    - shell: "curl ${API_URL}/endpoint"
```

**Resolution Order** (src/config/mapreduce.rs:395-449):
1. Workflow `env:` block
2. System environment variables
3. Error if not found

**Type Flexibility** (src/config/mapreduce.rs:354-393):
```yaml
# Both numeric literals and environment variables are supported
map:
  max_parallel: 3              # Numeric literal
  max_parallel: $MAX_PARALLEL  # Environment variable
```

See [Environment Variables](environment-variables.md) for comprehensive documentation.

## Protect Sensitive Values

Prodigy automatically masks common secret patterns in logs (src/cook/environment/config.rs:78-119).

**Automatic Masking Patterns:**
- API keys (contains "api" or "key")
- Tokens (contains "token")
- Passwords (contains "password", "pwd", "secret")
- Auth values (contains "auth")

**Example:**
```yaml
env:
  API_KEY: "sk-abc123def456"      # Auto-masked as "***" in logs
  PASSWORD: "hunter2"              # Auto-masked
  AUTH_TOKEN: "ghp_abcdefg"        # Auto-masked
  NORMAL_VAR: "visible-value"      # Not masked
```

**Explicit Secret Marking:**
```yaml
env:
  CUSTOM_SECRET:
    secret: true
    value: "my-sensitive-value"
```

**Best Practices:**
1. Never hardcode secrets in workflow files
2. Use environment variable references: `${SECRET_NAME}`
3. Store secrets in system environment or `.env` files (excluded from git)
4. Mark custom sensitive fields with `secret: true`
5. Review logs to verify sensitive values are masked

## Use Profiles for Different Environments

Profiles allow environment-specific configuration (src/cook/environment/config.rs:146-162):

```yaml
env:
  API_URL:
    default: "http://localhost:3000"
    staging: "https://staging-api.example.com"
    prod: "https://api.example.com"

  MAX_PARALLEL:
    default: 2
    prod: 10

  DEBUG_MODE:
    default: true
    prod: false
```

**Activate a profile:**
```bash
prodigy run workflow.yml --profile prod
```

**Use cases:**
- Development vs production API endpoints
- Resource limits (lower parallelism in dev)
- Feature flags (enable debug logging in dev)
- Storage backends (local files vs S3 in prod)

## Configure Storage Appropriately

**Global Storage (Default - Recommended):**
```yaml
storage:
  use_global: true  # Default
  base_path: "~/.prodigy"  # Default
```

**Benefits:**
- Cross-worktree event aggregation for parallel jobs
- Persistent state survives worktree cleanup
- Centralized monitoring and debugging
- Efficient storage deduplication

**Local Storage (Deprecated):**
```yaml
storage:
  use_global: false  # ⚠️ Deprecated
  base_path: ".prodigy"
```

**Warning:** Local storage is deprecated (src/storage/config.rs:71-73). Use global storage unless you have specific isolation requirements.

**Environment Variable Fallbacks** (src/storage/config.rs:244-286):
```bash
# Precedence: highest to lowest
export PRODIGY_STORAGE_TYPE="file"        # or "memory"
export PRODIGY_STORAGE_BASE_PATH="/data/.prodigy"
export PRODIGY_STORAGE_DIR="/tmp/.prodigy"  # Fallback
export PRODIGY_STORAGE_PATH="/var/.prodigy"  # Secondary fallback
```

See [Storage Configuration](storage-configuration.md) for detailed options.

## Set Appropriate Timeouts

Configure timeouts based on operation characteristics (src/app/config.rs:10-63):

**Workflow Timeouts:**
```yaml
# Default timeout: 30 seconds per command
commands:
  - shell: "quick-test"  # Uses default
  - shell: "long-build"
    timeout_secs: 600    # 10 minutes for long operations
```

**MapReduce Timeouts:**
```yaml
map:
  agent_timeout_secs: 300  # 5 minutes per agent
  max_parallel: 5
```

**Retry Configuration:**
```yaml
retry_config:
  max_attempts: 3           # Default
  initial_delay_ms: 1000    # 1 second
  max_delay_ms: 30000       # 30 seconds cap
  backoff: exponential      # Backoff strategy
```

**Backoff Strategies** (src/storage/config.rs:184-226):
- `exponential`: Delay doubles each retry (2^n * initial_delay)
- `linear`: Delay increases linearly (n * initial_delay)
- `fibonacci`: Delay follows fibonacci sequence

## Prefer Simplified YAML Syntax

Prodigy supports both simplified and verbose syntax (src/config/mapreduce.rs:287-351).

**Simplified Syntax (Recommended):**
```yaml
map:
  agent_template:
    - claude: "/process ${item.path}"
    - shell: "test -f ${item.path}"
```

**Verbose Syntax (Backward Compatible):**
```yaml
map:
  agent_template:
    commands:  # ⚠️ Deprecated nested array
      - claude: "/process ${item.path}"
      - shell: "test -f ${item.path}"
```

**Why simplified is better:**
- Less indentation
- Clearer intent
- Matches standard workflow syntax
- Forward-compatible

## Validate Configuration Early

**Type Safety** (src/config/mapreduce.rs:354-393):
- Workflow YAML is validated at parse time
- Clear error messages include context
- Type mismatches caught before execution

**Example Error Message:**
```
Error: Environment variable 'MAX_PARALLEL' not found
Context: Required by map.max_parallel in workflow.yml:12
Resolution order: workflow env → system environment
```

**Configuration Debugging:**
```bash
# View effective configuration after all precedence rules
prodigy config show

# Validate workflow without running
prodigy validate workflow.yml
```

## Common Configuration Patterns

**Capture Command Output:**
```yaml
setup:
  - shell: "git rev-parse HEAD"
    capture: "commit_hash"
  - shell: "echo 'Building commit ${commit_hash}'"
```

**Conditional Execution:**
```yaml
map:
  filter: "item.priority >= 5"  # Only process high-priority items
  sort_by: "item.priority DESC"  # Process highest priority first
```

**Error Handling:**
```yaml
map:
  agent_template:
    - shell: "risky-operation ${item.id}"
      on_failure:
        - claude: "/diagnose-failure ${item.id}"
```

**Resource Limits:**
```yaml
map:
  max_parallel: 10              # Concurrent agents
  max_items: 100                # Limit total items processed
  agent_timeout_secs: 300       # Per-agent timeout
```

## Anti-Patterns to Avoid

**❌ Don't: Hardcode Paths**
```yaml
commands:
  - shell: "cd /Users/alice/project && make test"
```

**✅ Do: Use Variables**
```yaml
env:
  PROJECT_DIR: "${PWD}"
commands:
  - shell: "cd ${PROJECT_DIR} && make test"
```

**❌ Don't: Expose Secrets**
```yaml
env:
  API_KEY: "sk-real-key-here"  # Committed to git!
```

**✅ Do: Reference Environment**
```yaml
env:
  API_KEY: "${CLAUDE_API_KEY}"  # From system environment
```

**❌ Don't: Disable Global Storage Unnecessarily**
```yaml
storage:
  use_global: false  # Deprecated and limits functionality
```

**✅ Do: Use Default Global Storage**
```yaml
# No storage configuration needed - global is default
```

**❌ Don't: Skip Timeouts for Long Operations**
```yaml
commands:
  - shell: "npm install"  # May hang indefinitely
```

**✅ Do: Set Explicit Timeouts**
```yaml
commands:
  - shell: "npm install"
    timeout_secs: 300
```

## Troubleshooting Configuration Issues

**Configuration Not Found:**
```bash
# Check search paths
prodigy config show --verbose

# Verify file exists and is valid YAML
cat .prodigy/config.toml
yamllint workflow.yml
```

**Wrong Precedence:**
```bash
# View effective configuration
prodigy config show

# Check which file is setting a value
PRODIGY_LOG_LEVEL=debug prodigy run workflow.yml -v
```

**Environment Variable Not Resolved:**
```yaml
# Check variable is defined
echo $MY_VAR

# Use correct syntax: $VAR or ${VAR}
# NOT: %VAR% (Windows) or $env:VAR (PowerShell)
```

**Timeout Too Short:**
```yaml
# Increase timeouts for slow operations
commands:
  - shell: "cargo build --release"
    timeout_secs: 600  # 10 minutes
```

See [Troubleshooting](troubleshooting.md) for more configuration issues and solutions.

## Advanced Configuration

**Custom Cleanup Presets:**
```yaml
# Aggressive cleanup for CI/CD
cleanup:
  preset: "aggressive"
  cleanup_delay_secs: 5
  max_worktrees_per_job: 20
  disk_threshold_mb: 512
```

**Custom Storage Backend:**
```yaml
storage:
  type: "file"  # or "memory" for testing
  base_path: "/mnt/fast-disk/.prodigy"
  connection_pool_size: 20  # High-concurrency workloads
```

**Workflow Validation:**
```yaml
validation:
  threshold: 80  # Minimum 80% success rate
  timeout_secs: 600
  output_schema: "output-schema.json"  # Validate JSON outputs
```

## Summary

**Key Takeaways:**
1. **Trust the defaults** - They're production-tested and sensible
2. **Layer your config** - Global → Project → Workflow hierarchy
3. **Parameterize everything** - Use environment variables for flexibility
4. **Protect secrets** - Automatic masking + explicit `secret: true`
5. **Use profiles** - Separate dev, staging, prod configurations
6. **Global storage** - Default and recommended for all use cases
7. **Set timeouts** - Prevent hanging on long operations
8. **Simplified syntax** - Clearer and forward-compatible
9. **Validate early** - Catch errors before execution
10. **Monitor effective config** - Use `prodigy config show` for debugging

## See Also

- [Configuration Precedence Rules](configuration-precedence-rules.md) - Detailed precedence examples
- [Environment Variables](environment-variables.md) - Comprehensive variable documentation
- [Storage Configuration](storage-configuration.md) - Storage backend options
- [Global Configuration Structure](global-configuration-structure.md) - GlobalConfig fields
- [Complete Configuration Examples](complete-configuration-examples.md) - Real-world examples
- [Troubleshooting](troubleshooting.md) - Common configuration issues
