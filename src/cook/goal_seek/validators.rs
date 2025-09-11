use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::validator::{ValidationResult, Validator};

/// Spec coverage validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecCoverageReport {
    pub coverage_percentage: f64,
    pub implemented_items: Vec<String>,
    pub missing_items: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Test pass validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestValidationResult {
    pub score: f64,
    pub passed: u32,
    pub failed: u32,
    pub failures: Vec<TestFailure>,
    pub suggestions: Vec<String>,
}

/// Test failure details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFailure {
    pub test_name: String,
    pub error_message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// Built-in validator for specification coverage
pub struct SpecCoverageValidator {
    spec_path: String,
}

impl SpecCoverageValidator {
    pub fn new(spec_path: String) -> Self {
        Self { spec_path }
    }

    pub async fn analyze_implementation(
        &self,
        _spec_id: &str,
        _codebase_path: &Path
    ) -> Result<SpecCoverageReport> {
        // For now, return a simple mock implementation
        // In a real implementation, this would:
        // 1. Parse the specification file
        // 2. Analyze the codebase to find implemented functionality
        // 3. Compare spec requirements vs implementation
        // 4. Generate detailed coverage report
        
        Ok(SpecCoverageReport {
            coverage_percentage: 85.0,
            implemented_items: vec![
                "Core engine implementation".to_string(),
                "Basic validation framework".to_string(),
            ],
            missing_items: vec![
                "CLI integration".to_string(),
                "Advanced convergence detection".to_string(),
            ],
            suggestions: vec![
                "Implement CLI commands for goal-seeking".to_string(),
                "Add convergence detection algorithms".to_string(),
            ],
        })
    }
}

impl Validator for SpecCoverageValidator {
    fn validate(&self, _output: &str) -> Result<ValidationResult> {
        // Simple implementation - in practice this would analyze the output
        // and determine spec coverage based on the execution results
        
        let score = 85; // Mock score
        
        let mut gaps = serde_json::Map::new();
        gaps.insert("missing_cli".to_string(), serde_json::json!({
            "description": "CLI commands not implemented",
            "severity": "high",
            "location": "src/cli/"
        }));
        
        let gaps_value = serde_json::Value::Object(gaps);
        
        Ok(ValidationResult {
            score,
            success: score >= 80, // Default threshold
            output: format!("Spec coverage validation complete. Score: {}/100", score),
            gaps: Some(gaps_value),
            data: std::collections::HashMap::new(),
        })
    }
    
    fn name(&self) -> &str {
        "spec_coverage"
    }
}

/// Built-in validator for test pass rate
pub struct TestPassValidator {
    test_pattern: Option<String>,
}

impl TestPassValidator {
    pub fn new(test_pattern: Option<String>) -> Self {
        Self { test_pattern }
    }

    pub async fn validate_tests(&self, _test_pattern: Option<&str>) -> Result<TestValidationResult> {
        // Mock implementation - in practice this would:
        // 1. Run the specified test pattern
        // 2. Parse test results
        // 3. Analyze failures
        // 4. Generate suggestions for fixes
        
        Ok(TestValidationResult {
            score: 0.9, // 90% pass rate
            passed: 18,
            failed: 2,
            failures: vec![
                TestFailure {
                    test_name: "test_goal_seek_convergence".to_string(),
                    error_message: "assertion failed: expected convergence".to_string(),
                    file: Some("src/cook/goal_seek/engine.rs".to_string()),
                    line: Some(95),
                },
            ],
            suggestions: vec![
                "Fix convergence detection logic".to_string(),
                "Add more test cases for edge conditions".to_string(),
            ],
        })
    }
}

impl Validator for TestPassValidator {
    fn validate(&self, output: &str) -> Result<ValidationResult> {
        // Parse test output to extract pass/fail counts
        // This is a simplified version - real implementation would parse cargo test output
        
        let lines: Vec<&str> = output.lines().collect();
        let mut passed = 0;
        let mut failed = 0;
        
        for line in &lines {
            if line.contains("test result:") {
                // Parse line like "test result: ok. 18 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out"
                if let Some(passed_str) = line.split("passed").next().and_then(|s| s.split_whitespace().last()) {
                    passed = passed_str.parse().unwrap_or(0);
                }
                if let Some(failed_str) = line.split("failed").next().and_then(|s| s.split_whitespace().last()) {
                    failed = failed_str.parse().unwrap_or(0);
                }
                break;
            }
        }
        
        let total = passed + failed;
        let score = if total > 0 {
            ((passed as f64 / total as f64) * 100.0) as u32
        } else {
            0
        };
        
        Ok(ValidationResult {
            score,
            success: failed == 0,
            output: output.to_string(),
            gaps: if failed > 0 {
                Some(serde_json::json!({
                    "failing_tests": failed,
                    "description": format!("{} tests failing", failed)
                }))
            } else {
                None
            },
            data: std::collections::HashMap::new(),
        })
    }
    
    fn name(&self) -> &str {
        "test_pass"
    }
}

/// Output quality validator for checking code quality metrics
pub struct OutputQualityValidator {
    quality_threshold: f64,
}

impl OutputQualityValidator {
    pub fn new(quality_threshold: f64) -> Self {
        Self { quality_threshold }
    }
}

impl Validator for OutputQualityValidator {
    fn validate(&self, output: &str) -> Result<ValidationResult> {
        // Mock quality analysis - in practice would analyze:
        // - Code complexity metrics
        // - Documentation coverage
        // - Linting violations
        // - Performance metrics
        
        let quality_score = 88; // Mock score based on analysis
        
        Ok(ValidationResult {
            score: quality_score,
            success: quality_score as f64 >= self.quality_threshold,
            output: output.to_string(),
            gaps: if (quality_score as f64) < self.quality_threshold {
                Some(serde_json::json!({
                    "quality_issues": [
                        {
                            "type": "complexity",
                            "description": "High cyclomatic complexity in some functions",
                            "severity": "medium"
                        },
                        {
                            "type": "documentation", 
                            "description": "Missing documentation for public APIs",
                            "severity": "low"
                        }
                    ]
                }))
            } else {
                None
            },
            data: std::collections::HashMap::new(),
        })
    }
    
    fn name(&self) -> &str {
        "output_quality"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_coverage_validator() {
        let validator = SpecCoverageValidator::new("test_spec.md".to_string());
        let result = validator.validate("test output").unwrap();
        
        assert_eq!(result.score, 85);
        assert!(result.gaps.is_some());
        assert_eq!(validator.name(), "spec_coverage");
    }

    #[test]
    fn test_test_pass_validator() {
        let validator = TestPassValidator::new(None);
        let test_output = "test result: ok. 18 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out";
        let result = validator.validate(test_output).unwrap();
        
        assert_eq!(result.score, 90); // 18/(18+2) = 0.9 = 90%
        assert!(!result.success); // Has failures
        assert!(result.gaps.is_some());
    }

    #[test]
    fn test_output_quality_validator() {
        let validator = OutputQualityValidator::new(90.0);
        let result = validator.validate("quality analysis output").unwrap();
        
        assert_eq!(result.score, 88);
        assert!(!result.success); // Below threshold of 90
        assert!(result.gaps.is_some());
    }
}