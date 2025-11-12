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

## Variable Interpolation

Prodigy supports two syntaxes for referencing environment variables in workflows:

### Simple Syntax: `$VAR`

The simple `$VAR` syntax works for basic variable references:

```yaml
env:
  API_URL: https://api.example.com
  PORT: "3000"

commands:
  - shell: "curl $API_URL/health"
  - shell: "echo Server running on port $PORT"
```

**Use `$VAR` when:**
- Variable name is standalone (not adjacent to other text)
- You're passing to shell commands
- Simple, clear usage without ambiguity

### Bracketed Syntax: `${VAR}`

The bracketed `${VAR}` syntax is preferred for clarity and is required in some cases:

```yaml
env:
  PROJECT: my-app
  VERSION: "1.0.0"
  ENVIRONMENT: prod

commands:
  - shell: "deploy-${PROJECT}-${VERSION}.sh"  # Required: adjacent to text
  - shell: "echo Deploying ${PROJECT} v${VERSION}"  # Preferred: explicit
  - shell: "mkdir -p /var/log/${PROJECT}/${ENVIRONMENT}"  # Required: in paths
```

**Use `${VAR}` when:**
- Variable is adjacent to other text (e.g., `${VAR}-suffix`, `prefix-${VAR}`)
- Variable is part of a path (e.g., `config/${VAR}/file.json`)
- Complex expressions or nested usage
- You want explicit, unambiguous references (recommended)

### When `${VAR}` is Required

**1. Adjacent to text:**
```yaml
env:
  NAME: api
  VERSION: "2.1"

commands:
  # Wrong - Shell interprets as variable named "NAME_VERSION"
  - shell: "echo $NAME_VERSION"

  # Correct - Explicitly separates variables
  - shell: "echo ${NAME}_${VERSION}"
```

**2. In file paths:**
```yaml
env:
  ENVIRONMENT: staging
  PROJECT: my-app

commands:
  # Preferred - Clear variable boundaries
  - shell: "cp config.json /etc/${PROJECT}/${ENVIRONMENT}/config.json"
```

**3. In URLs and complex strings:**
```yaml
env:
  API_BASE: https://api.example.com
  VERSION: v1

commands:
  # Required - Variable within URL
  - shell: "curl ${API_BASE}/${VERSION}/users"
```

### Interpolation in Different Contexts

**Shell commands:**
Both syntaxes work, but `${VAR}` is safer:
```yaml
env:
  DATABASE_URL: postgresql://localhost:5432/app
  TIMEOUT: "30"

commands:
  - shell: "psql $DATABASE_URL"           # Simple case: $VAR works
  - shell: "timeout ${TIMEOUT} ./app"     # Preferred: ${VAR} is explicit
```

**Claude commands:**
Use `${VAR}` for consistency:
```yaml
env:
  SPEC_FILE: spec-123.md
  PROJECT_NAME: my-project

commands:
  - claude: "/implement-spec ${SPEC_FILE} --project ${PROJECT_NAME}"
```

**File paths:**
Always use `${VAR}` in paths:
```yaml
env:
  OUTPUT_DIR: /tmp/results
  TIMESTAMP: "20240101"

commands:
  - shell: "mkdir -p ${OUTPUT_DIR}/${TIMESTAMP}"
  - write_file:
      path: ${OUTPUT_DIR}/${TIMESTAMP}/report.json
      content: "..."
```

**MapReduce configurations:**
Combine with MapReduce variables like `${item}`:
```yaml
env:
  MAX_WORKERS: "10"
  OUTPUT_PATH: /results

map:
  max_parallel: ${MAX_WORKERS}
  agent_template:
    - shell: "process ${item.file} --output ${OUTPUT_PATH}/${item.id}.result"
```

### Escaping Variables

If you need a literal `$` character, use shell escaping:

```yaml
commands:
  # Using single quotes (no interpolation)
  - shell: 'echo "Price: $100"'

  # Using double quotes with escape
  - shell: "echo \"Price: \\$100\""

  # Double $$ for literal $ in some contexts
  - shell: "echo Price: $$100"
```

# Both acceptable
- shell: "echo Port: $PORT"
- shell: "echo Port: ${PORT}"
```

**Complex case (requires `${VAR}`):**
```yaml
env:
  PROJECT: api
  VERSION: "1.0"
  ENVIRONMENT: prod

# Required - variables adjacent to text and in paths
- shell: "deploy-${PROJECT}-v${VERSION}.sh --env ${ENVIRONMENT}"
- shell: "cp /src/config.${ENVIRONMENT}.json /etc/${PROJECT}/config.json"
```

**Recommended approach (always use `${VAR}`):**
```yaml
env:
  DATABASE: myapp
  USER: admin
  HOST: localhost

commands:
  - shell: "psql -h ${HOST} -U ${USER} -d ${DATABASE}"
  - shell: "backup-${DATABASE}-$(date +%Y%m%d).sql"
```

---


## Additional Topics

### Environment Configuration Subsections

- [MapReduce Environment Variables](mapreduce-environment-variables.md) - Environment variables specific to MapReduce workflows
- [Environment Files](environment-files.md) - Using .env files for configuration
- [Secrets Management](secrets-management.md) - Handling sensitive data securely
- [Environment Profiles](environment-profiles.md) - Profile-based configuration for different environments
- [Per-Command Environment Overrides](per-command-environment-overrides.md) - Step-level environment customization
- [Environment Precedence](environment-precedence.md) - Understanding variable resolution order
- [Best Practices](best-practices.md) - Recommended patterns and approaches
- [Common Patterns](common-patterns.md) - Real-world usage examples

### Related Chapters

- [MapReduce Workflows](../mapreduce/index.md) - Parallel processing with environment configuration
- [Variables and Interpolation](../variables/index.md) - Understanding variable syntax and usage
- [Configuration](../configuration/index.md) - Overall workflow and project configuration
- [Workflow Configuration](../configuration/workflow-configuration.md) - Complete workflow file structure
