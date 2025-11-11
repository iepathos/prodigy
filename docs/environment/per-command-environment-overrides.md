## Per-Command Environment Overrides

**IMPORTANT:** WorkflowStepCommand does NOT have an `env` field. All per-command environment changes must use shell syntax.

**Note:** The legacy Command struct (structured format) has an `env` field via CommandMetadata, but the modern WorkflowStepCommand format does not. For workflows using the modern `claude:`/`shell:` syntax, use shell-level environment syntax (`ENV=value command`).

You can override environment variables for individual commands using shell environment syntax:

```yaml
env:
  RUST_LOG: info
  API_URL: "https://api.example.com"

# Steps go directly in the workflow
- shell: "cargo run"  # Uses RUST_LOG=info from global env

# Override environment for this command only using shell syntax
- shell: "RUST_LOG=debug cargo run --verbose"

# Change directory and set environment in shell
- shell: "cd frontend && PATH=./node_modules/.bin:$PATH npm run build"
```

## Shell-Based Environment Techniques

### Basic Overrides

- **Single variable override:** `ENV_VAR=value command`
- **Multiple variables:** `VAR1=value1 VAR2=value2 command`
- **Change directory:** `cd path && command`
- **Combine both:** `cd path && ENV_VAR=value command`

### Advanced Shell Patterns

You can combine shell environment overrides with redirection and other shell features:

```yaml
# Redirect output while overriding environment
- shell: "RUST_LOG=trace cargo test > test-output.txt 2>&1"

# Pipe commands with environment overrides
- shell: "COLUMNS=120 cargo fmt --check | tee fmt-report.txt"

# Multiple environment variables with complex shell operations
- shell: |
    RUST_BACKTRACE=1 RUST_LOG=debug cargo run \
      --release \
      --bin my-app > app.log 2>&1 || echo "Build failed"
```

**Source**: Implementation in `src/config/command.rs:320-401` (WorkflowStepCommand definition)

## Interaction with Environment Precedence

Shell-level environment overrides take **highest precedence** and apply only to the specific command where they're defined. These overrides shadow:

- Global `env` variables
- Profile-specific environment
- `.env` file values
- System environment variables

For detailed precedence rules, see [Environment Precedence](environment-precedence.md).

## Future Plans: StepEnvironment Struct

A `StepEnvironment` struct exists in the internal runtime (defined in `src/cook/environment/config.rs:126-144`) with support for:

- `env`: HashMap of environment variables
- `working_dir`: Optional working directory override
- `clear_env`: Clear parent environment before applying step env
- `temporary`: Restore environment after step execution

This struct may be exposed in future versions to provide more structured per-step environment control directly in YAML syntax, eliminating the need for shell-based workarounds. However, currently it is **not exposed** in WorkflowStepCommand, so all per-command environment changes must use shell syntax as demonstrated above.

**Source**: `src/cook/environment/config.rs:126-144`

## Troubleshooting

### Environment Variable Not Taking Effect

**Problem**: Shell environment override doesn't apply to command

**Cause**: Quote escaping or shell evaluation order issues

**Solution**: Use proper quoting and verify variable expansion:
```yaml
# ✓ Correct
- shell: 'API_URL="https://example.com" ./script.sh'

# ✗ Incorrect (quotes broken)
- shell: "API_URL="https://example.com" ./script.sh"
```

### Variable Expansion Issues

**Problem**: Variable contains shell special characters (`$`, `\`, etc.)

**Solution**: Use single quotes to prevent shell expansion:
```yaml
# If variable value is literal (no shell expansion needed)
- shell: "PASSWORD='$ecr3t!' ./deploy.sh"
```

### Debugging Environment Resolution

To debug which environment values are active:

```yaml
# Print all environment variables
- shell: "env | sort"

# Check specific variable resolution
- shell: 'echo "RUST_LOG is: $RUST_LOG"'

# Verify override works
- shell: 'RUST_LOG=trace sh -c "echo RUST_LOG is: $RUST_LOG"'
```

## See Also

- [Environment Precedence](environment-precedence.md) - How environment variables are resolved
- [Environment Profiles](environment-profiles.md) - Named environment configurations
- [Secrets Management](secrets-management.md) - Handling sensitive values

---

