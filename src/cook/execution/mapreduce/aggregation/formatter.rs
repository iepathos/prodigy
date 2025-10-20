//! Output formatting for MapReduce results
//!
//! This module provides formatting utilities for presenting
//! MapReduce execution results in various formats.

use super::AggregationSummary;
use crate::cook::execution::mapreduce::{AgentResult, AgentStatus};
use serde_json::json;
use std::fmt::Write;

/// Format type for output presentation
#[derive(Debug, Clone)]
pub enum FormatType {
    /// Plain text format
    Text,
    /// JSON format
    Json,
    /// Pretty-printed JSON
    JsonPretty,
    /// Markdown table format
    Markdown,
    /// CSV format
    Csv,
}

/// Output formatter for MapReduce results
pub struct OutputFormatter {
    format_type: FormatType,
}

impl OutputFormatter {
    /// Create a new formatter with the specified format type
    pub fn new(format_type: FormatType) -> Self {
        Self { format_type }
    }

    /// Format results according to the configured format type
    pub fn format(&self, results: &[AgentResult], summary: &AggregationSummary) -> String {
        match &self.format_type {
            FormatType::Text => self.format_text(results, summary),
            FormatType::Json => self.format_json(results, summary),
            FormatType::JsonPretty => self.format_json_pretty(results, summary),
            FormatType::Markdown => self.format_markdown(results, summary),
            FormatType::Csv => self.format_csv(results, summary),
        }
    }

    /// Format as plain text
    fn format_text(&self, results: &[AgentResult], summary: &AggregationSummary) -> String {
        let mut output = String::new();

        writeln!(&mut output, "=== MapReduce Results ===").unwrap();
        writeln!(&mut output, "Total: {}", summary.total).unwrap();
        writeln!(&mut output, "Successful: {}", summary.successful).unwrap();
        writeln!(&mut output, "Failed: {}", summary.failed).unwrap();
        writeln!(
            &mut output,
            "Avg Duration: {:.2}s",
            summary.avg_duration_secs
        )
        .unwrap();
        writeln!(&mut output).unwrap();

        for result in results {
            let status_str = match &result.status {
                AgentStatus::Success => "✓",
                AgentStatus::Failed(_) => "✗",
                AgentStatus::Timeout => "⏱",
                AgentStatus::Pending => "⋯",
                AgentStatus::Running => "→",
                AgentStatus::Retrying(_) => "↻",
            };

            writeln!(
                &mut output,
                "{} {} ({:.2}s)",
                status_str,
                result.item_id,
                result.duration.as_secs_f64()
            )
            .unwrap();

            if let Some(output_text) = &result.output {
                writeln!(
                    &mut output,
                    "  Output: {}",
                    output_text.lines().next().unwrap_or("")
                )
                .unwrap();
            }
        }

        output
    }

    /// Format as JSON
    fn format_json(&self, results: &[AgentResult], summary: &AggregationSummary) -> String {
        let output = json!({
            "summary": summary,
            "results": results
        });

        output.to_string()
    }

    /// Format as pretty-printed JSON
    fn format_json_pretty(&self, results: &[AgentResult], summary: &AggregationSummary) -> String {
        let output = json!({
            "summary": summary,
            "results": results
        });

        serde_json::to_string_pretty(&output).unwrap_or_else(|_| output.to_string())
    }

    /// Format as Markdown table
    fn format_markdown(&self, results: &[AgentResult], summary: &AggregationSummary) -> String {
        let mut output = String::new();

        writeln!(&mut output, "## MapReduce Results\n").unwrap();
        writeln!(&mut output, "**Summary:**").unwrap();
        writeln!(&mut output, "- Total: {}", summary.total).unwrap();
        writeln!(&mut output, "- Successful: {}", summary.successful).unwrap();
        writeln!(&mut output, "- Failed: {}", summary.failed).unwrap();
        writeln!(
            &mut output,
            "- Average Duration: {:.2}s\n",
            summary.avg_duration_secs
        )
        .unwrap();

        writeln!(&mut output, "| Item ID | Status | Duration | Commits |").unwrap();
        writeln!(&mut output, "|---------|--------|----------|---------|").unwrap();

        for result in results {
            let status_str = match &result.status {
                AgentStatus::Success => "Success ✓",
                AgentStatus::Failed(_) => "Failed ✗",
                AgentStatus::Timeout => "Timeout ⏱",
                AgentStatus::Pending => "Pending",
                AgentStatus::Running => "Running",
                AgentStatus::Retrying(n) => &format!("Retrying ({})", n),
            };

            writeln!(
                &mut output,
                "| {} | {} | {:.2}s | {} |",
                result.item_id,
                status_str,
                result.duration.as_secs_f64(),
                result.commits.len()
            )
            .unwrap();
        }

        output
    }

    /// Format as CSV
    fn format_csv(&self, results: &[AgentResult], _summary: &AggregationSummary) -> String {
        let mut output = String::new();

        writeln!(&mut output, "item_id,status,duration_secs,commits,output").unwrap();

        for result in results {
            let status_str = match &result.status {
                AgentStatus::Success => "success",
                AgentStatus::Failed(_) => "failed",
                AgentStatus::Timeout => "timeout",
                AgentStatus::Pending => "pending",
                AgentStatus::Running => "running",
                AgentStatus::Retrying(_) => "retrying",
            };

            let output_escaped = result
                .output
                .as_ref()
                .map(|o| format!("\"{}\"", o.replace("\"", "\"\"")))
                .unwrap_or_else(String::new);

            writeln!(
                &mut output,
                "{},{},{:.2},{},{}",
                result.item_id,
                status_str,
                result.duration.as_secs_f64(),
                result.commits.len(),
                output_escaped
            )
            .unwrap();
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_result(id: &str, status: AgentStatus) -> AgentResult {
        AgentResult {
            item_id: id.to_string(),
            status,
            output: Some(format!("Output for {}", id)),
            commits: vec!["commit1".to_string()],
            duration: Duration::from_secs(2),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        }
    }

    #[test]
    fn test_text_format() {
        let results = vec![
            create_test_result("item1", AgentStatus::Success),
            create_test_result("item2", AgentStatus::Failed("error".to_string())),
        ];

        let summary = AggregationSummary::from_results(&results);
        let formatter = OutputFormatter::new(FormatType::Text);
        let output = formatter.format(&results, &summary);

        assert!(output.contains("Total: 2"));
        assert!(output.contains("Successful: 1"));
        assert!(output.contains("Failed: 1"));
    }

    #[test]
    fn test_json_format() {
        let results = vec![create_test_result("item1", AgentStatus::Success)];
        let summary = AggregationSummary::from_results(&results);

        let formatter = OutputFormatter::new(FormatType::Json);
        let output = formatter.format(&results, &summary);

        assert!(output.contains("\"successful\":1"));
        assert!(output.contains("\"item_id\":\"item1\""));
    }

    #[test]
    fn test_csv_format() {
        let results = vec![
            create_test_result("item1", AgentStatus::Success),
            create_test_result("item2", AgentStatus::Failed("error".to_string())),
        ];

        let summary = AggregationSummary::from_results(&results);
        let formatter = OutputFormatter::new(FormatType::Csv);
        let output = formatter.format(&results, &summary);

        assert!(output.starts_with("item_id,status,duration_secs,commits,output"));
        assert!(output.contains("item1,success,2.00,1"));
        assert!(output.contains("item2,failed,2.00,1"));
    }
}
