//! Pure validation functions
//!
//! These functions validate data structures and business rules without performing any I/O operations.

use std::collections::HashMap;
use std::path::Path;

/// File existence check function type (passed as parameter to avoid I/O)
pub type FileExistsCheck = fn(&Path) -> bool;

/// Validation result with errors and warnings
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
}

/// Validate file paths without performing I/O
pub fn validate_paths(paths: &[&Path], exists_check: FileExistsCheck) -> ValidationResult {
    let mut result = ValidationResult::valid();

    for path in paths {
        if !exists_check(path) {
            result.add_error(format!("Path does not exist: {}", path.display()));
        }

        // Check for suspicious paths
        if path.starts_with("..") {
            result.add_warning(format!("Path uses parent directory: {}", path.display()));
        }

        if path.is_absolute() && path.starts_with("/tmp") {
            result.add_warning(format!("Path in temporary directory: {}", path.display()));
        }
    }

    result
}

/// Validate environment variables
pub fn validate_environment(
    required_vars: &[&str],
    env_vars: &HashMap<String, String>,
) -> ValidationResult {
    let mut result = ValidationResult::valid();

    for var in required_vars {
        if !env_vars.contains_key(*var) {
            result.add_error(format!("Missing required environment variable: {}", var));
        }
    }

    // Check for sensitive variables that shouldn't be empty
    for (key, value) in env_vars {
        if (key.contains("KEY") || key.contains("SECRET") || key.contains("TOKEN"))
            && value.trim().is_empty()
        {
            result.add_warning(format!("Sensitive variable '{}' is empty", key));
        }
    }

    result
}

/// Validate JSON structure
pub fn validate_json_schema(
    json: &serde_json::Value,
    required_fields: &[&str],
) -> ValidationResult {
    let mut result = ValidationResult::valid();

    if let Some(obj) = json.as_object() {
        for field in required_fields {
            if !obj.contains_key(*field) {
                result.add_error(format!("Missing required field: {}", field));
            }
        }

        // Check for null values in non-optional fields
        for (key, value) in obj {
            if value.is_null() && !key.contains("optional") {
                result.add_warning(format!("Field '{}' is null", key));
            }
        }
    } else {
        result.add_error("JSON is not an object".to_string());
    }

    result
}

/// Validate command string
pub fn validate_command(command: &str) -> ValidationResult {
    let mut result = ValidationResult::valid();

    if command.trim().is_empty() {
        result.add_error("Command is empty".to_string());
        return result;
    }

    // Check for dangerous commands
    let dangerous_patterns = [
        "rm -rf /",
        "dd if=/dev/zero",
        ":(){ :|:& };:", // Fork bomb
        "> /dev/sda",
        "chmod -R 777 /",
    ];

    for pattern in &dangerous_patterns {
        if command.contains(pattern) {
            result.add_error(format!("Dangerous command pattern detected: {}", pattern));
        }
    }

    // Check for suspicious patterns
    if command.contains("sudo") && !command.contains("sudo -n") {
        result.add_warning("Command uses sudo which may require password".to_string());
    }

    if command.contains("curl") && command.contains("| sh") {
        result
            .add_warning("Command pipes curl output to shell, potential security risk".to_string());
    }

    result
}

/// Validate iteration count
pub fn validate_iteration_count(count: usize, max_allowed: usize) -> ValidationResult {
    let mut result = ValidationResult::valid();

    if count == 0 {
        result.add_error("Iteration count cannot be zero".to_string());
    } else if count > max_allowed {
        result.add_error(format!(
            "Iteration count {} exceeds maximum allowed {}",
            count, max_allowed
        ));
    } else if count > 50 {
        result.add_warning(format!(
            "High iteration count ({}) may take a long time",
            count
        ));
    }

    result
}

/// Validate resource limits
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub memory_mb: usize,
    pub cpu_cores: usize,
    pub timeout_seconds: usize,
}

pub fn validate_resource_limits(limits: &ResourceLimits) -> ValidationResult {
    let mut result = ValidationResult::valid();

    // Memory validation
    if limits.memory_mb == 0 {
        result.add_error("Memory limit cannot be zero".to_string());
    } else if limits.memory_mb < 128 {
        result.add_warning("Memory limit may be too low for normal operation".to_string());
    } else if limits.memory_mb > 32768 {
        // 32GB
        result.add_warning("Memory limit may exceed available system memory".to_string());
    }

    // CPU validation
    if limits.cpu_cores == 0 {
        result.add_error("CPU cores cannot be zero".to_string());
    } else if limits.cpu_cores > 64 {
        result.add_warning("CPU core count may exceed available cores".to_string());
    }

    // Timeout validation
    if limits.timeout_seconds == 0 {
        result.add_error("Timeout cannot be zero".to_string());
    } else if limits.timeout_seconds < 10 {
        result.add_warning("Timeout may be too short for operations to complete".to_string());
    } else if limits.timeout_seconds > 3600 {
        // 1 hour
        result.add_warning("Long timeout may cause hanging processes".to_string());
    }

    result
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
}
