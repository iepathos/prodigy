//! Pure validation functions using stillwater for error accumulation
//!
//! These functions validate data structures and business rules without performing any I/O operations.
//! All validators return `Validation<T, Vec<ValidationError>>` to accumulate all errors before failing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use stillwater::Validation;

/// File existence check function type (passed as parameter to avoid I/O)
pub type FileExistsCheck = fn(&Path) -> bool;

/// Validation errors that can be accumulated
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    PathNotFound(PathBuf),
    PathInParentDir(PathBuf),
    PathInTempDir(PathBuf),
    EnvVarMissing(String),
    EnvVarEmpty(String),
    JsonNotObject,
    JsonFieldMissing(String),
    JsonFieldNull(String),
    CommandEmpty,
    CommandDangerous { cmd: String, pattern: String },
    CommandSuspicious { cmd: String, reason: String },
    IterationCountZero,
    IterationCountExceeded { count: usize, max: usize },
    IterationCountHigh(usize),
    MemoryLimitZero,
    MemoryLimitLow(usize),
    MemoryLimitHigh(usize),
    CpuCoresZero,
    CpuCoresHigh(usize),
    TimeoutZero,
    TimeoutLow(usize),
    TimeoutHigh(usize),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PathNotFound(p) => write!(f, "Path does not exist: {}", p.display()),
            Self::PathInParentDir(p) => write!(f, "Path uses parent directory: {}", p.display()),
            Self::PathInTempDir(p) => write!(f, "Path in temporary directory: {}", p.display()),
            Self::EnvVarMissing(v) => write!(f, "Missing required environment variable: {}", v),
            Self::EnvVarEmpty(v) => write!(f, "Sensitive variable '{}' is empty", v),
            Self::JsonNotObject => write!(f, "JSON is not an object"),
            Self::JsonFieldMissing(field) => write!(f, "Missing required field: {}", field),
            Self::JsonFieldNull(field) => write!(f, "Field '{}' is null", field),
            Self::CommandEmpty => write!(f, "Command is empty"),
            Self::CommandDangerous { cmd: _, pattern } => {
                write!(f, "Dangerous command pattern detected: {}", pattern)
            }
            Self::CommandSuspicious { cmd: _, reason } => write!(f, "{}", reason),
            Self::IterationCountZero => write!(f, "Iteration count cannot be zero"),
            Self::IterationCountExceeded { count, max } => {
                write!(
                    f,
                    "Iteration count {} exceeds maximum allowed {}",
                    count, max
                )
            }
            Self::IterationCountHigh(count) => {
                write!(f, "High iteration count ({}) may take a long time", count)
            }
            Self::MemoryLimitZero => write!(f, "Memory limit cannot be zero"),
            Self::MemoryLimitLow(mb) => {
                write!(
                    f,
                    "Memory limit {} MB may be too low for normal operation",
                    mb
                )
            }
            Self::MemoryLimitHigh(mb) => {
                write!(
                    f,
                    "Memory limit {} MB may exceed available system memory",
                    mb
                )
            }
            Self::CpuCoresZero => write!(f, "CPU cores cannot be zero"),
            Self::CpuCoresHigh(cores) => {
                write!(f, "CPU core count {} may exceed available cores", cores)
            }
            Self::TimeoutZero => write!(f, "Timeout cannot be zero"),
            Self::TimeoutLow(secs) => {
                write!(
                    f,
                    "Timeout {} seconds may be too short for operations to complete",
                    secs
                )
            }
            Self::TimeoutHigh(secs) => {
                write!(
                    f,
                    "Long timeout {} seconds may cause hanging processes",
                    secs
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Classification of validation errors
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Error,
    Warning,
}

impl ValidationError {
    /// Classify error severity
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::PathInParentDir(_)
            | Self::PathInTempDir(_)
            | Self::EnvVarEmpty(_)
            | Self::JsonFieldNull(_)
            | Self::CommandSuspicious { .. }
            | Self::IterationCountHigh(_)
            | Self::MemoryLimitLow(_)
            | Self::MemoryLimitHigh(_)
            | Self::CpuCoresHigh(_)
            | Self::TimeoutLow(_)
            | Self::TimeoutHigh(_) => ErrorSeverity::Warning,
            _ => ErrorSeverity::Error,
        }
    }
}

/// Validation result with errors and warnings (backward compatibility)
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a valid result
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Add an error
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.is_valid = false;
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Convert from stillwater Validation
    pub fn from_validation<T>(v: Validation<T, Vec<ValidationError>>) -> Self {
        match v.into_result() {
            Ok(_) => Self::valid(),
            Err(errors) => {
                let mut result = Self::valid();
                for error in errors {
                    match error.severity() {
                        ErrorSeverity::Error => result.add_error(error.to_string()),
                        ErrorSeverity::Warning => result.add_warning(error.to_string()),
                    }
                }
                result
            }
        }
    }
}

/// Validate a single path
fn validate_path(
    path: &Path,
    exists_check: FileExistsCheck,
) -> Validation<PathBuf, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if !exists_check(path) {
        errors.push(ValidationError::PathNotFound(path.to_path_buf()));
    }

    // Warnings for suspicious paths
    if path.starts_with("..") {
        errors.push(ValidationError::PathInParentDir(path.to_path_buf()));
    }

    if path.is_absolute() && path.starts_with("/tmp") {
        errors.push(ValidationError::PathInTempDir(path.to_path_buf()));
    }

    if errors.is_empty() {
        Validation::success(path.to_path_buf())
    } else {
        Validation::failure(errors)
    }
}

/// Validate file paths without performing I/O - accumulates all path errors
pub fn validate_paths(paths: &[&Path], exists_check: FileExistsCheck) -> ValidationResult {
    let mut all_errors = Vec::new();
    let mut all_paths = Vec::new();

    for &path in paths {
        match validate_path(path, exists_check).into_result() {
            Ok(p) => all_paths.push(p),
            Err(errors) => all_errors.extend(errors),
        }
    }

    let validation = if all_errors.is_empty() {
        Validation::success(all_paths)
    } else {
        Validation::failure(all_errors)
    };

    ValidationResult::from_validation(validation)
}

/// Validate a single environment variable
fn validate_env_var(
    var_name: &str,
    env_vars: &HashMap<String, String>,
) -> Validation<(String, String), Vec<ValidationError>> {
    if let Some(value) = env_vars.get(var_name) {
        // Check for sensitive variables that shouldn't be empty
        if (var_name.contains("KEY") || var_name.contains("SECRET") || var_name.contains("TOKEN"))
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

/// Validate environment variables - accumulates all missing/empty var errors
pub fn validate_environment(
    required_vars: &[&str],
    env_vars: &HashMap<String, String>,
) -> ValidationResult {
    let mut all_errors = Vec::new();
    let mut all_vars = Vec::new();

    // Validate required variables
    for &var in required_vars {
        match validate_env_var(var, env_vars).into_result() {
            Ok(pair) => all_vars.push(pair),
            Err(errors) => all_errors.extend(errors),
        }
    }

    // Check all environment variables for empty sensitive values (warnings)
    for (key, value) in env_vars {
        if (key.contains("KEY") || key.contains("SECRET") || key.contains("TOKEN"))
            && value.trim().is_empty()
        {
            all_errors.push(ValidationError::EnvVarEmpty(key.clone()));
        }
    }

    let validation = if all_errors.is_empty() {
        Validation::success(all_vars)
    } else {
        Validation::failure(all_errors)
    };

    ValidationResult::from_validation(validation)
}

/// Check if command contains dangerous patterns
fn check_dangerous_patterns(command: &str) -> Validation<(), Vec<ValidationError>> {
    let dangerous_patterns = [
        "rm -rf /",
        "dd if=/dev/zero",
        ":(){ :|:& };:", // Fork bomb
        "> /dev/sda",
        "chmod -R 777 /",
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

/// Check for suspicious command patterns
fn check_suspicious_patterns(command: &str) -> Validation<(), Vec<ValidationError>> {
    let mut warnings = Vec::new();

    if command.contains("sudo") && !command.contains("sudo -n") {
        warnings.push(ValidationError::CommandSuspicious {
            cmd: command.to_string(),
            reason: "Command uses sudo which may require password".to_string(),
        });
    }

    if command.contains("curl") && command.contains("| sh") {
        warnings.push(ValidationError::CommandSuspicious {
            cmd: command.to_string(),
            reason: "Command pipes curl output to shell, potential security risk".to_string(),
        });
    }

    if warnings.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(warnings)
    }
}

/// Validate command string - accumulates all command validation errors
pub fn validate_command(command: &str) -> ValidationResult {
    if command.trim().is_empty() {
        return ValidationResult::from_validation(Validation::<(), Vec<ValidationError>>::failure(vec![
            ValidationError::CommandEmpty,
        ]));
    }

    let mut all_errors = Vec::new();

    // Check for dangerous patterns
    if let Err(errors) = check_dangerous_patterns(command).into_result() {
        all_errors.extend(errors);
    }

    // Check for suspicious patterns
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

/// Validate a single required JSON field
fn validate_json_field(
    field_name: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Validation<(), Vec<ValidationError>> {
    if !obj.contains_key(field_name) {
        Validation::failure(vec![ValidationError::JsonFieldMissing(field_name.to_string())])
    } else {
        Validation::success(())
    }
}

/// Check for null values in non-optional fields
fn check_null_fields(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Validation<(), Vec<ValidationError>> {
    let warnings: Vec<ValidationError> = obj
        .iter()
        .filter(|(key, value)| value.is_null() && !key.contains("optional"))
        .map(|(key, _)| ValidationError::JsonFieldNull(key.clone()))
        .collect();

    if warnings.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(warnings)
    }
}

/// Validate JSON structure - accumulates all field errors
pub fn validate_json_schema(
    json: &serde_json::Value,
    required_fields: &[&str],
) -> ValidationResult {
    if let Some(obj) = json.as_object() {
        let mut all_errors = Vec::new();

        // Check required fields
        for &field in required_fields {
            if let Err(errors) = validate_json_field(field, obj).into_result() {
                all_errors.extend(errors);
            }
        }

        // Check for null values
        if let Err(errors) = check_null_fields(obj).into_result() {
            all_errors.extend(errors);
        }

        let validation = if all_errors.is_empty() {
            Validation::success(())
        } else {
            Validation::failure(all_errors)
        };

        ValidationResult::from_validation(validation)
    } else {
        ValidationResult::from_validation(Validation::<(), Vec<ValidationError>>::failure(vec![ValidationError::JsonNotObject]))
    }
}

/// Validate iteration count - accumulates all count errors
pub fn validate_iteration_count(count: usize, max_allowed: usize) -> ValidationResult {
    let mut errors = Vec::new();

    if count == 0 {
        errors.push(ValidationError::IterationCountZero);
    } else if count > max_allowed {
        errors.push(ValidationError::IterationCountExceeded {
            count,
            max: max_allowed,
        });
    } else if count > 50 {
        errors.push(ValidationError::IterationCountHigh(count));
    }

    ValidationResult::from_validation(if errors.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(errors)
    })
}

/// Resource limits structure
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub memory_mb: usize,
    pub cpu_cores: usize,
    pub timeout_seconds: usize,
}

/// Validate memory limit
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

/// Validate CPU cores
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

/// Validate timeout
fn validate_timeout(timeout_seconds: usize) -> Validation<usize, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if timeout_seconds == 0 {
        errors.push(ValidationError::TimeoutZero);
    } else if timeout_seconds < 10 {
        errors.push(ValidationError::TimeoutLow(timeout_seconds));
    } else if timeout_seconds > 3600 {
        errors.push(ValidationError::TimeoutHigh(timeout_seconds));
    }

    if errors.is_empty() {
        Validation::success(timeout_seconds)
    } else {
        Validation::failure(errors)
    }
}

/// Validate resource limits - accumulates all limit errors
pub fn validate_resource_limits(limits: &ResourceLimits) -> ValidationResult {
    let mut all_errors = Vec::new();

    // Validate memory
    if let Err(errors) = validate_memory_limit(limits.memory_mb).into_result() {
        all_errors.extend(errors);
    }

    // Validate CPU
    if let Err(errors) = validate_cpu_cores(limits.cpu_cores).into_result() {
        all_errors.extend(errors);
    }

    // Validate timeout
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn mock_exists_check(path: &Path) -> bool {
        path.to_str().map(|s| s.contains("exists")).unwrap_or(false)
    }

    #[test]
    fn test_validate_paths() {
        let paths: Vec<&Path> = vec![
            Path::new("/tmp/exists.txt"),
            Path::new("/tmp/missing.txt"),
            Path::new("../parent.txt"),
        ];

        let result = validate_paths(&paths, mock_exists_check);

        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 2); // missing paths (missing.txt and parent.txt don't exist)
        assert_eq!(result.warnings.len(), 3); // 2 tmp paths + 1 parent dir
    }

    #[test]
    fn test_validate_paths_accumulates_all_errors() {
        let paths: Vec<&Path> = vec![
            Path::new("/nonexistent1"),
            Path::new("/nonexistent2"),
            Path::new("/nonexistent3"),
        ];

        let result = validate_paths(&paths, mock_exists_check);

        // Should accumulate ALL errors, not just first
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 3);
        assert!(result.errors.iter().any(|e| e.contains("nonexistent1")));
        assert!(result.errors.iter().any(|e| e.contains("nonexistent2")));
        assert!(result.errors.iter().any(|e| e.contains("nonexistent3")));
    }

    #[test]
    fn test_validate_environment() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("API_KEY".to_string(), "".to_string());

        let required = vec!["PATH", "HOME"];
        let result = validate_environment(&required, &env);

        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("HOME")));
        assert!(result.warnings.iter().any(|w| w.contains("API_KEY")));
    }

    #[test]
    fn test_validate_environment_accumulates_all_errors() {
        let env = HashMap::new();
        let required = vec!["VAR1", "VAR2", "VAR3"];
        let result = validate_environment(&required, &env);

        // Should accumulate ALL missing variables
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 3);
        assert!(result.errors.iter().any(|e| e.contains("VAR1")));
        assert!(result.errors.iter().any(|e| e.contains("VAR2")));
        assert!(result.errors.iter().any(|e| e.contains("VAR3")));
    }

    #[test]
    fn test_validate_json_schema() {
        let json = serde_json::json!({
            "name": "test",
            "optional_field": null,
            "value": 42
        });

        let required = vec!["name", "value", "missing"];
        let result = validate_json_schema(&json, &required);

        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("missing")));
        assert!(result.warnings.is_empty()); // optional_field is ignored
    }

    #[test]
    fn test_validate_json_accumulates_all_missing_fields() {
        let json = serde_json::json!({
            "existing": "value"
        });

        let required = vec!["field1", "field2", "field3"];
        let result = validate_json_schema(&json, &required);

        // Should accumulate ALL missing fields
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 3);
        assert!(result.errors.iter().any(|e| e.contains("field1")));
        assert!(result.errors.iter().any(|e| e.contains("field2")));
        assert!(result.errors.iter().any(|e| e.contains("field3")));
    }

    #[test]
    fn test_validate_command() {
        // Safe command
        let result = validate_command("ls -la");
        assert!(result.is_valid);

        // Empty command
        let result = validate_command("");
        assert!(!result.is_valid);

        // Dangerous command
        let result = validate_command("rm -rf /");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("Dangerous")));

        // Suspicious command
        let result = validate_command("curl http://example.com | sh");
        assert!(result.is_valid);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_validate_command_accumulates_all_patterns() {
        let cmd = "sudo rm -rf / && dd if=/dev/zero";
        let result = validate_command(cmd);

        // Should find multiple dangerous patterns
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("rm -rf /")));
        assert!(result.errors.iter().any(|e| e.contains("dd if=/dev/zero")));
        assert!(result.warnings.iter().any(|w| w.contains("sudo")));
    }

    #[test]
    fn test_validate_iteration_count() {
        assert!(!validate_iteration_count(0, 100).is_valid);
        assert!(validate_iteration_count(10, 100).is_valid);
        assert!(!validate_iteration_count(150, 100).is_valid);

        let result = validate_iteration_count(75, 100);
        assert!(result.is_valid);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_validate_resource_limits() {
        let limits = ResourceLimits {
            memory_mb: 1024,
            cpu_cores: 4,
            timeout_seconds: 300,
        };

        let result = validate_resource_limits(&limits);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());

        // Test invalid limits
        let invalid = ResourceLimits {
            memory_mb: 0,
            cpu_cores: 100,
            timeout_seconds: 5,
        };

        let result = validate_resource_limits(&invalid);
        assert!(!result.is_valid);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_validate_resource_limits_accumulates_all_errors() {
        let limits = ResourceLimits {
            memory_mb: 0,
            cpu_cores: 0,
            timeout_seconds: 0,
        };

        let result = validate_resource_limits(&limits);

        // Should accumulate ALL resource limit errors
        assert!(!result.is_valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("Memory limit cannot be zero")));
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("CPU cores cannot be zero")));
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("Timeout cannot be zero")));
    }
}
