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

secrets:
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"

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

- [Setup Phase](./setup-phase.md) - Detailed setup phase configuration
- [Map Phase](./map-phase.md) - Map phase command templates
- [Reduce Phase](./reduce-phase.md) - Reduce phase aggregation
- [Basic Workflow Structure](../workflow-basics/basic-structure.md) - Standard workflow fundamentals

