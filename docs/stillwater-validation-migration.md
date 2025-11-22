# Stillwater Validation Migration Guide

This document provides before/after examples demonstrating the migration of Prodigy's validation code to use the stillwater library's error accumulation patterns.

## Overview

The migration to stillwater validation brings several key improvements:

- **Error Accumulation**: Collect all validation errors before failing, instead of stopping at the first error
- **Composable Validators**: Build complex validators from simple, reusable functions
- **Pure Functions**: Separate validation logic from I/O operations for better testability
- **Consistent Error Handling**: Uniform error types and severity classification across the codebase

## Migration Patterns

### Pattern 1: Single Field Validation with Early Return (Before)

**Before (Imperative Style)**:
```rust
fn validate_path(path: &Path) -> Result<PathBuf, ValidationError> {
    if !path.exists() {
        return Err(ValidationError::PathNotFound(path.to_path_buf()));
    }

    if path.starts_with("..") {
        return Err(ValidationError::PathInParentDir(path.to_path_buf()));
    }

    Ok(path.to_path_buf())
}

// Only reports FIRST error
fn validate_all_paths(paths: &[&Path]) -> Result<Vec<PathBuf>, ValidationError> {
    let mut results = Vec::new();
    for path in paths {
        results.push(validate_path(path)?); // Stops at first error
    }
    Ok(results)
}
```

**After (Stillwater Functional Style)**:
```rust
use stillwater::Validation;

fn validate_path(
    path: &Path,
    exists_check: FileExistsCheck,
) -> Validation<PathBuf, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if !exists_check(path) {
        errors.push(ValidationError::PathNotFound(path.to_path_buf()));
    }

    if path.starts_with("..") {
        errors.push(ValidationError::PathInParentDir(path.to_path_buf()));
    }

    if errors.is_empty() {
        Validation::success(path.to_path_buf())
    } else {
        Validation::failure(errors)
    }
}

// Accumulates ALL errors from ALL paths
pub fn validate_paths(paths: &[&Path], exists_check: FileExistsCheck) -> ValidationResult {
    let mut all_errors = Vec::new();
    let mut all_paths = Vec::new();

    for &path in paths {
        match validate_path(path, exists_check).into_result() {
            Ok(p) => all_paths.push(p),
            Err(errors) => all_errors.extend(errors), // Collect ALL errors
        }
    }

    let validation = if all_errors.is_empty() {
        Validation::success(all_paths)
    } else {
        Validation::failure(all_errors)
    };

    ValidationResult::from_validation(validation)
}
```

**Benefits**:
- Reports ALL path errors at once instead of just the first one
- Separates I/O (file existence check) from pure validation logic
- Allows testing without touching the filesystem

### Pattern 2: Multiple Independent Validations (Before)

**Before (Sequential with Early Returns)**:
```rust
fn validate_environment(required_vars: &[&str]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    for var in required_vars {
        if std::env::var(var).is_err() {
            errors.push(format!("Missing: {}", var));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(())
}
```

**After (Stillwater Accumulation)**:
```rust
use stillwater::Validation;

fn validate_env_var(
    var_name: &str,
    env_vars: &HashMap<String, String>,
) -> Validation<(String, String), Vec<ValidationError>> {
    if let Some(value) = env_vars.get(var_name) {
        if (var_name.contains("KEY") || var_name.contains("SECRET"))
            && value.trim().is_empty()
        {
            Validation::failure(vec![ValidationError::EnvVarEmpty(var_name.to_string())])
        } else {
            Validation::success((var_name.to_string(), value.clone()))
        }
    } else {
        Validation::failure(vec![ValidationError::EnvVarMissing(var_name.to_string())])
    }
}

pub fn validate_environment(
    required_vars: &[&str],
    env_vars: &HashMap<String, String>,
) -> ValidationResult {
    let mut all_errors = Vec::new();
    let mut all_vars = Vec::new();

    for &var in required_vars {
        match validate_env_var(var, env_vars).into_result() {
            Ok(pair) => all_vars.push(pair),
            Err(errors) => all_errors.extend(errors),
        }
    }

    let validation = if all_errors.is_empty() {
        Validation::success(all_vars)
    } else {
        Validation::failure(all_errors)
    };

    ValidationResult::from_validation(validation)
}
```

**Benefits**:
- Validates ALL environment variables, not just until first failure
- Separates env var access from validation logic (accepts HashMap parameter)
- Distinguishes between missing vars (errors) and empty sensitive vars (warnings)

### Pattern 3: Nested Validation with Multiple Error Types

**Before (Mixed Error Types)**:
```rust
fn validate_command(command: &str) -> Result<(), String> {
    if command.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    if command.contains("rm -rf /") {
        return Err("Dangerous command pattern".to_string());
    }

    if command.contains("sudo") {
        eprintln!("Warning: Command uses sudo");
    }

    Ok(())
}
```

**After (Unified Error Handling)**:
```rust
use stillwater::Validation;

fn check_dangerous_patterns(command: &str) -> Validation<(), Vec<ValidationError>> {
    let dangerous_patterns = [
        "rm -rf /",
        "dd if=/dev/zero",
        ":(){ :|:& };:", // Fork bomb
    ];

    let errors: Vec<ValidationError> = dangerous_patterns
        .iter()
        .filter(|&&pattern| command.contains(pattern))
        .map(|&pattern| ValidationError::CommandDangerous {
            cmd: command.to_string(),
            pattern: pattern.to_string(),
        })
        .collect();

    if errors.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(errors)
    }
}

fn check_suspicious_patterns(command: &str) -> Validation<(), Vec<ValidationError>> {
    let mut warnings = Vec::new();

    if command.contains("sudo") && !command.contains("sudo -n") {
        warnings.push(ValidationError::CommandSuspicious {
            cmd: command.to_string(),
            reason: "Command uses sudo which may require password".to_string(),
        });
    }

    if warnings.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(warnings)
    }
}

pub fn validate_command(command: &str) -> ValidationResult {
    if command.trim().is_empty() {
        return ValidationResult::from_validation(
            Validation::<(), Vec<ValidationError>>::failure(vec![
                ValidationError::CommandEmpty,
            ])
        );
    }

    let mut all_errors = Vec::new();

    // Accumulate errors from multiple checks
    if let Err(errors) = check_dangerous_patterns(command).into_result() {
        all_errors.extend(errors);
    }

    if let Err(errors) = check_suspicious_patterns(command).into_result() {
        all_errors.extend(errors);
    }

    let validation = if all_errors.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(all_errors)
    };

    ValidationResult::from_validation(validation)
}
```

**Benefits**:
- Finds ALL dangerous and suspicious patterns in a single pass
- Separates error severity (errors vs warnings) through error classification
- Composable validators can be tested independently
- Consistent error types instead of ad-hoc strings

### Pattern 4: Resource Limit Validation

**Before (Separate Functions)**:
```rust
fn validate_memory(memory_mb: usize) -> Result<(), String> {
    if memory_mb == 0 {
        return Err("Memory cannot be zero".to_string());
    }
    if memory_mb < 128 {
        eprintln!("Warning: Low memory limit");
    }
    Ok(())
}

fn validate_cpu(cpu_cores: usize) -> Result<(), String> {
    if cpu_cores == 0 {
        return Err("CPU cores cannot be zero".to_string());
    }
    Ok(())
}

fn validate_limits(limits: &ResourceLimits) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Err(e) = validate_memory(limits.memory_mb) {
        errors.push(e);
    }

    if let Err(e) = validate_cpu(limits.cpu_cores) {
        errors.push(e);
    }

    if !errors.is_empty() {
        Err(errors)
    } else {
        Ok(())
    }
}
```

**After (Composed Validation)**:
```rust
use stillwater::Validation;

fn validate_memory_limit(memory_mb: usize) -> Validation<usize, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if memory_mb == 0 {
        errors.push(ValidationError::MemoryLimitZero);
    } else if memory_mb < 128 {
        errors.push(ValidationError::MemoryLimitLow(memory_mb));
    } else if memory_mb > 32768 {
        errors.push(ValidationError::MemoryLimitHigh(memory_mb));
    }

    if errors.is_empty() {
        Validation::success(memory_mb)
    } else {
        Validation::failure(errors)
    }
}

fn validate_cpu_cores(cpu_cores: usize) -> Validation<usize, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if cpu_cores == 0 {
        errors.push(ValidationError::CpuCoresZero);
    } else if cpu_cores > 64 {
        errors.push(ValidationError::CpuCoresHigh(cpu_cores));
    }

    if errors.is_empty() {
        Validation::success(cpu_cores)
    } else {
        Validation::failure(errors)
    }
}

pub fn validate_resource_limits(limits: &ResourceLimits) -> ValidationResult {
    let mut all_errors = Vec::new();

    // Accumulate errors from all validators
    if let Err(errors) = validate_memory_limit(limits.memory_mb).into_result() {
        all_errors.extend(errors);
    }

    if let Err(errors) = validate_cpu_cores(limits.cpu_cores).into_result() {
        all_errors.extend(errors);
    }

    if let Err(errors) = validate_timeout(limits.timeout_seconds).into_result() {
        all_errors.extend(errors);
    }

    let validation = if all_errors.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(all_errors)
    };

    ValidationResult::from_validation(validation)
}
```

**Benefits**:
- Reports ALL resource limit issues at once
- Structured error types (not strings) enable programmatic handling
- Warnings and errors handled through error severity classification
- Each validator is independently testable

## Error Severity Classification

Stillwater validation introduces error severity classification:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Error,   // Must be fixed
    Warning, // Should review but not blocking
}

impl ValidationError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // Warnings
            Self::PathInParentDir(_)
            | Self::PathInTempDir(_)
            | Self::EnvVarEmpty(_)
            | Self::CommandSuspicious { .. }
            | Self::IterationCountHigh(_)
            | Self::MemoryLimitLow(_) => ErrorSeverity::Warning,

            // Errors
            _ => ErrorSeverity::Error,
        }
    }
}
```

**Usage Example**:
```rust
let result = validate_paths(&paths, exists_check);

// Separate errors and warnings
for error in result.errors {
    eprintln!("Error: {}", error);
}

for warning in result.warnings {
    eprintln!("Warning: {}", warning);
}

// Can proceed with warnings, but not errors
if !result.errors.is_empty() {
    return Err("Validation failed");
}
```

## Testing Benefits

**Before (Requires I/O)**:
```rust
#[test]
fn test_validate_path() {
    let temp_file = create_temp_file(); // Requires file system
    let result = validate_path(&temp_file);
    assert!(result.is_ok());
}
```

**After (Pure Function)**:
```rust
#[test]
fn test_validate_path() {
    // Mock exists check - no I/O required
    fn mock_exists_check(path: &Path) -> bool {
        path.to_str().map(|s| s.contains("exists")).unwrap_or(false)
    }

    let path = Path::new("/tmp/exists.txt");
    let result = validate_path(path, mock_exists_check);
    assert!(result.is_success());
}

#[test]
fn test_validate_multiple_errors() {
    let paths = vec![
        Path::new("/missing1"),
        Path::new("/missing2"),
        Path::new("/missing3"),
    ];

    let result = validate_paths(&paths, |_| false);

    // Accumulates ALL errors
    assert_eq!(result.errors.len(), 3);
}
```

## Performance Considerations

The stillwater validation migration maintains zero performance regression:

- **No allocation overhead**: Error accumulation uses `Vec` which is already allocated
- **No additional traversals**: Validation happens in a single pass over the data
- **Inline-friendly**: Small validators are optimized by the compiler
- **Benchmark verification**: See `benches/execution_benchmarks.rs::bench_validation_performance`

Run benchmarks to verify:
```bash
cargo bench --bench execution_benchmarks -- validation_performance
```

## Migration Checklist

When migrating validation code to stillwater:

1. **Identify validation functions** that return early on first error
2. **Extract I/O operations** as function parameters (e.g., `FileExistsCheck`)
3. **Replace early returns** with error accumulation in `Vec<ValidationError>`
4. **Use `Validation<T, Vec<ValidationError>>`** as return type
5. **Compose validators** by collecting errors from multiple validators
6. **Classify errors** using `ErrorSeverity` for warnings vs errors
7. **Convert to `ValidationResult`** for backward compatibility
8. **Add tests** that verify error accumulation (not just first error)

## Key Takeaways

The stillwater validation migration provides:

- **Better UX**: Users see ALL errors at once, not just the first one
- **Better testing**: Pure functions with injected dependencies
- **Better composition**: Build complex validators from simple ones
- **Better consistency**: Uniform error types across the codebase
- **Zero regression**: Maintains performance while improving functionality

This migration is part of Prodigy's broader adoption of functional programming patterns to improve code quality, testability, and maintainability.
