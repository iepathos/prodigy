//! Command handler for /prodigy-validate-debtmap-improvement
//!
//! This handler validates technical debt improvements by comparing debtmap
//! JSON output before and after changes.

use crate::commands::{
    AttributeSchema, AttributeValue, CommandHandler, CommandResult, ExecutionContext,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

mod validation;
pub use validation::*;

/// Handler for validating debtmap improvements
pub struct ValidateDebtmapHandler;

impl ValidateDebtmapHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ValidateDebtmapHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for validation execution
struct ExecutionConfig {
    before_path: PathBuf,
    after_path: PathBuf,
    output_path: PathBuf,
    is_automation: bool,
}

impl ValidateDebtmapHandler {
    /// Parse execution configuration from attributes
    fn parse_execution_config(
        context: &ExecutionContext,
        attributes: &HashMap<String, AttributeValue>,
    ) -> Result<ExecutionConfig, String> {
        let before_path = attributes
            .get("before")
            .and_then(|v| v.as_string())
            .ok_or_else(|| "Missing required argument: --before".to_string())
            .map(|path| context.resolve_path(path.as_ref()))?;

        let after_path = attributes
            .get("after")
            .and_then(|v| v.as_string())
            .ok_or_else(|| "Missing required argument: --after".to_string())
            .map(|path| context.resolve_path(path.as_ref()))?;

        let output_path = attributes
            .get("output")
            .and_then(|v| v.as_string())
            .map(|s| context.resolve_path(s.as_ref()))
            .unwrap_or_else(|| {
                context.resolve_path(&PathBuf::from(".prodigy/debtmap-validation.json"))
            });

        let is_automation = Self::check_automation_mode();

        Ok(ExecutionConfig {
            before_path,
            after_path,
            output_path,
            is_automation,
        })
    }

    /// Check if running in automation mode
    fn check_automation_mode() -> bool {
        let check_var = |var: &str| std::env::var(var).unwrap_or_default().to_lowercase() == "true";
        check_var("PRODIGY_AUTOMATION") || check_var("PRODIGY_VALIDATION")
    }

    /// Load both debtmaps with error handling
    fn load_debtmaps(config: &ExecutionConfig) -> Result<(DebtmapOutput, DebtmapOutput), String> {
        Self::print_if_interactive(config, || "Loading debtmap files...".to_string());

        let before = load_debtmap(&config.before_path)
            .map_err(|e| format!("Failed to load before debtmap: {}", e))?;

        let after = load_debtmap(&config.after_path)
            .map_err(|e| format!("Failed to load after debtmap: {}", e))?;

        Ok((before, after))
    }

    /// Print message only if not in automation mode
    fn print_if_interactive<F>(config: &ExecutionConfig, message_fn: F)
    where
        F: FnOnce() -> String,
    {
        if !config.is_automation {
            println!("{}", message_fn());
        }
    }

    /// Create and write error result JSON
    fn write_error_result(output_path: &PathBuf, error_msg: &str) {
        let error_result = json!({
            "completion_percentage": 0.0,
            "status": "failed",
            "improvements": [],
            "remaining_issues": [error_msg],
            "gaps": {},
            "before_summary": {
                "total_items": 0,
                "high_priority_items": 0,
                "average_score": 0.0
            },
            "after_summary": {
                "total_items": 0,
                "high_priority_items": 0,
                "average_score": 0.0
            }
        });

        if let Err(e) = write_validation_result(output_path, &error_result) {
            eprintln!("Failed to write validation result: {}", e);
        }
    }

    /// Output validation result to file and console
    fn output_validation_result(
        config: &ExecutionConfig,
        validation_result: &ValidationResult,
    ) -> CommandResult {
        let result_json = match serde_json::to_value(validation_result) {
            Ok(json) => json,
            Err(e) => {
                return CommandResult::error(format!(
                    "Failed to serialize validation result: {}",
                    e
                ))
            }
        };

        if let Err(e) = write_validation_result(&config.output_path, &result_json) {
            return CommandResult::error(format!("Failed to write validation result: {}", e));
        }

        Self::print_summary(config, validation_result);
        CommandResult::success(result_json)
    }

    /// Print validation summary if interactive
    fn print_summary(config: &ExecutionConfig, validation_result: &ValidationResult) {
        if config.is_automation {
            return;
        }

        println!("\n=== Validation Results ===");
        println!(
            "Completion: {:.1}%",
            validation_result.completion_percentage
        );
        println!("Status: {:?}", validation_result.status);

        Self::print_list("\nImprovements:", &validation_result.improvements);
        Self::print_list("\nRemaining Issues:", &validation_result.remaining_issues);
        Self::print_gaps(&validation_result.gaps);

        println!(
            "\nValidation result written to: {}",
            config.output_path.display()
        );
    }

    /// Print a list of items with bullet points
    fn print_list(header: &str, items: &[String]) {
        if items.is_empty() {
            return;
        }
        println!("{}", header);
        for item in items {
            println!("  • {}", item);
        }
    }

    /// Print validation gaps
    fn print_gaps(gaps: &HashMap<String, GapDetail>) {
        if gaps.is_empty() {
            return;
        }
        println!("\nGaps to Address ({}):", gaps.len());
        for gap in gaps.values() {
            println!(
                "  • [{}] {} at {}",
                gap.severity, gap.description, gap.location
            );
        }
    }
}

#[async_trait]
impl CommandHandler for ValidateDebtmapHandler {
    fn name(&self) -> &str {
        "validate_debtmap"
    }

    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("validate_debtmap");
        schema.add_required("before", "Path to debtmap JSON before changes");
        schema.add_required("after", "Path to debtmap JSON after changes");
        schema.add_optional_with_default(
            "output",
            "Path to write validation result JSON",
            AttributeValue::String(".prodigy/debtmap-validation.json".to_string()),
        );
        schema
    }

    async fn execute(
        &self,
        context: &ExecutionContext,
        mut attributes: HashMap<String, AttributeValue>,
    ) -> CommandResult {
        // Apply defaults
        self.schema().apply_defaults(&mut attributes);

        // Parse arguments
        let config = match Self::parse_execution_config(context, &attributes) {
            Ok(cfg) => cfg,
            Err(e) => return CommandResult::error(e),
        };

        // Load debtmaps with error handling
        let (before, after) = match Self::load_debtmaps(&config) {
            Ok(maps) => maps,
            Err(e) => {
                Self::write_error_result(&config.output_path, &e);
                return CommandResult::error(e);
            }
        };

        Self::print_if_interactive(&config, || {
            format!(
                "Comparing debtmaps: {} items before, {} items after",
                before.items.len(),
                after.items.len()
            )
        });

        // Compare and output results
        let validation_result = compare_debtmaps(&before, &after);
        Self::output_validation_result(&config, &validation_result)
    }

    fn description(&self) -> &str {
        "Validates that technical debt improvements have been made by comparing debtmap JSON output"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            "/prodigy-validate-debtmap-improvement --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json".to_string(),
            "/prodigy-validate-debtmap-improvement --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json --output .prodigy/validation.json".to_string(),
        ]
    }
}

/// Write validation result to JSON file
fn write_validation_result(path: &PathBuf, result: &Value) -> anyhow::Result<()> {
    use anyhow::Context;

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write JSON to file
    let json_string =
        serde_json::to_string_pretty(result).context("Failed to serialize validation result")?;

    std::fs::write(path, json_string)
        .with_context(|| format!("Failed to write validation result to: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::ExecutionContext;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_handler_schema() {
        let handler = ValidateDebtmapHandler::new();
        let schema = handler.schema();

        // Verify schema is created correctly
        let mut attrs = HashMap::new();
        attrs.insert(
            "before".to_string(),
            AttributeValue::String("before.json".to_string()),
        );
        attrs.insert(
            "after".to_string(),
            AttributeValue::String("after.json".to_string()),
        );

        // Should validate when both required attributes are present
        assert!(schema.validate(&attrs).is_ok());

        // Should fail when before is missing
        let mut missing_before = HashMap::new();
        missing_before.insert(
            "after".to_string(),
            AttributeValue::String("after.json".to_string()),
        );
        assert!(schema.validate(&missing_before).is_err());
    }

    #[tokio::test]
    async fn test_missing_before_parameter() {
        let handler = ValidateDebtmapHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/tmp"));

        let mut attributes = HashMap::new();
        attributes.insert(
            "after".to_string(),
            AttributeValue::String("after.json".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.error.is_some());
        assert!(result
            .error
            .unwrap()
            .contains("Missing required argument: --before"));
    }

    #[tokio::test]
    async fn test_missing_after_parameter() {
        let handler = ValidateDebtmapHandler::new();
        let context = ExecutionContext::new(PathBuf::from("/tmp"));

        let mut attributes = HashMap::new();
        attributes.insert(
            "before".to_string(),
            AttributeValue::String("before.json".to_string()),
        );

        let result = handler.execute(&context, attributes).await;
        assert!(result.error.is_some());
        assert!(result
            .error
            .unwrap()
            .contains("Missing required argument: --after"));
    }
}
