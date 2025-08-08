//! Analyze command handler for running project analysis

use crate::analysis::{run_analysis, AnalysisConfig, DefaultProgressReporter, OutputFormat};
use crate::commands::{
    AttributeSchema, AttributeValue, CommandHandler, CommandResult, ExecutionContext,
};
use crate::context::save_analysis_with_options;
use crate::cook::analysis::cache::{AnalysisCache, AnalysisCacheImpl};
use crate::subprocess::{runner::TokioProcessRunner, SubprocessManager};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// Handler for running project analysis
pub struct AnalyzeHandler;

impl AnalyzeHandler {
    /// Creates a new analyze handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for AnalyzeHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandHandler for AnalyzeHandler {
    fn name(&self) -> &str {
        "analyze"
    }

    fn description(&self) -> &str {
        "Run project analysis with caching and formatting options"
    }

    fn schema(&self) -> AttributeSchema {
        let mut schema = AttributeSchema::new("analyze");
        schema.add_optional_with_default(
            "force_refresh",
            "Force fresh analysis ignoring cache",
            AttributeValue::Boolean(false),
        );
        schema.add_optional_with_default(
            "max_cache_age",
            "Maximum cache age in seconds",
            AttributeValue::Number(3600.0),
        );
        schema.add_optional_with_default(
            "save",
            "Save results to .mmm directory",
            AttributeValue::Boolean(true),
        );
        schema.add_optional_with_default(
            "format",
            "Display format (json, pretty, summary)",
            AttributeValue::String("summary".to_string()),
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

        // Extract parameters
        let force_refresh = attributes
            .get("force_refresh")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_cache_age = attributes
            .get("max_cache_age")
            .and_then(|v| v.as_number())
            .unwrap_or(3600.0) as u64;

        let save = attributes
            .get("save")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let format_str = attributes
            .get("format")
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "summary".to_string());

        let output_format = match format_str.parse::<OutputFormat>() {
            Ok(fmt) => fmt,
            Err(_) => {
                return CommandResult::error(format!(
                    "Invalid format '{format_str}'. Must be one of: json, pretty, summary"
                ))
            }
        };

        // Start timing
        let start = Instant::now();

        // Handle dry run
        if context.dry_run {
            let duration = start.elapsed().as_millis() as u64;
            return CommandResult::success(Value::String(format!(
                "[DRY RUN] Would run analysis with force_refresh={force_refresh}, max_cache_age={max_cache_age}, save={save}, format={format_str}"
            )))
            .with_duration(duration);
        }

        // Get project path
        let project_path = PathBuf::from(&context.working_dir);

        // Check cache unless force refresh
        let mut analysis_result = None;
        if !force_refresh {
            // Create cache instance
            let cache = AnalysisCacheImpl::new(&project_path);

            // Check if cached analysis is still valid
            // Try to get from cache with age check
            if let Ok(is_valid) = cache
                .is_valid("analysis", chrono::Duration::seconds(max_cache_age as i64))
                .await
            {
                if is_valid {
                    if let Ok(Some(cached)) = cache.get("analysis").await {
                        analysis_result = Some(cached);
                        if false {
                            // TODO: add verbose flag to context
                            eprintln!("Using cached analysis (age < {max_cache_age} seconds)");
                        }
                    }
                }
            }
        }

        // Run analysis if not cached or force refresh
        if analysis_result.is_none() {
            // Build analysis config
            let config = AnalysisConfig::builder()
                .output_format(output_format)
                .save_results(save)
                .force_refresh(force_refresh)
                .verbose(false) // TODO: add verbose flag to context
                .run_coverage(true) // Always run coverage as per spec
                .build();

            // Create subprocess manager and progress reporter
            let subprocess = SubprocessManager::new(std::sync::Arc::new(TokioProcessRunner));
            let progress = std::sync::Arc::new(DefaultProgressReporter);

            // Run unified analysis
            match run_analysis(&project_path, config.clone(), subprocess, progress).await {
                Ok(results) => {
                    // Extract context from AnalysisResults
                    if let Some(context) = results.context {
                        analysis_result = Some(context);
                    } else {
                        return CommandResult::error("Analysis produced no context".to_string());
                    }

                    // Cache the results
                    if !force_refresh {
                        let cache = AnalysisCacheImpl::new(&project_path);
                        if let Some(ref result) = analysis_result {
                            let _ = cache.put("analysis", result).await;
                        }
                    }
                }
                Err(e) => {
                    return CommandResult::error(format!("Analysis failed: {e}"));
                }
            }
        }

        // Save results if requested
        if save {
            if let Some(ref result) = analysis_result {
                if let Err(e) = save_analysis_with_options(&project_path, result, false) {
                    eprintln!("Warning: Failed to save analysis results: {e}");
                }
            }
        }

        // Format output based on requested format
        let output = match analysis_result {
            Some(result) => {
                match output_format {
                    OutputFormat::Json => {
                        // Return raw JSON
                        serde_json::to_value(&result).unwrap_or(Value::Null)
                    }
                    OutputFormat::Pretty => {
                        // Format for human readability
                        Value::String(format_analysis_pretty(&result))
                    }
                    OutputFormat::Summary => {
                        // Concise summary
                        Value::String(format_analysis_summary(&result))
                    }
                }
            }
            None => Value::String("No analysis results available".to_string()),
        };

        let duration = start.elapsed().as_millis() as u64;
        CommandResult::success(output).with_duration(duration)
    }
}

/// Format analysis results in pretty human-readable format
fn format_analysis_pretty(result: &crate::context::AnalysisResult) -> String {
    let mut output = String::new();

    // Header
    output.push_str("=== Project Analysis Results ===\n\n");

    // Metadata
    output.push_str(&format!(
        "Analyzed {} files in {}ms\n",
        result.metadata.files_analyzed, result.metadata.duration_ms
    ));
    output.push_str(&format!(
        "Timestamp: {}\n\n",
        result.metadata.timestamp.format("%Y-%m-%d %H:%M:%S")
    ));

    // Dependency info
    output.push_str(&format!(
        "Dependencies: {} modules, {} cycles detected\n",
        result.dependency_graph.nodes.len(),
        result.dependency_graph.cycles.len()
    ));

    // Architecture info
    output.push_str(&format!(
        "Architecture: {} patterns, {} violations\n",
        result.architecture.patterns.len(),
        result.architecture.violations.len()
    ));

    // Technical debt
    let total_items = result.technical_debt.debt_items.len();
    let high_priority = result
        .technical_debt
        .debt_items
        .iter()
        .filter(|d| d.impact >= 7)
        .count();
    output.push_str(&format!(
        "Technical Debt: {total_items} items ({high_priority} high priority)\n"
    ));

    // Test coverage
    if let Some(coverage) = &result.test_coverage {
        output.push_str(&format!(
            "Test Coverage: {:.1}%\n",
            coverage.overall_coverage * 100.0
        ));
    }

    output
}

/// Format analysis results in concise summary format
fn format_analysis_summary(result: &crate::context::AnalysisResult) -> String {
    let mut parts = Vec::new();

    // Files analyzed
    parts.push(format!("{} files analyzed", result.metadata.files_analyzed));

    // Coverage
    if let Some(coverage) = &result.test_coverage {
        parts.push(format!(
            "{:.1}% coverage",
            coverage.overall_coverage * 100.0
        ));
    }

    // Technical debt
    parts.push(format!(
        "{} debt items",
        result.technical_debt.debt_items.len()
    ));

    // Architecture violations
    if !result.architecture.violations.is_empty() {
        parts.push(format!(
            "{} violations",
            result.architecture.violations.len()
        ));
    }

    // Join all parts
    format!("Analysis complete: {}", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_name() {
        let handler = AnalyzeHandler::new();
        assert_eq!(handler.name(), "analyze");
    }

    #[test]
    fn test_schema() {
        let handler = AnalyzeHandler::new();
        let schema = handler.schema();
        assert_eq!(schema.name(), "analyze");

        // Check schema name
        assert_eq!(schema.name(), "analyze");
    }

    #[test]
    fn test_output_format_parsing() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "pretty".parse::<OutputFormat>().unwrap(),
            OutputFormat::Pretty
        );
        assert_eq!(
            "summary".parse::<OutputFormat>().unwrap(),
            OutputFormat::Summary
        );
        // Default to summary for unknown
        assert_eq!(
            "unknown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Summary
        );
    }
}
