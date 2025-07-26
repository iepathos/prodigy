# Iteration 1: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Architecture Drift - Undocumented Modules
**Severity**: High
**Category**: Architecture Compliance
**File**: .mmm/ARCHITECTURE.md, src/lib.rs
**Line**: N/A

#### Current Code:
ARCHITECTURE.md lists these modules:
- CLI Interface (src/main.rs)
- Improve Command (src/improve/)
- Project Analysis (src/analyzer/)
- State Management (src/simple_state/)

But lib.rs exports additional modules:
```rust
pub mod analyzer;
pub mod claude;
pub mod config;
pub mod error;
pub mod improve;
pub mod project;
pub mod simple_state;
```

#### Required Change:
Either:
1. Update ARCHITECTURE.md to document the `claude`, `config`, `error`, and `project` modules
2. Or refactor to consolidate these modules into the documented architecture

#### Implementation Notes:
- Document the purpose and structure of each module
- Ensure architectural consistency
- Update the file organization section

### 2. Unsafe unwrap() Usage in Production Code
**Severity**: High
**Category**: Error Handling
**File**: src/analyzer/mod.rs
**Line**: 240, 265

#### Current Code:
```rust
// Line 240
ext.to_str().unwrap_or_default(),

// Line 265
name.to_str().unwrap_or_default(),
```

#### Required Change:
While these use `unwrap_or_default()` which is safer than bare `unwrap()`, the pattern could be clearer:
```rust
// Line 240
ext.to_str().unwrap_or(""),

// Line 265
name.to_str().unwrap_or(""),
```

#### Implementation Notes:
- Use explicit empty string instead of relying on Default trait
- Makes intent clearer

### 3. Production Code with Direct unwrap()
**Severity**: Critical
**Category**: Error Handling
**File**: src/project/manager.rs
**Line**: 38

#### Current Code:
```rust
Ok(self.projects.get(name).unwrap())
```

#### Required Change:
```rust
self.projects.get(name)
    .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", name))
```

#### Implementation Notes:
- Never use unwrap() in production code
- Return proper error instead
- Add context about which project was not found

### 4. Insufficient Test Coverage
**Severity**: High
**Category**: Testing
**File**: Multiple
**Line**: N/A

#### Current Code:
- Only 1 integration test file
- Only 1 unit test found in source
- Missing tests for critical components like improve/mod.rs

#### Required Change:
Add comprehensive tests for:
1. `src/improve/mod.rs` - Core improvement loop
2. `src/claude/` modules - Claude CLI integration
3. `src/config/` modules - Configuration handling
4. Error handling paths

#### Implementation Notes:
- Add unit tests for each major function
- Add integration tests for the full workflow
- Test error conditions and edge cases
- Aim for >80% coverage on critical paths

### 5. Missing Documentation for Public APIs
**Severity**: Medium
**Category**: Documentation
**File**: Multiple
**Line**: N/A

#### Current Code:
Many public structs and functions lack documentation comments.

#### Required Change:
Add rustdoc comments to all public APIs:
```rust
/// Brief description of what this does
/// 
/// # Arguments
/// * `param` - Description of parameter
/// 
/// # Returns
/// Description of return value
/// 
/// # Errors
/// When this function returns an error
pub fn function_name(param: Type) -> Result<ReturnType>
```

#### Implementation Notes:
- Focus on public API documentation first
- Include examples where helpful
- Document error conditions
- Use standard rustdoc format

### 6. Hardcoded Claude CLI Arguments
**Severity**: Medium
**Category**: Configuration
**File**: src/improve/mod.rs
**Line**: 107-110, 158-162

#### Current Code:
```rust
cmd.arg("--dangerously-skip-permissions")
    .arg("--print")
    .arg("/mmm-code-review")
```

#### Required Change:
Consider making these configurable or at least documenting why these specific flags are required.

#### Implementation Notes:
- Add comments explaining the purpose of each flag
- Consider if --dangerously-skip-permissions is truly necessary
- Document security implications

### 7. Regex Compilation in Methods
**Severity**: Low
**Category**: Performance
**File**: src/claude/response.rs
**Line**: 143-144, 199

#### Current Code:
```rust
code_block_regex: Regex::new(r"```(\w+)?\n([\s\S]*?)```").unwrap(),
file_path_regex: Regex::new(r"#\s*(?:File:|file:)\s*([^\n]+)").unwrap(),
command_regex: Regex::new(r"@mmm:(\w+)(?:\((.*?)\))?").unwrap(),
```

#### Required Change:
Use lazy_static or once_cell for compiled regexes:
```rust
use once_cell::sync::Lazy;

static CODE_BLOCK_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"```(\w+)?\n([\s\S]*?)```").expect("Invalid regex")
});
```

#### Implementation Notes:
- Compile regexes once at startup
- Better performance for repeated use
- Use expect() with descriptive message instead of unwrap()

## Success Criteria
- [ ] All production unwrap() calls replaced with proper error handling
- [ ] Architecture documentation updated to match implementation
- [ ] Test coverage increased for critical components
- [ ] Public APIs have complete documentation
- [ ] All files compile without warnings
- [ ] Tests pass