## Environment Precedence

Environment variables are resolved with the following precedence (highest to lowest):

1. **Shell-level overrides** - Using `ENV=value command` syntax
2. **Global `env`** - Defined at workflow level in YAML
3. **Environment files** - Loaded from `env_files` (later files override earlier)
4. **Parent environment** - Always inherited from the parent process

**Note:** The internal `EnvironmentConfig` runtime also supports profile-based precedence and step-level environment overrides, but these are not exposed in workflow YAML. The profile infrastructure exists internally (`active_profile` field), but there is no CLI flag to activate profiles. WorkflowStepCommand has no `env` field.

Example demonstrating precedence:

```yaml
# Parent environment: NODE_ENV=local

env_files:
  - .env  # Contains: NODE_ENV=development

env:
  NODE_ENV: production  # Overrides .env file and parent environment

# Steps go directly in the workflow
- shell: "echo $NODE_ENV"  # Prints: production (from global env)

# Override using shell syntax
- shell: "NODE_ENV=staging echo $NODE_ENV"  # Prints: staging (shell override)
```

In this example:
- Parent environment has `NODE_ENV=local` (lowest precedence)
- `.env` file sets `NODE_ENV=development` (overrides parent)
- Global `env` sets `NODE_ENV=production` (overrides .env file)
- Shell syntax `NODE_ENV=staging` (overrides everything for that command)

---

