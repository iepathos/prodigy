# Test Coverage Analysis & Improvement

Systematically analyze and improve test coverage in Rust projects using comprehensive tooling and best practices.

## Primary Command

```bash
just coverage
```

## Workflow Steps

### 1. Run Initial Coverage Analysis
```bash
# Generate baseline coverage report
just coverage

# Alternative: More detailed coverage with line-by-line analysis
cargo tarpaulin --out Html --out Xml --output-dir coverage/ --timeout 300
```

### 2. Evaluate Current Coverage
- Open `tarpaulin-report.html` in browser to review visual coverage report
- Identify modules/functions with low coverage (<80%)
- Focus on critical business logic and public APIs first
- Look for untested error paths and edge cases

### 3. Analyze Uncovered Code Patterns
Use these commands to find specific uncovered areas:

```bash
# Find functions without tests
rg -t rust "^pub fn|^fn" --no-heading | grep -v "#\[test\]"

# Identify error handling paths
rg -t rust "Result<|Error|panic!|unwrap\(\)|expect\(" 

# Find complex conditional logic
rg -t rust "if.*else|match.*=>|for.*in|while"
```

### 4. Strategic Test Development

#### Unit Tests
- Add `#[cfg(test)]` modules in each source file
- Test all public functions with multiple scenarios
- Cover error conditions and edge cases
- Use property-based testing for complex logic

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_happy_path() {
        // Test normal operation
    }

    #[test]
    fn test_error_conditions() {
        // Test error scenarios
    }

    proptest! {
        #[test]
        fn test_properties(input in any::<String>()) {
            // Property-based testing
        }
    }
}
```

#### Integration Tests
- Create tests in `tests/` directory for end-to-end scenarios
- Test module interactions and public APIs
- Cover real-world usage patterns

#### Test Organization Best Practices
- Group related tests in modules
- Use descriptive test names that explain the scenario
- Test one thing per test function
- Use test fixtures and helper functions for setup

### 5. Coverage Improvement Strategies

#### Target High-Value Areas First
1. **Public APIs** - All exported functions should have comprehensive tests
2. **Error Handling** - Every `Result` return should have error case tests  
3. **Business Logic** - Core algorithms and decision logic
4. **Data Validation** - Input parsing and validation functions

#### Common Uncovered Patterns to Address
- Error path testing: `Err(...)` branches in `match` statements
- Default implementations and trait methods
- Drop implementations and cleanup code
- Async/concurrent code paths
- Configuration and initialization code

### 6. Advanced Coverage Techniques

#### Mock External Dependencies
```rust
#[cfg(test)]
use mockall::predicate::*;

// Mock external services for isolated testing
```

#### Test Different Configurations
```rust
#[test]
#[cfg(feature = "specific-feature")]
fn test_feature_specific_behavior() {
    // Test feature-gated code
}
```

#### Benchmark Critical Paths
```rust
#[bench]
fn bench_critical_function(b: &mut Bencher) {
    b.iter(|| {
        // Benchmark performance-critical code
    });
}
```

### 7. Continuous Coverage Monitoring

```bash
# Set coverage thresholds in CI/CD
cargo tarpaulin --fail-under 80

# Generate coverage badges for README
cargo tarpaulin --out Xml --output-dir coverage/
```

## Coverage Quality Metrics

### Target Thresholds
- **Overall Project**: >85% line coverage
- **Public APIs**: >95% line coverage  
- **Critical Business Logic**: >90% line coverage
- **Error Handling**: >80% branch coverage

### Quality Indicators
- All public functions have at least one test
- Error conditions are explicitly tested
- Edge cases and boundary conditions covered
- Integration between modules tested

## Tools Integration

### Required Dependencies
Add to `Cargo.toml` for comprehensive testing:

```toml
[dev-dependencies]
proptest = "1.0"
mockall = "0.11"
tempfile = "3.0"
criterion = "0.5"
```

### IDE Integration
- Configure rust-analyzer to show inline coverage
- Use coverage gutters in VS Code
- Set up automated coverage reports in CI/CD

## Common Anti-Patterns to Avoid
- Testing implementation details instead of behavior
- Writing tests that duplicate production code logic
- Focusing only on line coverage without testing edge cases
- Ignoring error paths and exceptional conditions
- Creating brittle tests that break with refactoring

## Success Criteria
✅ Coverage report shows >85% overall coverage
✅ All public APIs have comprehensive tests  
✅ Error conditions are explicitly tested
✅ Integration tests cover main user workflows
✅ Performance-critical code is benchmarked
✅ Tests run fast (<30s for full suite)
✅ CI/CD enforces coverage thresholds