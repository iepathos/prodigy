# Iteration 1738353432: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from codebase analysis.

## Debt Items to Address

### 1. Excessive use of unwrap() and expect()
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Error Handling
**File**: Multiple files (241 occurrences across 26 files)
**Priority**: High

#### Current State:
```rust
// Examples from various files
let data = some_operation().unwrap();
let result = parse_value().expect("Failed to parse");
```

#### Required Changes:
```rust
// Use proper error propagation
let data = some_operation()?;
let result = parse_value().context("Failed to parse value")?;
```

#### Implementation Steps:
- Replace unwrap() calls with ? operator where possible
- Use anyhow::Context for expect() replacements to provide better error context
- Add proper error handling in test code where unwrap() is acceptable
- Mark test-only unwraps with comments explaining why they're safe

### 2. Large File Refactoring: cook/mod.rs
**Complexity Score**: High (2711 lines)
**Change Frequency**: High
**Risk Level**: High
**File**: src/cook/mod.rs

#### Refactoring Plan:
- Split into smaller, focused modules (command execution, state management, iteration control)
- Extract common patterns into helper functions
- Move test-specific code to separate test modules
- Create dedicated types for complex function parameters

### 3. Missing Documentation Attributes
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Documentation
**File**: Multiple files (73 occurrences)
**Priority**: Medium

#### Current State:
```rust
pub fn new() -> Self {
    // Missing #[must_use] attribute
}
```

#### Required Changes:
```rust
#[must_use]
pub fn new() -> Self {
    // Constructor that should be used
}
```

#### Implementation Steps:
- Add #[must_use] attributes to all constructors and builder methods
- Add #[must_use] to methods returning Self or important values
- Document why the return value must be used

### 4. Missing Error Documentation
**Impact Score**: 6/10
**Effort Score**: 4/10
**Category**: Documentation
**File**: Multiple files (57 occurrences)
**Priority**: Medium

#### Current State:
```rust
/// Does something important
pub fn process_data() -> Result<Data> {
    // Missing # Errors section
}
```

#### Required Changes:
```rust
/// Does something important
/// 
/// # Errors
/// 
/// Returns an error if:
/// - The input data is invalid
/// - The processing fails
pub fn process_data() -> Result<Data> {
    // Implementation
}
```

#### Implementation Steps:
- Add `# Errors` sections to all public functions returning Result
- Document specific error conditions
- Use consistent error documentation format

### 5. Redundant Closures
**Impact Score**: 5/10
**Effort Score**: 2/10
**Category**: Code Quality
**File**: Multiple files (16 occurrences)
**Priority**: Low

#### Current State:
```rust
values.map(|x| process(x))
```

#### Required Changes:
```rust
values.map(process)
```

#### Implementation Steps:
- Replace redundant closures with direct function references
- Use clippy --fix to automate most cases
- Manually review edge cases

### 6. Case-Sensitive File Extension Comparisons
**Impact Score**: 6/10
**Effort Score**: 3/10
**Category**: Correctness
**File**: Multiple files (11 occurrences)
**Priority**: Medium

#### Current State:
```rust
if path.ends_with(".rs") {
    // Case-sensitive comparison
}
```

#### Required Changes:
```rust
if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("rs")) {
    // Case-insensitive comparison
}
```

#### Implementation Steps:
- Use Path::extension() with case-insensitive comparison
- Create a helper function for consistent file extension checking
- Update all file extension comparisons

### 7. Unused self Arguments
**Impact Score**: 4/10
**Effort Score**: 3/10
**Category**: Code Quality
**File**: Multiple files (36 occurrences)
**Priority**: Low

#### Current State:
```rust
fn process(&self, data: Data) -> Result<()> {
    // self is never used
}
```

#### Required Changes:
```rust
fn process(_data: Data) -> Result<()> {
    // Make static or use self
}
```

#### Implementation Steps:
- Review methods with unused self to determine if they should be static
- Prefix unused parameters with underscore
- Consider if the method belongs on the struct

### 8. Type Casting Issues
**Impact Score**: 7/10
**Effort Score**: 5/10
**Category**: Type Safety
**File**: Multiple files (casting warnings)
**Priority**: High

#### Current State:
```rust
let count = values.len() as u32;  // May truncate
let ratio = count as f32;         // Loss of precision
```

#### Required Changes:
```rust
let count = u32::try_from(values.len()).context("Value count exceeds u32 max")?;
let ratio = f32::from(count);     // Safe conversion
```

#### Implementation Steps:
- Use TryFrom for potentially truncating conversions
- Use From trait for infallible conversions
- Add proper error handling for conversion failures
- Document why certain casts are safe with comments

## Dependency Cleanup

### Duplicate Dependencies to Consolidate:
- bitflags: Multiple versions (1.3.2 and 2.9.1) - Update all to 2.9.1
- crypto-common and digest: Multiple versions due to transitive dependencies

### Potentially Unused Dependencies:
- atty: Consider if terminal detection is still needed
- md5: Review if MD5 is actually used (prefer SHA-256)
- pest/pest_derive: Check if parser functionality is actively used

## Code Organization Changes

### Files to Split:
- src/cook/mod.rs (2711 lines) → Split into:
  - src/cook/mod.rs (public API)
  - src/cook/execution.rs (command execution)
  - src/cook/iteration.rs (iteration management)
  - src/cook/state.rs (state handling)

### Functions to Extract:
- Large workflow execution functions in cook/workflow.rs
- Complex analysis functions in context modules
- Repeated patterns in test files

## TODO/FIXME Items

### Critical TODOs to Address:
1. `src/context/conventions.rs`: "TODO: Analyze actual test files to detect patterns"
2. `src/context/dependencies.rs`: "TODO: Parse exports" and "TODO: Parse Cargo.toml"

## Success Criteria
- [x] All high-impact debt items (impact >= 7) identified
- [ ] Error handling improved - reduce unwrap() usage by 80%
- [ ] All public functions have proper documentation
- [ ] Large files refactored to under 500 lines
- [ ] All clippy pedantic warnings addressed or explicitly allowed
- [ ] Duplicate dependencies consolidated
- [ ] Type casting made safe with proper error handling
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved