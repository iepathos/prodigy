//! Step validation system for workflow execution
//!
//! Provides first-class validation support for workflow steps, enabling
//! verification of command success through custom validation commands
//! that run after the main command executes.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info};

/// Step validation specification - can be a single command or multiple
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum StepValidationSpec {
    /// Single validation command as string
    Single(String),
    /// Multiple validation commands
    Multiple(Vec<String>),
    /// Detailed validation configuration
    Detailed(StepValidationConfig),
}

/// Detailed validation configuration with advanced options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepValidationConfig {
    /// Validation commands to execute
    pub commands: Vec<ValidationCommand>,

    /// Success criteria for validation
    #[serde(default)]
    pub success_criteria: SuccessCriteria,

    /// Maximum validation attempts
    #[serde(default = "default_validation_attempts")]
    pub max_attempts: u32,

    /// Delay between validation attempts in seconds
    #[serde(default = "default_retry_delay")]
    pub retry_delay: u64,
}

/// Individual validation command with expectations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationCommand {
    /// The command to execute (shell or claude)
    pub command: String,

    /// Expected output pattern (regex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_output: Option<String>,

    /// Expected exit code (default: 0)
    #[serde(default)]
    pub expect_exit_code: i32,

    /// Command type hint (auto-detected if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_type: Option<ValidationCommandType>,
}

/// Type of validation command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationCommandType {
    Shell,
    Claude,
}

/// Success criteria for validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SuccessCriteria {
    /// All validations must pass
    #[default]
    All,
    /// At least one validation must pass
    Any,
    /// Specific number of validations must pass
    AtLeast(usize),
}

/// Result from a single validation command
#[derive(Debug, Clone, Serialize)]
pub struct SingleValidationResult {
    /// Whether the validation passed
    pub passed: bool,
    /// Descriptive message about the result
    pub message: String,
    /// Command output (stdout)
    pub output: String,
    /// Command exit code
    pub exit_code: i32,
    /// Duration of execution
    pub duration: Duration,
}

/// Overall validation result for a step
#[derive(Debug, Clone, Serialize)]
pub struct StepValidationResult {
    /// Whether validation passed overall
    pub passed: bool,
    /// Individual validation results
    pub results: Vec<SingleValidationResult>,
    /// Total duration of all validations
    pub duration: Duration,
    /// Number of attempts made
    pub attempts: u32,
}

impl StepValidationResult {
    /// Create a skipped validation result
    pub fn skipped() -> Self {
        Self {
            passed: true,
            results: Vec::new(),
            duration: Duration::from_secs(0),
            attempts: 0,
        }
    }
}

fn default_validation_attempts() -> u32 {
    1
}

fn default_retry_delay() -> u64 {
    5
}

impl StepValidationSpec {
    /// Parse the validation spec into a list of validation commands
    pub fn to_validation_commands(&self) -> Result<Vec<ValidationCommand>> {
        match self {
            StepValidationSpec::Single(cmd) => Ok(vec![Self::parse_command_string(cmd)?]),
            StepValidationSpec::Multiple(cmds) => cmds
                .iter()
                .map(|cmd| Self::parse_command_string(cmd))
                .collect(),
            StepValidationSpec::Detailed(config) => Ok(config.commands.clone()),
        }
    }

    /// Parse a command string into a ValidationCommand
    fn parse_command_string(cmd: &str) -> Result<ValidationCommand> {
        // Detect command type from prefix
        let (command_type, actual_command) = if cmd.starts_with("claude:") {
            (
                Some(ValidationCommandType::Claude),
                cmd.strip_prefix("claude:").unwrap().trim(),
            )
        } else if cmd.starts_with("shell:") {
            (
                Some(ValidationCommandType::Shell),
                cmd.strip_prefix("shell:").unwrap().trim(),
            )
        } else {
            // Default to shell for backward compatibility
            (Some(ValidationCommandType::Shell), cmd)
        };

        Ok(ValidationCommand {
            command: actual_command.to_string(),
            expect_output: None,
            expect_exit_code: 0,
            command_type,
        })
    }

    /// Get the maximum attempts for validation
    pub fn max_attempts(&self) -> u32 {
        match self {
            StepValidationSpec::Detailed(config) => config.max_attempts,
            _ => 1,
        }
    }

    /// Get the retry delay for validation
    pub fn retry_delay(&self) -> Duration {
        match self {
            StepValidationSpec::Detailed(config) => Duration::from_secs(config.retry_delay),
            _ => Duration::from_secs(5),
        }
    }

    /// Get the success criteria
    pub fn success_criteria(&self) -> SuccessCriteria {
        match self {
            StepValidationSpec::Detailed(config) => config.success_criteria.clone(),
            _ => SuccessCriteria::All,
        }
    }
}

/// Executor for step validation
pub struct StepValidationExecutor {
    /// Command executor for running validation commands
    command_executor: Arc<dyn crate::cook::execution::CommandExecutor>,
    /// Metrics collector for validation
    metrics: Arc<ValidationMetrics>,
}

impl StepValidationExecutor {
    /// Create a new validation executor
    pub fn new(command_executor: Arc<dyn crate::cook::execution::CommandExecutor>) -> Self {
        Self {
            command_executor,
            metrics: Arc::new(ValidationMetrics::new()),
        }
    }

    /// Validate a workflow step
    pub async fn validate_step(
        &self,
        validation_spec: &StepValidationSpec,
        context: &crate::cook::execution::ExecutionContext,
        step_name: &str,
    ) -> Result<StepValidationResult> {
        let commands = validation_spec.to_validation_commands()?;
        let max_attempts = validation_spec.max_attempts();
        let retry_delay = validation_spec.retry_delay();
        let success_criteria = validation_spec.success_criteria();

        let mut attempt = 0;
        let start_time = Instant::now();

        loop {
            attempt += 1;
            info!(
                "Running validation attempt {}/{} for step '{}'",
                attempt, max_attempts, step_name
            );

            let mut results = Vec::new();
            let mut passed_count = 0;

            for (idx, cmd) in commands.iter().enumerate() {
                info!(
                    "Executing validation {}/{}: {}",
                    idx + 1,
                    commands.len(),
                    cmd.command
                );

                let result = self.execute_validation_command(cmd, context).await?;

                if result.passed {
                    passed_count += 1;
                } else {
                    error!("Validation {} failed: {}", idx + 1, result.message);
                }

                results.push(result);

                // For "Any" criteria, we can stop early if one passes
                if matches!(success_criteria, SuccessCriteria::Any) && passed_count > 0 {
                    break;
                }
            }

            // Check if validation passed based on criteria
            let passed = match &success_criteria {
                SuccessCriteria::All => passed_count == commands.len(),
                SuccessCriteria::Any => passed_count > 0,
                SuccessCriteria::AtLeast(n) => passed_count >= *n,
            };

            if passed || attempt >= max_attempts {
                // Record metrics
                self.metrics
                    .record_validation(step_name, passed, start_time.elapsed());

                return Ok(StepValidationResult {
                    passed,
                    results,
                    duration: start_time.elapsed(),
                    attempts: attempt,
                });
            }

            // Wait before retry
            if attempt < max_attempts {
                info!(
                    "Validation failed, retrying in {} seconds...",
                    retry_delay.as_secs()
                );
                tokio::time::sleep(retry_delay).await;
            }
        }
    }

    /// Execute a single validation command
    async fn execute_validation_command(
        &self,
        cmd: &ValidationCommand,
        context: &crate::cook::execution::ExecutionContext,
    ) -> Result<SingleValidationResult> {
        let start_time = Instant::now();

        // Prepare the command based on type
        let (command_type, command_string) = match &cmd.command_type {
            Some(ValidationCommandType::Claude) => ("claude", cmd.command.clone()),
            Some(ValidationCommandType::Shell) | None => ("shell", cmd.command.clone()),
        };

        // Execute the command
        let result = self
            .command_executor
            .execute(
                command_type,
                std::slice::from_ref(&command_string),
                context.clone(),
            )
            .await
            .context("Failed to execute validation command")?;

        // Check exit code
        let exit_code = result.exit_code.unwrap_or(-1);
        let mut passed = exit_code == cmd.expect_exit_code;

        // Check expected output if specified
        if passed && cmd.expect_output.is_some() {
            let expected_pattern = cmd.expect_output.as_ref().unwrap();
            let regex = regex::Regex::new(expected_pattern)
                .context("Invalid regex pattern in expect_output")?;

            if !regex.is_match(&result.stdout) {
                passed = false;
            }
        }

        let message = if passed {
            format!("Validation passed (exit code: {})", exit_code)
        } else if exit_code != cmd.expect_exit_code {
            format!(
                "Expected exit code {}, got {}",
                cmd.expect_exit_code, exit_code
            )
        } else {
            "Output did not match expected pattern".to_string()
        };

        Ok(SingleValidationResult {
            passed,
            message,
            output: result.stdout.clone(),
            exit_code,
            duration: start_time.elapsed(),
        })
    }
}

/// Metrics collector for validation
pub struct ValidationMetrics {
    validations: std::sync::Mutex<HashMap<String, ValidationMetric>>,
}

#[derive(Debug, Clone)]
struct ValidationMetric {
    total_runs: u64,
    successful_runs: u64,
    total_duration: Duration,
}

impl ValidationMetrics {
    fn new() -> Self {
        Self {
            validations: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn record_validation(&self, step_name: &str, success: bool, duration: Duration) {
        let mut validations = self.validations.lock().unwrap();
        let metric = validations
            .entry(step_name.to_string())
            .or_insert(ValidationMetric {
                total_runs: 0,
                successful_runs: 0,
                total_duration: Duration::from_secs(0),
            });

        metric.total_runs += 1;
        if success {
            metric.successful_runs += 1;
        }
        metric.total_duration += duration;
    }

    /// Get validation metrics summary
    pub fn get_summary(&self) -> HashMap<String, (u64, u64, Duration)> {
        let validations = self.validations.lock().unwrap();
        validations
            .iter()
            .map(|(name, metric)| {
                (
                    name.clone(),
                    (
                        metric.total_runs,
                        metric.successful_runs,
                        metric.total_duration,
                    ),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_validation() {
        let spec = StepValidationSpec::Single("cargo test".to_string());
        let commands = spec.to_validation_commands().unwrap();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "cargo test");
        assert_eq!(commands[0].expect_exit_code, 0);
    }

    #[test]
    fn test_parse_multiple_validations() {
        let spec = StepValidationSpec::Multiple(vec![
            "cargo test".to_string(),
            "cargo clippy".to_string(),
        ]);
        let commands = spec.to_validation_commands().unwrap();

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].command, "cargo test");
        assert_eq!(commands[1].command, "cargo clippy");
    }

    #[test]
    fn test_parse_claude_validation() {
        let spec = StepValidationSpec::Single("claude: /check-quality".to_string());
        let commands = spec.to_validation_commands().unwrap();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "/check-quality");
        assert_eq!(
            commands[0].command_type,
            Some(ValidationCommandType::Claude)
        );
    }

    #[test]
    fn test_detailed_validation_config() {
        let config = StepValidationConfig {
            commands: vec![ValidationCommand {
                command: "test.sh".to_string(),
                expect_output: Some("SUCCESS".to_string()),
                expect_exit_code: 0,
                command_type: Some(ValidationCommandType::Shell),
            }],
            success_criteria: SuccessCriteria::All,
            max_attempts: 3,
            retry_delay: 10,
        };

        let spec = StepValidationSpec::Detailed(config);
        assert_eq!(spec.max_attempts(), 3);
        assert_eq!(spec.retry_delay(), Duration::from_secs(10));
    }

    #[test]
    fn test_success_criteria() {
        // Test All criteria
        let all = SuccessCriteria::All;
        assert!(matches!(all, SuccessCriteria::All));

        // Test Any criteria
        let any = SuccessCriteria::Any;
        assert!(matches!(any, SuccessCriteria::Any));

        // Test AtLeast criteria
        let at_least = SuccessCriteria::AtLeast(2);
        if let SuccessCriteria::AtLeast(n) = at_least {
            assert_eq!(n, 2);
        } else {
            panic!("Expected AtLeast");
        }
    }
}
