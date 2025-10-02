//! Spec implementation validation system
//!
//! Provides mechanisms to validate that specifications have been fully implemented
//! by checking for missing requirements and enabling automatic retry or gap-filling operations.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for spec validation
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationConfig {
    /// Shell command to run for validation (deprecated, use 'shell' instead)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Shell command to run for validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// Claude command to run for validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<String>,

    /// Array of commands to run for validation (supports multi-step validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<crate::config::WorkflowCommand>>,

    /// Expected JSON schema for validation output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_schema: Option<serde_json::Value>,

    /// Completion threshold percentage (default: 100)
    #[serde(default = "default_threshold")]
    pub threshold: f64,

    /// Timeout in seconds for validation command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    /// Configuration for handling incomplete implementations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_incomplete: Option<OnIncompleteConfig>,

    /// Optional file path to read validation results from (instead of stdout)
    /// If specified, the command should write JSON results to this file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_file: Option<String>,
}

impl<'de> serde::Deserialize<'de> for ValidationConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ValidationConfigHelper {
            // Array format: direct list of commands
            Array(Vec<crate::config::WorkflowCommand>),
            // Object format: struct with fields
            Object {
                #[serde(skip_serializing_if = "Option::is_none")]
                command: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                shell: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                claude: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                commands: Option<Vec<crate::config::WorkflowCommand>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                expected_schema: Option<serde_json::Value>,
                #[serde(default = "default_threshold")]
                threshold: f64,
                #[serde(skip_serializing_if = "Option::is_none")]
                timeout: Option<u64>,
                #[serde(skip_serializing_if = "Option::is_none")]
                on_incomplete: Box<Option<OnIncompleteConfig>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                result_file: Option<String>,
            },
        }

        let helper = ValidationConfigHelper::deserialize(deserializer)?;
        match helper {
            ValidationConfigHelper::Array(cmds) => Ok(ValidationConfig {
                command: None,
                shell: None,
                claude: None,
                commands: Some(cmds),
                expected_schema: None,
                threshold: default_threshold(),
                timeout: None,
                on_incomplete: None,
                result_file: None,
            }),
            ValidationConfigHelper::Object {
                command,
                shell,
                claude,
                commands,
                expected_schema,
                threshold,
                timeout,
                on_incomplete,
                result_file,
            } => Ok(ValidationConfig {
                command,
                shell,
                claude,
                commands,
                expected_schema,
                threshold,
                timeout,
                on_incomplete: *on_incomplete,
                result_file,
            }),
        }
    }
}

/// Configuration for handling incomplete implementations
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OnIncompleteConfig {
    /// Claude command to execute for gap filling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<String>,

    /// Shell command to execute for gap filling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// Array of commands to execute for gap filling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<crate::config::WorkflowCommand>>,

    /// Interactive prompt for user guidance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// Maximum number of attempts to complete
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    /// Whether to fail the workflow if completion fails
    #[serde(default = "default_fail_workflow")]
    pub fail_workflow: bool,

    /// Whether the completion command should create a commit
    #[serde(default)]
    pub commit_required: bool,
}

impl<'de> serde::Deserialize<'de> for OnIncompleteConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum OnIncompleteConfigHelper {
            // Array format: direct list of commands
            Array(Vec<crate::config::WorkflowCommand>),
            // Object format: struct with fields
            Object {
                #[serde(skip_serializing_if = "Option::is_none")]
                claude: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                shell: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                commands: Option<Vec<crate::config::WorkflowCommand>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                prompt: Option<String>,
                #[serde(default = "default_max_attempts")]
                max_attempts: u32,
                #[serde(default = "default_fail_workflow")]
                fail_workflow: bool,
                #[serde(default)]
                commit_required: bool,
            },
        }

        let helper = OnIncompleteConfigHelper::deserialize(deserializer)?;
        match helper {
            OnIncompleteConfigHelper::Array(cmds) => Ok(OnIncompleteConfig {
                claude: None,
                shell: None,
                commands: Some(cmds),
                prompt: None,
                max_attempts: default_max_attempts(),
                fail_workflow: default_fail_workflow(),
                commit_required: false,
            }),
            OnIncompleteConfigHelper::Object {
                claude,
                shell,
                commands,
                prompt,
                max_attempts,
                fail_workflow,
                commit_required,
            } => Ok(OnIncompleteConfig {
                claude,
                shell,
                commands,
                prompt,
                max_attempts,
                fail_workflow,
                commit_required,
            }),
        }
    }
}

/// Result of validation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Percentage of spec completed (0-100)
    pub completion_percentage: f64,

    /// Overall validation status
    pub status: ValidationStatus,

    /// List of implemented requirements
    #[serde(default)]
    pub implemented: Vec<String>,

    /// List of missing requirements
    #[serde(default)]
    pub missing: Vec<String>,

    /// Detailed gap information
    #[serde(default)]
    pub gaps: HashMap<String, GapDetail>,

    /// Raw output from validation command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<String>,
}

/// Detailed information about a gap in implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapDetail {
    /// Description of what's missing
    pub description: String,

    /// Location in code where gap exists
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,

    /// Severity of the gap
    pub severity: Severity,

    /// Suggested fix for the gap
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

/// Severity levels for gaps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// Status of validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    Complete,
    Incomplete,
    Failed,
    Skipped,
}

// Default functions for serde
fn default_threshold() -> f64 {
    100.0
}

fn default_max_attempts() -> u32 {
    2
}

fn default_fail_workflow() -> bool {
    true
}

impl ValidationConfig {
    /// Check if validation passed based on threshold
    pub fn is_complete(&self, result: &ValidationResult) -> bool {
        result.completion_percentage >= self.threshold
    }

    /// Validate that the configuration is properly formed
    pub fn validate(&self) -> Result<()> {
        // Handle backward compatibility: prefer 'shell' over deprecated 'command'
        let has_shell_cmd = self.shell.is_some() || self.command.is_some();

        // Must have either shell/command or claude
        if !has_shell_cmd && self.claude.is_none() {
            return Err(anyhow!(
                "Validation requires either shell/command or claude to be specified"
            ));
        }

        // Can't have both shell-type and claude commands
        if has_shell_cmd && self.claude.is_some() {
            return Err(anyhow!(
                "Cannot specify both shell/command and claude for validation"
            ));
        }

        // Can't have both shell and command (deprecated)
        if self.shell.is_some() && self.command.is_some() {
            return Err(anyhow!(
                "Cannot specify both 'shell' and 'command' (command is deprecated, use shell)"
            ));
        }

        if self.threshold < 0.0 || self.threshold > 100.0 {
            return Err(anyhow!("Threshold must be between 0 and 100"));
        }

        if let Some(on_incomplete) = &self.on_incomplete {
            on_incomplete.validate()?;
        }

        Ok(())
    }
}

impl OnIncompleteConfig {
    /// Validate that the configuration has required fields
    pub fn validate(&self) -> Result<()> {
        // Must have either a command or interactive prompt
        if self.claude.is_none() && self.shell.is_none() && self.prompt.is_none() {
            return Err(anyhow!(
                "OnIncomplete requires either claude, shell, or prompt to be specified"
            ));
        }

        if self.max_attempts == 0 {
            return Err(anyhow!("max_attempts must be greater than 0"));
        }

        Ok(())
    }

    /// Check if there's a command to execute
    pub fn has_command(&self) -> bool {
        self.claude.is_some() || self.shell.is_some()
    }
}

impl ValidationResult {
    /// Create a complete validation result
    pub fn complete() -> Self {
        Self {
            completion_percentage: 100.0,
            status: ValidationStatus::Complete,
            implemented: Vec::new(),
            missing: Vec::new(),
            gaps: HashMap::new(),
            raw_output: None,
        }
    }

    /// Create an incomplete validation result
    pub fn incomplete(
        percentage: f64,
        missing: Vec<String>,
        gaps: HashMap<String, GapDetail>,
    ) -> Self {
        Self {
            completion_percentage: percentage,
            status: ValidationStatus::Incomplete,
            implemented: Vec::new(),
            missing,
            gaps,
            raw_output: None,
        }
    }

    /// Create a failed validation result
    pub fn failed(error: String) -> Self {
        Self {
            completion_percentage: 0.0,
            status: ValidationStatus::Failed,
            implemented: Vec::new(),
            missing: vec![error],
            gaps: HashMap::new(),
            raw_output: None,
        }
    }

    /// Parse validation result from JSON string
    pub fn from_json(json_str: &str) -> Result<Self> {
        serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Failed to parse validation result: {}", e))
    }

    /// Convert to JSON for context variables
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| anyhow!("Failed to serialize validation result: {}", e))
    }

    /// Get a summary of gaps for context interpolation
    pub fn gaps_summary(&self) -> String {
        if self.gaps.is_empty() {
            return String::new();
        }

        let gap_list: Vec<String> = self
            .gaps
            .iter()
            .map(|(key, detail)| {
                format!(
                    "{}: {} ({})",
                    key,
                    detail.description,
                    format!("{:?}", detail.severity).to_lowercase()
                )
            })
            .collect();

        gap_list.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_config_defaults() {
        let yaml = r#"
claude: "/prodigy-validate-spec 01"
"#;
        let config: ValidationConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.claude, Some("/prodigy-validate-spec 01".to_string()));
        assert_eq!(config.threshold, 100.0);
        assert!(config.on_incomplete.is_none());
    }

    #[test]
    fn test_validation_config_with_on_incomplete() {
        let yaml = r#"
command: "cargo test"
threshold: 90
on_incomplete:
  claude: "/prodigy-fix-tests ${validation.gaps}"
  max_attempts: 3
  fail_workflow: false
  commit_required: false
"#;
        let config: ValidationConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.command, Some("cargo test".to_string()));
        assert_eq!(config.threshold, 90.0);

        let on_incomplete = config.on_incomplete.unwrap();
        assert_eq!(
            on_incomplete.claude,
            Some("/prodigy-fix-tests ${validation.gaps}".to_string())
        );
        assert_eq!(on_incomplete.max_attempts, 3);
        assert!(!on_incomplete.fail_workflow);
    }

    #[test]
    fn test_validation_result_serialization() {
        let mut gaps = HashMap::new();
        gaps.insert(
            "auth".to_string(),
            GapDetail {
                description: "Authentication not implemented".to_string(),
                location: Some("src/auth.rs".to_string()),
                severity: Severity::Critical,
                suggested_fix: Some("Implement JWT validation".to_string()),
            },
        );

        let result = ValidationResult {
            completion_percentage: 75.0,
            status: ValidationStatus::Incomplete,
            implemented: vec!["Database schema".to_string()],
            missing: vec!["Authentication".to_string()],
            gaps,
            raw_output: None,
        };

        let json = result.to_json().unwrap();
        let parsed: ValidationResult = ValidationResult::from_json(&json).unwrap();

        assert_eq!(parsed.completion_percentage, 75.0);
        assert_eq!(parsed.status, ValidationStatus::Incomplete);
        assert_eq!(parsed.implemented.len(), 1);
        assert_eq!(parsed.missing.len(), 1);
        assert_eq!(parsed.gaps.len(), 1);
    }

    #[test]
    fn test_validation_config_validation() {
        let mut config = ValidationConfig {
            command: None,
            shell: None,
            claude: None,
            commands: None,
            expected_schema: None,
            threshold: 100.0,
            timeout: None,
            on_incomplete: None,
            result_file: None,
        };

        // No command or claude should fail
        assert!(config.validate().is_err());

        // Fix command
        config.command = Some("/prodigy-validate".to_string());
        assert!(config.validate().is_ok());

        // Invalid threshold
        config.threshold = 150.0;
        assert!(config.validate().is_err());

        // Valid threshold
        config.threshold = 95.0;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_config_shell_field() {
        // Test the new shell field and backward compatibility with command
        let mut config = ValidationConfig {
            command: None,
            shell: None,
            claude: None,
            commands: None,
            expected_schema: None,
            threshold: 95.0,
            timeout: None,
            on_incomplete: None,
            result_file: None,
        };

        // Shell field should work
        config.shell = Some("bash -c 'echo test'".to_string());
        assert!(config.validate().is_ok());

        // Can't have both shell and command
        config.command = Some("echo old".to_string());
        assert!(config.validate().is_err());

        // Command alone (backward compat)
        config.shell = None;
        assert!(config.validate().is_ok());

        // Can't have shell/command with claude
        config.shell = Some("echo test".to_string());
        config.command = None;
        config.claude = Some("/claude-cmd".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_on_incomplete_validation() {
        let mut config = OnIncompleteConfig {
            claude: None,
            shell: None,
            commands: None,
            prompt: None,
            max_attempts: 2,
            fail_workflow: true,
            commit_required: false,
        };

        // No command or prompt should fail
        assert!(config.validate().is_err());

        // Add claude command
        config.claude = Some("/prodigy-fix".to_string());
        assert!(config.validate().is_ok());

        // Test with shell command
        config.claude = None;
        config.shell = Some("echo test".to_string());
        assert!(config.validate().is_ok());

        // Test with prompt
        config.shell = None;
        config.prompt = Some("Continue?".to_string());
        assert!(config.validate().is_ok());

        // Zero max_attempts should fail
        config.max_attempts = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_result_helpers() {
        let result = ValidationResult::complete();
        assert_eq!(result.completion_percentage, 100.0);
        assert_eq!(result.status, ValidationStatus::Complete);

        let mut gaps = HashMap::new();
        gaps.insert(
            "tests".to_string(),
            GapDetail {
                description: "Missing unit tests".to_string(),
                location: None,
                severity: Severity::High,
                suggested_fix: None,
            },
        );

        let incomplete = ValidationResult::incomplete(60.0, vec!["Unit tests".to_string()], gaps);
        assert_eq!(incomplete.completion_percentage, 60.0);
        assert_eq!(incomplete.status, ValidationStatus::Incomplete);

        let failed = ValidationResult::failed("Command not found".to_string());
        assert_eq!(failed.completion_percentage, 0.0);
        assert_eq!(failed.status, ValidationStatus::Failed);
    }

    #[test]
    fn test_gaps_summary() {
        let mut gaps = HashMap::new();
        gaps.insert(
            "auth".to_string(),
            GapDetail {
                description: "Missing authentication".to_string(),
                location: None,
                severity: Severity::Critical,
                suggested_fix: None,
            },
        );
        gaps.insert(
            "tests".to_string(),
            GapDetail {
                description: "No test coverage".to_string(),
                location: None,
                severity: Severity::High,
                suggested_fix: None,
            },
        );

        let result = ValidationResult::incomplete(50.0, vec![], gaps);
        let summary = result.gaps_summary();

        // Should contain both gaps
        assert!(summary.contains("Missing authentication"));
        assert!(summary.contains("No test coverage"));
        assert!(summary.contains("critical"));
        assert!(summary.contains("high"));
    }

    #[test]
    fn test_validation_config_array_format() {
        // Test parsing ValidationConfig with array of commands
        let yaml = r#"
- shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-after.json --format json"
- shell: "debtmap compare --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json --output .prodigy/comparison.json --format json"
- claude: "/prodigy-validate-debtmap-improvement --comparison .prodigy/comparison.json --output .prodigy/debtmap-validation.json"
  result_file: ".prodigy/debtmap-validation.json"
  threshold: 75
"#;

        let config: ValidationConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse validation config array");

        // Should parse as commands array
        assert!(config.commands.is_some());
        let commands = config.commands.unwrap();
        assert_eq!(commands.len(), 3);
        // Threshold defaults to 100.0 when parsing as array
        // (the threshold: 75 in the YAML is inside a command, not at the ValidationConfig level)
        assert_eq!(config.threshold, 100.0);
    }

    #[test]
    fn test_on_incomplete_array_format() {
        // Test parsing OnIncompleteConfig with array of commands
        let yaml = r#"
- claude: "/prodigy-complete-debtmap-fix --gaps ${validation.gaps}"
  commit_required: true
- shell: "just coverage-lcov"
- shell: "debtmap analyze . --output .prodigy/debtmap-after.json"
"#;

        let config: OnIncompleteConfig =
            serde_yaml::from_str(yaml).expect("Failed to parse on_incomplete config array");

        assert!(config.commands.is_some());
        let commands = config.commands.unwrap();
        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn test_nested_validation_with_arrays() {
        // Test parsing validation as array
        let yaml = r#"
validate:
  - shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-after.json --format json"
  - shell: "debtmap compare --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json --output .prodigy/comparison.json --format json"
  - claude: "/prodigy-validate-debtmap-improvement --comparison .prodigy/comparison.json --output .prodigy/debtmap-validation.json"
"#;

        #[derive(serde::Deserialize)]
        struct TestStruct {
            validate: ValidationConfig,
        }

        let result: TestStruct =
            serde_yaml::from_str(yaml).expect("Failed to parse nested validation config");

        // Validate outer config has commands
        assert!(result.validate.commands.is_some());
        let commands = result.validate.commands.unwrap();
        assert_eq!(commands.len(), 3);

        // When parsed as array, threshold and on_incomplete come from defaults
        assert_eq!(result.validate.threshold, 100.0); // default threshold
        assert!(result.validate.on_incomplete.is_none()); // no on_incomplete at this level
    }

    #[test]
    fn test_parse_actual_debtmap_workflow() {
        // Test parsing the actual debtmap.yml file structure
        let yaml = r#"
- shell: "just coverage-lcov"
- shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-before.json --format json"
- claude: "/prodigy-debtmap-plan --before .prodigy/debtmap-before.json --output .prodigy/IMPLEMENTATION_PLAN.md"
  capture_output: true
  validate:
    - claude: "/prodigy-validate-debtmap-plan --before .prodigy/debtmap-before.json --plan .prodigy/IMPLEMENTATION_PLAN.md --output .prodigy/plan-validation.json"
      result_file: ".prodigy/plan-validation.json"
      threshold: 75
      on_incomplete:
        - claude: "/prodigy-revise-debtmap-plan --gaps ${validation.gaps} --plan .prodigy/IMPLEMENTATION_PLAN.md"
          max_attempts: 3
          fail_workflow: false
"#;

        let result: Result<Vec<crate::config::WorkflowCommand>, _> = serde_yaml::from_str(yaml);
        match result {
            Ok(cmds) => {
                assert_eq!(cmds.len(), 3);
            }
            Err(e) => {
                panic!("Failed to parse workflow: {}", e);
            }
        }
    }
}
