---
number: 49
title: Refactor Analysis from Command Attributes to Standalone Commands
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-08
---

# Specification 49: Refactor Analysis from Command Attributes to Standalone Commands

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

With the recent implementation of the modular command handler architecture in `src/commands/`, we currently have analysis as a sub-attribute that gets attached to commands when they require analysis. This approach is counter to our new modular command approach where each command type (shell, claude, git, etc.) is self-contained.

The current system uses `analysis:` attributes in workflow configurations like:

```yaml
- claude: "/mmm-code-review"
  analysis:
    max_cache_age: 300
    force_refresh: false
```

This creates coupling between analysis functionality and other command types, making the system less modular and harder to maintain. Analysis should be treated as a first-class command that can be composed with other commands in workflows.

## Objective

Refactor the analysis system to be a standalone command in workflows while maintaining the existing `mmm analyze` CLI functionality. This will improve modularity, make workflows more explicit about when analysis occurs, and simplify command handler implementations.

## Requirements

### Functional Requirements

- **Standalone Analysis Command**: Create an `analyze:` command type that can be used independently in workflows
- **Analysis Command Handler**: Implement `AnalyzeCommandHandler` following the `CommandHandler` trait
- **Workflow Integration**: Update workflow parsing to support `analyze:` commands
- **Legacy Support**: Maintain existing `mmm analyze` CLI command functionality
- **Cache Control**: Support cache control parameters (max_cache_age, force_refresh) as command attributes
- **Output Control**: Support different output formats and save options
- **Coverage Integration**: Support running coverage analysis as part of the command

### Non-Functional Requirements

- **Backward Compatibility**: Existing `mmm analyze` command behavior must remain unchanged
- **Performance**: Analysis command should leverage existing caching mechanisms
- **Modularity**: Analysis command handler should be self-contained with no dependencies on other command types
- **Configurability**: All analysis options should be configurable through command attributes

## Acceptance Criteria

- [ ] `AnalyzeCommandHandler` struct implemented with `CommandHandler` trait
- [ ] Analysis command supports all attributes: `force_refresh`, `max_cache_age`, `save`, `output`, `run_coverage`
- [ ] Workflow parser recognizes `analyze:` command syntax
- [ ] Example workflows updated to use `analyze:` commands instead of `analysis:` attributes
- [ ] All existing `analysis:` attribute functionality migrated to command attributes
- [ ] Command registry includes the new analyze handler
- [ ] Integration tests pass for workflows using `analyze:` commands
- [ ] Existing `mmm analyze` CLI command continues to work unchanged
- [ ] Documentation updated to reflect new workflow syntax

## Technical Details

### Implementation Approach

1. **Create AnalyzeCommandHandler**: Implement a new command handler in `src/commands/handlers/analyze.rs`
2. **Define Attribute Schema**: Create attribute schema supporting all analysis configuration options
3. **Update Workflow Parser**: Modify workflow parsing to recognize `analyze:` commands
4. **Update Examples**: Migrate example workflows to use new syntax
5. **Remove Analysis Attributes**: Remove analysis attribute handling from other command handlers

### Architecture Changes

```rust
// New command handler structure
pub struct AnalyzeCommandHandler {
    analyzer: Arc<dyn AnalysisCoordinator>,
}

impl CommandHandler for AnalyzeCommandHandler {
    fn name(&self) -> &str { "analyze" }
    
    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("analyze");
        schema.add_optional("force_refresh", "Force fresh analysis ignoring cache");
        schema.add_optional_with_default("max_cache_age", "Maximum cache age in seconds", AttributeValue::Number(3600.0));
        schema.add_optional_with_default("save", "Save results to .mmm directory", AttributeValue::Boolean(true));
        schema.add_optional_with_default("output", "Output format (json, pretty, summary)", AttributeValue::String("summary".to_string()));
        schema.add_optional_with_default("run_coverage", "Run coverage analysis", AttributeValue::Boolean(false));
        schema
    }
    
    async fn execute(&self, context: &ExecutionContext, attributes: HashMap<String, AttributeValue>) -> CommandResult;
}
```

### Data Structures

```yaml
# New workflow syntax
- analyze:
    force_refresh: true
    max_cache_age: 300
    save: true
    output: "json"
    run_coverage: false

# Multiple analysis steps
- analyze:
    output: "summary"
- claude: "/mmm-implement-spec $ARG"
- analyze:
    force_refresh: true
    output: "json"
```

### APIs and Interfaces

The analyze command will use existing analysis infrastructure:
- `AnalysisCoordinator` trait for running analysis
- `AnalysisCache` for caching results
- `MetricsCollector` for gathering metrics
- Context generation systems

## Dependencies

- **Prerequisites**: Modular command handler architecture (already implemented)
- **Affected Components**: 
  - Workflow parser and executor
  - Command registry
  - Example workflow files
- **External Dependencies**: None (uses existing analysis infrastructure)

## Testing Strategy

### Unit Tests
- Test `AnalyzeCommandHandler` attribute validation
- Test command execution with different attribute combinations
- Test integration with existing analysis infrastructure

### Integration Tests
- Test workflows with `analyze:` commands
- Test cache behavior with different `max_cache_age` values
- Test `force_refresh` functionality
- Test different output formats

### Performance Tests
- Verify analysis command performance matches existing `mmm analyze`
- Test cache hit/miss scenarios
- Validate memory usage with large projects

### User Acceptance
- Verify existing `mmm analyze` command unchanged
- Verify workflows with new syntax work as expected
- Validate error handling and user feedback

## Documentation Requirements

### Code Documentation
- Document `AnalyzeCommandHandler` struct and methods
- Add inline comments explaining attribute handling
- Document integration with analysis infrastructure

### User Documentation
- Update workflow syntax documentation
- Provide migration guide from old `analysis:` attributes
- Add examples of different analyze command configurations
- Update CLI reference for consistency

### Architecture Updates
- Update ARCHITECTURE.md with new command structure
- Document separation of concerns between commands
- Explain analysis command lifecycle

## Implementation Notes

### Command Attribute Mapping

| Old Analysis Attribute | New Analyze Command Attribute |
|----------------------|------------------------------|
| `max_cache_age` | `max_cache_age` |
| `force_refresh` | `force_refresh` |
| N/A | `save` (always true in workflows) |
| N/A | `output` (defaults to "summary") |
| N/A | `run_coverage` (defaults to false) |

### Workflow Migration Strategy

1. **Identify Usage**: Scan all example workflows for `analysis:` attributes
2. **Convert Syntax**: Replace with equivalent `analyze:` commands
3. **Position Commands**: Place `analyze:` commands at logical points in workflows
4. **Test Conversion**: Verify converted workflows produce same results

### Error Handling

- Validate all attributes according to schema
- Provide clear error messages for invalid configurations
- Handle analysis failures gracefully
- Support dry-run mode for testing configurations

## Migration and Compatibility

### Breaking Changes

- Workflow files using `analysis:` attributes will need updates
- Command handlers that expect analysis context may need adjustments

### Migration Path

1. **Phase 1**: Implement new analyze command handler
2. **Phase 2**: Update example workflows to new syntax
3. **Phase 3**: Add deprecation warnings for old `analysis:` attributes
4. **Phase 4**: Remove support for old attributes in future version

### Compatibility Considerations

- Existing `mmm analyze` CLI command remains unchanged
- Analysis cache format and location unchanged
- Analysis results structure unchanged
- Backward compatibility maintained for core functionality

## Example Usage

### Before (Current System)
```yaml
- claude: "/mmm-code-review"
  analysis:
    max_cache_age: 300
    force_refresh: false

- claude: "/mmm-cleanup-tech-debt" 
  analysis:
    force_refresh: true
```

### After (New System)
```yaml
- analyze:
    max_cache_age: 300
    force_refresh: false
- claude: "/mmm-code-review"

- analyze:
    force_refresh: true
- claude: "/mmm-cleanup-tech-debt"
```

### Advanced Configuration
```yaml
# Comprehensive analysis with coverage
- analyze:
    force_refresh: true
    run_coverage: true
    output: "json"
    save: true

# Quick analysis check
- analyze:
    max_cache_age: 600
    output: "summary"

# Analysis for metrics only
- analyze:
    force_refresh: false
    output: "json"
```

This refactoring will make workflows more explicit about when analysis occurs, improve command modularity, and maintain the flexibility of the current system while providing a cleaner architectural foundation.