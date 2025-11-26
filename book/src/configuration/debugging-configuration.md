## Debugging Configuration

Prodigy provides powerful configuration tracing tools to help you understand where configuration values come from and diagnose configuration issues. This is especially useful when values are being overridden unexpectedly or when troubleshooting complex multi-source configurations.

### Configuration Value Tracing

The `prodigy config trace` command shows exactly where each configuration value originates and how it may have been overridden by different sources.

#### Understanding Configuration Sources

Prodigy loads configuration from multiple sources in this precedence order (highest to lowest):

1. **Environment variables** (e.g., `PRODIGY__LOG_LEVEL`)
2. **Project config** (`.prodigy/config.yml`)
3. **Global config** (`~/.prodigy/config.yml`)
4. **Defaults** (built-in values)

Higher-priority sources override lower-priority ones.

### Basic Commands

#### View Effective Configuration

Show the final resolved values for all configuration:

```bash
prodigy config show
```

Example output:
```
Effective configuration:

  log_level: info
  max_concurrent_specs: 5
  auto_commit: true
  claude_api_key: sk-ant-a...

Storage:
  backend: File
  base_path: /home/user/.prodigy
  compression_level: 6

Plugins:
  enabled: false
```

#### Show Specific Value

Display a single configuration value:

```bash
prodigy config show log_level
```

Output:
```
log_level: "info"
```

### Tracing Value Origins

#### Trace All Values

See where every configuration value comes from:

```bash
prodigy config trace
```

Example output:
```
Configuration values:

log_level: "info"
  └── default: "info" ← final value

max_concurrent_specs: 5
  └── default: 5 ← final value

auto_commit: true
  └── default: true ← final value
```

#### Trace Specific Value

Trace the origin of a single value:

```bash
prodigy config trace log_level
```

Output shows the complete override chain:
```
log_level: "debug"
  ├── default: "info" (overridden)
  └── ~/.prodigy/config.yml:3: "debug" ← final value
```

#### View Override Chain

When a value is overridden multiple times, you see the full history:

```bash
# With ~/.prodigy/config.yml containing: log_level: debug
# And PRODIGY__LOG_LEVEL=warn in environment

prodigy config trace log_level
```

Output:
```
log_level: "warn"
  ├── default: "info" (overridden)
  ├── ~/.prodigy/config.yml:3: "debug" (overridden)
  └── $PRODIGY__LOG_LEVEL: "warn" ← final value
```

### Finding Overridden Values

To see only values that have been changed from their defaults:

```bash
prodigy config trace --overrides
```

This is useful for auditing what custom configuration is active:

```
Overridden configuration values:

log_level: "debug"
  ├── default: "info" (overridden)
  └── ~/.prodigy/config.yml:3: "debug" ← final value

max_concurrent_specs: 10
  ├── default: 5 (overridden)
  └── $PRODIGY__MAX_CONCURRENT_SPECS: 10 ← final value
```

### Diagnosing Configuration Issues

The `--diagnose` flag detects common configuration problems:

```bash
prodigy config trace --diagnose
```

#### Detected Issue Types

**Empty Environment Variables**

When an environment variable is set but empty:

```
Configuration issues detected:

Warning: default_editor is set but empty from $PRODIGY__DEFAULT_EDITOR
  Suggestion: Unset the variable or provide a value: unset PRODIGY__DEFAULT_EDITOR
```

**Multiple Override Chain**

When a value is set in many places:

```
Info: "log_level" was set in 3 places: default → ~/.prodigy/config.yml:3 → $PRODIGY__LOG_LEVEL
  Suggestion: Review if all overrides are intentional
```

**Environment Overriding File Config**

When an environment variable takes precedence over file-based config:

```
Info: "log_level" is set in config file but overridden by $PRODIGY__LOG_LEVEL
```

### JSON Output

All trace commands support JSON output for scripting and automation:

```bash
# All traces as JSON
prodigy config trace --json

# Specific value as JSON
prodigy config trace log_level --json

# Overrides as JSON
prodigy config trace --overrides --json

# Diagnostics as JSON
prodigy config trace --diagnose --json
```

#### JSON Output Format

```json
[
  {
    "path": "log_level",
    "final_value": "debug",
    "final_source": {
      "type": "file",
      "source": "~/.prodigy/config.yml",
      "line": 3
    },
    "history": [
      {
        "value": "info",
        "source": {
          "type": "default",
          "source": "defaults"
        },
        "overridden": true
      },
      {
        "value": "debug",
        "source": {
          "type": "file",
          "source": "~/.prodigy/config.yml",
          "line": 3
        },
        "overridden": false
      }
    ]
  }
]
```

### Practical Debugging Scenarios

#### Scenario 1: Value Not Taking Effect

You set `log_level: debug` in your config file but logs are still at info level:

```bash
prodigy config trace log_level
```

If you see:
```
log_level: "info"
  ├── default: "info" (overridden)
  ├── ~/.prodigy/config.yml:3: "debug" (overridden)
  └── $PRODIGY__LOG_LEVEL: "info" ← final value
```

The environment variable `PRODIGY__LOG_LEVEL` is overriding your file config. Either:
- Unset the environment variable: `unset PRODIGY__LOG_LEVEL`
- Change the environment variable: `export PRODIGY__LOG_LEVEL=debug`

#### Scenario 2: Unknown Configuration Source

A value is set but you don't know where it's coming from:

```bash
prodigy config trace max_concurrent_specs
```

Output shows the exact file and line:
```
max_concurrent_specs: 20
  ├── default: 5 (overridden)
  └── .prodigy/config.yml:7: 20 ← final value
```

Now you know to check `.prodigy/config.yml` line 7.

#### Scenario 3: Empty Environment Variable Warning

You're getting unexpected behavior with an optional config:

```bash
prodigy config trace --diagnose
```

Output reveals the issue:
```
Warning: default_editor is set but empty from $PRODIGY__DEFAULT_EDITOR
  Suggestion: Unset the variable or provide a value: unset PRODIGY__DEFAULT_EDITOR
```

Fix by unsetting the empty variable:
```bash
unset PRODIGY__DEFAULT_EDITOR
```

#### Scenario 4: Auditing Production Configuration

Before deploying, verify exactly what configuration will be active:

```bash
prodigy config trace --overrides --json > config-audit.json
```

This creates a complete record of all non-default configuration for review.

### Integration with Workflows

You can use configuration tracing in workflow automation:

```yaml
name: config-validation
commands:
  # Check for configuration issues before proceeding
  - shell: "prodigy config trace --diagnose --json | jq '.[] | select(.severity==\"error\")' | grep -q . && exit 1 || true"

  # Verify specific required settings
  - shell: |
      LOG_LEVEL=$(prodigy config show log_level --json | jq -r '.log_level')
      if [ "$LOG_LEVEL" != "info" ] && [ "$LOG_LEVEL" != "debug" ]; then
        echo "Error: log_level must be info or debug, got: $LOG_LEVEL"
        exit 1
      fi
```

### Environment Variable Naming

Prodigy environment variables follow this pattern:

| Config Path | Environment Variable |
|------------|---------------------|
| `log_level` | `PRODIGY__LOG_LEVEL` |
| `max_concurrent_specs` | `PRODIGY__MAX_CONCURRENT_SPECS` |
| `storage.backend` | `PRODIGY__STORAGE__BACKEND` |
| `project.name` | `PRODIGY__PROJECT__NAME` |

Note the double underscore (`__`) separating segments.

### Best Practices

1. **Start with `--diagnose`**: Run diagnostics first to catch obvious issues
2. **Use `--overrides` for auditing**: See only what's been customized
3. **Check specific paths**: When debugging, trace the exact value in question
4. **Use JSON for automation**: Parse JSON output in scripts for validation
5. **Document your overrides**: Keep notes on why certain values are overridden

### Troubleshooting Tracing Commands

#### "No value found at path"

The configuration path doesn't exist:

```bash
prodigy config trace nonexistent.path
# Error: No value found at path: nonexistent.path
```

Check available paths with:
```bash
prodigy config trace --all --json | jq '.[].path'
```

#### "Failed to load configuration"

There's a syntax error in your config files:

```
Failed to load configuration:
  - ~/.prodigy/config.yml: invalid YAML at line 5
```

Fix the YAML syntax and retry.

### See Also

- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [Environment Variables](environment-variables.md)
- [Troubleshooting](troubleshooting.md)
- [Global Configuration Structure](global-configuration-structure.md)
