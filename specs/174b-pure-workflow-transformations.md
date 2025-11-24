---
number: 174b
title: Pure Workflow Transformations
category: foundation
priority: high
status: draft
dependencies: [172, 173]
parent: 174
created: 2025-11-24
---

# Specification 174b: Pure Workflow Transformations

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 172 (Stillwater Foundation), Spec 173 (Parallel Execution Effects)
**Parent**: Spec 174 (Pure Core Extraction)

## Context

This is the second phase of Spec 174 (Pure Core Extraction). The workflow executor (`src/cook/workflow/executor/commands.rs`, 2,243 LOC) currently mixes command building and output parsing with execution. This spec extracts the pure transformation logic.

**Scope**: Create pure transformation modules only. No executor refactoring yet (that's in 174f).

## Objective

Extract workflow transformation logic into pure, testable functions:
- Command building from templates
- Variable expansion (${VAR}, $VAR patterns)
- Variable reference extraction
- Output parsing (regex, JSON path, line-based)

## Requirements

### Functional Requirements

#### FR1: Command Building
- **MUST** implement `build_command(template: &str, variables: &HashMap<String, String>) -> String`
- **MUST** expand variables in templates
- **MUST** preserve unexpanded variables if not found
- **MUST** handle edge cases (empty strings, special characters)

#### FR2: Variable Expansion
- **MUST** implement `expand_variables(template: &str, variables: &HashMap<String, String>) -> String`
- **MUST** support `${VAR}` syntax (braced variables)
- **MUST** support `$VAR` syntax (simple variables)
- **MUST** use word boundaries for simple variables to avoid partial matches
- **MUST** preserve original text if variable not found

#### FR3: Variable Reference Extraction
- **MUST** implement `extract_variable_references(template: &str) -> HashSet<String>`
- **MUST** find all `${VAR}` references
- **MUST** find all `$VAR` references
- **MUST** return unique set of variable names
- **MUST** handle nested braces correctly

#### FR4: Output Parsing
- **MUST** implement `parse_output_variables(output: &str, patterns: &[OutputPattern]) -> HashMap<String, String>`
- **MUST** support regex pattern extraction
- **MUST** support JSON path extraction
- **MUST** support line number extraction
- **MUST** handle parsing errors gracefully

#### FR5: Pattern Types
- **MUST** define `OutputPattern` enum with:
  - `Regex { name: String, regex: Regex }`
  - `Json { name: String, json_path: String }`
  - `Line { name: String, line_number: usize }`

### Non-Functional Requirements

#### NFR1: Purity
- **MUST** have zero I/O operations
- **MUST** be deterministic
- **MUST** have no side effects
- **MUST** pass clippy with no warnings

#### NFR2: Testability
- **MUST** achieve 100% test coverage
- **MUST** require zero mocking in tests
- **MUST** have fast tests (< 1ms per test)
- **MUST** include property tests for idempotence and determinism

#### NFR3: Performance
- **MUST** handle templates up to 10KB efficiently
- **MUST** handle output up to 1MB efficiently
- **MUST** avoid unnecessary allocations

## Acceptance Criteria

- [ ] Module created at `src/cook/workflow/pure/`
- [ ] `command_builder.rs` with build and expansion functions
- [ ] `variable_expansion.rs` with expansion logic
- [ ] `output_parser.rs` with parsing functions
- [ ] `OutputPattern` enum defined
- [ ] Unit tests achieve 100% coverage
- [ ] No mocking used in any test
- [ ] Property tests verify idempotence and determinism
- [ ] All tests pass in < 100ms total
- [ ] `cargo fmt` and `cargo clippy` pass with no warnings
- [ ] Module properly exposed in `src/cook/workflow/mod.rs`

## Technical Details

### Module Structure

```
src/cook/workflow/pure/
├── mod.rs                  # Module exports
├── command_builder.rs      # Command building functions
├── variable_expansion.rs   # Variable expansion logic
└── output_parser.rs        # Output parsing functions
```

### Command Building

```rust
// src/cook/workflow/pure/command_builder.rs

use std::collections::HashMap;
use super::variable_expansion::expand_variables;

/// Pure: Build command string from template
pub fn build_command(
    template: &str,
    variables: &HashMap<String, String>,
) -> String {
    expand_variables(template, variables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command() {
        let template = "echo ${name} ${value}";
        let vars = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ].iter().cloned().collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_build_command_with_missing_vars() {
        let template = "echo ${exists} ${missing}";
        let vars = [("exists".into(), "value".into())].iter().cloned().collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "echo value ${missing}");
    }
}
```

### Variable Expansion

```rust
// src/cook/workflow/pure/variable_expansion.rs

use std::collections::{HashMap, HashSet};
use regex::Regex;

/// Pure: Expand variables in template
pub fn expand_variables(
    template: &str,
    variables: &HashMap<String, String>,
) -> String {
    let mut result = template.to_string();

    // Expand ${VAR} first (more specific)
    for (key, value) in variables {
        let placeholder = format!("${{{}}}", key);
        result = result.replace(&placeholder, value);
    }

    // Expand $VAR with word boundaries
    for (key, value) in variables {
        if !key.is_empty() {
            let pattern = format!(r"\${}(?![a-zA-Z0-9_])", regex::escape(key));
            let re = Regex::new(&pattern).expect("Valid regex pattern");
            result = re.replace_all(&result, value.as_str()).into_owned();
        }
    }

    result
}

/// Pure: Extract variable references from template
pub fn extract_variable_references(template: &str) -> HashSet<String> {
    let mut refs = HashSet::new();

    // Match ${VAR} pattern
    let braced_regex = Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}")
        .expect("Valid regex pattern");
    for cap in braced_regex.captures_iter(template) {
        refs.insert(cap[1].to_string());
    }

    // Match $VAR pattern
    let simple_regex = Regex::new(r"\$([a-zA-Z_][a-zA-Z0-9_]*)(?![a-zA-Z0-9_])")
        .expect("Valid regex pattern");
    for cap in simple_regex.captures_iter(template) {
        refs.insert(cap[1].to_string());
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_variables_braced() {
        let template = "echo ${name} ${value}";
        let vars = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_expand_variables_simple() {
        let template = "echo $name $value";
        let vars = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_expand_variables_mixed() {
        let template = "echo ${name} $value";
        let vars = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_expand_variables_preserves_missing() {
        let template = "echo ${exists} ${missing} $also_missing";
        let vars = [("exists".into(), "value".into())].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo value ${missing} $also_missing");
    }

    #[test]
    fn test_expand_variables_avoids_partial_match() {
        let template = "$name $name_with_suffix";
        let vars = [("name".into(), "test".into())].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        // Should only replace $name, not $name_with_suffix
        assert_eq!(result, "test $name_with_suffix");
    }

    #[test]
    fn test_extract_variable_references_braced() {
        let template = "echo ${name} ${value} ${name}";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 2);
        assert!(refs.contains("name"));
        assert!(refs.contains("value"));
    }

    #[test]
    fn test_extract_variable_references_simple() {
        let template = "echo $name $value $name";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 2);
        assert!(refs.contains("name"));
        assert!(refs.contains("value"));
    }

    #[test]
    fn test_extract_variable_references_mixed() {
        let template = "echo ${name} $value";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 2);
        assert!(refs.contains("name"));
        assert!(refs.contains("value"));
    }
}
```

### Output Parsing

```rust
// src/cook/workflow/pure/output_parser.rs

use std::collections::HashMap;
use regex::Regex;
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum OutputPattern {
    Regex { name: String, regex: Regex },
    Json { name: String, json_path: String },
    Line { name: String, line_number: usize },
}

/// Pure: Extract variables from command output
pub fn parse_output_variables(
    output: &str,
    patterns: &[OutputPattern],
) -> HashMap<String, String> {
    patterns
        .iter()
        .filter_map(|pattern| extract_match(output, pattern))
        .collect()
}

/// Pure: Extract single variable match
fn extract_match(output: &str, pattern: &OutputPattern) -> Option<(String, String)> {
    match pattern {
        OutputPattern::Regex { name, regex } => {
            regex.captures(output).and_then(|cap| {
                cap.get(1).map(|m| (name.clone(), m.as_str().to_string()))
            })
        }
        OutputPattern::Json { name, json_path } => {
            extract_json_path(output, json_path)
                .map(|value| (name.clone(), value))
        }
        OutputPattern::Line { name, line_number } => {
            output.lines().nth(*line_number)
                .map(|line| (name.clone(), line.to_string()))
        }
    }
}

/// Pure: Extract value from JSON path
fn extract_json_path(json_str: &str, path: &str) -> Option<String> {
    let value: Value = serde_json::from_str(json_str).ok()?;

    // Simple JSON path implementation ($.field or $.field.nested)
    if !path.starts_with('$') {
        return None;
    }

    let path = &path[1..]; // Remove $
    if path.is_empty() || path == "." {
        return Some(value.to_string());
    }

    let path = path.strip_prefix('.').unwrap_or(path);
    let mut current = &value;

    for segment in path.split('.') {
        current = current.get(segment)?;
    }

    Some(match current {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_output_regex() {
        let output = "Result: success\nValue: 42";
        let patterns = vec![
            OutputPattern::Regex {
                name: "result".into(),
                regex: Regex::new(r"Result: (\w+)").unwrap(),
            },
            OutputPattern::Regex {
                name: "value".into(),
                regex: Regex::new(r"Value: (\d+)").unwrap(),
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert_eq!(vars.get("result").unwrap(), "success");
        assert_eq!(vars.get("value").unwrap(), "42");
    }

    #[test]
    fn test_parse_output_json() {
        let output = r#"{"status": "ok", "count": 10}"#;
        let patterns = vec![
            OutputPattern::Json {
                name: "status".into(),
                json_path: "$.status".into(),
            },
            OutputPattern::Json {
                name: "count".into(),
                json_path: "$.count".into(),
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert_eq!(vars.get("status").unwrap(), "ok");
        assert_eq!(vars.get("count").unwrap(), "10");
    }

    #[test]
    fn test_parse_output_line() {
        let output = "Line 0\nLine 1\nLine 2";
        let patterns = vec![
            OutputPattern::Line {
                name: "first".into(),
                line_number: 0,
            },
            OutputPattern::Line {
                name: "second".into(),
                line_number: 1,
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert_eq!(vars.get("first").unwrap(), "Line 0");
        assert_eq!(vars.get("second").unwrap(), "Line 1");
    }

    #[test]
    fn test_parse_output_no_match() {
        let output = "No matches here";
        let patterns = vec![
            OutputPattern::Regex {
                name: "missing".into(),
                regex: Regex::new(r"Result: (\w+)").unwrap(),
            },
        ];

        let vars = parse_output_variables(output, &patterns);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_json_path_nested() {
        let json = r#"{"user": {"name": "Alice", "age": 30}}"#;

        let result = extract_json_path(json, "$.user.name");

        assert_eq!(result.unwrap(), "Alice");
    }

    #[test]
    fn test_extract_json_path_invalid() {
        let json = r#"{"user": {"name": "Alice"}}"#;

        let result = extract_json_path(json, "$.user.missing");

        assert!(result.is_none());
    }
}
```

### Property Tests

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_variable_expansion_is_deterministic(
            template in ".*",
            vars in prop::collection::hash_map(".*", ".*", 0..10),
        ) {
            let result1 = expand_variables(&template, &vars);
            let result2 = expand_variables(&template, &vars);

            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_variable_expansion_idempotent(
            template in ".*",
            vars in prop::collection::hash_map(".*", ".*", 0..10),
        ) {
            let result1 = expand_variables(&template, &vars);
            let result2 = expand_variables(&result1, &vars);

            // Should be idempotent after first expansion
            // (assumes no variable values contain variable references)
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_extract_references_is_deterministic(template in ".*") {
            let refs1 = extract_variable_references(&template);
            let refs2 = extract_variable_references(&template);

            prop_assert_eq!(refs1, refs2);
        }
    }
}
```

## Testing Strategy

### Unit Tests (No Mocking!)
- Test all expansion patterns
- Test edge cases (empty strings, special characters)
- Test missing variables
- Test partial matches
- Test all output pattern types
- Test parsing errors

### Property Tests
- Determinism: Same input → same output
- Idempotence: Expanding twice = expanding once
- Consistency: References match actual expansions

### Performance Tests
- Large templates (10KB)
- Large outputs (1MB)
- Many variables (100+)

## Implementation Notes

### Critical Success Factors
1. **Zero I/O** - All pure transformations
2. **100% coverage** - Every branch tested
3. **No mocking** - Direct function calls
4. **Fast tests** - < 1ms per test

### Integration with Existing Code
- Module should compile independently
- Will be consumed by workflow executor in spec 174f
- Will be used by effect modules in spec 174d

### Migration Path
1. Create module structure
2. Implement variable expansion
3. Implement command building
4. Implement output parsing
5. Write comprehensive unit tests
6. Add property tests
7. Verify no I/O operations
8. Commit and close spec

## Dependencies

### Prerequisites
- Spec 172 (Stillwater Foundation) - for Effect types used later
- Spec 173 (Parallel Execution Effects) - for composition patterns

### Blocks
- Spec 174d (Effect Modules) - needs these transformations
- Spec 174f (Refactor Workflow Executor) - needs these transformations

### Parallel Work
- Can be developed in parallel with 174a (Pure Execution Planning)
- Can be developed in parallel with 174c (Pure Session Updates)

## Documentation Requirements

- Module-level documentation explaining pure transformations
- Function documentation with examples
- Test documentation showing common patterns
- Update `src/cook/workflow/mod.rs` to expose new module

## Success Metrics

- [ ] All 11 acceptance criteria met
- [ ] 100% test coverage achieved
- [ ] All tests pass in < 100ms
- [ ] Zero clippy warnings
- [ ] Module successfully imports in executor (compile check)
