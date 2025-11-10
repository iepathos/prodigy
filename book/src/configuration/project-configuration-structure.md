## Project Configuration Structure

Project configuration is stored in `.prodigy/config.yml` within your project repository. These settings override global configuration for this specific project and are typically committed to version control (except for secrets).

### Location

- **File**: `.prodigy/config.yml` (in project root)
- **Created**: Manually or via `prodigy init`
- **Format**: YAML
- **Version Control**: Committed to git (recommended, except for secrets)

### Fields

#### `name` (required)

**Type**: String
**Default**: None (required field)

Project identifier used in logs, events, and UI.

```yaml
name: my-project
```

#### `description`

**Type**: String (optional)
**Default**: None

Human-readable project description.

```yaml
description: "AI-powered code analysis tool"
```

#### `version`

**Type**: String (optional)
**Default**: None

Project version (semantic versioning recommended).

```yaml
version: "1.2.3"
```

#### `spec_dir`

**Type**: Path (optional)
**Default**: `specs`

Directory containing Prodigy specification files.

```yaml
spec_dir: custom/specs
```

#### `claude_api_key`

**Type**: String (optional)
**Default**: None (inherits from global config or environment)

Project-specific Claude API key. **Overrides** global config and **is overridden by** environment variable.

```yaml
claude_api_key: "sk-ant-api03-..."
```

**Security Warning**: Do NOT commit API keys to version control. Use environment variables or `.prodigy/config.local.yml` (gitignored) instead.

#### `auto_commit`

**Type**: Boolean (optional)
**Default**: Inherits from global config (default: `true`)

Whether to automatically commit changes after successful command execution.

```yaml
auto_commit: false
```

#### `variables`

**Type**: Object (optional)
**Default**: `{}`

Project-specific variables available in workflows and commands.

```yaml
variables:
  deploy_branch: production
  test_timeout: 300
  feature_flags:
    new_ui: true
    beta_features: false
```

These variables can be referenced in workflows using `${variable_name}` syntax.

**Note**: For workflow-level environment variables (with secrets, profiles, and step-level overrides), use the `env:` block in workflow files instead. See [Environment Variables](environment-variables.md) for details.

#### `storage`

**Type**: Object (optional)
**Default**: Inherits from global config

Project-specific storage configuration. See [Storage Configuration](storage-configuration.md) for details.

```yaml
storage:
  backend: file
  backend_config:
    base_dir: /custom/project/storage
```

### Complete Example

```yaml
# .prodigy/config.yml
name: prodigy
description: "Workflow orchestration tool for Claude Code"
version: "0.1.0"
spec_dir: specs

auto_commit: true

variables:
  default_branch: master
  test_suite: full
  timeout_seconds: 600
```

### Secrets Management

For sensitive values like API keys, use one of these approaches:

#### Option 1: Environment Variables (Recommended)

```yaml
# .prodigy/config.yml (committed)
name: my-project
# No claude_api_key here
```

```bash
# Set in environment
export PRODIGY_CLAUDE_API_KEY="sk-ant-api03-..."
```

#### Option 2: Local Config File (Not Committed)

Create `.prodigy/config.local.yml` and add it to `.gitignore`:

```yaml
# .prodigy/config.local.yml (gitignored)
claude_api_key: "sk-ant-api03-..."
```

```bash
# .gitignore
.prodigy/config.local.yml
```

#### Option 3: Secret Management Service

Use a secret management service (AWS Secrets Manager, HashiCorp Vault, etc.) and retrieve the key at runtime via environment variables.

### Relationship to Global Config

Project config **overrides** global config on a per-field basis:

- Fields specified in project config **replace** global config values
- Fields NOT specified in project config **inherit** global config values
- See [Configuration Precedence Rules](configuration-precedence-rules.md)

**Example**:

```yaml
# ~/.prodigy/config.yml (global)
log_level: info
auto_commit: true
max_concurrent_specs: 1
```

```yaml
# .prodigy/config.yml (project)
name: my-project
log_level: debug  # Override global
# auto_commit and max_concurrent_specs inherited from global
```

**Result**: Project uses `log_level: debug` but inherits `auto_commit: true` and `max_concurrent_specs: 1` from global config.

### Project Variables in Workflows

Variables defined in project config are available in workflows:

```yaml
# .prodigy/config.yml
name: my-project
variables:
  environment: staging
  api_url: https://staging.api.example.com
```

```yaml
# .prodigy/workflow.yml
commands:
  - name: deploy
    args: ["${environment}"]
    options:
      api_url: "${api_url}"
```

### Creating Project Config

Initialize a new project:

```bash
cd my-project
mkdir -p .prodigy
cat > .prodigy/config.yml << 'EOF'
name: my-project
auto_commit: true
EOF
```

Or use the init command (if available):

```bash
prodigy init
```

### See Also

- [Global Configuration Structure](global-configuration-structure.md)
- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [Workflow Configuration](workflow-configuration.md)
- [Environment Variables](environment-variables.md)

