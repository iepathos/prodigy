## Timeout Configuration

Set execution timeouts at the command level to prevent workflows from hanging indefinitely.

### Basic Timeouts

```yaml
# Command-level timeout (in seconds)
- shell: "cargo bench"
  timeout: 600  # 10 minutes

# Timeout for long-running operations
- claude: "/analyze-codebase"
  timeout: 1800  # 30 minutes
```

### Environment Variable Support

Timeout values can reference environment variables, allowing you to parameterize timeouts based on environment or configuration:

```yaml
# Use environment variable for timeout
- shell: "cargo bench"
  timeout: $BENCH_TIMEOUT

# With default value using shell syntax
- claude: "/analyze-codebase"
  timeout: ${ANALYSIS_TIMEOUT:-1800}  # Defaults to 1800 if ANALYSIS_TIMEOUT not set

# Profile-specific timeouts
- shell: "cargo test"
  timeout: ${TEST_TIMEOUT}  # Different values for dev/ci/prod profiles
```

**Use Cases:**
- **Environment-specific timeouts**: Different timeout values for CI vs local development
- **Configurable timeouts**: Allow users to customize timeout behavior without editing workflow files
- **Dynamic timeouts**: Adjust timeouts based on system resources or workload

### Workflow-Level Timeouts

Define timeouts at the workflow level to apply defaults across all commands:

```yaml
name: "build-workflow"
timeout: 3600  # Default 1 hour timeout for all steps

commands:
  - shell: "cargo build"  # Inherits 3600s timeout
  - shell: "cargo test"
    timeout: 1200  # Override with 20 minutes
```

### Best Practices

**Set Appropriate Timeouts:**
- Set timeouts high enough to complete under normal conditions
- Consider worst-case scenarios (slow CI, cold caches)
- Use shorter timeouts for quick operations to fail fast

**Use Environment Variables:**
- Parameterize timeouts for different environments
- Allow overrides without editing workflow files
- Document expected timeout ranges in workflow comments
