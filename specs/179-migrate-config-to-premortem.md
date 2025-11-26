---
number: 179
title: Migrate GlobalConfig and ProjectConfig to Premortem
category: foundation
priority: high
status: draft
dependencies: [178]
created: 2025-11-25
---

# Specification 179: Migrate GlobalConfig and ProjectConfig to Premortem

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 178 (Premortem Integration Foundation)

## Context

After establishing the premortem foundation (Spec 178), the next step is migrating Prodigy's existing configuration types to use the new system. Currently, configuration is loaded through:

1. **GlobalConfig** (`src/config/mod.rs`): User-wide settings in `~/.prodigy/`
2. **ProjectConfig** (`src/config/mod.rs`): Project-specific settings in `.prodigy/config.yml`
3. **ConfigLoader** (`src/config/loader.rs`): Orchestrates loading with manual precedence handling

### Current Pain Points Being Addressed

1. **Manual Precedence Logic**: `get_claude_api_key()`, `get_auto_commit()` manually check project → global → env
2. **Scattered Defaults**: Default values defined in multiple places with `fn default_*()` functions
3. **Limited Validation**: Only basic serde deserialization, no cross-field validation
4. **Poor Error Messages**: Generic deserialization errors without source location context

### Target State

- Single `ProdigyConfig` struct loaded via premortem with automatic precedence
- Declarative validation with `#[validate(...)]` attributes
- All errors accumulated and reported with source locations
- Backward-compatible APIs that delegate to new system

## Objective

Migrate `GlobalConfig` and `ProjectConfig` to be loaded through the premortem-based `ProdigyConfig` system, maintaining full backward compatibility while gaining error accumulation, source tracking, and cleaner validation.

## Requirements

### Functional Requirements

#### FR1: Migrate GlobalConfig Fields
- **MUST** migrate all `GlobalConfig` fields to `ProdigyConfig`:
  - `log_level: String`
  - `claude_api_key: Option<String>`
  - `max_concurrent_specs: usize`
  - `auto_commit: bool`
  - `default_editor: Option<String>`
  - `plugins: Vec<PluginConfig>`
- **MUST** preserve existing default values
- **MUST** validate `log_level` is valid (trace, debug, info, warn, error)
- **MUST** validate `max_concurrent_specs` is in range 1-100

#### FR2: Migrate ProjectConfig Fields
- **MUST** migrate all `ProjectConfig` fields to `ProdigyConfig.project`:
  - `name: Option<String>`
  - `description: Option<String>`
  - `version: Option<String>`
  - `spec_dir: Option<PathBuf>`
  - `claude_api_key: Option<String>` (overrides global)
  - `auto_commit: Option<bool>` (overrides global)
  - `variables: HashMap<String, Value>`
- **MUST** validate `spec_dir` exists when provided
- **MUST** validate `name` is non-empty when provided

#### FR3: Automatic Precedence Resolution
- **MUST** implement layered loading order:
  1. Hardcoded defaults
  2. Global config (`~/.prodigy/config.yml`)
  3. Project config (`.prodigy/config.yml`)
  4. Environment variables (`PRODIGY_*`)
- **MUST** later sources override earlier sources
- **MUST** support nested overrides (e.g., `PRODIGY_PROJECT_NAME`)

#### FR4: Backward Compatible APIs
- **MUST** provide `GlobalConfig::load()` that delegates to new system
- **MUST** provide `ProjectConfig::load()` that delegates to new system
- **MUST** maintain existing method signatures for gradual migration
- **SHOULD** mark old methods as `#[deprecated]` with migration guidance

#### FR5: Environment Variable Mapping
- **MUST** support existing env vars:
  - `PRODIGY_CLAUDE_API_KEY` → `claude_api_key`
  - `PRODIGY_LOG_LEVEL` → `log_level`
  - `PRODIGY_AUTO_COMMIT` → `auto_commit`
  - `PRODIGY_EDITOR` → `default_editor`
  - `PRODIGY_MAX_CONCURRENT` → `max_concurrent_specs`
- **MUST** support nested paths with separator:
  - `PRODIGY_PROJECT__NAME` → `project.name`
  - `PRODIGY_STORAGE__BACKEND` → `storage.backend`

#### FR6: Cross-Field Validation
- **MUST** validate that `spec_dir` is under project root when both are set
- **MUST** validate that conflicting options are not set (future expansion)
- **MUST** accumulate cross-field errors with other validation errors

### Non-Functional Requirements

#### NFR1: Zero Functional Regression
- **MUST** pass all existing configuration tests
- **MUST** load same config files with same results
- **MUST** behave identically to existing system for valid configs

#### NFR2: Improved Error Experience
- **MUST** report all validation errors at once
- **MUST** include file path and line number in errors
- **MUST** provide actionable error messages

#### NFR3: Performance
- **MUST NOT** add measurable latency to startup
- **SHOULD** cache loaded configuration

## Acceptance Criteria

- [ ] All `GlobalConfig` fields migrated to `ProdigyConfig`
- [ ] All `ProjectConfig` fields migrated to `ProdigyConfig.project`
- [ ] Layered loading order implemented and tested
- [ ] Environment variable mapping working for all existing vars
- [ ] `GlobalConfig::load()` delegates to premortem system
- [ ] `ProjectConfig::load()` delegates to premortem system
- [ ] Old methods marked `#[deprecated]` with migration notes
- [ ] Cross-field validation implemented for spec_dir/project_root
- [ ] All existing config tests pass
- [ ] New tests verify error accumulation
- [ ] New tests verify source location in errors
- [ ] Error messages include file:line information

## Technical Details

### Implementation Approach

#### Phase 1: Extend ProdigyConfig
```rust
// src/config/prodigy_config.rs
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ProdigyConfig {
    // Global settings
    #[validate(one_of("trace", "debug", "info", "warn", "error"))]
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default)]
    pub claude_api_key: Option<String>,

    #[validate(range(1..=100))]
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_specs: usize,

    #[serde(default)]
    pub auto_commit: bool,

    #[serde(default)]
    pub default_editor: Option<String>,

    #[serde(default)]
    pub plugins: Vec<PluginConfig>,

    // Project settings (optional, only present if in project)
    #[serde(default)]
    #[validate(nested)]
    pub project: Option<ProjectSettings>,
}

#[derive(Debug, Clone, Deserialize, Validate, Default)]
pub struct ProjectSettings {
    #[validate(non_empty_if_present)]
    pub name: Option<String>,

    pub description: Option<String>,

    pub version: Option<String>,

    #[validate(directory_exists_if_present)]
    pub spec_dir: Option<PathBuf>,

    pub claude_api_key: Option<String>,

    pub auto_commit: Option<bool>,

    #[serde(default)]
    pub variables: HashMap<String, Value>,
}
```

#### Phase 2: Implement Custom Validation
```rust
impl Validate for ProdigyConfig {
    fn validate(&self) -> ConfigValidation<()> {
        // Derive macro handles field validations
        // Add cross-field validation here

        let mut errors = Vec::new();

        // Cross-field: spec_dir should be relative to project
        if let Some(ref project) = self.project {
            if let Some(ref spec_dir) = project.spec_dir {
                if spec_dir.is_absolute() {
                    errors.push(ConfigError::validation_error(
                        "project.spec_dir",
                        "spec_dir should be relative to project root"
                    ));
                }
            }
        }

        if errors.is_empty() {
            Validation::Success(())
        } else {
            Validation::Failure(errors.into())
        }
    }
}
```

#### Phase 3: Create Backward-Compatible Wrappers
```rust
// src/config/mod.rs

impl GlobalConfig {
    #[deprecated(since = "0.x.0", note = "Use ProdigyConfig::load() instead")]
    pub fn load() -> Result<Self, ConfigError> {
        let config = load_prodigy_config()?;
        Ok(Self::from_prodigy_config(&config))
    }

    fn from_prodigy_config(config: &ProdigyConfig) -> Self {
        GlobalConfig {
            log_level: config.log_level.clone(),
            claude_api_key: config.claude_api_key.clone(),
            max_concurrent_specs: config.max_concurrent_specs,
            auto_commit: config.auto_commit,
            default_editor: config.default_editor.clone(),
            plugins: config.plugins.clone(),
        }
    }
}

impl ProjectConfig {
    #[deprecated(since = "0.x.0", note = "Use ProdigyConfig::load() instead")]
    pub fn load() -> Result<Option<Self>, ConfigError> {
        let config = load_prodigy_config()?;
        Ok(config.project.map(Self::from_project_settings))
    }

    fn from_project_settings(settings: ProjectSettings) -> Self {
        ProjectConfig {
            name: settings.name,
            description: settings.description,
            version: settings.version,
            spec_dir: settings.spec_dir,
            claude_api_key: settings.claude_api_key,
            auto_commit: settings.auto_commit,
            variables: settings.variables,
        }
    }
}
```

#### Phase 4: Environment Variable Mapping
```rust
// In builder.rs
pub fn load_prodigy_config_with<E: ConfigEnv>(env: &E) -> Result<Config<ProdigyConfig>, ConfigErrors> {
    Config::<ProdigyConfig>::builder()
        .source(Defaults::from(ProdigyConfig::defaults()))
        .source(Yaml::file_optional(global_config_path()))
        .source(Yaml::file_optional(".prodigy/config.yml"))
        .source(Env::builder()
            .prefix("PRODIGY_")
            .separator("__")  // PRODIGY_PROJECT__NAME → project.name
            .build())
        .build_with_env(env)
}
```

### Migration Path for Existing Code

```rust
// Before (scattered calls)
let global = GlobalConfig::load()?;
let project = ProjectConfig::load()?;
let api_key = project
    .and_then(|p| p.claude_api_key.clone())
    .or_else(|| global.claude_api_key.clone())
    .or_else(|| std::env::var("PRODIGY_CLAUDE_API_KEY").ok());

// After (single unified access)
let config = load_prodigy_config()?;
let api_key = config.effective_api_key();  // Precedence handled internally
```

### Helper Methods
```rust
impl ProdigyConfig {
    /// Get API key with proper precedence (project → global → env already applied)
    pub fn effective_api_key(&self) -> Option<&str> {
        self.project
            .as_ref()
            .and_then(|p| p.claude_api_key.as_deref())
            .or(self.claude_api_key.as_deref())
    }

    /// Get auto_commit with proper precedence
    pub fn effective_auto_commit(&self) -> bool {
        self.project
            .as_ref()
            .and_then(|p| p.auto_commit)
            .unwrap_or(self.auto_commit)
    }
}
```

## Dependencies

- **Prerequisites**: Spec 178 (Premortem Integration Foundation)
- **Affected Components**:
  - `src/config/mod.rs` - Deprecate old types, add wrappers
  - `src/config/loader.rs` - May be simplified or deprecated
  - `src/config/prodigy_config.rs` - Extend with all fields
  - All callers of `GlobalConfig::load()` and `ProjectConfig::load()`
- **External Dependencies**: None new

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_global_config_migration() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", r#"
            log_level: debug
            max_concurrent_specs: 10
            auto_commit: true
        "#);

    let config = load_prodigy_config_with(&env).unwrap();

    // Verify all fields migrated correctly
    assert_eq!(config.log_level, "debug");
    assert_eq!(config.max_concurrent_specs, 10);
    assert!(config.auto_commit);
}

#[test]
fn test_project_overrides_global() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", r#"
            claude_api_key: global-key
            auto_commit: false
        "#)
        .with_file(".prodigy/config.yml", r#"
            project:
              name: my-project
              claude_api_key: project-key
              auto_commit: true
        "#);

    let config = load_prodigy_config_with(&env).unwrap();

    assert_eq!(config.effective_api_key(), Some("project-key"));
    assert!(config.effective_auto_commit());
}

#[test]
fn test_env_overrides_all() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", "log_level: info")
        .with_file(".prodigy/config.yml", "project:\n  name: test")
        .with_env("PRODIGY_LOG_LEVEL", "trace");

    let config = load_prodigy_config_with(&env).unwrap();

    assert_eq!(config.log_level, "trace");  // Env wins
}
```

### Validation Error Tests
```rust
#[test]
fn test_multiple_validation_errors() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", r#"
            log_level: invalid_level
            max_concurrent_specs: 500
        "#);

    let result = load_prodigy_config_with(&env);

    match result {
        Err(errors) => {
            assert_eq!(errors.len(), 2);  // Both errors reported
            assert!(errors.iter().any(|e| e.path == "log_level"));
            assert!(errors.iter().any(|e| e.path == "max_concurrent_specs"));
        }
        Ok(_) => panic!("Expected validation errors"),
    }
}
```

### Backward Compatibility Tests
```rust
#[test]
#[allow(deprecated)]
fn test_deprecated_global_config_still_works() {
    // Existing test should continue to pass
    let config = GlobalConfig::load().unwrap();
    assert!(!config.log_level.is_empty());
}
```

## Documentation Requirements

- **Code Documentation**: Document all `ProdigyConfig` fields with examples
- **User Documentation**: Update CLAUDE.md with new config system notes
- **Migration Guide**: Document path from old to new APIs

## Implementation Notes

1. **Gradual Rollout**: Keep old types functional during migration period
2. **Feature Flag**: Consider `PRODIGY_USE_PREMORTEM_CONFIG=1` for early adopters
3. **Warning on Deprecated**: Log warning when deprecated APIs are used
4. **Test Coverage**: Ensure 100% coverage of migration code paths

## Migration and Compatibility

- **Breaking Changes**: None - old APIs deprecated but functional
- **Deprecation Timeline**: Old APIs removed in next major version
- **Migration Script**: Consider tool to update call sites (optional)
