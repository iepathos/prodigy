---
number: 122
title: Dry-Run Validation for Setup-Generated Input Files
category: testing
priority: medium
status: draft
dependencies: []
created: 2025-10-06
---

# Specification 122: Dry-Run Validation for Setup-Generated Input Files

**Category**: testing
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

MapReduce workflows in Prodigy often use a setup phase to generate input data files that are then consumed by the map phase. For example, the `debtmap-reduce.yml` workflow runs `debtmap analyze` in the setup phase to create `.prodigy/debtmap-before.json`, which is then used as input for parallel debt item processing.

Currently, dry-run validation (`prodigy run --dry-run`) fails when the input file doesn't exist yet, because the validator checks for file existence before the setup phase would have created it. This prevents users from validating workflow configurations without actually running them.

**Example Error**:
```
❌ Session failed: Dry-run validation failed: Input source error: Invalid input source
```

This occurs because `InputValidator::validate_input_source()` returns `valid: false` for non-existent files, and `load_work_items()` returns an error when the file doesn't exist.

## Objective

Enable dry-run validation to succeed for MapReduce workflows where the input file will be created by the setup phase, while still providing useful validation of command structure and workflow configuration.

## Requirements

### Functional Requirements

1. **Graceful Handling of Missing Files**: Dry-run validation should not fail when input files don't exist yet
2. **Setup Phase Detection**: The validator should recognize when an input file will be created by the setup phase
3. **Clear Warnings**: Users should be informed when validation is limited due to missing input files
4. **Mock Validation Results**: Provide reasonable mock validation data for non-existent files
5. **Preserve Existing Behavior**: File validation should still work normally when files exist

### Non-Functional Requirements

1. **User Experience**: Clear messaging about validation limitations
2. **Consistency**: Match existing pattern for `shell:` input validation
3. **Maintainability**: Simple implementation that's easy to understand
4. **Backward Compatibility**: No breaking changes to existing workflows

## Acceptance Criteria

- [ ] `prodigy run workflows/debtmap-reduce.yml --dry-run` completes successfully
- [ ] Warning message is logged when input file doesn't exist: "Input file does not exist yet: {path} (may be created by setup phase)"
- [ ] `InputValidation` returns `valid: true` for non-existent files in dry-run mode
- [ ] `load_work_items()` returns empty vector with warning instead of error for missing files
- [ ] Existing file validation still works when files exist
- [ ] Data structure description clearly indicates "file not yet created (dry-run mode)"
- [ ] All existing tests pass
- [ ] No clippy warnings
- [ ] Documentation updated with dry-run limitations

## Technical Details

### Implementation Approach

Modify `InputValidator` in `src/cook/execution/mapreduce/dry_run/input_validator.rs` to handle non-existent files gracefully by returning mock validation results instead of errors.

### Architecture Changes

**File**: `src/cook/execution/mapreduce/dry_run/input_validator.rs`

**Function 1**: `validate_input_source` (lines 22-38)

**Current Behavior**:
```rust
pub async fn validate_input_source(&self, input: &str) -> Result<InputValidation, DryRunError> {
    if input.starts_with("shell:") {
        self.validate_command_input(input).await
    } else if Path::new(input).exists() {
        self.validate_file_input(input).await
    } else {
        Ok(InputValidation {
            source: input.to_string(),
            valid: false,  // ❌ Causes dry-run to fail
            size_bytes: 0,
            item_count_estimate: 0,
            data_structure: "unknown".to_string(),
        })
    }
}
```

**Proposed Change**:
```rust
pub async fn validate_input_source(&self, input: &str) -> Result<InputValidation, DryRunError> {
    if input.starts_with("shell:") {
        self.validate_command_input(input).await
    } else if Path::new(input).exists() {
        self.validate_file_input(input).await
    } else {
        // In dry-run, file may not exist yet (created by setup phase)
        warn!(
            "Input file does not exist yet: {} (may be created by setup phase)",
            input
        );
        Ok(InputValidation {
            source: input.to_string(),
            valid: true,  // ✅ Assume valid - will be created by setup
            size_bytes: 0,
            item_count_estimate: 0,
            data_structure: "file not yet created (dry-run mode)".to_string(),
        })
    }
}
```

**Function 2**: `load_work_items` (lines 99-132)

**Current Behavior**:
```rust
pub async fn load_work_items(
    &self,
    input: &str,
    json_path: Option<&str>,
) -> Result<Vec<Value>, DryRunError> {
    if input.starts_with("shell:") {
        warn!("Command input in dry-run mode, returning empty work items");
        return Ok(Vec::new());
    }

    // Load from file
    if !Path::new(input).exists() {
        return Err(DryRunError::InputError(format!(  // ❌ Error breaks validation
            "Input file does not exist: {}",
            input
        )));
    }

    // ... rest of function
}
```

**Proposed Change**:
```rust
pub async fn load_work_items(
    &self,
    input: &str,
    json_path: Option<&str>,
) -> Result<Vec<Value>, DryRunError> {
    if input.starts_with("shell:") {
        warn!("Command input in dry-run mode, returning empty work items");
        return Ok(Vec::new());
    }

    // Load from file
    if !Path::new(input).exists() {
        warn!(  // ✅ Warning instead of error
            "Input file does not exist yet: {} (may be created by setup phase)",
            input
        );
        return Ok(Vec::new());  // ✅ Return empty items to allow validation to continue
    }

    // ... rest of function
}
```

### Data Structures

No new data structures needed. Existing `InputValidation` struct already supports the required fields.

### APIs and Interfaces

No API changes. Internal behavior modification only.

## Dependencies

**Prerequisites**: None

**Affected Components**:
- `src/cook/execution/mapreduce/dry_run/input_validator.rs` - Main changes
- `src/cook/execution/mapreduce/dry_run/validator.rs` - Consumes InputValidation results

**External Dependencies**: None

## Testing Strategy

### Unit Tests

**Test 1**: Non-existent input file validation
```rust
#[tokio::test]
async fn test_validate_nonexistent_file_in_dry_run() {
    let validator = InputValidator::new();
    let result = validator
        .validate_input_source(".prodigy/debtmap-before.json")
        .await
        .unwrap();

    assert!(result.valid);
    assert_eq!(result.data_structure, "file not yet created (dry-run mode)");
    assert_eq!(result.size_bytes, 0);
}
```

**Test 2**: Load work items from non-existent file
```rust
#[tokio::test]
async fn test_load_work_items_nonexistent_file() {
    let validator = InputValidator::new();
    let items = validator
        .load_work_items(".prodigy/missing.json", Some("$.items[*]"))
        .await
        .unwrap();

    assert!(items.is_empty());
}
```

**Test 3**: Existing file validation still works
```rust
#[tokio::test]
async fn test_validate_existing_file() {
    // Create temporary file with test data
    let temp_file = create_test_json_file();

    let validator = InputValidator::new();
    let result = validator
        .validate_input_source(temp_file.path())
        .await
        .unwrap();

    assert!(result.valid);
    assert!(result.size_bytes > 0);
    assert_ne!(result.data_structure, "file not yet created (dry-run mode)");
}
```

### Integration Tests

**Test**: Full dry-run validation with setup-generated input
```bash
# Should succeed even though debtmap-before.json doesn't exist yet
prodigy run workflows/debtmap-reduce.yml --dry-run

# Expected output:
# ⚠️  Input file does not exist yet: .prodigy/debtmap-before.json (may be created by setup phase)
# ✓ Dry-run validation passed
```

### User Acceptance

1. Run `prodigy run workflows/debtmap-reduce.yml --dry-run` on a clean repository
2. Verify it completes successfully with appropriate warnings
3. Verify actual workflow run still works correctly
4. Verify error messages are clear and actionable

## Documentation Requirements

### Code Documentation

- Add rustdoc comments explaining dry-run behavior for non-existent files
- Document the assumption that setup phase will create the file
- Add examples in function documentation

### User Documentation

Update `CLAUDE.md` section on dry-run validation:

```markdown
## Dry-Run Validation Limitations

When running `prodigy run --dry-run`, the following limitations apply:

1. **Setup-Generated Files**: If the setup phase creates input files for the map phase,
   dry-run validation cannot analyze the actual data structure. The validator will
   assume the file will be created and return mock validation results.

2. **Command Inputs**: Input sources using `shell:` prefix cannot be validated
   against actual data in dry-run mode.

3. **JSONPath Validation**: JSONPath expressions can only be validated if the
   input file already exists.

**Example Warning**:
```
⚠️  Input file does not exist yet: .prodigy/debtmap-before.json (may be created by setup phase)
```

This is expected behavior and allows workflow structure validation without executing
setup commands.
```

### Architecture Updates

No `ARCHITECTURE.md` updates needed - this is an internal implementation detail.

## Implementation Notes

### Design Rationale

**Option Chosen**: Mock validation results for non-existent files
- **Pros**: Simple, matches existing pattern for shell inputs, preserves dry-run semantics
- **Cons**: Cannot validate JSONPath or data structure until runtime

**Alternative Considered**: Execute setup phase during dry-run
- **Pros**: Most accurate validation
- **Cons**: Not truly a dry-run (has side effects), slower, may fail on dependencies

### Best Practices

1. **Clear Warnings**: Use `warn!` level logging to inform users about limitations
2. **Consistent Behavior**: Match the existing pattern for `shell:` inputs
3. **Graceful Degradation**: Provide as much validation as possible with available data
4. **User Guidance**: Help users understand what can/cannot be validated

### Error Handling

- No new error types needed
- Convert error returns to warning + empty/mock results
- Preserve error handling for actual validation failures (malformed JSON, invalid JSONPath, etc.)

## Migration and Compatibility

### Breaking Changes

None. This is purely an enhancement to dry-run validation.

### Compatibility Considerations

- Existing workflows that already have input files will continue to work exactly as before
- New workflows can be validated before setup phase creates files
- No changes to runtime behavior - only affects dry-run mode

### Migration Requirements

None. Automatic improvement for all workflows using dry-run validation.
