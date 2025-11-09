## Environment Variables

Prodigy recognizes environment variables for configuration, overriding file-based settings. Environment variables have higher precedence than file configuration but lower than CLI flags.

See [Configuration Precedence Rules](configuration-precedence-rules.md) for details on how environment variables interact with other configuration sources.

### Claude API Configuration

#### `PRODIGY_CLAUDE_API_KEY`

**Purpose**: Claude API key for AI-powered commands
**Default**: None
**Overrides**: Global and project `claude_api_key` settings

```bash
export PRODIGY_CLAUDE_API_KEY="sk-ant-api03-..."
```

This is the **recommended** way to provide API keys (more secure than storing in config files).

#### `PRODIGY_CLAUDE_STREAMING`

**Purpose**: Control Claude JSON streaming output
**Default**: `true` (streaming enabled by default)
**Valid values**: `true`, `false`

```bash
export PRODIGY_CLAUDE_STREAMING=false  # Disable streaming
```

When `false`, uses legacy print mode instead of JSON streaming.

### General Configuration

#### `PRODIGY_LOG_LEVEL`

**Purpose**: Logging verbosity
**Default**: `info`
**Valid values**: `trace`, `debug`, `info`, `warn`, `error`
**Overrides**: Global and project `log_level` settings

```bash
export PRODIGY_LOG_LEVEL=debug
```

#### `PRODIGY_EDITOR`

**Purpose**: Default text editor for interactive operations
**Default**: None
**Overrides**: Global `default_editor` setting
**Fallback**: `EDITOR` environment variable

```bash
export PRODIGY_EDITOR=vim
```

If neither `PRODIGY_EDITOR` nor `EDITOR` is set, Prodigy uses system defaults.

#### `EDITOR`

**Purpose**: Standard Unix editor variable (fallback)
**Default**: None
**Fallback for**: `PRODIGY_EDITOR`

```bash
export EDITOR=nano
```

**Precedence**: `PRODIGY_EDITOR` takes precedence over `EDITOR` if both are set.

#### `PRODIGY_AUTO_COMMIT`

**Purpose**: Automatic commit after successful commands
**Default**: `true`
**Valid values**: `true`, `false`
**Overrides**: Global and project `auto_commit` settings

```bash
export PRODIGY_AUTO_COMMIT=false
```

### Storage Configuration

#### `PRODIGY_STORAGE_TYPE`

**Purpose**: Storage backend type
**Default**: `file`
**Valid values**: `file`, `memory`
**Overrides**: Storage `backend` setting

```bash
export PRODIGY_STORAGE_TYPE=file
```

#### `PRODIGY_STORAGE_BASE_PATH`

**Purpose**: Base directory for file storage
**Default**: `~/.prodigy`
**Overrides**: Storage `backend_config.base_dir` setting

```bash
export PRODIGY_STORAGE_BASE_PATH=/custom/storage/path
```

**Alternative names** (deprecated, use `PRODIGY_STORAGE_BASE_PATH`):
- `PRODIGY_STORAGE_DIR`
- `PRODIGY_STORAGE_PATH`

### Workflow Execution

#### `PRODIGY_AUTOMATION`

**Purpose**: Signal automated execution mode
**Default**: Not set
**Set by**: Prodigy when executing workflows

```bash
export PRODIGY_AUTOMATION=true
```

This variable is **set automatically** by Prodigy during workflow execution. It signals to Claude and other tools that execution is automated (not interactive).

#### `PRODIGY_CLAUDE_CONSOLE_OUTPUT`

**Purpose**: Force Claude streaming output regardless of verbosity
**Default**: Not set
**Valid values**: `true`, `false`

```bash
export PRODIGY_CLAUDE_CONSOLE_OUTPUT=true
```

When set to `true`, forces JSON streaming output even when verbosity is 0. Useful for debugging specific runs without changing command flags.

### Complete Example

Set up a complete Prodigy environment:

```bash
# API key (recommended method)
export PRODIGY_CLAUDE_API_KEY="sk-ant-api03-..."

# Logging
export PRODIGY_LOG_LEVEL=info

# Editor
export PRODIGY_EDITOR=code

# Behavior
export PRODIGY_AUTO_COMMIT=true

# Storage
export PRODIGY_STORAGE_TYPE=file
export PRODIGY_STORAGE_BASE_PATH=/Users/username/.prodigy

# Development/debugging
export PRODIGY_CLAUDE_STREAMING=true
export PRODIGY_CLAUDE_CONSOLE_OUTPUT=false
```

### Environment Files

You can use `.env` files (not committed to version control) to manage environment variables:

```bash
# .env (add to .gitignore)
PRODIGY_CLAUDE_API_KEY=sk-ant-api03-...
PRODIGY_LOG_LEVEL=debug
PRODIGY_AUTO_COMMIT=false
```

Load with:

```bash
# Using direnv
eval "$(cat .env)"

# Using dotenv tool
dotenv run prodigy run workflow.yml

# Manually
export $(cat .env | xargs)
```

### Security Best Practices

1. **Never commit API keys** to version control
2. **Use environment variables** for secrets (not config files)
3. **Use `.env` files** (gitignored) for local development
4. **Use secret managers** (AWS Secrets Manager, Vault) in production
5. **Rotate keys regularly** and use project-specific keys when possible

**Example `.gitignore`**:

```
.env
.env.*
!.env.example
.prodigy/config.local.yml
```

### Precedence Summary

For any given setting, the effective value comes from (highest to lowest):

1. **CLI flags** (if applicable)
2. **Environment variables** ← This level
3. **Project config** (`.prodigy/config.yml`)
4. **Global config** (`~/.prodigy/config.yml`)
5. **Defaults** (built-in values)

Example:

```yaml
# ~/.prodigy/config.yml
log_level: info
```

```yaml
# .prodigy/config.yml
log_level: warn
```

```bash
export PRODIGY_LOG_LEVEL=debug  # This wins
```

**Result**: `log_level: debug`

### Checking Environment Variables

To see which environment variables are active:

```bash
# List all PRODIGY_* variables
env | grep PRODIGY_

# Check effective configuration (merges all sources)
prodigy config show
```

### See Also

- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [Global Configuration Structure](global-configuration-structure.md)
- [Project Configuration Structure](project-configuration-structure.md)
- [Storage Configuration](storage-configuration.md)

