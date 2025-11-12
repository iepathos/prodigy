## Configuration Precedence Rules

Prodigy loads configuration from multiple sources with a clear precedence hierarchy. Understanding how configuration is merged helps you control which settings take effect.

### Precedence Hierarchy

From highest to lowest priority:

1. **Project Config** (`.prodigy/config.yml`) - Highest priority
   - Project-specific settings in your repository
   - Located at `.prodigy/config.yml` in your project directory
   - Overrides all default values
   - Committed to version control (be careful with secrets)

2. **Defaults** (lowest priority)
   - Built-in default values defined in the code
   - Used when project config doesn't provide a value
   - See source: `src/config/mod.rs:88-100`

**Note**: Global config (`~/.prodigy/config.yml`), environment variables (`PRODIGY_*`), and CLI flag overrides are defined in the code but not currently loaded in production. Only project-level configuration and defaults are active.

### How Settings Are Loaded

When Prodigy starts, it builds the final configuration with this process:

1. **Initialize with defaults** - Create `GlobalConfig` with built-in defaults (source: `src/config/mod.rs:88-100`)
2. **Load project config** - Read `.prodigy/config.yml` from project directory (source: `src/config/loader.rs:85-104`)
3. **Merge at field level** - Project config values override defaults on a per-field basis (source: `src/config/mod.rs:133-154`)

Project config **overrides** default values at the individual field level. Settings not specified in project config inherit from defaults.

### Examples

#### Example 1: Using Defaults

```yaml
# No .prodigy/config.yml file exists
```

**Result**: Prodigy uses all default values:
- `log_level: "info"`
- `auto_commit: true`
- `max_concurrent_specs: 1`

(Source: `src/config/mod.rs:88-100`)

#### Example 2: Project Config Override

```yaml
# .prodigy/config.yml (project config)
name: my-project
claude_api_key: "sk-project-key"
auto_commit: false
```

**Result**:
- `claude_api_key: "sk-project-key"` (from project config)
- `auto_commit: false` (from project config)
- `log_level: "info"` (from defaults - not specified in project)
- `max_concurrent_specs: 1` (from defaults - not specified in project)

Field-level precedence is implemented via getter methods (source: `src/config/mod.rs:133-154`):
```rust
pub fn get_auto_commit(&self) -> bool {
    self.project
        .as_ref()
        .and_then(|p| p.auto_commit)
        .or(self.global.auto_commit)
        .unwrap_or(true)  // Default if neither provides value
}
```

#### Example 3: Partial Project Override

```yaml
# .prodigy/config.yml
name: my-project
claude_api_key: "sk-abc123"
# Other fields not specified
```

**Result**:
- `claude_api_key: "sk-abc123"` (from project config)
- `log_level: "info"` (from defaults)
- `auto_commit: true` (from defaults)
- `max_concurrent_specs: 1` (from defaults)

### Field-Level Precedence

Precedence is applied **per field**, not per file. Each configuration field is resolved independently using the precedence rules.

```yaml
# .prodigy/config.yml (project config)
name: my-project
auto_commit: false  # Only override auto_commit
# Other fields inherited from defaults
```

**Precedence Logic** (source: `src/config/mod.rs:133-154`):
1. Check if project config has the field → use it
2. Otherwise, check if global config has the field → use it
3. Otherwise, use the default value

This allows fine-grained configuration: override only what you need, inherit the rest.

### Configuration Loading Implementation

The configuration loading happens in these steps (source: `src/config/loader.rs`):

**Step 1: Initialize**
```rust
// ConfigLoader::new() - line 23
let config = Config::new();  // Creates Config with GlobalConfig defaults
```

**Step 2: Load Project Config** (optional)
```rust
// ConfigLoader::load_project() - line 85
let config_path = project_path.join(".prodigy").join("config.yml");
if config_path.exists() {
    let content = fs::read_to_string(&config_path).await?;
    let project_config = parse_project_config(&content)?;
    *config = merge_project_config(config.clone(), project_config);
}
```

**Step 3: Access with Precedence**
```rust
// Config::get_claude_api_key() - line 133
self.project
    .as_ref()
    .and_then(|p| p.claude_api_key.as_deref())  // Try project first
    .or(self.global.claude_api_key.as_deref())   // Fall back to global
```

### Default Values

Built-in defaults (source: `src/config/mod.rs:88-100`):

```rust
impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            prodigy_home: get_global_prodigy_dir()
                .unwrap_or_else(|_| PathBuf::from("~/.prodigy")),
            default_editor: None,
            log_level: Some("info".to_string()),
            claude_api_key: None,
            max_concurrent_specs: Some(1),
            auto_commit: Some(true),
            plugins: None,
        }
    }
}
```

### Future: Global Config and Environment Variables

The codebase includes infrastructure for additional configuration sources, but these are not currently loaded in production:

**Global Config** (`~/.prodigy/config.yml`):
- Mentioned in documentation (line 49: `src/config/mod.rs`)
- No loader implementation yet
- Would provide user-level defaults across all projects

**Environment Variables**:
- Defined in `Config::merge_env_vars()` (lines 111-131: `src/config/mod.rs`)
- Supports: `PRODIGY_CLAUDE_API_KEY`, `PRODIGY_LOG_LEVEL`, `PRODIGY_EDITOR`, `PRODIGY_AUTO_COMMIT`
- Only called in tests, not in production code
- Would override file-based configuration when implemented

**CLI Flag Overrides**:
- No implementation yet
- Would provide highest-priority overrides for individual runs

### Test Coverage

Configuration precedence behavior is validated through comprehensive tests (source: `src/config/loader.rs:113-334`):

**Test: Default Configuration**
```rust
// Line 120: test_new_creates_default_config
// Verifies GlobalConfig defaults are set correctly
assert_eq!(config.global.log_level, Some("info".to_string()));
assert_eq!(config.global.max_concurrent_specs, Some(1));
assert_eq!(config.global.auto_commit, Some(true));
```

**Test: Project Config Loading**
```rust
// Line 230: test_load_project_config
// Verifies .prodigy/config.yml is loaded and merged
let project = config.project.unwrap();
assert_eq!(project.name, "test-project");
assert_eq!(project.claude_api_key, Some("test-key".to_string()));
assert_eq!(project.auto_commit, Some(false));
```

**Test: Field-Level Override**
```rust
// src/config/mod.rs:471 - test shows project overrides global
config.project = Some(ProjectConfig {
    name: "test".into(),
    claude_api_key: Some("project-key".into()),
    // ... other fields
});
assert_eq!(config.get_claude_api_key(), Some("project-key"));
```
