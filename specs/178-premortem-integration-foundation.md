---
number: 178
title: Premortem Integration Foundation
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-11-25
---

# Specification 178: Premortem Integration Foundation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None (foundational)

## Context

Prodigy's configuration system has grown organically across 8+ modules with several pain points:

1. **Scattered Configuration**: Config types spread across `src/config/`, `src/storage/config.rs`, `src/cook/environment/config.rs`, etc.
2. **One-at-a-Time Errors**: Configuration validation reports errors sequentially - users must fix and retry repeatedly
3. **Unsafe Test Patterns**: Tests use `std::env::set_var()` which is unsafe in Rust 2024 and requires `#[serial]` attributes
4. **No Source Tracking**: When config values merge from multiple sources, there's no way to know where a value originated
5. **Manual Precedence**: Getter functions manually implement precedence (global → project → env vars)

The `premortem` crate (recently published to crates.io) addresses these issues with:
- **Error accumulation**: All config errors discovered in one run using Stillwater's `Validation<T, E>`
- **Source location tracking**: Know exactly which file/env var/line caused each error
- **Testable I/O**: `MockEnv` for isolated testing without global state mutation
- **Value tracing**: See override history across multiple sources
- **Layered sources**: Declarative source ordering (defaults → TOML → env vars)

This spec establishes the foundation for integrating premortem into Prodigy.

## Objective

Add the `premortem` crate as a dependency and create a unified `ProdigyConfig` struct that will serve as the single entry point for all configuration access, using premortem's builder pattern and validation system.

## Requirements

### Functional Requirements

#### FR1: Add Premortem Dependency
- **MUST** add `premortem` to `Cargo.toml` with appropriate features (`toml`, `yaml`, `derive`)
- **MUST** ensure compatibility with existing `stillwater` dependency
- **MUST NOT** break existing configuration loading during migration period

#### FR2: Create Unified ProdigyConfig Struct
- **MUST** define `ProdigyConfig` struct that combines:
  - Global settings (log level, API keys, defaults)
  - Project settings (name, spec directory, variables)
  - Runtime settings (verbosity, auto-accept flags)
- **MUST** implement `serde::Deserialize` for YAML/TOML loading
- **MUST** implement premortem's `Validate` trait for comprehensive validation
- **MUST** use premortem's derive macros where appropriate

#### FR3: Implement ConfigBuilder Pattern
- **MUST** create `ProdigyConfigBuilder` using premortem's `Config::builder()`
- **MUST** support layered sources:
  1. Hardcoded defaults (via `Defaults::from()`)
  2. Global config file (`~/.prodigy/config.yml`)
  3. Project config file (`.prodigy/config.yml`)
  4. Environment variables (`PRODIGY_*` prefix)
- **MUST** support optional config files (missing files not an error)
- **MUST** accumulate all validation errors before returning

#### FR4: Validation Rules
- **MUST** validate required fields are present
- **MUST** validate field types match expected types
- **MUST** validate path fields point to valid directories where required
- **MUST** validate numeric fields are within acceptable ranges
- **MUST** use premortem's `#[validate(...)]` attributes where possible

#### FR5: Error Reporting
- **MUST** report all validation errors at once (not fail-fast)
- **MUST** include source location (file, line, column) in error messages
- **MUST** clearly indicate which source provided invalid values
- **SHOULD** support pretty-printed error output for CLI

### Non-Functional Requirements

#### NFR1: Backward Compatibility
- **MUST** maintain existing config file formats during migration
- **MUST** support existing environment variable names
- **MUST** allow gradual migration of existing code to new system

#### NFR2: Performance
- **MUST** have zero runtime overhead for production config access (via `Deref`)
- **SHOULD** cache parsed configuration to avoid repeated file reads

#### NFR3: Testability
- **MUST** design all config loading to accept `ConfigEnv` trait object
- **MUST** enable `MockEnv` usage from day one

## Acceptance Criteria

- [ ] `premortem` crate added to Cargo.toml with `toml`, `yaml`, `derive` features
- [ ] `ProdigyConfig` struct defined with all major configuration fields
- [ ] `ProdigyConfigBuilder` created supporting layered sources
- [ ] Validation errors accumulate and report all issues at once
- [ ] Source location included in validation error messages
- [ ] `cargo build` succeeds with new dependencies
- [ ] `cargo test` passes with no regressions
- [ ] Example usage documented in code comments
- [ ] Existing `ConfigLoader` still works (backward compatibility)

## Technical Details

### Implementation Approach

#### Phase 1: Add Dependency
```toml
# Cargo.toml
[dependencies]
premortem = { version = "0.1", features = ["toml", "yaml", "derive"] }
```

#### Phase 2: Define ProdigyConfig
```rust
// src/config/prodigy_config.rs
use premortem::{Config, Validate};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ProdigyConfig {
    #[validate(non_empty)]
    pub log_level: String,

    #[serde(default)]
    pub claude_api_key: Option<String>,

    #[validate(range(1..=100))]
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_specs: usize,

    #[serde(default)]
    pub auto_commit: bool,

    #[serde(default)]
    pub project: Option<ProjectSettings>,

    #[serde(default)]
    pub storage: StorageSettings,
}

#[derive(Debug, Clone, Deserialize, Validate, Default)]
pub struct ProjectSettings {
    pub name: Option<String>,
    pub spec_dir: Option<PathBuf>,
    pub variables: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct StorageSettings {
    #[serde(default = "default_backend")]
    pub backend: BackendType,

    pub base_path: Option<PathBuf>,
}
```

#### Phase 3: Implement Builder
```rust
// src/config/builder.rs
use premortem::{Config, ConfigEnv, Toml, Yaml, Env, Defaults};

pub fn load_prodigy_config() -> Result<Config<ProdigyConfig>, ConfigErrors> {
    load_prodigy_config_with(&RealEnv)
}

pub fn load_prodigy_config_with<E: ConfigEnv>(env: &E) -> Result<Config<ProdigyConfig>, ConfigErrors> {
    let defaults = ProdigyConfig::defaults();

    Config::<ProdigyConfig>::builder()
        .source(Defaults::from(defaults))
        .source(Yaml::file_optional(global_config_path()))
        .source(Yaml::file_optional(".prodigy/config.yml"))
        .source(Env::prefix("PRODIGY_"))
        .build_with_env(env)
}
```

### File Structure
```
src/config/
├── mod.rs              # Existing (unchanged)
├── loader.rs           # Existing (unchanged)
├── prodigy_config.rs   # NEW: ProdigyConfig struct
├── builder.rs          # NEW: Config builder using premortem
└── defaults.rs         # NEW: Default values
```

### Data Structures

```rust
// Validation error with source location
pub struct ProdigyConfigError {
    pub path: String,           // e.g., "storage.backend"
    pub message: String,        // e.g., "invalid backend type: 'foo'"
    pub source: SourceLocation, // file:line:col
}

// Builder configuration
pub struct BuilderOptions {
    pub global_config_path: Option<PathBuf>,
    pub project_config_path: Option<PathBuf>,
    pub env_prefix: String,
}
```

## Dependencies

- **Prerequisites**: None (foundational spec)
- **Affected Components**:
  - `src/config/` - New modules added
  - `Cargo.toml` - New dependency
- **External Dependencies**:
  - `premortem` crate (0.1.x)
  - Existing `stillwater` (already in use)

## Testing Strategy

- **Unit Tests**: Test `ProdigyConfig` deserialization and validation
- **Integration Tests**: Test layered source loading with `MockEnv`
- **Validation Tests**: Test error accumulation with multiple invalid fields
- **Backward Compatibility**: Ensure existing `ConfigLoader` tests still pass

### Example Test with MockEnv
```rust
#[test]
fn test_config_from_multiple_sources() {
    let env = MockEnv::new()
        .with_file("~/.prodigy/config.yml", r#"
            log_level: info
            max_concurrent_specs: 5
        "#)
        .with_file(".prodigy/config.yml", r#"
            project:
              name: my-project
        "#)
        .with_env("PRODIGY_LOG_LEVEL", "debug");  // Override

    let config = load_prodigy_config_with(&env).unwrap();

    assert_eq!(config.log_level, "debug");  // Env wins
    assert_eq!(config.max_concurrent_specs, 5);  // From global
    assert_eq!(config.project.unwrap().name, Some("my-project".into()));
}
```

## Documentation Requirements

- **Code Documentation**: Document `ProdigyConfig` fields and validation rules
- **User Documentation**: None yet (internal infrastructure)
- **Architecture Updates**: Add to CLAUDE.md configuration section when complete

## Implementation Notes

1. **Start Small**: Only include core fields in initial `ProdigyConfig` - expand in later specs
2. **Parallel Systems**: Old `ConfigLoader` and new `ProdigyConfig` will coexist during migration
3. **Feature Flag**: Consider `cfg` flag to enable new config system for testing
4. **Validation Messages**: Use clear, actionable error messages with fix suggestions

## Migration and Compatibility

- **No Breaking Changes**: This spec adds new code alongside existing
- **Gradual Adoption**: Existing code continues to use `ConfigLoader` until explicitly migrated
- **Deprecation Path**: Future specs will migrate components and eventually deprecate `ConfigLoader`
