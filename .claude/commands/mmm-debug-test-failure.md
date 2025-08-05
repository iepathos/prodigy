# /mmm-debug-test-failure

Fix failing tests by analyzing test output and applying targeted fixes.

## Variables

--spec: Path to spec file that generated the tests
--output: Test failure output from cargo test

## Execute

1. **Parse current test output** to identify what's failing NOW:
   - Failed test names and file locations (may differ from previous attempts)
   - Error types (assertion, panic, compile, async)
   - Specific error messages and stack traces

2. **Read the spec file** to understand test intent and implementation

3. **Apply fixes based on current failure type**:
   ```rust
   // Assertion failure → Update expected values
   assert_eq!(result, 42); // Change to actual value
   
   // Missing imports → Add use statements  
   use tempfile::TempDir;
   use mockall::predicate::*;
   
   // Async test → Convert to tokio::test
   #[tokio::test]
   async fn test_async() { ... }
   
   // Missing setup → Add fixtures
   let temp_dir = TempDir::new()?;
   std::env::set_current_dir(&temp_dir)?;
   ```

4. **Fix strategy (apply all relevant fixes)**:
   - Import errors → Add missing use statements
   - Assertion failures → Adjust expected values to match actual
   - Async issues → Convert to #[tokio::test]
   - Missing setup → Add fixtures, mocks, or test data
   - Each run may reveal new failures after fixing others

5. **Verify all tests pass**:
   ```bash
   cargo test  # Run full suite, not just specific tests
   ```

6. **Output**:
   - Success: "✓ All tests passing after fixing N tests"
   - Failed: "✗ Fixed N tests but M still failing"

## Common Patterns

**Import fixes**:
```rust
use std::path::PathBuf;
use anyhow::Result;
```

**Async runtime**:
```rust
#[tokio::test]  // Not #[test]
async fn test_name() { }
```

**Test doubles**:
```rust
let mut mock = MockService::new();
mock.expect_call().returning(|| Ok(42));
```

**File system**:
```rust
let temp = TempDir::new()?;
let path = temp.path().join("test.txt");
```