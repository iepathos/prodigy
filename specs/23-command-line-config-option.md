# Specification 23: Command Line Configuration Option

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [21-configurable-workflow]

## Context

Currently, MMM uses a hardcoded approach to configuration file loading. Spec 21 introduced the concept of a `.mmm/workflow.toml` configuration file that is automatically loaded if present. However, based on user feedback and real-world usage patterns, we've identified that users need more flexibility in specifying configuration file locations. This is particularly important for:

1. Shared configuration across multiple projects
2. CI/CD environments where config may be in non-standard locations
3. Testing different configurations without modifying project files
4. Organizations that maintain centralized configuration repositories

The current implementation only checks for `.mmm/workflow.toml` in the project root, limiting flexibility and requiring users to duplicate configuration files across projects.

## Objective

Add a command-line option `--config` (or `-c`) that allows users to specify a custom path to a configuration file. The system should follow this precedence order:

1. If `--config` is specified, use that file (error if not found)
2. Otherwise, if `.mmm/config.toml` exists, use it
3. Otherwise, fall back to sensible legacy defaults (current behavior)

This maintains backward compatibility while providing the flexibility users need.

## Requirements

### Functional Requirements
- Add `--config` (short form: `-c`) command-line option to `mmm improve`
- Accept both absolute and relative paths for the config file
- Support both `.toml` and `.yml`/`.yaml` file formats
- Load configuration from the specified file path
- If `--config` is provided but file doesn't exist, fail with clear error message
- If no `--config` is provided, check for `.mmm/config.toml` (not workflow.toml)
- If neither exists, use the built-in default workflow
- Validate configuration structure and provide helpful error messages

### Non-Functional Requirements
- Maintain backward compatibility with existing behavior
- Clear error messages for missing files or invalid configurations
- Minimal performance impact on startup
- Support for future configuration extensions

## Acceptance Criteria

- [ ] Command `mmm improve --config /path/to/config.yml` loads configuration from specified file
- [ ] Command `mmm improve -c ./custom-config.toml` loads configuration from relative path
- [ ] Error message displayed when `--config` points to non-existent file
- [ ] `.mmm/config.toml` is automatically loaded when present (no --config flag)
- [ ] Legacy default workflow used when no config file is found
- [ ] Both TOML and YAML formats are supported
- [ ] Invalid configuration files produce clear, actionable error messages
- [ ] Help text (`mmm improve --help`) documents the new option
- [ ] Configuration file path is shown in verbose output
- [ ] Existing projects without config files continue to work unchanged

## Technical Details

### Implementation Approach

1. Update `ImproveCommand` to add the `config` field
2. Modify configuration loading logic to check multiple sources
3. Add YAML support alongside existing TOML support
4. Implement clear precedence rules for configuration loading
5. Update error handling for missing or invalid config files

### Architecture Changes

- Rename existing workflow configuration to use `config.toml` instead of `workflow.toml`
- Extend configuration loader to support multiple file formats
- Add command-line argument to specify config path

### Data Structures

```rust
// In src/improve/command.rs
pub struct ImproveCommand {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,

    /// Show detailed progress
    #[arg(long)]
    pub verbose: bool,

    /// Focus directive for improvements
    #[arg(long)]
    pub focus: Option<String>,
    
    /// Path to configuration file
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,
}

// In src/config/loader.rs
pub fn load_config(explicit_path: Option<&Path>) -> Result<Config> {
    match explicit_path {
        Some(path) => {
            // Load from explicit path, error if not found
            load_from_path(path)
        }
        None => {
            // Check for .mmm/config.toml
            let default_path = Path::new(".mmm/config.toml");
            if default_path.exists() {
                load_from_path(default_path)
            } else {
                // Return default configuration
                Ok(Config::default())
            }
        }
    }
}
```

### APIs and Interfaces

- CLI interface extended with `--config` option
- Configuration loader API updated to accept optional explicit path
- Support for both TOML and YAML parsers

## Dependencies

- **Prerequisites**: 
  - Spec 21 (configurable workflow) provides the base configuration structure
- **Affected Components**:
  - `src/improve/command.rs` - Add config path argument
  - `src/config/loader.rs` - Update loading logic with precedence rules
  - `src/improve/mod.rs` - Pass config path to loader
  - `src/config/mod.rs` - Rename workflow.toml references to config.toml
- **External Dependencies**:
  - Add `serde_yaml` for YAML support (in addition to existing `toml`)

## Testing Strategy

- **Unit Tests**:
  - Test configuration loading with explicit path
  - Test fallback to .mmm/config.toml
  - Test fallback to default configuration
  - Test error handling for missing files
  - Test both TOML and YAML parsing
- **Integration Tests**:
  - Run improve command with various config scenarios
  - Verify correct workflow execution based on config
- **Performance Tests**:
  - Ensure config loading doesn't significantly impact startup time
- **User Acceptance**:
  - Test in real projects with different config locations
  - Verify CI/CD compatibility

## Documentation Requirements

- **Code Documentation**:
  - Document the configuration precedence rules
  - Add examples for both TOML and YAML formats
- **User Documentation**:
  - Update README.md with --config usage examples
  - Document configuration file format and options
  - Add migration guide from workflow.toml to config.toml
- **Architecture Updates**:
  - Update ARCHITECTURE.md to reflect new config loading approach

## Implementation Notes

- Consider using `PathBuf` for the config argument to handle cross-platform paths correctly
- The error message for missing config file should suggest checking the path and creating a default config
- When showing verbose output, display which configuration source was used (explicit, default, or built-in)
- Future consideration: Support for config file includes/imports for modular configurations
- YAML support allows for more readable configuration for complex workflows

## Migration and Compatibility

- **Breaking Changes**: 
  - Projects using `.mmm/workflow.toml` need to rename to `.mmm/config.toml`
  - This is a minor breaking change but improves consistency
- **Migration Path**:
  1. Check for existing `.mmm/workflow.toml` and warn users to rename
  2. Temporarily support both filenames with deprecation warning
  3. Remove workflow.toml support in next major version
- **Compatibility**:
  - Projects without any config file continue to work with default behavior
  - Command-line flag is optional, maintaining backward compatibility