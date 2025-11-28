// Allow large error types from premortem's API design
#![allow(clippy::result_large_err)]

//! Configuration builder using premortem for layered source loading.
//!
//! This module provides functions to load `ProdigyConfig` from multiple sources
//! with proper precedence and comprehensive error accumulation.
//!
//! # Source Precedence
//!
//! Sources are loaded in order from lowest to highest priority:
//!
//! 1. **Defaults** - Hardcoded default values
//! 2. **Global config** - `~/.prodigy/config.yml` (optional)
//! 3. **Project config** - `.prodigy/config.yml` (optional)
//! 4. **Environment variables** - `PRODIGY_*` prefix (highest priority)
//!
//! # Example
//!
//! ```no_run
//! use prodigy::config::load_prodigy_config;
//!
//! let config = load_prodigy_config().expect("failed to load config");
//! println!("Log level: {}", config.log_level);
//! ```
//!
//! # Testing
//!
//! Use `load_prodigy_config_with` with a `MockEnv` for testing:
//!
//! ```
//! use prodigy::config::load_prodigy_config_with;
//! use premortem::MockEnv;
//!
//! let env = MockEnv::new();
//! let config = load_prodigy_config_with(&env).expect("failed to load config");
//! assert_eq!(config.log_level, "info"); // default value
//! ```

use super::prodigy_config::{global_config_path, project_config_path, ProdigyConfig};
use premortem::config::Config;
use premortem::prelude::*;

/// Load Prodigy configuration from all sources using real I/O.
///
/// This function loads configuration from:
/// 1. Hardcoded defaults
/// 2. Global config file (`~/.prodigy/config.yml`) - optional
/// 3. Project config file (`.prodigy/config.yml`) - optional
/// 4. Environment variables (`PRODIGY_*`)
///
/// Returns a `Config<ProdigyConfig>` wrapper that implements `Deref<Target=ProdigyConfig>`,
/// so you can access fields directly via dot notation.
///
/// # Errors
///
/// Returns `ConfigErrors` if any validation fails. All errors are accumulated
/// and reported together, not just the first one encountered.
///
/// # Example
///
/// ```no_run
/// use prodigy::config::load_prodigy_config;
///
/// let config = load_prodigy_config().expect("failed to load config");
/// println!("Log level: {}", config.log_level);
/// ```
pub fn load_prodigy_config() -> Result<Config<ProdigyConfig>, ConfigErrors> {
    load_prodigy_config_with(&RealEnv)
}

/// Load Prodigy configuration with a custom environment.
///
/// This function is designed for testing with `MockEnv` to avoid actual I/O
/// and enable isolated, deterministic tests.
///
/// # Arguments
///
/// * `env` - A `ConfigEnv` implementation (`RealEnv` for production, `MockEnv` for tests)
///
/// # Environment Variable Support
///
/// The following environment variables are supported:
/// - `PRODIGY_CLAUDE_API_KEY` → `claude_api_key`
/// - `PRODIGY_LOG_LEVEL` → `log_level`
/// - `PRODIGY_AUTO_COMMIT` → `auto_commit`
/// - `PRODIGY_EDITOR` → `default_editor`
/// - `PRODIGY_MAX_CONCURRENT` → `max_concurrent_specs`
///
/// For nested fields, use double underscore:
/// - `PRODIGY__PROJECT__NAME` → `project.name`
/// - `PRODIGY__STORAGE__BACKEND` → `storage.backend`
///
/// # Example
///
/// ```
/// use prodigy::config::load_prodigy_config_with;
/// use premortem::MockEnv;
///
/// let env = MockEnv::new()
///     .with_env("PRODIGY__LOG_LEVEL", "debug")
///     .with_env("PRODIGY__MAX_CONCURRENT_SPECS", "10");
///
/// let config = load_prodigy_config_with(&env).expect("failed to load");
/// assert_eq!(config.log_level, "debug");
/// assert_eq!(config.max_concurrent_specs, 10);
/// ```
pub fn load_prodigy_config_with<E: ConfigEnv>(
    env: &E,
) -> Result<Config<ProdigyConfig>, ConfigErrors> {
    let global_path = global_config_path();
    let project_path = project_config_path();

    // First, build with file sources and structured env vars
    let mut builder = Config::<ProdigyConfig>::builder()
        // Layer 1: Hardcoded defaults (lowest priority)
        .source(Defaults::from(ProdigyConfig::default()))
        // Layer 2: Global config file (optional - missing file is OK)
        .source(
            Yaml::file(global_path.to_string_lossy().to_string())
                .optional()
                .named("global config"),
        )
        // Layer 3: Project config file (optional - missing file is OK)
        .source(
            Yaml::file(project_path.to_string_lossy().to_string())
                .optional()
                .named("project config"),
        )
        // Layer 4: Environment variables (highest priority)
        // Use "__" as separator so single underscores in field names are preserved
        // E.g., PRODIGY__LOG_LEVEL -> log_level, PRODIGY__STORAGE__COMPRESSION_LEVEL -> storage.compression_level
        .source(Env::prefix("PRODIGY__").separator("__"));

    // Layer 5: Legacy environment variable mappings for backward compatibility
    // These use single underscore and have explicit field mappings
    builder = builder.source(
        Env::prefix("PRODIGY_")
            .map("CLAUDE_API_KEY", "claude_api_key")
            .map("LOG_LEVEL", "log_level")
            .map("AUTO_COMMIT", "auto_commit")
            .map("EDITOR", "default_editor")
            .map("MAX_CONCURRENT", "max_concurrent_specs"),
    );

    builder.build_with_env(env)
}

/// Load Prodigy configuration with tracing for debugging.
///
/// Returns a `TracedConfig` that tracks where each value originated,
/// useful for debugging configuration issues.
///
/// # Example
///
/// ```no_run
/// use prodigy::config::load_prodigy_config_traced;
///
/// let traced = load_prodigy_config_traced().expect("failed to load");
///
/// // See where a value came from
/// if let Some(trace) = traced.trace("max_concurrent_specs") {
///     println!("Value: {:?}", trace.final_value.value);
///     println!("Source: {:?}", trace.final_value.source);
/// }
///
/// // Get the actual config
/// let config = traced.into_inner();
/// ```
pub fn load_prodigy_config_traced() -> Result<TracedConfig<ProdigyConfig>, ConfigErrors> {
    load_prodigy_config_traced_with(&RealEnv)
}

/// Load Prodigy configuration with tracing using a custom environment.
pub fn load_prodigy_config_traced_with<E: ConfigEnv>(
    env: &E,
) -> Result<TracedConfig<ProdigyConfig>, ConfigErrors> {
    let global_path = global_config_path();
    let project_path = project_config_path();

    Config::<ProdigyConfig>::builder()
        .source(Defaults::from(ProdigyConfig::default()))
        .source(
            Yaml::file(global_path.to_string_lossy().to_string())
                .optional()
                .named("global config"),
        )
        .source(
            Yaml::file(project_path.to_string_lossy().to_string())
                .optional()
                .named("project config"),
        )
        .source(Env::prefix("PRODIGY__").separator("__"))
        // Legacy environment variable mappings for backward compatibility
        .source(
            Env::prefix("PRODIGY_")
                .map("CLAUDE_API_KEY", "claude_api_key")
                .map("LOG_LEVEL", "log_level")
                .map("AUTO_COMMIT", "auto_commit")
                .map("EDITOR", "default_editor")
                .map("MAX_CONCURRENT", "max_concurrent_specs"),
        )
        .build_traced_with_env(env)
}

/// Options for customizing config loading.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Path to a specific config file to load (instead of searching).
    pub config_path: Option<std::path::PathBuf>,
    /// Skip loading the global config file.
    pub skip_global: bool,
    /// Skip loading the project config file.
    pub skip_project: bool,
    /// Skip environment variables.
    pub skip_env: bool,
}

/// Load Prodigy configuration with custom options.
///
/// This function allows fine-grained control over which sources are loaded.
pub fn load_prodigy_config_with_options(
    options: &LoadOptions,
) -> Result<Config<ProdigyConfig>, ConfigErrors> {
    load_prodigy_config_with_options_and_env(options, &RealEnv)
}

/// Load Prodigy configuration with custom options and environment.
pub fn load_prodigy_config_with_options_and_env<E: ConfigEnv>(
    options: &LoadOptions,
    env: &E,
) -> Result<Config<ProdigyConfig>, ConfigErrors> {
    let mut builder = Config::<ProdigyConfig>::builder();

    // Always start with defaults
    builder = builder.source(Defaults::from(ProdigyConfig::default()));

    // Add global config if not skipped
    if !options.skip_global {
        let global_path = global_config_path();
        builder = builder.source(
            Yaml::file(global_path.to_string_lossy().to_string())
                .optional()
                .named("global config"),
        );
    }

    // Add specific config path if provided
    if let Some(ref path) = options.config_path {
        builder = builder.source(
            Yaml::file(path.to_string_lossy().to_string())
                .required()
                .named("specified config"),
        );
    } else if !options.skip_project {
        // Add project config if not skipped and no specific path provided
        let project_path = project_config_path();
        builder = builder.source(
            Yaml::file(project_path.to_string_lossy().to_string())
                .optional()
                .named("project config"),
        );
    }

    // Add environment variables if not skipped
    if !options.skip_env {
        builder = builder
            .source(Env::prefix("PRODIGY__").separator("__"))
            // Legacy environment variable mappings for backward compatibility
            .source(
                Env::prefix("PRODIGY_")
                    .map("CLAUDE_API_KEY", "claude_api_key")
                    .map("LOG_LEVEL", "log_level")
                    .map("AUTO_COMMIT", "auto_commit")
                    .map("EDITOR", "default_editor")
                    .map("MAX_CONCURRENT", "max_concurrent_specs"),
            );
    }

    builder.build_with_env(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_with_defaults_only() {
        let env = MockEnv::new();
        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.log_level, "info");
        assert_eq!(config.max_concurrent_specs, 4);
        assert!(config.auto_commit);
    }

    #[test]
    fn test_load_with_env_vars_only() {
        // Test env override with TOML + snake_case field names
        // Use double underscore separator for snake_case fields
        #[derive(Debug, serde::Deserialize, DeriveValidate)]
        struct SnakeCaseConfig {
            #[validate(non_empty)]
            log_level: String,
            #[validate(range(1..=100))]
            max_items: usize,
        }

        let env = MockEnv::new()
            .with_file(
                "config.toml",
                r#"
log_level = "info"
max_items = 10
"#,
            )
            // Use double underscore separator - APP__LOG_LEVEL -> log_level
            .with_env("APP__LOG_LEVEL", "debug")
            .with_env("APP__MAX_ITEMS", "50");

        let config = Config::<SnakeCaseConfig>::builder()
            .source(Toml::file("config.toml"))
            .source(Env::prefix("APP__").separator("__"))
            .build_with_env(&env)
            .expect("should load successfully");

        assert_eq!(config.log_level, "debug"); // From env
        assert_eq!(config.max_items, 50); // From env
    }

    #[test]
    fn test_load_with_global_config() {
        let global_path = global_config_path();
        let env = MockEnv::new().with_file(
            global_path.to_string_lossy().to_string(),
            r#"
log_level: debug
max_concurrent_specs: 8
"#,
        );

        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.log_level, "debug");
        assert_eq!(config.max_concurrent_specs, 8);
    }

    #[test]
    fn test_load_with_env_override() {
        let global_path = global_config_path();
        let env = MockEnv::new()
            .with_file(global_path.to_string_lossy().to_string(), "log_level: info")
            // Use double underscore prefix for ProdigyConfig
            .with_env("PRODIGY__LOG_LEVEL", "debug");

        let config = load_prodigy_config_with(&env).unwrap();

        // Environment variable takes precedence
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn test_load_with_project_config() {
        let project_path = project_config_path();
        let env = MockEnv::new().with_file(
            project_path.to_string_lossy().to_string(),
            r#"
project:
  name: test-project
  spec_dir: custom-specs
"#,
        );

        let config = load_prodigy_config_with(&env).unwrap();

        assert!(config.project.is_some());
        let project = config.project.clone().unwrap();
        assert_eq!(project.name, Some("test-project".to_string()));
        assert_eq!(
            project.spec_dir,
            Some(std::path::PathBuf::from("custom-specs"))
        );
    }

    #[test]
    fn test_load_with_options_skip_global() {
        let global_path = global_config_path();
        let env = MockEnv::new().with_file(
            global_path.to_string_lossy().to_string(),
            "log_level: debug",
        );

        let options = LoadOptions {
            skip_global: true,
            ..Default::default()
        };

        let config = load_prodigy_config_with_options_and_env(&options, &env).unwrap();

        // Should use default because global was skipped
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_validation_error_accumulation() {
        let project_path = project_config_path();
        let env = MockEnv::new().with_file(
            project_path.to_string_lossy().to_string(),
            r#"
log_level: ""
max_concurrent_specs: 0
storage:
  compression_level: 15
"#,
        );

        let result = load_prodigy_config_with(&env);

        // Should fail with multiple validation errors
        assert!(result.is_err());

        let errors = result.unwrap_err();
        // Should have multiple errors (empty log_level, out-of-range max_concurrent_specs, out-of-range compression_level)
        assert!(
            errors.len() >= 2,
            "Expected multiple errors, got {}",
            errors.len()
        );
    }

    #[test]
    fn test_missing_config_files_ok() {
        // No files configured - should still work with defaults
        let env = MockEnv::new();
        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_layered_precedence() {
        let global_path = global_config_path();
        let project_path = project_config_path();

        let env = MockEnv::new()
            .with_file(
                global_path.to_string_lossy().to_string(),
                r#"
log_level: debug
max_concurrent_specs: 8
auto_commit: true
"#,
            )
            .with_file(
                project_path.to_string_lossy().to_string(),
                r#"
log_level: warn
"#,
            )
            // Use double underscore prefix
            .with_env("PRODIGY__LOG_LEVEL", "error");

        let config = load_prodigy_config_with(&env).unwrap();

        // env > project > global > defaults
        assert_eq!(config.log_level, "error"); // from env
        assert_eq!(config.max_concurrent_specs, 8); // from global (project didn't override)
        assert!(config.auto_commit); // from global
    }

    #[test]
    fn test_traced_config() {
        let global_path = global_config_path();
        let env = MockEnv::new()
            .with_file(
                global_path.to_string_lossy().to_string(),
                "log_level: debug",
            )
            // Use double underscore prefix
            .with_env("PRODIGY__LOG_LEVEL", "warn");

        let traced = load_prodigy_config_traced_with(&env).unwrap();

        // The final value should be from env
        assert_eq!(traced.log_level, "warn");

        // Check the trace shows the override
        if let Some(trace) = traced.trace("log_level") {
            assert!(trace.was_overridden());
        }

        // Can extract the config
        let config = traced.into_inner();
        assert_eq!(config.log_level, "warn");
    }

    #[test]
    fn test_legacy_env_vars_claude_api_key() {
        // Test legacy single-underscore env var PRODIGY_CLAUDE_API_KEY
        let env = MockEnv::new().with_env("PRODIGY_CLAUDE_API_KEY", "test-api-key-123");

        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.claude_api_key, Some("test-api-key-123".to_string()));
        assert_eq!(config.effective_api_key(), Some("test-api-key-123"));
    }

    #[test]
    fn test_legacy_env_vars_log_level() {
        // Test legacy single-underscore env var PRODIGY_LOG_LEVEL
        let env = MockEnv::new().with_env("PRODIGY_LOG_LEVEL", "debug");

        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.log_level, "debug");
        assert_eq!(config.effective_log_level(), "debug");
    }

    #[test]
    fn test_legacy_env_vars_auto_commit() {
        // Test legacy single-underscore env var PRODIGY_AUTO_COMMIT
        let env = MockEnv::new().with_env("PRODIGY_AUTO_COMMIT", "false");

        let config = load_prodigy_config_with(&env).unwrap();

        assert!(!config.auto_commit);
        assert!(!config.effective_auto_commit());
    }

    #[test]
    fn test_legacy_env_vars_editor() {
        // Test legacy single-underscore env var PRODIGY_EDITOR
        let env = MockEnv::new().with_env("PRODIGY_EDITOR", "vim");

        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.default_editor, Some("vim".to_string()));
        assert_eq!(config.effective_editor(), Some("vim"));
    }

    #[test]
    fn test_legacy_env_vars_max_concurrent() {
        // Test legacy single-underscore env var PRODIGY_MAX_CONCURRENT
        let env = MockEnv::new().with_env("PRODIGY_MAX_CONCURRENT", "16");

        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.max_concurrent_specs, 16);
        assert_eq!(config.effective_max_concurrent(), 16);
    }

    #[test]
    fn test_legacy_env_vars_override_file() {
        // Legacy env vars should override file config
        let global_path = global_config_path();
        let env = MockEnv::new()
            .with_file(
                global_path.to_string_lossy().to_string(),
                r#"
log_level: info
auto_commit: true
"#,
            )
            .with_env("PRODIGY_LOG_LEVEL", "trace")
            .with_env("PRODIGY_AUTO_COMMIT", "false");

        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.log_level, "trace"); // env override
        assert!(!config.auto_commit); // env override
    }

    #[test]
    fn test_all_legacy_env_vars_together() {
        // Test all legacy env vars together
        let env = MockEnv::new()
            .with_env("PRODIGY_CLAUDE_API_KEY", "my-key")
            .with_env("PRODIGY_LOG_LEVEL", "warn")
            .with_env("PRODIGY_AUTO_COMMIT", "true")
            .with_env("PRODIGY_EDITOR", "nano")
            .with_env("PRODIGY_MAX_CONCURRENT", "8");

        let config = load_prodigy_config_with(&env).unwrap();

        assert_eq!(config.claude_api_key, Some("my-key".to_string()));
        assert_eq!(config.log_level, "warn");
        assert!(config.auto_commit);
        assert_eq!(config.default_editor, Some("nano".to_string()));
        assert_eq!(config.max_concurrent_specs, 8);
    }
}
