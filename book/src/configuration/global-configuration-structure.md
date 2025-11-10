## Global Configuration Structure

Global configuration is stored in `~/.prodigy/config.yml` in your home directory. These settings apply to all Prodigy projects unless overridden by project-specific configuration.

### Location

- **File**: `~/.prodigy/config.yml`
- **Created**: Automatically on first run with defaults
- **Format**: YAML

### Fields

#### `prodigy_home`

**Type**: Path
**Default**: `~/.prodigy` (or platform-specific data directory)

Base directory for global Prodigy data including events, DLQ, state, and worktrees.

```yaml
prodigy_home: /Users/username/.prodigy
```

#### `default_editor`

**Type**: String (optional)
**Default**: None

Default text editor for interactive operations. Falls back to `EDITOR` environment variable if not set.

```yaml
default_editor: vim
```

#### `log_level`

**Type**: String (optional)
**Default**: `info`
**Valid values**: `trace`, `debug`, `info`, `warn`, `error`

Controls logging verbosity for Prodigy operations.

```yaml
log_level: debug
```

#### `claude_api_key`

**Type**: String (optional)
**Default**: None

Claude API key for AI-powered commands. Can be overridden by project config or `PRODIGY_CLAUDE_API_KEY` environment variable.

```yaml
claude_api_key: "sk-ant-api03-..."
```

**Security Note**: Store API keys in environment variables or project config (not committed to version control) rather than global config.

#### `max_concurrent_specs`

**Type**: Integer (optional)
**Default**: `1`

Maximum number of concurrent spec implementations to run in parallel.

```yaml
max_concurrent_specs: 3
```

#### `auto_commit`

**Type**: Boolean (optional)
**Default**: `true`

Whether to automatically commit changes after successful command execution.

```yaml
auto_commit: false
```

#### `storage`

**Type**: Object (optional)
**Default**: File storage in `~/.prodigy`

Storage backend configuration for events, DLQ, state, and worktrees. See [Storage Configuration](storage-configuration.md) for details.

```yaml
storage:
  backend: file
  backend_config:
    base_dir: ~/.prodigy
    repository_grouping: true
```

**Storage Fields**:
- `backend`: Storage type (`file` or `memory`)
- `backend_config.base_dir`: Base directory for file storage
- `backend_config.repository_grouping`: Group data by repository name (default: true)

See [Storage Configuration](storage-configuration.md) for complete documentation.

#### `plugins`

**Type**: Object (optional)
**Default**: None

Plugin system configuration. See [Plugin Configuration](#plugin-configuration) below.

### Complete Example

```yaml
# ~/.prodigy/config.yml
prodigy_home: /Users/username/.prodigy
default_editor: code
log_level: info
claude_api_key: "sk-ant-api03-..."
max_concurrent_specs: 2
auto_commit: true

storage:
  backend: file
  backend_config:
    base_dir: /Users/username/.prodigy
    repository_grouping: true

plugins:
  enabled: true
  directory: /Users/username/.prodigy/plugins
  auto_load:
    - github-integration
    - slack-notifications
```

### Plugin Configuration

The `plugins` field controls the plugin system:

#### `enabled`

**Type**: Boolean
**Default**: `false`

Enable or disable the plugin system.

```yaml
plugins:
  enabled: true
```

#### `directory`

**Type**: Path
**Default**: `~/.prodigy/plugins`

Directory to search for plugins.

```yaml
plugins:
  directory: /custom/plugin/path
```

#### `auto_load`

**Type**: Array of strings
**Default**: `[]`

List of plugin names to automatically load on startup.

```yaml
plugins:
  auto_load:
    - plugin-name-1
    - plugin-name-2
```

### Creating Global Config

If the file doesn't exist, create it manually:

```bash
mkdir -p ~/.prodigy
cat > ~/.prodigy/config.yml << 'EOF'
log_level: info
auto_commit: true
EOF
```

### Relationship to Project Config

- Global config applies to **all projects**
- Project config (`.prodigy/config.yml`) **overrides** global config per field
- Settings not specified in project config are **inherited** from global config
- See [Configuration Precedence Rules](configuration-precedence-rules.md) for details

### See Also

- [Project Configuration Structure](project-configuration-structure.md)
- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [Environment Variables](environment-variables.md)
- [Default Values Reference](default-values-reference.md)

