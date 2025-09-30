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
        let before_path = match attributes.get("before").and_then(|v| v.as_string()) {
            Some(path) => context.resolve_path(path.as_ref()),
            None => return CommandResult::error("Missing required argument: --before".to_string()),
        };

        let after_path = match attributes.get("after").and_then(|v| v.as_string()) {
            Some(path) => context.resolve_path(path.as_ref()),
            None => return CommandResult::error("Missing required argument: --after".to_string()),
        };

        let output_path = attributes
            .get("output")
            .and_then(|v| v.as_string())
            .map(|s| context.resolve_path(s.as_ref()))
            .unwrap_or_else(|| {
                context.resolve_path(&PathBuf::from(".prodigy/debtmap-validation.json"))
            });

        // Check automation mode
        let is_automation = std::env::var("PRODIGY_AUTOMATION")
            .unwrap_or_default()
            .to_lowercase()
            == "true"
            || std::env::var("PRODIGY_VALIDATION")
                .unwrap_or_default()
                .to_lowercase()
                == "true";

        if !is_automation {
            println!("Loading debtmap files...");
        }

        // Load debtmap files
        let before = match load_debtmap(&before_path) {
            Ok(dm) => dm,
            Err(e) => {
                let error_result = json!({
                    "completion_percentage": 0.0,
                    "status": "failed",
                    "improvements": [],
                    "remaining_issues": [format!("Failed to load before debtmap: {}", e)],
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

                if let Err(write_err) = write_validation_result(&output_path, &error_result) {
                    eprintln!("Failed to write validation result: {}", write_err);
                }

                return CommandResult::error(format!("Failed to load before debtmap: {}", e));
            }
        };

        let after = match load_debtmap(&after_path) {
            Ok(dm) => dm,
            Err(e) => {
                let error_result = json!({
                    "completion_percentage": 0.0,
                    "status": "failed",
                    "improvements": [],
                    "remaining_issues": [format!("Failed to load after debtmap: {}", e)],
                    "gaps": {},
                    "before_summary": {
                        "total_items": before.items.len(),
                        "high_priority_items": before.items.iter().filter(|i| i.unified_score.final_score >= 6.0).count(),
                        "average_score": calculate_summary(&before).average_score
                    },
                    "after_summary": {
                        "total_items": 0,
                        "high_priority_items": 0,
                        "average_score": 0.0
                    }
                });

                if let Err(write_err) = write_validation_result(&output_path, &error_result) {
                    eprintln!("Failed to write validation result: {}", write_err);
                }

                return CommandResult::error(format!("Failed to load after debtmap: {}", e));
            }
        };

        if !is_automation {
            println!(
                "Comparing debtmaps: {} items before, {} items after",
                before.items.len(),
                after.items.len()
            );
        }

        // Compare debtmaps
        let validation_result = compare_debtmaps(&before, &after);

        // Write result to file
        let result_json = match serde_json::to_value(&validation_result) {
            Ok(json) => json,
            Err(e) => {
                return CommandResult::error(format!(
                    "Failed to serialize validation result: {}",
                    e
                ))
            }
        };

        if let Err(e) = write_validation_result(&output_path, &result_json) {
            return CommandResult::error(format!("Failed to write validation result: {}", e));
        }

        // Output summary
        if !is_automation {
            println!("\n=== Validation Results ===");
            println!(
                "Completion: {:.1}%",
                validation_result.completion_percentage
            );
            println!("Status: {:?}", validation_result.status);

            if !validation_result.improvements.is_empty() {
                println!("\nImprovements:");
                for improvement in &validation_result.improvements {
                    println!("  • {}", improvement);
                }
            }

            if !validation_result.remaining_issues.is_empty() {
                println!("\nRemaining Issues:");
                for issue in &validation_result.remaining_issues {
                    println!("  • {}", issue);
                }
            }

            if !validation_result.gaps.is_empty() {
                println!("\nGaps to Address ({}):", validation_result.gaps.len());
                for gap in validation_result.gaps.values() {
                    println!(
                        "  • [{}] {} at {}",
                        gap.severity, gap.description, gap.location
                    );
                }
            }

            println!("\nValidation result written to: {}", output_path.display());
        }

        CommandResult::success(result_json)
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
