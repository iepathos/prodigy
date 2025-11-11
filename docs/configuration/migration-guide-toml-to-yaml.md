## Migration Guide: TOML to YAML

Prodigy has migrated from TOML to YAML for all configuration files. This guide helps you migrate existing configurations.

### Quick Migration

**Before (TOML):**
```toml
# .prodigy/config.toml
name = "my-project"
description = "My project"

[variables]
PROJECT_ROOT = "/app"
VERSION = "1.0.0"

[claude]
api_key = "sk-ant-api03-..."
model = "claude-3-sonnet"
```

**After (YAML):**
```yaml
# .prodigy/config.yml
name: my-project
description: My project

variables:
  PROJECT_ROOT: /app
  VERSION: 1.0.0

claude:
  api_key: sk-ant-api03-...
  model: claude-3-sonnet
```

### Key Syntax Differences

| Feature | TOML | YAML |
|---------|------|------|
| Assignment | `key = value` | `key: value` |
| Sections | `[section]` | `section:` (nested with indentation) |
| Strings | `"quoted"` or `'quoted'` | Usually unquoted (quote if contains `:`, `#`, etc.) |
| Indentation | Doesn't matter | **Critical** - use 2 spaces per level |
| Comments | `# comment` | `# comment` (same) |
| Booleans | `true`, `false` | `true`, `false` (same) |
| Arrays | `[1, 2, 3]` | `[1, 2, 3]` or newline-separated with `-` |
| Nested tables | `[table.subtable]` | Indented structure |

### Configuration File Locations

Both global and project configurations have moved from `.toml` to `.yml`:

| Type | Old Location | New Location |
|------|--------------|---------------|
| Global Config | `~/.prodigy/config.toml` | `~/.prodigy/config.yml` |
| Project Config | `.prodigy/config.toml` | `.prodigy/config.yml` |

### Global Configuration Migration

**Before (`~/.prodigy/config.toml`):**
```toml
prodigy_home = "/Users/username/.prodigy"
default_editor = "vim"
log_level = "info"
auto_commit = true
max_concurrent_specs = 2

[storage]
backend = "file"

[storage.backend_config]
base_dir = "/Users/username/.prodigy"
repository_grouping = true

[plugins]
enabled = true
directory = "/Users/username/.prodigy/plugins"
auto_load = ["plugin1", "plugin2"]
```

**After (`~/.prodigy/config.yml`):**
```yaml
prodigy_home: /Users/username/.prodigy
default_editor: vim
log_level: info
auto_commit: true
max_concurrent_specs: 2

storage:
  backend: file
  backend_config:
    base_dir: /Users/username/.prodigy
    repository_grouping: true

plugins:
  enabled: true
  directory: /Users/username/.prodigy/plugins
  auto_load:
    - plugin1
    - plugin2
```

### Project Configuration Migration

**Before (`.prodigy/config.toml`):**
```toml
name = "my-project"
description = "A sample project"

[variables]
PROJECT_ROOT = "/app"
VERSION = "1.0.0"

[claude]
api_key = "sk-ant-api03-..."
model = "claude-3-sonnet"

[storage]
backend = "file"
```

**After (`.prodigy/config.yml`):**
```yaml
name: my-project
description: A sample project

variables:
  PROJECT_ROOT: /app
  VERSION: "1.0.0"

claude:
  api_key: sk-ant-api03-...
  model: claude-3-sonnet

storage:
  backend: file
```

### Array Syntax Migration

TOML and YAML handle arrays differently:

**TOML:**
```toml
auto_load = ["plugin1", "plugin2", "plugin3"]
```

**YAML (inline style):**
```yaml
auto_load: [plugin1, plugin2, plugin3]
```

**YAML (block style - recommended):**
```yaml
auto_load:
  - plugin1
  - plugin2
  - plugin3
```

### Nested Structure Migration

TOML uses dotted keys or table headers for nesting. YAML uses indentation.

**TOML:**
```toml
[storage]
backend = "file"

[storage.backend_config]
base_dir = "/Users/username/.prodigy"
repository_grouping = true
```

**YAML:**
```yaml
storage:
  backend: file
  backend_config:
    base_dir: /Users/username/.prodigy
    repository_grouping: true
```

### String Quoting Rules

YAML is more relaxed about string quoting:

**When quotes are NOT needed:**
```yaml
name: my-project
path: /usr/local/bin
url: https://example.com
```

**When quotes ARE needed:**
```yaml
# Contains colon
message: "Error: something failed"

# Contains hash (would be a comment)
tag: "#important"

# Starts with special character
value: "@username"

# Looks like a number but should be string
version: "1.0"

# Contains YAML reserved words
status: "yes"  # Would be boolean without quotes
```

### Boolean Values

Both use the same boolean syntax:

```yaml
auto_commit: true
enabled: false
```

**Note**: In YAML, these are also booleans: `yes`, `no`, `on`, `off`, `true`, `false`. To use them as strings, quote them: `"yes"`, `"no"`.

### Comments

Both use `#` for comments:

```yaml
# This is a comment
log_level: info  # Inline comment
```

### Migrating Workflows

Workflows were always YAML, so no migration is needed. However, if you referenced TOML config in workflows, update the file extension:

**Before:**
```yaml
commands:
  - shell: "cat .prodigy/config.toml"
```

**After:**
```yaml
commands:
  - shell: "cat .prodigy/config.yml"
```

### Migration Checklist

- [ ] Rename `~/.prodigy/config.toml` → `~/.prodigy/config.yml`
- [ ] Rename `.prodigy/config.toml` → `.prodigy/config.yml`
- [ ] Convert TOML syntax to YAML syntax
- [ ] Update section headers `[section]` to `section:` with indentation
- [ ] Convert assignments `key = value` to `key: value`
- [ ] Fix array syntax (use `-` for lists)
- [ ] Ensure consistent 2-space indentation
- [ ] Quote strings with special characters
- [ ] Test configuration with `prodigy config show`
- [ ] Update any workflow references to config files

### Validation

After migration, validate your configuration:

```bash
# Check effective configuration (merges global + project)
prodigy config show

# Validate YAML syntax
yamllint .prodigy/config.yml

# Run a simple workflow to verify settings work
prodigy run test-workflow.yml
```

### Troubleshooting

**"Invalid YAML syntax" error:**
- Check indentation (must be 2 spaces, no tabs)
- Ensure colons have space after them (`key: value`, not `key:value`)
- Quote strings with special characters

**"Configuration not found" error:**
- Ensure file is named `config.yml` (not `config.yaml` or `config.toml`)
- Check file is in correct location (`.prodigy/` or `~/.prodigy/`)

**"Invalid field" warnings:**
- Remove deprecated TOML-specific fields
- Check spelling of field names
- Refer to [Configuration Structure](global-configuration-structure.md) for valid fields

### Backward Compatibility

**TOML support is deprecated** and may be removed in future versions. Migration to YAML is strongly recommended.

If you must support both formats temporarily:
1. Keep both `config.toml` and `config.yml`
2. YAML takes precedence if both exist
3. Migrate fully before upgrading to newer Prodigy versions

### See Also

- [Global Configuration Structure](global-configuration-structure.md)
- [Project Configuration Structure](project-configuration-structure.md)
- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [YAML Specification](https://yaml.org/spec/)
