## Environment Precedence

Environment variables are resolved with a clear precedence order, ensuring predictable behavior when the same variable is defined in multiple locations.

**Source**: Implementation in `src/cook/environment/manager.rs:95-137`

## Precedence Order

Environment variables are applied in the following order (later sources override earlier ones):

1. **Parent environment** - Inherited from the parent process
2. **Environment files** - Loaded from `env_files` (later files override earlier)
3. **Global `env`** - Defined at workflow level in YAML
4. **Active profile** - Applied if a profile is set (internal infrastructure)
5. **Step-specific `env`** - Per-step environment variables
6. **Secrets** - Loaded from secrets configuration (applied after step env)
7. **Shell-level overrides** - Using `ENV=value command` syntax

**Source**: Precedence implementation in `src/cook/environment/manager.rs:95-137`

## Implementation Details

### Parent Environment Inheritance

By default, workflows inherit all environment variables from the parent process. You can disable this with `inherit: false` in the environment configuration.

```yaml
# Disable parent environment inheritance
inherit: false

env:
  # Only these variables will be available
  NODE_ENV: production
```

**Source**: Parent environment loading at `src/cook/environment/manager.rs:98-102`

### Environment Files Precedence

When multiple environment files are specified, later files override earlier ones. This allows layering of configuration (base + environment-specific).

```yaml
env_files:
  - .env              # Base configuration
  - .env.production   # Overrides base values
```

**Source**: Environment file loading at `src/cook/environment/manager.rs:107-109`

### Global Environment

The global `env` block at the workflow level overrides both parent environment and environment files.

```yaml
env:
  NODE_ENV: production  # Overrides .env files and parent environment
  API_URL: https://api.example.com
```

**Source**: Global environment application at `src/cook/environment/manager.rs:112-115`

### Profile Infrastructure

Prodigy includes internal profile infrastructure that can activate different environment configurations. However, this feature is not currently exposed via CLI flags.

```yaml
# Profile infrastructure exists but no --profile CLI flag available
profiles:
  development:
    NODE_ENV: development
    API_URL: http://localhost:3000
```

**Source**: Profile application at `src/cook/environment/manager.rs:118-120`; No CLI flag in `src/cli/args.rs`

### Step-Specific Environment

**Note**: The YAML command syntax (`WorkflowStepCommand`) does not expose step-level environment configuration. However, the internal runtime (`StepEnvironment`) supports it for future extensibility.

**Source**:
- `StepEnvironment` struct at `src/cook/environment/config.rs:128-144`
- Step environment application at `src/cook/environment/manager.rs:123-127`

### Secrets Loading

Secrets are loaded AFTER step-specific environment variables, ensuring they cannot be accidentally overridden by step configuration.

```yaml
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

# This takes precedence over global env, env_files, and step env
```

**Source**: Secrets application at `src/cook/environment/manager.rs:130-137`

### Shell-Level Overrides

Shell syntax provides the highest precedence override, applied at command execution time.

```yaml
- shell: "NODE_ENV=test echo $NODE_ENV"  # Prints: test
```

This override is handled by the shell itself and takes precedence over all Prodigy environment sources.

## Complete Example

Here's a comprehensive example demonstrating all precedence levels:

```yaml
# Parent environment: NODE_ENV=local (inherited by default)

env_files:
  - .env  # Contains: NODE_ENV=development, API_URL=http://localhost:3000

env:
  NODE_ENV: production      # Overrides .env file and parent
  API_URL: https://api.prod.example.com  # Overrides .env file

secrets:
  API_KEY: "${env:SECRET_API_KEY}"  # Loaded after global env

commands:
  - shell: "echo $NODE_ENV"          # Prints: production (from global env)
  - shell: "echo $API_URL"           # Prints: https://api.prod.example.com
  - shell: "echo $API_KEY"           # Prints: *** (masked, from secrets)

  # Override using shell syntax (highest precedence)
  - shell: "NODE_ENV=staging echo $NODE_ENV"  # Prints: staging
```

**Source**: Real-world example from `workflows/environment-example.yml`

## Precedence Resolution Flow

When resolving an environment variable, Prodigy follows this flow:

```
1. Start with parent environment (if inherit: true, default)
2. Apply each env_file in order (later files override)
3. Apply global env block (overrides files and parent)
4. Apply active profile if set (internal feature)
5. Apply step-specific env (runtime capability)
6. Apply secrets (highest Prodigy-level precedence)
7. Shell overrides apply at execution time (highest overall)
```

**Result**: The last value set wins, creating a predictable override chain.

## Debugging Precedence Issues

When troubleshooting which environment source is active:

```yaml
# Print all environment variables to debug precedence
- shell: "env | sort"
  capture_output: true

# Print specific variable to verify its value
- shell: "echo NODE_ENV=$NODE_ENV"
```

Common debugging scenarios:
- **Variable not set**: Check if parent environment is being inherited
- **Wrong value**: Check which precedence level last set the variable
- **Secrets not working**: Verify secrets are loaded after step environment
- **Override not applying**: Ensure shell syntax is correct

## Related Topics

- [Environment Files](environment-files.md) - Loading configuration from files
- [Secrets Management](secrets-management.md) - Secure handling of sensitive values
- [Environment Profiles](environment-profiles.md) - Profile infrastructure details
- [Per-Command Overrides](per-command-environment-overrides.md) - Command-level environment control
