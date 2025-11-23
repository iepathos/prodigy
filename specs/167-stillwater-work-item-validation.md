---
number: 167
title: Stillwater Work Item Validation with Error Accumulation
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-11-23
---

# Specification 167: Stillwater Work Item Validation with Error Accumulation

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, MapReduce work item validation fails on the first error encountered, requiring users to fix errors iteratively. With 100 work items, this can require 10+ validation cycles to discover all errors. This creates a frustrating user experience and wastes significant time.

**Current Behavior**:
- User submits 100 work items
- Validation fails on item #1: "Invalid path"
- User fixes item #1, resubmits
- Validation fails on item #7: "Missing required field"
- User fixes item #7, resubmits
- **Repeat 10+ times to find all errors**

**Root Cause**: Sequential validation using `?` operator stops at first error (`src/cook/execution/data_pipeline/mod.rs:428-512`).

## Objective

Implement comprehensive error accumulation for work item validation using Stillwater's `Validation<T, E>` type, enabling users to see ALL validation errors in a single pass.

## Requirements

### Functional Requirements

1. **Error Accumulation**
   - Validate all work items and collect ALL errors before failing
   - Display comprehensive error report with item indices
   - Preserve individual error details (field, reason, value)

2. **Pure Validation Functions**
   - Extract validation logic to pure functions (no I/O)
   - Enable testing without file system or JSON parsing
   - Clear separation: parsing (I/O) vs validation (pure)

3. **Backward Compatibility**
   - Maintain existing error types and messages
   - Preserve CLI output format expectations
   - No breaking changes to workflow YAML syntax

### Non-Functional Requirements

1. **Performance**: Validation performance must not degrade (still O(n) complexity)
2. **Testability**: 100% of validation logic testable without I/O
3. **User Experience**: Single validation pass reveals all errors
4. **Code Quality**: Pure functions <20 lines, well-documented

## Acceptance Criteria

- [ ] Work item validation accumulates all errors across all items
- [ ] Error messages include item index (e.g., "Item 7: Missing field 'data'")
- [ ] Validation errors display in clear, actionable format
- [ ] Pure validation functions in separate `validation.rs` module
- [ ] All validation logic testable without file I/O
- [ ] Existing error types extended to support error accumulation
- [ ] CLI displays all accumulated errors when validation fails
- [ ] Performance benchmarks show <5% overhead vs sequential validation
- [ ] 20+ unit tests for pure validation functions
- [ ] Integration tests verify error accumulation end-to-end
- [ ] Documentation updated with validation architecture
- [ ] No breaking changes to existing workflows

## Technical Details

### Implementation Approach

**Phase 1: Add Stillwater Dependency**
```toml
# Cargo.toml
[dependencies]
stillwater = "0.1"  # Or appropriate version
```

**Phase 2: Create Pure Validation Module**
```rust
// src/cook/execution/data_pipeline/validation.rs

use stillwater::Validation;
use crate::cook::execution::data_pipeline::{WorkItem, ValidationError};

/// Validate work item ID (pure function)
pub fn validate_item_id(id: &str) -> Validation<String, Vec<ValidationError>> {
    if id.is_empty() {
        Validation::failure(vec![ValidationError::EmptyId])
    } else if id.len() > 255 {
        Validation::failure(vec![ValidationError::IdTooLong(id.len())])
    } else {
        Validation::success(id.to_string())
    }
}

/// Validate work item path (pure function)
pub fn validate_item_path(path: &str) -> Validation<PathBuf, Vec<ValidationError>> {
    let path_buf = PathBuf::from(path);
    if !path_buf.is_absolute() {
        Validation::failure(vec![ValidationError::PathNotAbsolute(path.to_string())])
    } else {
        Validation::success(path_buf)
    }
}

/// Validate single work item (pure composition)
pub fn validate_work_item(item: &WorkItem) -> Validation<ValidWorkItem, Vec<ValidationError>> {
    Validation::all((
        validate_item_id(&item.id),
        validate_item_path(&item.path),
        validate_item_data(&item.data),
        validate_item_filter(&item.filter),
    ))
    .map(|(id, path, data, filter)| ValidWorkItem {
        id,
        path,
        data,
        filter,
    })
}

/// Validate all work items (error accumulation)
pub fn validate_all_items(items: &[WorkItem]) -> Validation<Vec<ValidWorkItem>, Vec<ValidationError>> {
    Validation::all(
        items.iter().enumerate().map(|(idx, item)| {
            validate_work_item(item)
                .map_err(|errors| {
                    // Add item index to each error
                    errors.into_iter()
                        .map(|e| e.with_item_index(idx))
                        .collect()
                })
        })
    )
}
```

**Phase 3: Update Error Types**
```rust
// src/cook/execution/data_pipeline/error.rs

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    EmptyId,
    IdTooLong(usize),
    PathNotAbsolute(String),
    MissingRequiredField(String),
    InvalidFilterExpression(String),
    // ... existing variants
}

impl ValidationError {
    /// Add item index context to error
    pub fn with_item_index(self, index: usize) -> Self {
        // Wrap error with context
        ValidationError::ItemValidationFailed {
            index,
            error: Box::new(self),
        }
    }
}

/// New error variant for multiple validation failures
#[derive(Debug)]
pub struct MultipleValidationErrors {
    pub errors: Vec<ValidationError>,
    pub total_items: usize,
    pub failed_items: usize,
}

impl Display for MultipleValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Validation failed for {} of {} items:",
                 self.failed_items, self.total_items)?;
        for error in &self.errors {
            writeln!(f, "  - {}", error)?;
        }
        Ok(())
    }
}
```

**Phase 4: Update Pipeline Integration**
```rust
// src/cook/execution/data_pipeline/mod.rs

use validation::validate_all_items;

pub fn load_and_validate_items(path: &Path) -> Result<Vec<ValidWorkItem>> {
    // I/O: Parse JSON file
    let items: Vec<WorkItem> = parse_work_items_file(path)?;

    // Pure: Validate all items
    validate_all_items(&items)
        .into_result()  // Convert Validation -> Result
        .map_err(|errors| {
            WorkItemError::MultipleValidationErrors(MultipleValidationErrors {
                errors,
                total_items: items.len(),
                failed_items: count_failed_items(&errors),
            })
        })
}
```

### Architecture Changes

**New Module Structure**:
```
src/cook/execution/data_pipeline/
├── mod.rs              (existing - integration)
├── validation.rs       (NEW - pure validation functions)
├── error.rs            (updated - accumulation support)
└── parser.rs           (existing - I/O operations)
```

**Separation of Concerns**:
- **Parser** (`parser.rs`): File I/O, JSON parsing (impure)
- **Validation** (`validation.rs`): Business rules, error checking (pure)
- **Integration** (`mod.rs`): Orchestration, error handling (impure)

### Data Structures

```rust
/// Validated work item (newtype for type safety)
#[derive(Debug, Clone)]
pub struct ValidWorkItem {
    pub id: String,
    pub path: PathBuf,
    pub data: serde_json::Value,
    pub filter: Option<String>,
}

impl ValidWorkItem {
    /// Convert to runtime work item
    pub fn into_work_item(self) -> WorkItem {
        WorkItem {
            id: self.id,
            path: self.path.to_string_lossy().to_string(),
            data: self.data,
            filter: self.filter,
        }
    }
}
```

### APIs and Interfaces

**Public API** (no breaking changes):
```rust
// Existing function signature maintained
pub fn load_work_items(path: &Path) -> Result<Vec<WorkItem>> {
    load_and_validate_items(path)
        .map(|validated| validated.into_iter().map(|v| v.into_work_item()).collect())
}
```

**New Internal API** (for testing and composition):
```rust
// Pure validation functions (exported for testing)
pub use validation::{
    validate_work_item,
    validate_all_items,
    validate_item_id,
    validate_item_path,
};
```

## Dependencies

### Prerequisites
- Stillwater library added to dependencies
- Understanding of `Validation<T, E>` type and error accumulation pattern

### Affected Components
- `src/cook/execution/data_pipeline/mod.rs` - Integration point
- `src/cook/execution/mapreduce/coordination/executor.rs` - Work item loading
- Error display logic in CLI output

### External Dependencies
- `stillwater = "0.1"` (or latest stable version)

## Testing Strategy

### Unit Tests (Pure Functions)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_item_id_success() {
        let result = validate_item_id("valid-id-123");
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_item_id_empty() {
        let result = validate_item_id("");
        assert!(matches!(result, Validation::Failure(_)));
    }

    #[test]
    fn test_validate_all_items_accumulates_errors() {
        let items = vec![
            WorkItem { id: "", path: "not-absolute", data: json!({}), filter: None },
            WorkItem { id: "valid", path: "/absolute", data: json!({}), filter: None },
            WorkItem { id: "x".repeat(300), path: "/valid", data: json!({}), filter: None },
        ];

        let result = validate_all_items(&items);

        match result {
            Validation::Failure(errors) => {
                // Should have errors from items 0 and 2
                assert_eq!(errors.len(), 3);  // EmptyId, PathNotAbsolute, IdTooLong
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_error_includes_item_index() {
        let items = vec![
            WorkItem { id: "valid1", path: "/path1", data: json!({}), filter: None },
            WorkItem { id: "", path: "/path2", data: json!({}), filter: None },
        ];

        let result = validate_all_items(&items);

        match result {
            Validation::Failure(errors) => {
                assert!(errors.iter().any(|e| {
                    matches!(e, ValidationError::ItemValidationFailed { index: 1, .. })
                }));
            }
            _ => panic!("Expected validation failure"),
        }
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_work_item_validation_end_to_end() {
    let temp_file = create_temp_work_items_file(&[
        json!({"id": "", "path": "relative/path"}),
        json!({"id": "valid", "path": "/absolute/path"}),
        json!({"id": "x".repeat(300), "path": "/another"}),
    ]);

    let result = load_work_items(&temp_file).await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Verify all errors are reported
    assert!(error.to_string().contains("Item 0"));
    assert!(error.to_string().contains("Item 2"));
    assert!(error.to_string().contains("EmptyId"));
    assert!(error.to_string().contains("IdTooLong"));
}
```

### Performance Tests

```rust
#[test]
fn benchmark_validation_performance() {
    let items: Vec<WorkItem> = (0..10_000).map(|i| create_valid_item(i)).collect();

    let start = Instant::now();
    let _ = validate_all_items(&items);
    let duration = start.elapsed();

    // Validation should complete in <100ms for 10k items
    assert!(duration < Duration::from_millis(100));
}
```

## Documentation Requirements

### Code Documentation
- Rustdoc for all public validation functions
- Examples in doc comments showing accumulation behavior
- Module-level documentation explaining pure vs I/O separation

### User Documentation
- Update CLAUDE.md with validation architecture
- Add section to ARCHITECTURE.md on error accumulation pattern
- Document expected error message format

### Architecture Updates

Add to `ARCHITECTURE.md`:
```markdown
## Work Item Validation Architecture

### Pure Validation Functions

All work item validation logic resides in `data_pipeline/validation.rs` as pure functions:

- **Pure**: No I/O, no side effects, deterministic
- **Testable**: Can test without file system or JSON parsing
- **Composable**: Small functions combine via `Validation::all()`

### Error Accumulation

Uses Stillwater's `Validation<T, E>` type to accumulate ALL errors:

- **Fail-Completely**: Reports all errors, not just first
- **User Experience**: Single validation pass reveals all issues
- **Performance**: O(n) complexity, minimal overhead

### Separation of Concerns

- **Parser** (`parser.rs`): File I/O, JSON deserialization
- **Validation** (`validation.rs`): Business rules (pure)
- **Integration** (`mod.rs`): Orchestration, error handling
```

## Implementation Notes

### Migration Strategy

1. **Backward Compatibility**: Keep existing `load_work_items()` signature
2. **Gradual Adoption**: Internal use of validation module first
3. **Error Message Format**: Preserve existing format, add item indices
4. **Testing**: Comprehensive tests before integration

### Edge Cases

- **Empty work item list**: Should succeed with empty result
- **Single item failure**: Should still show item index
- **All items fail**: Should report all errors, not overflow
- **Duplicate item IDs**: Should detect and report

### Performance Considerations

- **No N^2 complexity**: Each item validated once
- **Error collection**: Use `Vec<E>` with pre-allocated capacity
- **String allocation**: Minimize cloning in error messages

## Migration and Compatibility

### Breaking Changes
None - maintains existing public API.

### Compatibility Guarantees
- Existing workflows continue to work unchanged
- Error messages enhanced with item indices (additive)
- CLI output format preserved

### Migration Path
1. Add Stillwater dependency
2. Create validation module
3. Update error types (additive only)
4. Integrate validation in pipeline
5. Add comprehensive tests
6. Update documentation

### Rollback Strategy
If issues arise:
- Pure validation functions are independent (easy to disable)
- Fallback to sequential validation by changing integration point
- No data structure changes required
