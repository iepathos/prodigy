---
number: 130
title: MapReduce Executor - Extract Pure Functions
category: foundation
priority: high
status: draft
dependencies: [129]
created: 2025-10-11
---

# Specification 130: MapReduce Executor - Extract Pure Functions

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: 129 (StepResult fix)

## Context

The MapReduce executor (`src/cook/execution/mapreduce/coordination/executor.rs`) is 2,316 lines long (11.5x over the 200-line guideline). Much of this code consists of pure functions - logic that doesn't depend on mutable state or perform I/O - embedded as methods or inline code.

###Current Problems

1. **Testing difficulty**: Pure logic mixed with I/O makes unit testing hard
2. **Code reuse**: Utility functions buried in executor can't be easily shared
3. **Readability**: File too large to comprehend as a whole
4. **Maintainability**: Changes risk breaking unrelated functionality

### What Qualifies as Pure

A function is pure if it:
- Has no side effects (no I/O, no mutations, no state changes)
- Is deterministic (same inputs always produce same outputs)
- Doesn't access mutable shared state
- Can be tested in isolation

## Objective

Extract all pure functions from the MapReduce executor into focused modules, reducing the main file by ~30% while improving testability, reusability, and maintainability.

## Requirements

### Functional Requirements

**Module 1: `mapreduce/pure/aggregation.rs`**
- Extract result aggregation logic
- Functions for filtering, grouping, and summarizing results
- Calculations for success/failure rates
- Pure transformations of agent results

**Module 2: `mapreduce/pure/planning.rs`**
- Extract execution planning logic
- Phase ordering decisions
- Parallelism calculations
- Work distribution strategies

**Module 3: `mapreduce/pure/formatting.rs`**
- Extract error message formatting
- Result display formatting
- Log message construction
- Output sanitization

**Module 4: `mapreduce/pure/interpolation.rs`**
- Extract variable interpolation logic
- Template processing
- Context building
- Variable flattening from JSON

### Non-Functional Requirements

- Each module under 200 lines
- Each function under 20 lines
- 100% test coverage for extracted functions
- Zero behavior changes
- All existing tests pass without modification
- No performance regression (< 2% overhead acceptable)

## Acceptance Criteria

- [ ] Created `src/cook/execution/mapreduce/pure/` directory
- [ ] Created `aggregation.rs` with at least 8 pure functions
- [ ] Created `planning.rs` with at least 6 pure functions
- [ ] Created `formatting.rs` with at least 10 pure functions
- [ ] Created `interpolation.rs` with at least 5 pure functions
- [ ] `executor.rs` reduced by at least 700 lines
- [ ] Each extracted function has comprehensive unit tests
- [ ] All existing integration tests pass unchanged
- [ ] Performance benchmarks show < 2% regression
- [ ] Each function has doc comments with examples

## Technical Details

### Module 1: aggregation.rs (~180 lines)

**Pure Functions to Extract**:

```rust
/// Calculate success rate from agent results
pub fn calculate_success_rate(results: &[AgentResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    let successful = results.iter().filter(|r| r.success).count();
    (successful as f64 / results.len() as f64) * 100.0
}

/// Filter successful results
pub fn filter_successful(results: &[AgentResult]) -> Vec<&AgentResult> {
    results.iter().filter(|r| r.success).collect()
}

/// Filter failed results
pub fn filter_failed(results: &[AgentResult]) -> Vec<&AgentResult> {
    results.iter().filter(|r| !r.success).collect()
}

/// Group results by error type
pub fn group_by_error(results: &[AgentResult]) -> HashMap<String, Vec<&AgentResult>> {
    let mut groups = HashMap::new();
    for result in results.iter().filter(|r| !r.success) {
        let error_type = categorize_error(&result.error);
        groups.entry(error_type).or_insert_with(Vec::new).push(result);
    }
    groups
}

/// Aggregate execution statistics
pub fn aggregate_stats(results: &[AgentResult]) -> AggregationSummary {
    AggregationSummary {
        total: results.len(),
        successful: results.iter().filter(|r| r.success).count(),
        failed: results.iter().filter(|r| !r.success).count(),
        total_duration: results.iter().map(|r| r.duration).sum(),
        avg_duration: calculate_avg_duration(results),
        success_rate: calculate_success_rate(results),
    }
}

/// Calculate average duration
fn calculate_avg_duration(results: &[AgentResult]) -> Duration {
    if results.is_empty() {
        return Duration::ZERO;
    }
    let total: Duration = results.iter().map(|r| r.duration).sum();
    total / results.len() as u32
}

/// Categorize error type
fn categorize_error(error: &Option<String>) -> String {
    match error {
        None => "unknown".to_string(),
        Some(e) if e.contains("timeout") => "timeout".to_string(),
        Some(e) if e.contains("command failed") => "command_failure".to_string(),
        Some(e) if e.contains("git") => "git_error".to_string(),
        Some(_) => "other".to_string(),
    }
}

/// Collect outputs from successful results
pub fn collect_outputs(results: &[AgentResult]) -> Vec<String> {
    results
        .iter()
        .filter(|r| r.success)
        .filter_map(|r| r.output.clone())
        .collect()
}
```

### Module 2: planning.rs (~150 lines)

**Pure Functions to Extract**:

```rust
/// Calculate optimal parallelism level
pub fn calculate_parallelism(total_items: usize, max_parallel: usize) -> usize {
    if total_items == 0 {
        return 0;
    }
    std::cmp::min(total_items, max_parallel)
}

/// Determine execution phases
pub fn plan_execution_phases(
    has_setup: bool,
    has_reduce: bool,
) -> Vec<Phase> {
    let mut phases = Vec::new();

    if has_setup {
        phases.push(Phase::Setup);
    }
    phases.push(Phase::Map);
    if has_reduce {
        phases.push(Phase::Reduce);
    }

    phases
}

/// Distribute work items across agents
pub fn distribute_work(
    items: Vec<Value>,
    parallelism: usize,
) -> Vec<Vec<Value>> {
    if items.is_empty() || parallelism == 0 {
        return vec![];
    }

    let chunk_size = (items.len() + parallelism - 1) / parallelism;
    items
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// Calculate batch size for processing
pub fn calculate_batch_size(
    total_items: usize,
    available_resources: usize,
    max_batch: usize,
) -> usize {
    let ideal_batch = (total_items + available_resources - 1) / available_resources;
    std::cmp::min(ideal_batch, max_batch)
}

/// Determine if work should be batched
pub fn should_batch(item_count: usize, threshold: usize) -> bool {
    item_count > threshold
}

/// Sort items by priority
pub fn sort_by_priority(
    mut items: Vec<Value>,
    priority_field: &str,
    descending: bool,
) -> Vec<Value> {
    items.sort_by(|a, b| {
        let a_priority = extract_priority(a, priority_field);
        let b_priority = extract_priority(b, priority_field);

        if descending {
            b_priority.cmp(&a_priority)
        } else {
            a_priority.cmp(&b_priority)
        }
    });
    items
}

fn extract_priority(item: &Value, field: &str) -> i64 {
    item.get(field)
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}
```

### Module 3: formatting.rs (~200 lines)

**Pure Functions to Extract**:

```rust
/// Format setup error message
pub fn format_setup_error(
    step_index: usize,
    result: &StepResult,
    is_claude_command: bool,
) -> String {
    let mut msg = format!("Setup command {} failed", step_index + 1);

    if let Some(exit_code) = result.exit_code {
        msg.push_str(&format!(" with exit code {}", exit_code));
    }

    if !result.stderr.is_empty() {
        msg.push_str(&format!("\nStderr: {}", truncate_output(&result.stderr, 500)));
    }

    if is_claude_command {
        if let Some(log_path) = &result.json_log_location {
            msg.push_str(&format!("\n📝 Claude log: {}", log_path));
        }
    }

    msg
}

/// Format commit requirement error
pub fn format_commit_requirement_error(
    step_name: &str,
    json_log_location: Option<&str>,
) -> String {
    let mut msg = format!(
        "Step '{}' has commit_required=true but no commits were created",
        step_name
    );

    if let Some(log_path) = json_log_location {
        msg.push_str(&format!("\n📝 Claude log: {}", log_path));
    }

    msg
}

/// Format agent execution summary
pub fn format_execution_summary(
    total: usize,
    successful: usize,
    failed: usize,
    duration: Duration,
) -> String {
    format!(
        "Executed {} agents ({} successful, {} failed) in {:?}",
        total, successful, failed, duration
    )
}

/// Format phase completion message
pub fn format_phase_completion(
    phase: &str,
    duration: Duration,
) -> String {
    format!("{} phase completed in {:?}", phase, duration)
}

/// Truncate output for display
pub fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        output.to_string()
    } else {
        format!("{}... ({} more chars)", &output[..max_chars], output.len() - max_chars)
    }
}

/// Sanitize output for logging (remove sensitive data)
pub fn sanitize_output(output: &str) -> String {
    // Remove potential secrets
    let patterns = [
        (r"password[=:]\s*\S+", "password=***"),
        (r"token[=:]\s*\S+", "token=***"),
        (r"api[_-]?key[=:]\s*\S+", "api_key=***"),
    ];

    let mut result = output.to_string();
    for (pattern, replacement) in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            result = re.replace_all(&result, replacement).to_string();
        }
    }
    result
}

/// Format duration in human-readable format
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Format file size
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

/// Format progress percentage
pub fn format_progress(current: usize, total: usize) -> String {
    if total == 0 {
        return "0%".to_string();
    }
    let percentage = (current as f64 / total as f64) * 100.0;
    format!("{:.1}%", percentage)
}

/// Build error context for debugging
pub fn build_error_context(
    agent_id: &str,
    item: &Value,
    error: &str,
) -> String {
    format!(
        "Agent {} failed processing item:\n  Item: {}\n  Error: {}",
        agent_id,
        serde_json::to_string(item).unwrap_or_else(|_| "unknown".to_string()),
        error
    )
}
```

### Module 4: interpolation.rs (~120 lines)

**Pure Functions to Extract**:

```rust
/// Build item variables from JSON value
pub fn build_item_variables(item: &Value, item_id: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Always include item ID
    vars.insert("item.id".to_string(), item_id.to_string());

    // Flatten JSON object to variables
    if let Some(obj) = item.as_object() {
        for (key, value) in obj {
            let var_name = format!("item.{}", key);
            if let Some(string_value) = value_to_string(value) {
                vars.insert(var_name, string_value);
            }
        }
    }

    vars
}

/// Convert JSON value to string
fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".to_string()),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).ok()
        }
    }
}

/// Flatten nested JSON object to dot-notation variables
pub fn flatten_json_to_vars(obj: &Map<String, Value>) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    for (key, value) in obj {
        flatten_value(&mut vars, key, value);
    }

    vars
}

fn flatten_value(vars: &mut HashMap<String, String>, prefix: &str, value: &Value) {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj {
                let new_prefix = format!("{}.{}", prefix, key);
                flatten_value(vars, &new_prefix, val);
            }
        }
        _ => {
            if let Some(string_value) = value_to_string(value) {
                vars.insert(prefix.to_string(), string_value);
            }
        }
    }
}

/// Extract variable names from template
pub fn extract_variable_names(template: &str) -> Vec<String> {
    let mut vars = Vec::new();

    // Match ${var} patterns
    let braced_regex = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    for cap in braced_regex.captures_iter(template) {
        vars.push(cap[1].to_string());
    }

    // Match $var patterns
    let unbraced_regex = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in unbraced_regex.captures_iter(template) {
        vars.push(cap[1].to_string());
    }

    vars
}

/// Validate interpolation context has required variables
pub fn validate_context(
    template: &str,
    context: &HashMap<String, String>,
) -> Result<(), Vec<String>> {
    let required = extract_variable_names(template);
    let missing: Vec<String> = required
        .into_iter()
        .filter(|var| !context.contains_key(var))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}
```

### Migration Strategy

1. **Create directory structure**:
```bash
mkdir -p src/cook/execution/mapreduce/pure
touch src/cook/execution/mapreduce/pure/mod.rs
touch src/cook/execution/mapreduce/pure/aggregation.rs
touch src/cook/execution/mapreduce/pure/planning.rs
touch src/cook/execution/mapreduce/pure/formatting.rs
touch src/cook/execution/mapreduce/pure/interpolation.rs
```

2. **Extract functions incrementally**:
   - Copy function to new module
   - Add unit tests
   - Update executor.rs to import and use
   - Delete original implementation
   - Run tests

3. **Keep thin wrappers for compatibility**:
```rust
// In executor.rs
impl MapReduceCoordinator {
    fn calculate_success_rate(&self, results: &[AgentResult]) -> f64 {
        pure::aggregation::calculate_success_rate(results)
    }
}
```

## Implementation Steps

1. Create `pure/` module structure with mod.rs exports
2. Extract and test aggregation functions
3. Extract and test planning functions
4. Extract and test formatting functions
5. Extract and test interpolation functions
6. Update executor.rs to use pure modules
7. Remove original implementations from executor.rs
8. Run full test suite
9. Benchmark performance

## Testing Strategy

### Unit Tests (New)

Each pure function must have:
- **Happy path test**: Normal inputs produce expected outputs
- **Edge case tests**: Empty inputs, boundary values, special cases
- **Property-based tests** (where applicable): Verify invariants hold for random inputs

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_success_rate_all_successful() {
        let results = vec![
            AgentResult { success: true, ..Default::default() },
            AgentResult { success: true, ..Default::default() },
        ];
        assert_eq!(calculate_success_rate(&results), 100.0);
    }

    #[test]
    fn test_calculate_success_rate_all_failed() {
        let results = vec![
            AgentResult { success: false, ..Default::default() },
            AgentResult { success: false, ..Default::default() },
        ];
        assert_eq!(calculate_success_rate(&results), 0.0);
    }

    #[test]
    fn test_calculate_success_rate_empty() {
        assert_eq!(calculate_success_rate(&[]), 0.0);
    }

    #[test]
    fn test_calculate_success_rate_mixed() {
        let results = vec![
            AgentResult { success: true, ..Default::default() },
            AgentResult { success: false, ..Default::default() },
            AgentResult { success: true, ..Default::default() },
        ];
        assert!((calculate_success_rate(&results) - 66.67).abs() < 0.1);
    }
}
```

### Integration Tests (Existing)

All existing MapReduce tests must pass unchanged. These verify that:
- End-to-end workflows still work
- Phase coordination is correct
- Results are properly aggregated

### Performance Tests

Benchmark key functions to ensure no regression:
```rust
#[bench]
fn bench_calculate_success_rate(b: &mut Bencher) {
    let results = vec![AgentResult::default(); 1000];
    b.iter(|| calculate_success_rate(&results));
}
```

## Dependencies

**Prerequisites**:
- Spec 129 (StepResult fix) must be completed first

**Affected Components**:
- MapReduce coordinator
- Setup phase executor
- Map phase executor
- Reduce phase executor

**External Dependencies**:
- `regex` crate (already in use)

## Implementation Notes

### Why These Functions are Pure

Each extracted function:
- Takes inputs as parameters (no hidden dependencies)
- Returns outputs as values (no side effects)
- Doesn't mutate inputs (immutable)
- Doesn't perform I/O
- Is deterministic

### Benefits of Extraction

1. **Testability**: Each function can be tested in isolation with simple inputs
2. **Reusability**: Pure functions can be used in other contexts without modification
3. **Reasoning**: Pure functions are easier to understand and reason about
4. **Composition**: Pure functions can be easily composed to build more complex logic
5. **Parallelization**: Pure functions are inherently thread-safe

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing functionality | Extensive testing, incremental extraction |
| Performance regression | Benchmark before/after, inline critical paths |
| Import cycles | Pure functions have no dependencies on executor |
| Over-extraction | Only extract truly pure functions, keep I/O separate |
| Test maintenance burden | Use property-based tests to reduce test cases |

## Success Metrics

- `executor.rs` reduced from 2,316 to ~1,600 lines (30% reduction)
- 4 new pure modules created, each under 200 lines
- 100% test coverage for pure functions (aim for 200+ test cases)
- Zero test failures
- Performance within 2% of baseline
- Reduced cyclomatic complexity in executor.rs

## Documentation Requirements

### Code Documentation

- Each pure function must have:
  - Doc comment describing purpose
  - Examples in doc comments
  - Parameter descriptions
  - Return value description
  - Panic conditions (if any)

Example:
```rust
/// Calculate success rate from agent results.
///
/// # Arguments
///
/// * `results` - Slice of agent results to analyze
///
/// # Returns
///
/// Success rate as a percentage (0.0-100.0), or 0.0 if results is empty
///
/// # Examples
///
/// ```
/// let results = vec![
///     AgentResult { success: true, ..Default::default() },
///     AgentResult { success: false, ..Default::default() },
/// ];
/// assert_eq!(calculate_success_rate(&results), 50.0);
/// ```
pub fn calculate_success_rate(results: &[AgentResult]) -> f64 {
    // ...
}
```

### User Documentation

No user-facing documentation changes needed - this is an internal refactoring.

### Architecture Updates

Update ARCHITECTURE.md to reflect:
- New `pure/` module organization
- Functional programming principles in MapReduce
- Clear separation of pure logic and I/O

## Migration and Compatibility

### Breaking Changes

None - All changes are internal implementation details.

### Rollback Plan

Since changes are incremental and each function extraction is committed separately:
1. Identify problematic extraction via git bisect
2. Revert specific commit
3. Keep other successful extractions

### Deployment

Can be deployed incrementally as each module is completed. No coordination required between deployments.
