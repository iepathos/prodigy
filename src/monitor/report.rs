use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tera::Tera;

use super::analytics::{Analysis, AnalyticsEngine};
use super::metrics::MetricsDatabase;
use super::TimeFrame;
use crate::claude::ClaudeManager;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub id: String,
    pub name: String,
    pub title: String,
    pub generated_at: DateTime<Utc>,
    pub timeframe: TimeFrame,
    pub sections: Vec<ReportSection>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ReportSection {
    Summary {
        title: String,
        metrics: Vec<SummaryMetric>,
    },
    Chart {
        title: String,
        chart_type: ChartType,
        data: ChartData,
    },
    Table {
        title: String,
        columns: Vec<TableColumn>,
        rows: Vec<HashMap<String, serde_json::Value>>,
    },
    Insights {
        title: String,
        content: String,
    },
    Analysis {
        title: String,
        analysis: Analysis,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetric {
    pub name: String,
    pub value: f64,
    pub unit: Option<String>,
    pub change: Option<f64>,
    pub trend: Option<Trend>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Trend {
    Up,
    Down,
    Stable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Bar,
    Pie,
    Area,
    Scatter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub labels: Vec<String>,
    pub datasets: Vec<Dataset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub label: String,
    pub data: Vec<f64>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColumn {
    pub name: String,
    pub field: String,
    pub format: Option<ColumnFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnFormat {
    Number { decimals: usize },
    Percentage,
    Date,
    Duration,
    Currency { symbol: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTemplate {
    pub name: String,
    pub title: String,
    pub schedule: Option<String>,
    pub sections: Vec<ReportSectionTemplate>,
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ReportSectionTemplate {
    Summary {
        title: String,
        metrics: Vec<String>,
    },
    Chart {
        title: String,
        chart_type: ChartType,
        x_metric: String,
        y_metric: String,
        group_by: Option<String>,
    },
    Table {
        title: String,
        query: String,
        columns: Vec<TableColumn>,
    },
    Insights {
        title: String,
        prompt: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    pub formats: Vec<ExportFormat>,
    pub email: Option<EmailConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExportFormat {
    PDF,
    HTML,
    Markdown,
    JSON,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub to: Vec<String>,
    pub subject: String,
}

pub struct ReportGenerator {
    metrics_db: Arc<MetricsDatabase>,
    analytics_engine: Arc<AnalyticsEngine>,
    claude_manager: Arc<ClaudeManager>,
    #[allow(dead_code)]
    template_engine: Tera,
}

impl ReportGenerator {
    pub fn new(
        metrics_db: Arc<MetricsDatabase>,
        analytics_engine: Arc<AnalyticsEngine>,
        claude_manager: Arc<ClaudeManager>,
    ) -> Result<Self> {
        let template_engine = Tera::default();

        Ok(Self {
            metrics_db,
            analytics_engine,
            claude_manager,
            template_engine,
        })
    }

    pub async fn generate_from_template(
        &self,
        template: &ReportTemplate,
        timeframe: TimeFrame,
    ) -> Result<Report> {
        let mut sections = Vec::new();

        for section_template in &template.sections {
            let section = match section_template {
                ReportSectionTemplate::Summary { title, metrics } => {
                    self.generate_summary_section(title, metrics, &timeframe)
                        .await?
                }
                ReportSectionTemplate::Chart {
                    title,
                    chart_type,
                    x_metric,
                    y_metric,
                    group_by,
                } => {
                    self.generate_chart_section(
                        title,
                        *chart_type,
                        x_metric,
                        y_metric,
                        group_by.as_deref(),
                        &timeframe,
                    )
                    .await?
                }
                ReportSectionTemplate::Table {
                    title,
                    query,
                    columns,
                } => {
                    self.generate_table_section(title, query, columns, &timeframe)
                        .await?
                }
                ReportSectionTemplate::Insights { title, prompt } => {
                    self.generate_insights_section(title, prompt, &timeframe)
                        .await?
                }
            };
            sections.push(section);
        }

        // Add analytics results
        let analyses = self
            .analytics_engine
            .run_analysis(timeframe.clone())
            .await?;
        for analysis in analyses {
            sections.push(ReportSection::Analysis {
                title: format!("Analysis: {}", analysis.name),
                analysis,
            });
        }

        Ok(Report {
            id: uuid::Uuid::new_v4().to_string(),
            name: template.name.clone(),
            title: template.title.clone(),
            generated_at: Utc::now(),
            timeframe,
            sections,
            metadata: HashMap::new(),
        })
    }

    async fn generate_summary_section(
        &self,
        title: &str,
        metric_names: &[String],
        timeframe: &TimeFrame,
    ) -> Result<ReportSection> {
        let mut metrics = Vec::new();

        for metric_name in metric_names {
            let value = self
                .metrics_db
                .aggregate_metrics(
                    metric_name,
                    timeframe.start,
                    timeframe.end,
                    super::metrics::AggregationType::Sum,
                )
                .await?;

            // Calculate change from previous period
            let prev_timeframe = TimeFrame {
                start: timeframe.start - (timeframe.end - timeframe.start),
                end: timeframe.start,
            };

            let prev_value = self
                .metrics_db
                .aggregate_metrics(
                    metric_name,
                    prev_timeframe.start,
                    prev_timeframe.end,
                    super::metrics::AggregationType::Sum,
                )
                .await?;

            let change = if prev_value != 0.0 {
                Some(((value - prev_value) / prev_value) * 100.0)
            } else {
                None
            };

            let trend = change.map(|c| {
                if c > 5.0 {
                    Trend::Up
                } else if c < -5.0 {
                    Trend::Down
                } else {
                    Trend::Stable
                }
            });

            metrics.push(SummaryMetric {
                name: metric_name.clone(),
                value,
                unit: Self::get_metric_unit(metric_name),
                change,
                trend,
            });
        }

        Ok(ReportSection::Summary {
            title: title.to_string(),
            metrics,
        })
    }

    async fn generate_chart_section(
        &self,
        title: &str,
        chart_type: ChartType,
        _x_metric: &str,
        y_metric: &str,
        _group_by: Option<&str>,
        timeframe: &TimeFrame,
    ) -> Result<ReportSection> {
        // For simplicity, generate daily data points
        let mut labels = Vec::new();
        let mut data_points = Vec::new();

        let days = (timeframe.end - timeframe.start).num_days();
        for i in 0..days {
            let day = timeframe.start + chrono::Duration::days(i);
            let next_day = day + chrono::Duration::days(1);

            labels.push(day.format("%Y-%m-%d").to_string());

            let value = self
                .metrics_db
                .aggregate_metrics(
                    y_metric,
                    day,
                    next_day,
                    super::metrics::AggregationType::Average,
                )
                .await?;

            data_points.push(value);
        }

        let dataset = Dataset {
            label: y_metric.to_string(),
            data: data_points,
            color: Some("#3b82f6".to_string()), // Blue
        };

        Ok(ReportSection::Chart {
            title: title.to_string(),
            chart_type,
            data: ChartData {
                labels,
                datasets: vec![dataset],
            },
        })
    }

    async fn generate_table_section(
        &self,
        title: &str,
        _query: &str,
        columns: &[TableColumn],
        _timeframe: &TimeFrame,
    ) -> Result<ReportSection> {
        // For now, return empty table
        // In a real implementation, this would execute the query
        Ok(ReportSection::Table {
            title: title.to_string(),
            columns: columns.to_vec(),
            rows: vec![],
        })
    }

    async fn generate_insights_section(
        &self,
        title: &str,
        prompt: &str,
        timeframe: &TimeFrame,
    ) -> Result<ReportSection> {
        // Gather relevant metrics for the timeframe
        let metrics_summary = self.generate_metrics_summary(timeframe).await?;

        // Use Claude to generate insights
        let full_prompt = format!(
            "{}\n\nTimeframe: {} to {}\n\nMetrics Summary:\n{}",
            prompt,
            timeframe.start.format("%Y-%m-%d"),
            timeframe.end.format("%Y-%m-%d"),
            metrics_summary
        );

        let insights = self.claude_manager.generate_response(&full_prompt).await?;

        Ok(ReportSection::Insights {
            title: title.to_string(),
            content: insights,
        })
    }

    async fn generate_metrics_summary(&self, timeframe: &TimeFrame) -> Result<String> {
        let mut summary = String::new();

        // Gather key metrics
        let metrics = vec![
            ("Total Specs Completed", "specs.completed"),
            ("Average Completion Time", "specs.completion_time_hours"),
            ("Total Claude Tokens", "claude.tokens.total"),
            ("Total Cost", "claude.cost.usd"),
            ("Error Count", "claude.errors"),
        ];

        for (label, metric_name) in metrics {
            let value = self
                .metrics_db
                .aggregate_metrics(
                    metric_name,
                    timeframe.start,
                    timeframe.end,
                    super::metrics::AggregationType::Sum,
                )
                .await
                .unwrap_or(0.0);

            summary.push_str(&format!("- {label}: {value:.2}\n"));
        }

        Ok(summary)
    }

    fn get_metric_unit(metric_name: &str) -> Option<String> {
        match metric_name {
            name if name.ends_with("_ms") => Some("ms".to_string()),
            name if name.ends_with("_hours") => Some("hours".to_string()),
            name if name.ends_with("_usd") => Some("$".to_string()),
            name if name.ends_with("_percentage") => Some("%".to_string()),
            name if name.contains("tokens") => Some("tokens".to_string()),
            _ => None,
        }
    }
}

// Built-in report templates
pub fn default_report_templates() -> Vec<ReportTemplate> {
    vec![
        ReportTemplate {
            name: "weekly-progress".to_string(),
            title: "Weekly Progress Report".to_string(),
            schedule: Some("0 9 * * MON".to_string()),
            sections: vec![
                ReportSectionTemplate::Summary {
                    title: "Executive Summary".to_string(),
                    metrics: vec![
                        "specs.completed".to_string(),
                        "specs.total".to_string(),
                        "specs.completion_percentage".to_string(),
                        "claude.cost.usd".to_string(),
                    ],
                },
                ReportSectionTemplate::Chart {
                    title: "Progress Trend".to_string(),
                    chart_type: ChartType::Line,
                    x_metric: "date".to_string(),
                    y_metric: "specs.completion_percentage".to_string(),
                    group_by: None,
                },
                ReportSectionTemplate::Insights {
                    title: "AI-Generated Insights".to_string(),
                    prompt: "Analyze the weekly progress data and provide insights on bottlenecks, improvements, and recommendations for the development team.".to_string(),
                },
            ],
            export: ExportConfig {
                formats: vec![ExportFormat::PDF, ExportFormat::HTML],
                email: None,
            },
        },
        ReportTemplate {
            name: "cost-analysis".to_string(),
            title: "Cost Analysis Report".to_string(),
            schedule: None,
            sections: vec![
                ReportSectionTemplate::Summary {
                    title: "Cost Overview".to_string(),
                    metrics: vec![
                        "claude.cost.usd".to_string(),
                        "claude.cost.input_usd".to_string(),
                        "claude.cost.output_usd".to_string(),
                        "claude.tokens.total".to_string(),
                    ],
                },
                ReportSectionTemplate::Chart {
                    title: "Daily Cost Trend".to_string(),
                    chart_type: ChartType::Bar,
                    x_metric: "date".to_string(),
                    y_metric: "claude.cost.usd".to_string(),
                    group_by: None,
                },
                ReportSectionTemplate::Insights {
                    title: "Cost Optimization Recommendations".to_string(),
                    prompt: "Analyze the cost data and provide specific recommendations for reducing API costs while maintaining productivity.".to_string(),
                },
            ],
            export: ExportConfig {
                formats: vec![ExportFormat::PDF, ExportFormat::JSON],
                email: None,
            },
        },
    ]
}
