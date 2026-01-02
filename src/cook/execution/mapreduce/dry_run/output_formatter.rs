//! Output formatting for dry-run reports
//!
//! Formats dry-run validation reports for human-readable or JSON output.

use super::types::*;
use serde_json::Value;
use std::fmt::Write;

/// Formatter for dry-run reports
pub struct OutputFormatter;

impl OutputFormatter {
    /// Create a new output formatter
    pub fn new() -> Self {
        Self
    }

    /// Format a dry-run report as human-readable text
    pub fn format_human(&self, report: &DryRunReport) -> String {
        let mut output = String::new();

        writeln!(&mut output, "\n🔍 MapReduce Workflow Dry-Run Report").unwrap();
        writeln!(&mut output, "{}", "═".repeat(50)).unwrap();

        // Overall status
        let status = if report.validation_results.is_valid {
            "✅ READY"
        } else {
            "❌ NEEDS FIXES"
        };
        writeln!(&mut output, "\nStatus: {}", status).unwrap();

        // Validation results
        writeln!(&mut output, "\n📋 Validation Results:").unwrap();
        self.format_validation_results(&mut output, &report.validation_results);

        // Work item preview
        if report.work_item_preview.total_count > 0 {
            writeln!(&mut output, "\n📊 Work Item Preview:").unwrap();
            self.format_work_item_preview(&mut output, &report.work_item_preview);
        }

        // Resource estimates
        writeln!(&mut output, "\n💻 Resource Estimates:").unwrap();
        self.format_resource_estimates(&mut output, &report.resource_estimates);

        // Variable preview
        if !report.variable_preview.undefined_references.is_empty() {
            writeln!(&mut output, "\n🔤 Variable Analysis:").unwrap();
            self.format_variable_preview(&mut output, &report.variable_preview);
        }

        // Warnings
        if !report.warnings.is_empty() {
            writeln!(&mut output, "\n⚠️  Warnings:").unwrap();
            for warning in &report.warnings {
                writeln!(&mut output, "  • {}", warning).unwrap();
            }
        }

        // Errors
        if !report.errors.is_empty() {
            writeln!(&mut output, "\n❌ Errors:").unwrap();
            for error in &report.errors {
                writeln!(&mut output, "  • {}", error).unwrap();
            }
        }

        // Estimated duration
        writeln!(
            &mut output,
            "\n⏱️  Estimated Duration: {}",
            self.format_duration(report.estimated_duration.as_secs())
        )
        .unwrap();

        output
    }

    /// Format validation results section
    fn format_validation_results(&self, output: &mut String, results: &ValidationResults) {
        // Setup phase
        if let Some(setup) = &results.setup_phase {
            self.format_phase_validation(output, "Setup", setup);
        }

        // Map phase
        self.format_phase_validation(output, "Map", &results.map_phase);

        // Reduce phase
        if let Some(reduce) = &results.reduce_phase {
            self.format_phase_validation(output, "Reduce", reduce);
        }
    }

    /// Format a single phase validation
    fn format_phase_validation(
        &self,
        output: &mut String,
        phase_name: &str,
        validation: &PhaseValidation,
    ) {
        let status_icon = if validation.valid { "✓" } else { "✗" };

        writeln!(
            output,
            "  {} {} Phase: {} commands, est. {}",
            status_icon,
            phase_name,
            validation.command_count,
            self.format_duration(validation.estimated_duration.as_secs())
        )
        .unwrap();

        // Show issues if any
        for issue in &validation.issues {
            match issue {
                ValidationIssue::Error(msg) => {
                    writeln!(output, "      ERROR: {}", msg).unwrap();
                }
                ValidationIssue::Warning(msg) => {
                    writeln!(output, "      WARN: {}", msg).unwrap();
                }
            }
        }
    }

    /// Format work item preview section
    fn format_work_item_preview(&self, output: &mut String, preview: &WorkItemPreview) {
        writeln!(output, "  Total items: {}", preview.total_count).unwrap();

        if let Some(filtered) = preview.filtered_count {
            writeln!(output, "  After filtering: {}", filtered).unwrap();
        }

        if let Some(sort) = &preview.sort_description {
            writeln!(output, "  Sort order: {}", sort).unwrap();
        }

        // Show distribution
        if !preview.distribution.is_empty() {
            writeln!(output, "  Distribution across agents:").unwrap();
            let mut agents: Vec<_> = preview.distribution.iter().collect();
            agents.sort_by_key(|(k, _)| **k);

            for (agent_id, count) in agents.iter().take(5) {
                writeln!(output, "    Agent {}: {} items", agent_id, count).unwrap();
            }

            if agents.len() > 5 {
                writeln!(output, "    ... and {} more agents", agents.len() - 5).unwrap();
            }
        }

        // Show sample items
        if !preview.sample_items.is_empty() {
            writeln!(output, "  Sample items:").unwrap();
            for (idx, item) in preview.sample_items.iter().enumerate().take(3) {
                let item_str = self.format_json_value(item, 60);
                writeln!(output, "    [{}] {}", idx, item_str).unwrap();
            }
        }
    }

    /// Format resource estimates section
    fn format_resource_estimates(&self, output: &mut String, estimates: &ResourceEstimates) {
        // Memory
        writeln!(
            output,
            "  Memory: {} MB total ({} MB per agent × {} agents)",
            estimates.memory_usage.total_mb,
            estimates.memory_usage.per_agent_mb,
            estimates.memory_usage.peak_concurrent_agents
        )
        .unwrap();

        // Disk
        writeln!(
            output,
            "  Disk: {} MB total ({} worktrees × {} MB + {} MB temp)",
            estimates.disk_usage.total_mb,
            estimates.worktree_count,
            estimates.disk_usage.per_worktree_mb,
            estimates.disk_usage.temp_space_mb
        )
        .unwrap();

        // Network
        if estimates.network_usage.api_calls > 0 {
            writeln!(
                output,
                "  Network: {} MB transfer, {} API calls",
                estimates.network_usage.data_transfer_mb, estimates.network_usage.api_calls
            )
            .unwrap();
        }

        // Checkpoints
        writeln!(
            output,
            "  Checkpoints: {} checkpoints, {} MB storage",
            estimates.checkpoint_storage.checkpoint_count, estimates.checkpoint_storage.total_mb
        )
        .unwrap();
    }

    /// Format variable preview section
    fn format_variable_preview(&self, output: &mut String, preview: &VariablePreview) {
        if !preview.undefined_references.is_empty() {
            writeln!(output, "  ⚠️ undefined variable references:").unwrap();
            for var_ref in &preview.undefined_references {
                writeln!(output, "    • ${{{}}}", var_ref).unwrap();
            }
        }

        // Show available variables
        let total_vars = preview.setup_variables.len()
            + preview.reduce_variables.len()
            + preview.item_variables.first().map(|v| v.len()).unwrap_or(0);

        writeln!(output, "  Available variables: {}", total_vars).unwrap();
    }

    /// Format duration in human-readable format
    fn format_duration(&self, seconds: u64) -> String {
        if seconds < 60 {
            format!("{}s", seconds)
        } else if seconds < 3600 {
            format!("{}m {}s", seconds / 60, seconds % 60)
        } else {
            format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
        }
    }

    /// Format JSON value with max length
    fn format_json_value(&self, value: &Value, max_len: usize) -> String {
        let str_value = match value {
            Value::String(s) => format!("\"{}\"", s),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(arr) => format!("[{} items]", arr.len()),
            Value::Object(obj) => {
                if obj.is_empty() {
                    "{}".to_string()
                } else {
                    let keys: Vec<_> = obj.keys().take(3).map(|k| k.as_str()).collect();
                    format!("{{{}...}}", keys.join(", "))
                }
            }
        };

        if str_value.len() > max_len {
            format!("{}...", &str_value[..max_len - 3])
        } else {
            str_value
        }
    }

    /// Format report as JSON
    pub fn format_json(&self, report: &DryRunReport) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(report)
    }

    /// Format report as YAML
    pub fn format_yaml(&self, report: &DryRunReport) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(report)
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}
