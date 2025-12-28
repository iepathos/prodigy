# Error Accumulation: Work Item Validation

## Current Problem
**Location**: `src/cook/execution/data_pipeline/mod.rs:428-512`

**Symptom**: Users submit 100 work items, get error about item #1. Fix item #1, resubmit, get error about item #7. Repeat 10+ times.

```rust
// Current: Sequential validation (fails fast)
pub fn validate_and_load_items(path: &Path) -> Result<Vec<WorkItem>> {
    let items: Vec<WorkItem> = serde_json::from_reader(File::open(path)?)?;

    for item in &items {
        validate_item_structure(item)?;  // Stops here
        validate_item_path(item)?;
        validate_item_filter(item)?;
    }

    Ok(items)
}
```

**Problem**: Each validation error requires a full workflow restart. With 100 items, could take 100 iterations to find all errors.

## Stillwater Solution: Validation<T, E>

```rust
use stillwater::Validation;

// NEW: Accumulate ALL errors
pub fn validate_item(item: &WorkItem) -> Validation<ValidWorkItem, Vec<ValidationError>> {
    Validation::all((
        validate_item_structure(item),
        validate_item_path(item),
        validate_item_filter(item),
        validate_item_dependencies(item),
    ))
    .map(|(structure, path, filter, deps)| {
        ValidWorkItem { structure, path, filter, deps }
    })
}

pub fn validate_all_items(items: &[WorkItem]) -> Validation<Vec<ValidWorkItem>, Vec<ValidationError>> {
    Validation::all(
        items.iter().enumerate().map(|(i, item)| {
            validate_item(item)
                .map_err(|errors| {
                    errors.into_iter()
                        .map(|e| e.with_item_index(i))
                        .collect()
                })
        })
    )
}

// Usage
pub fn validate_and_load_items(path: &Path) -> Result<Vec<ValidWorkItem>> {
    let items: Vec<WorkItem> = serde_json::from_reader(File::open(path)?)?;

    validate_all_items(&items)
        .into_result()  // Convert Validation -> Result for ? operator
        .map_err(|errors| {
            // User sees ALL errors at once:
            // - Item 1: Invalid path (must be absolute)
            // - Item 7: Missing required field 'data'
            // - Item 23: Filter expression syntax error
            // - Item 45: Circular dependency detected
            WorkItemError::MultipleValidationErrors(errors)
        })
}
```

## Benefit

Single workflow run reveals ALL validation errors. User fixes all issues at once.

## Impact

- Estimated time savings: 90% reduction in validation iteration cycles
- User experience: From frustrating to delightful
- Code clarity: Validation intent explicit (accumulate vs fail-fast)
