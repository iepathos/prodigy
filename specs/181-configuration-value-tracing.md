---
number: 181
title: Configuration Value Tracing for Debugging
category: foundation
priority: medium
status: draft
dependencies: [178, 179]
created: 2025-11-25
---

# Specification 181: Configuration Value Tracing for Debugging

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 178, 179 (Premortem Integration)

## Context

When configuration values come from multiple sources (defaults, global config, project config, environment variables), debugging "why does this setting have this value?" becomes difficult. Users and developers frequently encounter situations where:

1. **Unexpected Values**: A config value isn't what they expected, but they don't know which source provided it
2. **Override Confusion**: They set a value in one place but it's being overridden by another source
3. **Silent Failures**: A config file has a typo in a key name, so the value is never loaded
4. **Environment Leakage**: An environment variable from a previous session affects behavior

### Current Debugging Experience

```bash
# User's current debugging workflow
$ cat ~/.prodigy/config.yml | grep log_level
log_level: info

$ cat .prodigy/config.yml | grep log_level
# (nothing - user thinks global should apply)

$ echo $PRODIGY_LOG_LEVEL
debug

# Why is log_level "debug"? User has to manually check each source
```

### Ideal Debugging Experience

```bash
$ prodigy config trace log_level
log_level: "debug"
  ├── default: "info"
  ├── ~/.prodigy/config.yml:3: "info"  (overridden)
  └── PRODIGY_LOG_LEVEL: "debug"  ← final value

$ prodigy config trace --all-overrides
Overridden values:
  log_level: default → ~/.prodigy/config.yml → PRODIGY_LOG_LEVEL
  auto_commit: default → .prodigy/config.yml

Values with no overrides:
  max_concurrent_specs: default (5)
  storage.backend: default (file)
```

## Objective

Implement configuration value tracing using premortem's `TracedConfig` feature, enabling developers and users to understand exactly where each configuration value originates and how it was overridden.

## Requirements

### Functional Requirements

#### FR1: TracedConfig Integration
- **MUST** use premortem's `build_traced()` to capture value origins
- **MUST** track all sources: defaults, global file, project file, env vars
- **MUST** preserve override history showing all values, not just final
- **MUST** include source location (file:line) for file-based values

#### FR2: CLI Tracing Commands
- **MUST** implement `prodigy config trace <path>` command
- **MUST** implement `prodigy config trace --all` to show all values
- **MUST** implement `prodigy config trace --overrides` to show only overridden values
- **SHOULD** support JSON output with `--json` flag
- **SHOULD** colorize output for terminal readability

#### FR3: Trace Query API
- **MUST** provide `TracedProdigyConfig::trace(path: &str)` method
- **MUST** return `Option<ValueTrace>` with:
  - Final value
  - Source of final value
  - Override history (all previous values)
- **MUST** support dotted paths (e.g., "project.name", "storage.backend")

#### FR4: Source Identification
- **MUST** identify sources clearly:
  - `default` - Hardcoded default value
  - `{file_path}:{line}` - From config file with line number
  - `${ENV_VAR}` - From environment variable
- **MUST** handle missing sources gracefully (file doesn't exist → skipped)

#### FR5: Override Detection
- **MUST** detect when values are overridden (same path, different source)
- **MUST** track order of overrides (first → last)
- **MUST** identify "shadowed" values that were set but overridden
- **SHOULD** warn about potentially unintentional overrides

#### FR6: Debugging Helpers
- **MUST** provide `explain()` method that generates human-readable explanation
- **SHOULD** detect common issues:
  - Typos in config keys (value never loaded)
  - Empty environment variables overriding with ""
  - Relative vs absolute path mismatches
- **SHOULD** suggest fixes for detected issues

### Non-Functional Requirements

#### NFR1: Performance
- **MUST** tracing be optional (not enabled in production by default)
- **MUST** `build()` remain zero-overhead when tracing not needed
- **SHOULD** cache traced config if multiple queries expected

#### NFR2: Usability
- **MUST** output be clear and unambiguous
- **MUST** work in both interactive terminals and CI/scripts
- **SHOULD** support piping output to other tools

## Acceptance Criteria

- [ ] `prodigy config trace <path>` command implemented
- [ ] `prodigy config trace --all` shows all values with sources
- [ ] `prodigy config trace --overrides` shows only overridden values
- [ ] Override history preserved and displayable
- [ ] Source locations include file:line for config files
- [ ] Environment variable sources clearly identified
- [ ] JSON output option available
- [ ] Human-readable explanations generated
- [ ] Unit tests verify trace accuracy
- [ ] Integration tests verify CLI output
- [ ] Documentation updated with tracing examples

## Technical Details

### Implementation Approach

#### Phase 1: TracedConfig Builder

```rust
// src/config/builder.rs

use premortem::{Config, TracedConfig, ConfigEnv, RealEnv};

/// Load config with full value tracing
pub fn load_prodigy_config_traced() -> Result<TracedConfig<ProdigyConfig>, ConfigErrors> {
    load_prodigy_config_traced_with(&RealEnv)
}

pub fn load_prodigy_config_traced_with<E: ConfigEnv>(
    env: &E
) -> Result<TracedConfig<ProdigyConfig>, ConfigErrors> {
    Config::<ProdigyConfig>::builder()
        .source(Defaults::from(ProdigyConfig::defaults()))
        .source(Yaml::file_optional(global_config_path()))
        .source(Yaml::file_optional(".prodigy/config.yml"))
        .source(Env::prefix("PRODIGY_").separator("__"))
        .build_traced_with_env(env)
}
```

#### Phase 2: Trace Query Types

```rust
// src/config/tracing.rs

/// Information about a single value's origin
#[derive(Debug, Clone)]
pub struct ValueTrace {
    /// The final resolved value
    pub final_value: serde_json::Value,

    /// Source of the final value
    pub final_source: ValueSource,

    /// All values this key had, in order of application
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub value: serde_json::Value,
    pub source: ValueSource,
    pub was_overridden: bool,
}

#[derive(Debug, Clone)]
pub enum ValueSource {
    Default,
    File {
        path: PathBuf,
        line: Option<usize>,
        column: Option<usize>,
    },
    Environment {
        var_name: String,
    },
}

impl ValueSource {
    pub fn display(&self) -> String {
        match self {
            ValueSource::Default => "default".to_string(),
            ValueSource::File { path, line, .. } => {
                match line {
                    Some(l) => format!("{}:{}", path.display(), l),
                    None => path.display().to_string(),
                }
            }
            ValueSource::Environment { var_name } => format!("${}", var_name),
        }
    }
}
```

#### Phase 3: CLI Commands

```rust
// src/cli/config.rs

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Trace where configuration values come from
    Trace {
        /// Configuration path to trace (e.g., "log_level", "project.name")
        #[arg()]
        path: Option<String>,

        /// Show all configuration values
        #[arg(long)]
        all: bool,

        /// Show only overridden values
        #[arg(long)]
        overrides: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

pub fn handle_config_trace(
    path: Option<String>,
    all: bool,
    overrides: bool,
    json: bool,
) -> Result<()> {
    let traced = load_prodigy_config_traced()?;

    if let Some(path) = path {
        // Trace single value
        match traced.trace(&path) {
            Some(trace) => print_trace(&path, &trace, json),
            None => eprintln!("No value found at path: {}", path),
        }
    } else if overrides {
        // Show only overridden values
        for path in traced.overridden_paths() {
            if let Some(trace) = traced.trace(&path) {
                print_trace(&path, &trace, json);
            }
        }
    } else if all {
        // Show all values
        for (path, trace) in traced.all_traces() {
            print_trace(&path, &trace, json);
        }
    }

    Ok(())
}
```

#### Phase 4: Human-Readable Output

```rust
// src/config/tracing.rs

impl ValueTrace {
    /// Generate human-readable explanation
    pub fn explain(&self, path: &str) -> String {
        let mut lines = Vec::new();

        lines.push(format!("{}: {:?}", path, self.final_value));

        for (i, entry) in self.history.iter().enumerate() {
            let prefix = if i == self.history.len() - 1 {
                "  └──"
            } else {
                "  ├──"
            };

            let marker = if entry.was_overridden {
                " (overridden)"
            } else {
                " ← final value"
            };

            lines.push(format!(
                "{} {}: {:?}{}",
                prefix,
                entry.source.display(),
                entry.value,
                marker
            ));
        }

        lines.join("\n")
    }
}

// Example output:
// log_level: "debug"
//   ├── default: "info" (overridden)
//   ├── ~/.prodigy/config.yml:3: "info" (overridden)
//   └── $PRODIGY_LOG_LEVEL: "debug" ← final value
```

#### Phase 5: Issue Detection

```rust
// src/config/diagnostics.rs

#[derive(Debug)]
pub enum ConfigIssue {
    /// Environment variable is empty string (may be unintentional)
    EmptyEnvVar { var_name: String, path: String },

    /// Key in config file doesn't match any known field
    UnknownKey { file: PathBuf, line: usize, key: String },

    /// Value was set in multiple places
    MultipleOverrides { path: String, count: usize },

    /// Relative path might resolve differently
    RelativePathAmbiguity { path: String, value: PathBuf },
}

impl TracedProdigyConfig {
    /// Detect potential configuration issues
    pub fn detect_issues(&self) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();

        // Check for empty env var overrides
        for (path, trace) in self.all_traces() {
            if let ValueSource::Environment { var_name } = &trace.final_source {
                if trace.final_value == serde_json::Value::String(String::new()) {
                    issues.push(ConfigIssue::EmptyEnvVar {
                        var_name: var_name.clone(),
                        path: path.clone(),
                    });
                }
            }
        }

        // Check for multiple overrides (potential confusion)
        for path in self.overridden_paths() {
            if let Some(trace) = self.trace(&path) {
                if trace.history.len() > 2 {
                    issues.push(ConfigIssue::MultipleOverrides {
                        path: path.clone(),
                        count: trace.history.len(),
                    });
                }
            }
        }

        issues
    }
}
```

### CLI Output Examples

```bash
# Trace single value
$ prodigy config trace log_level
log_level: "debug"
  ├── default: "info" (overridden)
  ├── ~/.prodigy/config.yml:3: "info" (overridden)
  └── $PRODIGY_LOG_LEVEL: "debug" ← final value

# Trace nested value
$ prodigy config trace project.name
project.name: "my-project"
  └── .prodigy/config.yml:5: "my-project" ← final value

# Show all overrides
$ prodigy config trace --overrides
Overridden configuration values:

log_level: "debug"
  ├── default: "info" (overridden)
  └── $PRODIGY_LOG_LEVEL: "debug" ← final value

auto_commit: true
  ├── default: false (overridden)
  └── .prodigy/config.yml:8: true ← final value

# JSON output
$ prodigy config trace log_level --json
{
  "path": "log_level",
  "final_value": "debug",
  "final_source": {
    "type": "environment",
    "var_name": "PRODIGY_LOG_LEVEL"
  },
  "history": [
    {"value": "info", "source": {"type": "default"}, "overridden": true},
    {"value": "info", "source": {"type": "file", "path": "~/.prodigy/config.yml", "line": 3}, "overridden": true},
    {"value": "debug", "source": {"type": "environment", "var_name": "PRODIGY_LOG_LEVEL"}, "overridden": false}
  ]
}

# Show detected issues
$ prodigy config trace --diagnose
Configuration issues detected:

⚠ Empty environment variable
  $PRODIGY_API_KEY is set but empty at path "claude_api_key"
  Suggestion: Unset the variable or provide a value

⚠ Multiple overrides
  "log_level" was set in 3 places
  History: default → ~/.prodigy/config.yml → $PRODIGY_LOG_LEVEL
  Suggestion: Review if all overrides are intentional
```

## Dependencies

- **Prerequisites**: Spec 178, 179 (Premortem integration must be complete)
- **Affected Components**:
  - `src/config/builder.rs` - Add `build_traced()` function
  - `src/config/tracing.rs` - New module for trace types
  - `src/config/diagnostics.rs` - New module for issue detection
  - `src/cli/` - New `config trace` subcommand
- **External Dependencies**: `premortem` crate's `TracedConfig` feature

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_trace_default_value() {
    let env = MockEnv::new();  // No files, no env vars
    let traced = load_prodigy_config_traced_with(&env).unwrap();

    let trace = traced.trace("log_level").unwrap();
    assert_eq!(trace.final_value, json!("info"));
    assert!(matches!(trace.final_source, ValueSource::Default));
    assert_eq!(trace.history.len(), 1);
}

#[test]
fn test_trace_file_override() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", "log_level: debug");

    let traced = load_prodigy_config_traced_with(&env).unwrap();
    let trace = traced.trace("log_level").unwrap();

    assert_eq!(trace.final_value, json!("debug"));
    assert!(matches!(trace.final_source, ValueSource::File { .. }));
    assert_eq!(trace.history.len(), 2);  // default + file
    assert!(trace.history[0].was_overridden);
}

#[test]
fn test_trace_env_override() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", "log_level: info")
        .with_env("PRODIGY_LOG_LEVEL", "trace");

    let traced = load_prodigy_config_traced_with(&env).unwrap();
    let trace = traced.trace("log_level").unwrap();

    assert_eq!(trace.final_value, json!("trace"));
    assert!(matches!(trace.final_source, ValueSource::Environment { .. }));
    assert_eq!(trace.history.len(), 3);  // default + file + env
}

#[test]
fn test_overridden_paths() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", "log_level: debug\nauto_commit: true")
        .with_env("PRODIGY_LOG_LEVEL", "trace");

    let traced = load_prodigy_config_traced_with(&env).unwrap();
    let overridden: Vec<_> = traced.overridden_paths().collect();

    assert!(overridden.contains(&"log_level".to_string()));
    assert!(overridden.contains(&"auto_commit".to_string()));
}
```

### CLI Integration Tests
```rust
#[test]
fn test_cli_trace_command() {
    let output = Command::new("prodigy")
        .args(["config", "trace", "log_level"])
        .env("PRODIGY_LOG_LEVEL", "debug")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("log_level"));
    assert!(stdout.contains("debug"));
    assert!(stdout.contains("PRODIGY_LOG_LEVEL"));
}
```

## Documentation Requirements

- **Code Documentation**: Document `TracedConfig` API and trace types
- **User Documentation**: Add "Debugging Configuration" section to docs
- **CLI Help**: Add help text for `prodigy config trace` command
- **Examples**: Include real-world debugging scenarios

## Implementation Notes

1. **Lazy Tracing**: Only call `build_traced()` when tracing is needed
2. **Cache Results**: If multiple trace queries, compute once
3. **Color Support**: Use `termcolor` or similar for terminal colors
4. **CI Friendly**: Disable colors when not a TTY

## Migration and Compatibility

- **No Breaking Changes**: New functionality only
- **Optional Feature**: Tracing is opt-in via CLI command
- **Performance**: Normal config loading unaffected
