# Environment Configuration

Prodigy provides flexible environment configuration for workflows, allowing you to manage environment variables, secrets, profiles, and step-specific settings. This chapter explains the user-facing configuration options available in workflow YAML files.

## Architecture Overview

Prodigy uses a two-layer architecture for environment management:

1. **WorkflowConfig**: User-facing YAML configuration with `env`, `secrets`, `profiles`, and `env_files` fields
2. **EnvironmentConfig**: Internal runtime configuration that extends workflow config with additional features

This chapter documents the WorkflowConfig layer - the fields you write in workflow YAML files (`env`, `secrets`, `env_files`, `profiles`). The EnvironmentConfig is Prodigy's internal runtime that processes these YAML fields and adds internal-only features like dynamic command-based values and conditional expressions.

**Internal vs. User-Facing Capabilities:**

The internal `EnvironmentConfig` supports richer environment value types through the `EnvValue` enum:
- `Static`: Simple string values (what WorkflowConfig exposes)
- `Dynamic`: Values from command output (internal only)
- `Conditional`: Expression-based values (internal only)

In workflow YAML, the `env` field only supports static string values (`HashMap<String, String>`). The Dynamic and Conditional variants are internal runtime features not exposed in workflow configuration.

**Note on Internal Features:** The `EnvironmentConfig` runtime layer includes a `StepEnvironment` struct with fields like `env`, `working_dir`, `clear_env`, and `temporary`. These are internal implementation details not exposed in `WorkflowStepCommand` YAML syntax. Per-command environment changes must use shell syntax (e.g., `ENV=value command`).

---

## Global Environment Variables

Define static environment variables that apply to all commands in your workflow:

```yaml
# Global environment variables (static strings only)
env:
  NODE_ENV: production
  PORT: "3000"
  API_URL: https://api.example.com
  DEBUG: "false"

commands:
  - shell: "echo $NODE_ENV"  # Uses global environment
```

**Important:** The `env` field at the workflow level only supports static string values. Dynamic or conditional environment variables are handled internally by the runtime but are not directly exposed in workflow YAML.

**Environment Inheritance:** Parent process environment variables are always inherited by default. All global environment variables are merged with the parent environment.

---


## Additional Topics

See also:
- [MapReduce Environment Variables](mapreduce-environment-variables.md)
- [Environment Files](environment-files.md)
- [Secrets Management](secrets-management.md)
- [Environment Profiles](environment-profiles.md)
- [Per-Command Environment Overrides](per-command-environment-overrides.md)
- [Environment Precedence](environment-precedence.md)
- [Best Practices](best-practices.md)
- [Common Patterns](common-patterns.md)
