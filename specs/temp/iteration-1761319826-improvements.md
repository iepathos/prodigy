# Iteration 1761319826: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Potential Panic in Path Handling
**Severity**: High
**Category**: Error Handling
**File**: src/analyzer/health.rs
**Line**: 292

#### Current Code:
```rust
if matches!(
    ext.to_str().unwrap_or_default(),
    "rs" | "py" | "js" | "ts" | "go" | "java" | "cs" | "rb" | "swift" | "kt"
)
```

#### Required Change:
```rust
if let Some(ext_str) = ext.to_str() {
    if matches!(
        ext_str,
        "rs" | "py" | "js" | "ts" | "go" | "java" | "cs" | "rb" | "swift" | "kt"
    ) {
        search_todos_in_file(&path, todos).await?;
    }
}
```

#### Implementation Notes:
- Remove the unwrap_or_default() pattern which could hide encoding issues
- Use proper Option handling with if let
- Maintain the same logic flow but with safer error handling

### 2. Missing Documentation for Public Functions
**Severity**: Medium
**Category**: Documentation
**File**: src/improve/mod.rs
**Line**: 25

#### Current Code:
```rust
pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
    println!("🔍 Analyzing project...");
```

#### Required Change:
Add comprehensive documentation above the function. The doc comment is already present but could be enhanced with examples:
```rust
/// Run the improve command to automatically enhance code quality
///
/// # Arguments
/// * `cmd` - The improve command with optional target score, verbosity, and focus directive
///
/// # Returns
/// Result indicating success or failure of the improvement process
///
/// # Errors
/// Returns an error if:
/// - Project analysis fails
/// - Claude CLI is not available
/// - File operations fail
/// - Git operations fail
///
/// # Example
/// ```no_run
/// use mmm::improve::{run, command::ImproveCommand};
/// 
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let cmd = ImproveCommand {
///         target: 8.0,
///         show_progress: true,
///         focus: Some("performance".to_string()),
///     };
///     run(cmd).await?;
///     Ok(())
/// }
/// ```
pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
```

#### Implementation Notes:
- Add usage example to the existing documentation
- Ensure all public functions have similar comprehensive documentation

### 3. Inefficient TODO Search Implementation
**Severity**: Medium  
**Category**: Performance
**File**: src/analyzer/health.rs
**Line**: 278-309

#### Current Code:
```rust
fn search_todos_in_dir<'a>(
    dir: &'a Path,
    todos: &'a mut Vec<TodoItem>,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        // ... recursive implementation
    })
}
```

#### Required Change:
```rust
use futures::stream::{FuturesUnordered, StreamExt};

async fn search_todos_in_dir(dir: &Path, todos: &mut Vec<TodoItem>) -> Result<()> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    let mut futures = FuturesUnordered::new();
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if let Some(ext_str) = ext.to_str() {
                    if matches!(
                        ext_str,
                        "rs" | "py" | "js" | "ts" | "go" | "java" | "cs" | "rb" | "swift" | "kt"
                    ) {
                        // Process file immediately
                        search_todos_in_file(&path, todos).await?;
                        
                        // Stop early if we have enough TODOs
                        if todos.len() >= 50 {
                            return Ok(());
                        }
                    }
                }
            }
        } else if path.is_dir() {
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
                
            // Skip common ignored directories
            if !matches!(file_name, "node_modules" | "target" | ".git" | "dist" | "build") {
                futures.push(Box::pin(search_todos_in_dir(&path, todos)));
            }
        }
    }
    
    // Process subdirectories concurrently
    while let Some(result) = futures.next().await {
        result?;
        if todos.len() >= 50 {
            break;
        }
    }
    
    Ok(())
}
```

#### Implementation Notes:
- Remove the complex Pin<Box<dyn Future>> pattern
- Add early termination when 50 TODOs are found
- Skip more common directories that shouldn't be searched
- Use concurrent processing for subdirectories
- Add proper imports at the top of the file

### 4. Test Coverage Gaps
**Severity**: Medium
**Category**: Testing
**File**: tests/cli_tests.rs
**Line**: N/A

#### Current Code:
Only basic CLI parsing tests exist.

#### Required Change:
Add integration tests for the core improvement flow:
```rust
#[test]
#[ignore] // Requires Claude CLI to be installed
fn test_improve_command_dry_run() {
    use std::env;
    
    // Set up test environment
    env::set_var("MMM_DRY_RUN", "true");
    
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["improve", "--target", "7.0"])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success();
}

#[test]
fn test_improve_with_focus() {
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["improve", "--focus", "performance", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("focus"));
}

#[test]
fn test_invalid_target_score() {
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["improve", "--target", "11.0"])
        .assert()
        .failure();
}
```

#### Implementation Notes:
- Add tests for the improve command with various options
- Include tests for error conditions
- Add dry-run mode support for testing without Claude CLI
- Ensure tests don't hang by adding timeouts

### 5. Redundant Error Type Variants
**Severity**: Low
**Category**: Code Quality  
**File**: src/error.rs
**Line**: 53-84

#### Current Code:
```rust
#[error("Anyhow error: {0}")]
Anyhow(#[from] anyhow::Error),

#[error("Other error: {0}")]
Other(String),

// ... many similar variants
```

#### Required Change:
Remove redundant error variants and consolidate similar ones:
```rust
// Remove these redundant variants:
// - Anyhow (conflicts with proper error handling)
// - Other (too generic)
// - Internal (duplicates Other)
// - Deserialization (duplicates Serialization)

// Keep only specific, meaningful error types
```

#### Implementation Notes:
- Remove the Anyhow variant - it defeats the purpose of using thiserror
- Consolidate Other, Internal, and similar generic variants
- Keep only error variants that provide specific context
- Update any code using removed variants

## Success Criteria
- [ ] All unwrap() calls replaced with proper error handling
- [ ] Public API functions have complete documentation with examples
- [ ] TODO search completes in under 1 second for large projects
- [ ] Test coverage includes integration tests for core functionality
- [ ] Error types are consolidated and meaningful
- [ ] All files compile without warnings
- [ ] Tests pass