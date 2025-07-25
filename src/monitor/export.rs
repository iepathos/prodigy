use async_trait::async_trait;
use serde_json;
use std::path::Path;
use tera::{Context, Tera};

use super::report::{ExportFormat, Report, ReportSection};
use crate::error::Result;

#[async_trait]
pub trait ReportExporter: Send + Sync {
    async fn export(&self, report: &Report, format: ExportFormat) -> Result<Vec<u8>>;
    fn supported_formats(&self) -> Vec<ExportFormat>;
}

pub struct HTMLExporter {
    template_engine: Tera,
}

impl HTMLExporter {
    pub fn new() -> Result<Self> {
        let mut template_engine = Tera::default();

        // Add default report template
        template_engine.add_raw_template("report.html", DEFAULT_HTML_TEMPLATE)?;

        Ok(Self { template_engine })
    }
}

#[async_trait]
impl ReportExporter for HTMLExporter {
    async fn export(&self, report: &Report, _format: ExportFormat) -> Result<Vec<u8>> {
        let mut context = Context::new();
        context.insert("report", report);

        let html = self.template_engine.render("report.html", &context)?;
        Ok(html.into_bytes())
    }

    fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat::HTML]
    }
}

pub struct MarkdownExporter;

impl MarkdownExporter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ReportExporter for MarkdownExporter {
    async fn export(&self, report: &Report, _format: ExportFormat) -> Result<Vec<u8>> {
        let mut markdown = String::new();

        // Title and metadata
        markdown.push_str(&format!("# {}\n\n", report.title));
        markdown.push_str(&format!(
            "**Generated:** {}\n\n",
            report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        markdown.push_str(&format!(
            "**Period:** {} to {}\n\n",
            report.timeframe.start.format("%Y-%m-%d"),
            report.timeframe.end.format("%Y-%m-%d")
        ));

        // Sections
        for section in &report.sections {
            match section {
                ReportSection::Summary { title, metrics } => {
                    markdown.push_str(&format!("## {}\n\n", title));

                    for metric in metrics {
                        let value_str = if let Some(unit) = &metric.unit {
                            format!("{:.2} {}", metric.value, unit)
                        } else {
                            format!("{:.2}", metric.value)
                        };

                        let change_str = if let Some(change) = metric.change {
                            let arrow = if change > 0.0 { "↑" } else { "↓" };
                            format!(" ({}{:.1}%)", arrow, change.abs())
                        } else {
                            String::new()
                        };

                        markdown.push_str(&format!(
                            "- **{}:** {}{}\n",
                            metric.name, value_str, change_str
                        ));
                    }
                    markdown.push_str("\n");
                }

                ReportSection::Chart {
                    title,
                    chart_type,
                    data,
                } => {
                    markdown.push_str(&format!("## {}\n\n", title));
                    markdown.push_str(&format!("*Chart: {:?}*\n\n", chart_type));

                    // Simple ASCII chart for markdown
                    if !data.datasets.is_empty() && !data.datasets[0].data.is_empty() {
                        let max_value = data.datasets[0].data.iter().fold(0.0f64, |a, &b| a.max(b));
                        let scale = 20.0 / max_value;

                        for (i, value) in data.datasets[0].data.iter().enumerate() {
                            if i < data.labels.len() {
                                let bar_length = (*value * scale) as usize;
                                let bar = "█".repeat(bar_length);
                                markdown.push_str(&format!(
                                    "{:<12} {} {:.2}\n",
                                    data.labels[i], bar, value
                                ));
                            }
                        }
                    }
                    markdown.push_str("\n");
                }

                ReportSection::Table {
                    title,
                    columns,
                    rows,
                } => {
                    markdown.push_str(&format!("## {}\n\n", title));

                    // Table header
                    markdown.push_str("|");
                    for col in columns {
                        markdown.push_str(&format!(" {} |", col.name));
                    }
                    markdown.push_str("\n|");
                    for _ in columns {
                        markdown.push_str(" --- |");
                    }
                    markdown.push_str("\n");

                    // Table rows
                    for row in rows {
                        markdown.push_str("|");
                        for col in columns {
                            let value = row
                                .get(&col.field)
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            markdown.push_str(&format!(" {} |", value));
                        }
                        markdown.push_str("\n");
                    }
                    markdown.push_str("\n");
                }

                ReportSection::Insights { title, content } => {
                    markdown.push_str(&format!("## {}\n\n", title));
                    markdown.push_str(content);
                    markdown.push_str("\n\n");
                }

                ReportSection::Analysis { title, analysis } => {
                    markdown.push_str(&format!("## {}\n\n", title));

                    // Findings
                    if !analysis.findings.is_empty() {
                        markdown.push_str("### Findings\n\n");
                        for finding in &analysis.findings {
                            let severity_icon = match finding.severity {
                                super::analytics::FindingSeverity::Critical => "🔴",
                                super::analytics::FindingSeverity::High => "🟠",
                                super::analytics::FindingSeverity::Medium => "🟡",
                                super::analytics::FindingSeverity::Low => "🟢",
                                super::analytics::FindingSeverity::Info => "ℹ️",
                            };

                            markdown
                                .push_str(&format!("{} **{}**\n\n", severity_icon, finding.title));
                            markdown.push_str(&format!("{}\n\n", finding.description));
                        }
                    }

                    // Recommendations
                    if !analysis.recommendations.is_empty() {
                        markdown.push_str("### Recommendations\n\n");
                        for rec in &analysis.recommendations {
                            markdown.push_str(&format!("- {}\n", rec));
                        }
                        markdown.push_str("\n");
                    }
                }
            }
        }

        Ok(markdown.into_bytes())
    }

    fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat::Markdown]
    }
}

pub struct JSONExporter;

impl JSONExporter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ReportExporter for JSONExporter {
    async fn export(&self, report: &Report, _format: ExportFormat) -> Result<Vec<u8>> {
        let json = serde_json::to_vec_pretty(report)?;
        Ok(json)
    }

    fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat::JSON]
    }
}

pub struct PDFExporter;

impl PDFExporter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ReportExporter for PDFExporter {
    async fn export(&self, report: &Report, _format: ExportFormat) -> Result<Vec<u8>> {
        // For now, convert to HTML first, then would use a library like wkhtmltopdf
        // In a real implementation, this would generate a proper PDF
        let html_exporter = HTMLExporter::new()?;
        let html = html_exporter.export(report, ExportFormat::HTML).await?;

        // TODO: Convert HTML to PDF using a library like printpdf or wkhtmltopdf
        // For now, just return the HTML
        Ok(html)
    }

    fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat::PDF]
    }
}

pub struct MultiFormatExporter {
    exporters: Vec<Box<dyn ReportExporter>>,
}

impl MultiFormatExporter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            exporters: vec![
                Box::new(HTMLExporter::new()?),
                Box::new(MarkdownExporter::new()),
                Box::new(JSONExporter::new()),
                Box::new(PDFExporter::new()),
            ],
        })
    }

    pub async fn export(&self, report: &Report, format: ExportFormat) -> Result<Vec<u8>> {
        for exporter in &self.exporters {
            if exporter.supported_formats().contains(&format) {
                return exporter.export(report, format).await;
            }
        }

        Err(crate::Error::NotFound(format!(
            "No exporter found for format: {:?}",
            format
        )))
    }

    pub async fn export_to_file(
        &self,
        report: &Report,
        format: ExportFormat,
        path: &Path,
    ) -> Result<()> {
        let data = self.export(report, format).await?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }
}

const DEFAULT_HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ report.title }}</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            line-height: 1.6;
            color: #333;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f5f5f5;
        }
        .report-header {
            background-color: white;
            padding: 30px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            margin-bottom: 30px;
        }
        h1 {
            margin: 0 0 10px 0;
            color: #2c3e50;
        }
        .metadata {
            color: #666;
            font-size: 14px;
        }
        .section {
            background-color: white;
            padding: 25px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            margin-bottom: 20px;
        }
        h2 {
            color: #34495e;
            border-bottom: 2px solid #ecf0f1;
            padding-bottom: 10px;
            margin-bottom: 20px;
        }
        .metric {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 10px 0;
            border-bottom: 1px solid #ecf0f1;
        }
        .metric:last-child {
            border-bottom: none;
        }
        .metric-name {
            font-weight: 500;
        }
        .metric-value {
            font-size: 18px;
            font-weight: bold;
            color: #2c3e50;
        }
        .metric-change {
            font-size: 14px;
            margin-left: 10px;
        }
        .metric-change.positive {
            color: #27ae60;
        }
        .metric-change.negative {
            color: #e74c3c;
        }
        .chart-container {
            margin: 20px 0;
            height: 300px;
            background-color: #f8f9fa;
            border-radius: 4px;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #666;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 20px;
        }
        th, td {
            text-align: left;
            padding: 12px;
            border-bottom: 1px solid #ecf0f1;
        }
        th {
            background-color: #f8f9fa;
            font-weight: 600;
        }
        .finding {
            margin: 15px 0;
            padding: 15px;
            border-left: 4px solid;
            background-color: #f8f9fa;
            border-radius: 4px;
        }
        .finding.critical {
            border-color: #e74c3c;
        }
        .finding.high {
            border-color: #e67e22;
        }
        .finding.medium {
            border-color: #f39c12;
        }
        .finding.low {
            border-color: #27ae60;
        }
        .finding.info {
            border-color: #3498db;
        }
        .recommendations {
            background-color: #ecf0f1;
            padding: 20px;
            border-radius: 4px;
            margin-top: 20px;
        }
        .recommendations ul {
            margin: 10px 0;
            padding-left: 20px;
        }
    </style>
</head>
<body>
    <div class="report-header">
        <h1>{{ report.title }}</h1>
        <div class="metadata">
            <p><strong>Generated:</strong> {{ report.generated_at | date(format="%Y-%m-%d %H:%M:%S UTC") }}</p>
            <p><strong>Period:</strong> {{ report.timeframe.start | date(format="%Y-%m-%d") }} to {{ report.timeframe.end | date(format="%Y-%m-%d") }}</p>
        </div>
    </div>

    {% for section in report.sections %}
    <div class="section">
        {% if section.type == "Summary" %}
            <h2>{{ section.title }}</h2>
            {% for metric in section.metrics %}
            <div class="metric">
                <span class="metric-name">{{ metric.name }}</span>
                <div>
                    <span class="metric-value">
                        {{ metric.value | round(method="common", precision=2) }}
                        {% if metric.unit %}{{ metric.unit }}{% endif %}
                    </span>
                    {% if metric.change %}
                    <span class="metric-change {% if metric.change > 0 %}positive{% else %}negative{% endif %}">
                        {% if metric.change > 0 %}↑{% else %}↓{% endif %}
                        {{ metric.change | abs | round(method="common", precision=1) }}%
                    </span>
                    {% endif %}
                </div>
            </div>
            {% endfor %}
        {% elif section.type == "Chart" %}
            <h2>{{ section.title }}</h2>
            <div class="chart-container">
                <p>Chart: {{ section.chart_type }}</p>
            </div>
        {% elif section.type == "Table" %}
            <h2>{{ section.title }}</h2>
            <table>
                <thead>
                    <tr>
                        {% for column in section.columns %}
                        <th>{{ column.name }}</th>
                        {% endfor %}
                    </tr>
                </thead>
                <tbody>
                    {% for row in section.rows %}
                    <tr>
                        {% for column in section.columns %}
                        <td>{{ row[column.field] | default(value="-") }}</td>
                        {% endfor %}
                    </tr>
                    {% endfor %}
                </tbody>
            </table>
        {% elif section.type == "Insights" %}
            <h2>{{ section.title }}</h2>
            <p>{{ section.content }}</p>
        {% elif section.type == "Analysis" %}
            <h2>{{ section.title }}</h2>
            {% if section.analysis.findings %}
                <h3>Findings</h3>
                {% for finding in section.analysis.findings %}
                <div class="finding {{ finding.severity | lower }}">
                    <strong>{{ finding.title }}</strong>
                    <p>{{ finding.description }}</p>
                </div>
                {% endfor %}
            {% endif %}
            {% if section.analysis.recommendations %}
                <div class="recommendations">
                    <h3>Recommendations</h3>
                    <ul>
                        {% for rec in section.analysis.recommendations %}
                        <li>{{ rec }}</li>
                        {% endfor %}
                    </ul>
                </div>
            {% endif %}
        {% endif %}
    </div>
    {% endfor %}
</body>
</html>
"#;
