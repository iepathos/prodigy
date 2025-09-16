---
number: 63
title: Simplified YAML Syntax Implementation
category: compatibility
priority: medium
status: draft
dependencies: []
created: 2025-01-15
---

# Specification 63: Simplified YAML Syntax Implementation

**Category**: compatibility
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The current YAML parser sometimes requires nested `commands` arrays under `agent_template` and `reduce` sections, while the whitepaper shows a cleaner syntax with commands directly under these keys. This inconsistency creates confusion and makes workflows more verbose than necessary. Additionally, the parser should be more flexible in accepting both formats to ease migration and improve user experience.

## Objective

Implement a flexible YAML parser that accepts both the simplified syntax from the whitepaper and the current nested format, with automatic normalization to ensure backward compatibility while encouraging the cleaner syntax.

## Requirements

### Functional Requirements

1. **Simplified Agent Template Syntax**
   ```yaml
   # New simplified syntax (preferred)
   agent_template:
     - claude: "/process '${item}'"
     - shell: "validate ${item}"

   # Current nested syntax (still supported)
   agent_template:
     commands:
       - claude: "/process '${item}'"
       - shell: "validate ${item}"
   ```

2. **Simplified Reduce Syntax**
   ```yaml
   # New simplified syntax (preferred)
   reduce:
     - claude: "/summarize ${map.results}"
     - shell: "generate-report"

   # Current nested syntax (still supported)
   reduce:
     commands:
       - claude: "/summarize ${map.results}"
   ```

3. **Simplified Setup Syntax**
   ```yaml
   # Direct command list (new)
   setup:
     - shell: "prepare-data"
     - claude: "/analyze-requirements"

   # Full configuration (when needed)
   setup:
     timeout: 600
     commands:
       - shell: "prepare-data"
   ```

4. **Smart Format Detection**
   - Automatically detect format based on content
   - Support mixed formats in same file (with warning)
   - Provide clear migration messages
   - Validate against both schemas

5. **Migration Assistance**
   - Tool to convert old format to new
   - Deprecation warnings for nested format
   - Automatic format upgrade option
   - Format validation command

### Non-Functional Requirements

1. **Compatibility**
   - 100% backward compatibility
   - No breaking changes to existing workflows
   - Graceful handling of edge cases

2. **Performance**
   - No significant parsing overhead
   - Efficient format detection
   - Fast validation

3. **Developer Experience**
   - Clear error messages
   - Helpful migration hints
   - Good IDE support

## Acceptance Criteria

- [ ] Simplified syntax works for all MapReduce sections
- [ ] Old nested syntax continues to work without changes
- [ ] Parser automatically detects and normalizes both formats
- [ ] Mixed formats generate helpful warnings
- [ ] Migration tool successfully converts workflows
- [ ] Validation catches syntax errors in both formats
- [ ] Documentation shows simplified syntax as primary
- [ ] Examples use simplified syntax exclusively
- [ ] Performance impact is negligible (<1ms per parse)
- [ ] Error messages guide users to simplified syntax

## Technical Details

### Implementation Approach

```rust
impl MapPhaseYaml {
    /// Parse agent_template supporting both formats
    pub fn parse_agent_template(value: &Value) -> Result<Vec<WorkflowStep>> {
        match value {
            Value::Sequence(steps) => {
                // New simplified format: direct array of steps
                Self::parse_workflow_steps(steps)
            }
            Value::Mapping(map) => {
                // Old format: check for 'commands' key
                if let Some(commands) = map.get("commands") {
                    warn!("Using deprecated nested 'commands' syntax. Consider using direct array format.");
                    Self::parse_workflow_steps(commands.as_sequence()?)
                } else {
                    // Might be a single command object
                    Ok(vec![Self::parse_single_step(value)?])
                }
            }
            _ => Err(anyhow!("Invalid agent_template format"))
        }
    }
}

/// Unified parser supporting both formats
pub struct FlexibleYamlParser {
    strict_mode: bool,
    auto_upgrade: bool,
    warn_deprecated: bool,
}

impl FlexibleYamlParser {
    pub fn parse_workflow(&self, yaml: &str) -> Result<WorkflowConfig> {
        let value = serde_yaml::from_str(yaml)?;

        // Detect format
        let format = self.detect_format(&value)?;

        // Parse with appropriate strategy
        let config = match format {
            YamlFormat::Simplified => self.parse_simplified(value)?,
            YamlFormat::Nested => {
                if self.warn_deprecated {
                    warn!("Workflow uses deprecated nested syntax");
                }
                self.parse_nested(value)?
            }
            YamlFormat::Mixed => {
                warn!("Workflow mixes simplified and nested syntax");
                self.parse_mixed(value)?
            }
        };

        // Optionally upgrade format
        if self.auto_upgrade && format != YamlFormat::Simplified {
            return Ok(self.upgrade_format(config)?);
        }

        Ok(config)
    }
}
```

### Architecture Changes

1. **Parser Enhancement**
   - New flexible parser module
   - Format detection logic
   - Normalization layer

2. **Migration Tools**
   - Format converter utility
   - Validation command
   - Batch migration script

3. **Documentation Updates**
   - Update all examples
   - Migration guide
   - Format reference

### Data Structures

```rust
pub enum YamlFormat {
    Simplified,  // New clean format
    Nested,      // Old nested format
    Mixed,       // Contains both (not recommended)
}

pub struct FormatMigration {
    pub source_format: YamlFormat,
    pub target_format: YamlFormat,
    pub changes: Vec<FormatChange>,
    pub warnings: Vec<String>,
}

pub struct FormatChange {
    pub path: String,
    pub old_syntax: String,
    pub new_syntax: String,
    pub line_number: usize,
}

/// Trait for format-agnostic parsing
pub trait FlexibleDeserialize {
    fn from_flexible_value(value: Value) -> Result<Self> where Self: Sized;
    fn supports_simplified() -> bool { true }
    fn migration_hint() -> Option<String> { None }
}
```

### APIs and Interfaces

```rust
pub trait YamlNormalizer {
    fn normalize(&self, value: Value) -> Result<Value>;
    fn detect_format(&self, value: &Value) -> YamlFormat;
    fn upgrade_format(&self, value: Value) -> Result<Value>;
}

pub trait FormatValidator {
    fn validate_simplified(&self, yaml: &str) -> Result<()>;
    fn validate_nested(&self, yaml: &str) -> Result<()>;
    fn suggest_improvements(&self, yaml: &str) -> Vec<Suggestion>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/config/mapreduce.rs`
  - `src/config/mod.rs`
  - `src/cook/workflow/mod.rs`
  - All workflow parsing code
- **External Dependencies**:
  - `serde_yaml` for parsing
  - `serde` derive macros

## Testing Strategy

- **Unit Tests**:
  - Parse both formats correctly
  - Format detection accuracy
  - Migration logic
  - Error handling

- **Compatibility Tests**:
  - All existing workflows still parse
  - Mixed format handling
  - Edge cases and malformed YAML

- **Migration Tests**:
  - Automated conversion accuracy
  - Round-trip conversion
  - Large workflow migration

- **User Acceptance**:
  - Real workflow migration
  - Documentation clarity
  - Tool usability

## Documentation Requirements

- **Code Documentation**:
  - Parser API documentation
  - Format specifications
  - Migration tool usage

- **User Documentation**:
  - Update all workflow examples
  - Migration guide from old to new
  - Format reference card

- **Architecture Updates**:
  - Parser architecture diagram
  - Format detection flow
  - Normalization process

## Implementation Notes

1. **Phased Rollout**:
   - Phase 1: Support both formats silently
   - Phase 2: Add deprecation warnings
   - Phase 3: Simplified as default in docs
   - Phase 4: Consider removing nested support (v2.0)

2. **IDE Support**: Provide schema files for both formats
3. **Validation**: Strict mode for CI/CD pipelines
4. **Performance**: Cache parsed configurations
5. **Debugging**: Add `--debug-yaml` flag to show normalization

## Migration and Compatibility

- **Zero Breaking Changes**: All existing workflows continue to work
- **Automatic Detection**: Parser determines format automatically
- **Migration Tool**: `prodigy migrate-yaml <file>` to upgrade format
- **Batch Migration**: `prodigy migrate-yaml --all` for entire project
- **Validation Mode**: `prodigy validate --format simplified`
- **Deprecation Timeline**:
  - v1.x: Both formats supported, warnings for nested
  - v2.0: Simplified format required (with migration tool)