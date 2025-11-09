## Configuration Precedence Rules

Prodigy merges configuration from multiple sources with clear precedence. Understanding the precedence hierarchy helps you control which settings take effect when configuration is specified in multiple places.

### Precedence Hierarchy

From highest to lowest priority:

1. **CLI Flags** (highest priority)
   - Command-line arguments override all other settings
   - Example: `prodigy run workflow.yml --auto-commit false`

2. **Environment Variables**
   - Environment variables like `PRODIGY_CLAUDE_API_KEY`
   - Override file-based configuration
   - See [Environment Variables](environment-variables.md) for full list

3. **Project Config** (`.prodigy/config.yml`)
   - Project-specific settings in your repository
   - Override global configuration
   - Committed to version control (except secrets)

4. **Global Config** (`~/.prodigy/config.yml`)
   - User-level configuration in your home directory
   - Applies to all projects unless overridden
   - Not version controlled

5. **Defaults** (lowest priority)
   - Built-in default values defined in the code
   - Used when no other source provides a value
   - See [Default Values Reference](default-values-reference.md)

### How Settings Override Each Other

When Prodigy starts, it builds the final configuration by:

1. Starting with default values
2. Loading global config from `~/.prodigy/config.yml` (if exists)
3. Loading project config from `.prodigy/config.yml` (if exists)
4. Applying environment variables
5. Applying CLI flags (if provided)

Each step **overrides** values from the previous step. Settings not specified in higher-priority sources are preserved from lower-priority sources.

### Examples

#### Example 1: API Key Precedence

```yaml
# ~/.prodigy/config.yml (global config)
claude_api_key: "sk-global-key"
```

```yaml
# .prodigy/config.yml (project config)
name: my-project
claude_api_key: "sk-project-key"
```

```bash
# Environment variable (highest priority)
export PRODIGY_CLAUDE_API_KEY="sk-env-key"
```

**Result**: Prodigy uses `sk-env-key` because environment variables override file-based configuration.

#### Example 2: Auto-Commit Setting

```yaml
# ~/.prodigy/config.yml
auto_commit: false  # Global default: don't auto-commit
```

```yaml
# .prodigy/config.yml
name: my-project
# No auto_commit specified - inherits from global
```

**Result**: Prodigy uses `auto_commit: false` from the global config.

#### Example 3: Mixed Sources

```yaml
# ~/.prodigy/config.yml
log_level: info
auto_commit: true
max_concurrent_specs: 1
```

```yaml
# .prodigy/config.yml
name: my-project
log_level: debug  # Override global
# auto_commit and max_concurrent_specs inherited from global
```

```bash
export PRODIGY_AUTO_COMMIT=false  # Override both configs
```

**Result**:
- `log_level: debug` (from project config)
- `auto_commit: false` (from environment variable)
- `max_concurrent_specs: 1` (from global config)

### Field-Level Precedence

Precedence is applied **per field**, not per file. This means you can override individual settings while inheriting others:

```yaml
# Global config
claude_api_key: "sk-global"
log_level: info
auto_commit: true
```

```yaml
# Project config
name: my-project
log_level: debug  # Only override log_level
# claude_api_key and auto_commit are inherited from global
```

The project inherits `claude_api_key` and `auto_commit` from global config while overriding `log_level`.

### Checking Effective Configuration

To see which configuration is actually being used, run:

```bash
prodigy config show
```

This displays the merged configuration with all precedence rules applied.

