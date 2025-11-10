## Workflow Format Comparison

Prodigy supports multiple workflow format styles to balance simplicity for quick tasks with power for production workflows. This section explains the differences and helps you choose the right format.

### Standard Workflow Formats

Standard workflows (non-MapReduce) can be written in two formats:

#### Simple Array Format

For quick workflows, use a simple array of commands:

```yaml
# Simple array format - minimal syntax
- claude: "/prodigy-coverage"
  commit_required: true

- shell: "just test"
  on_failure:
    claude: "/prodigy-debug-test-failure"
```

**Use this format when:**
- Creating quick automation scripts
- No environment variables or profiles needed
- Workflow is self-contained and straightforward

#### Full Configuration Format

For production workflows, use the full configuration format with metadata:

```yaml
# Full config format - includes metadata and environment
name: mapreduce-env-example
mode: mapreduce

env:
  PROJECT_NAME: "example-project"
  OUTPUT_DIR: "output"
  DEBUG_MODE: "false"

# Secrets are a separate top-level configuration
# They support two formats: Simple and Provider-based
secrets:
  # Simple format - directly references environment variable
  SIMPLE_SECRET: "ENV_VAR_NAME"

  # Provider format - explicit provider configuration
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"

  # File-based secret provider
  FILE_SECRET:
    provider: file
    key: "/path/to/secret/file"

profiles:
  development:
    DEBUG_MODE: "true"
  production:
    DEBUG_MODE: "false"

# Commands go here
setup:
  - shell: "echo Starting $PROJECT_NAME"
```

**Use this format when:**
- Deploying to multiple environments (dev, staging, prod)
- Need environment variables or secrets
- Workflow requires parameterization
- Building reusable workflow templates

**Secret Format Details:**

Secrets support two formats (defined in `src/cook/environment/config.rs:86-95`):

1. **Simple format** - Direct environment variable reference:
   ```yaml
   secrets:
     API_KEY: "ENV_VAR_NAME"  # Reads from $ENV_VAR_NAME
   ```

2. **Provider format** - Explicit provider configuration:
   ```yaml
   secrets:
     API_KEY:
       provider: env          # Providers: env, file, vault, aws
       key: "GITHUB_TOKEN"    # Source key/path
       version: "v1"          # Optional version
   ```

**Available Providers** (defined in `src/cook/environment/config.rs:101-109`):
- `env` - Environment variables (most common)
- `file` - File-based secrets
- `vault` - HashiCorp Vault integration
- `aws` - AWS Secrets Manager integration

Both formats mask secret values in logs. The Simple format is convenient for environment variables, while Provider format supports advanced secret management systems like Vault and AWS Secrets Manager.

**Note**: For detailed Vault and AWS provider configuration, see [Secrets Management](../environment/secrets-management.md) and [Environment Variables in Configuration](./environment-variables-in-configuration.md).

**Source**: Example workflow at `workflows/mapreduce-env-example.yml:23-26`

### MapReduce Syntax Evolution

MapReduce workflows have evolved to use simpler, more concise syntax.

#### Preferred Syntax (Current)

Commands are listed directly under `agent_template` and `reduce`:

```yaml
name: parallel-debt-elimination
mode: mapreduce

setup:
  - shell: "debtmap analyze . --output debt_items.json"

map:
  input: debt_items.json
  json_path: "$.debt_items[*]"

  # Direct array syntax - preferred
  agent_template:
    - claude: "/fix-issue ${item.description}"
    - shell: "cargo test"
      on_failure:
        claude: "/debug-test"

  max_parallel: 10

# Direct array syntax for reduce - preferred
reduce:
  - claude: "/summarize-fixes ${map.results}"
  - shell: "echo Processed ${map.total} items"
```

**Benefits:**
- Less nesting, easier to read
- Cleaner YAML structure
- Follows YAML array conventions
- Consistent with standard workflow format
- Forward compatibility - the nested format may be removed in future versions

#### Legacy Syntax (Deprecated)

The old format nested commands under a `commands` field:

```yaml
# Old syntax - deprecated but still supported
map:
  input: "work-items.json"
  json_path: "$.items[*]"

  # Nested under 'commands' - deprecated
  agent_template:
    commands:
      - shell: echo "Processing item ${item.id}"
      - shell: echo "Completed ${item.task}"

  max_parallel: 3

# Nested under 'commands' - deprecated
reduce:
  commands:
    - shell: echo "Processed ${map.total} items"
```

**Deprecation Notice:**
- This format is still supported for backward compatibility
- New workflows should use the direct array syntax
- Future versions may remove support for nested `commands`
- When using the old format, Prodigy emits a warning: "Using deprecated nested 'commands' syntax in agent_template. Consider using the simplified array format directly under 'agent_template'."

**Source**: Deprecation warnings in `src/config/mapreduce.rs:310, 347`

### Migration Guide

To migrate from old to new syntax:

**Before (Old):**
```yaml
agent_template:
  commands:
    - claude: "/process ${item}"
    - shell: "test ${item.path}"

reduce:
  commands:
    - claude: "/summarize ${map.results}"
```

**After (New):**
```yaml
agent_template:
  - claude: "/process ${item}"
  - shell: "test ${item.path}"

reduce:
  - claude: "/summarize ${map.results}"
```

**Migration Steps:**
1. Remove the `commands:` line from `agent_template`
2. Remove the `commands:` line from `reduce`
3. Unindent the command list by one level
4. Test the workflow to ensure it works correctly

**Important Notes:**
- The workflow format is all-or-nothing - you cannot mix old and new formats within the same workflow
- Both `agent_template` and `reduce` must use the same format (both direct array or both nested)
- After migration, run `prodigy run workflow.yml --dry-run` to validate syntax before executing
- If the workflow fails after migration, check for indentation errors - YAML is whitespace-sensitive

### Format Decision Tree

Choose your format based on these questions:

1. **Is this a MapReduce workflow?**
   - Yes → Use `mode: mapreduce` with direct array syntax
   - No → Continue to question 2

2. **Do you need environment variables or profiles?**
   - Yes → Use full configuration format
   - No → Continue to question 3

3. **Is this a quick one-off workflow?**
   - Yes → Use simple array format
   - No → Use full configuration format for maintainability

### Cross-References

- [Setup Phase Advanced](./setup-phase-advanced.md) - Detailed setup phase configuration and patterns
- [MapReduce Overview](./index.md) - MapReduce workflow fundamentals and phase documentation
- [Full Workflow Structure](../workflow-basics/full-workflow-structure.md) - Complete workflow configuration reference
- [Environment Variables in Configuration](./environment-variables-in-configuration.md) - Using variables and secrets in workflows

