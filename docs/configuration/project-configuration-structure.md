## Project Configuration Structure

Project configuration is stored in `.prodigy/config.yml` within your project repository. These settings override global configuration for this specific project and are typically committed to version control (except for secrets).

### Location

- **File**: `.prodigy/config.yml` (in project root)
- **Created**: Manually or via `prodigy init`
- **Format**: YAML
- **Version Control**: Committed to git (recommended, except for secrets)

### Field Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | String | None | Project identifier for logs and UI |
| `description` | String | None | Human-readable description |
| `version` | String | None | Project version (semver recommended) |
| `spec_dir` | Path | `specs` | Directory for specification files |
| `claude_api_key` | String | Inherited | Project-specific API key |
| `auto_commit` | Boolean | `true` | Auto-commit after operations |
| `variables` | Object | `{}` | Project variables for workflows |
| `storage` | Object | Inherited | Storage backend configuration |
| `plugins` | Object | Disabled | Plugin system configuration |

### Fields

#### `name` (recommended)

**Type**: String (optional)
**Default**: None

Project identifier used in logs, events, and UI. While not strictly required by the parser, providing a name is recommended for better observability.

```yaml
# Source: .prodigy/config.yml
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

!!! warning "Security Warning"
    Do NOT commit API keys to version control. Use environment variables or `.prodigy/config.local.yml` (gitignored) instead. See [Secrets Management](#secrets-management) below.

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

!!! note "Workflow Environment Variables"
    For workflow-level environment variables (with secrets, profiles, and step-level overrides), use the `env:` block in workflow files instead. See [Environment Variables](environment-variables.md) for details.

#### `storage`

**Type**: Object (optional)
**Default**: Inherits from global config

Project-specific storage configuration. See [Storage Configuration](storage-configuration.md) for details.

```yaml
# Source: src/config/prodigy_config.rs:127-139
storage:
  backend: filesystem        # or "memory" for testing
  base_path: /custom/project/storage
  compression_level: 6       # 0-9, 0 = no compression
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `backend` | String | `filesystem` | Storage backend: `filesystem` or `memory` |
| `base_path` | Path | `~/.prodigy/` | Base directory for storage |
| `compression_level` | Integer | `0` | Checkpoint compression (0-9) |

#### `plugins`

**Type**: Object (optional)
**Default**: Disabled

Configuration for the Prodigy plugin system.

```yaml
# Source: src/config/prodigy_config.rs:146-158
plugins:
  enabled: true
  directory: .prodigy/plugins
  auto_load:
    - my-custom-plugin
    - another-plugin
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable plugin system |
| `directory` | Path | None | Directory containing plugin files |
| `auto_load` | List | `[]` | Plugins to load on startup |

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

storage:
  backend: filesystem
  compression_level: 6

plugins:
  enabled: false
```

### Secrets Management

For sensitive values like API keys, use one of these approaches:

=== "Environment Variables (Recommended)"

    ```yaml
    # .prodigy/config.yml (committed)
    name: my-project
    # No claude_api_key here
    ```

    ```bash
    # Set in environment
    export PRODIGY_CLAUDE_API_KEY="sk-ant-api03-..."
    ```

=== "Local Config File"

    Create `.prodigy/config.local.yml` and add it to `.gitignore`:

    ```yaml
    # .prodigy/config.local.yml (gitignored)
    claude_api_key: "sk-ant-api03-..."
    ```

    ```bash
    # .gitignore
    .prodigy/config.local.yml
    ```

=== "Secret Management Service"

    Use a secret management service (AWS Secrets Manager, HashiCorp Vault, etc.) and retrieve the key at runtime via environment variables.

!!! tip "Recommended Approach"
    Environment variables provide the best balance of security and flexibility. They work across all environments (local, CI/CD, production) without code changes.

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
max_concurrent_specs: 4
```

```yaml
# .prodigy/config.yml (project)
name: my-project
log_level: debug  # Override global
# auto_commit and max_concurrent_specs inherited from global
```

**Result**: Project uses `log_level: debug` but inherits `auto_commit: true` and `max_concurrent_specs: 4` from global config.

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

Or use the init command:

```bash
prodigy init
```
