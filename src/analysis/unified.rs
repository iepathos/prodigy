//! Unified analysis function for MMM
//!
//! Provides a single entry point for both command-line and workflow-based analysis,
//! ensuring consistent behavior and reducing code duplication.

use crate::context::{
    save_analysis_with_options, AnalysisResult, ContextAnalyzer, ProjectAnalyzer,
};
use crate::metrics::{ImprovementMetrics, MetricsCollector, MetricsStorage};
use crate::scoring::ProjectHealthScore;
use crate::subprocess::SubprocessManager;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

/// Output format options for analysis results
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    /// JSON output format
    Json,
    /// Pretty human-readable format
    Pretty,
    /// Condensed summary format
    Summary,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "pretty" => Ok(Self::Pretty),
            "summary" => Ok(Self::Summary),
            _ => Ok(Self::Summary),
        }
    }
}

/// Configuration for unified analysis
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Output format for results
    pub output_format: OutputFormat,
    /// Save results to disk
    pub save_results: bool,
    /// Commit changes to git
    pub commit_changes: bool,
    /// Force refresh (ignore cache)
    pub force_refresh: bool,
    /// Run metrics analysis
    pub run_metrics: bool,
    /// Run context analysis
    pub run_context: bool,
    /// Verbose output
    pub verbose: bool,
    /// Run coverage analysis
    pub run_coverage: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Summary,
            save_results: false,
            commit_changes: false,
            force_refresh: false,
            run_metrics: true,
            run_context: true,
            verbose: false,
            run_coverage: true,
        }
    }
}

impl AnalysisConfig {
    /// Create a new builder
    pub fn builder() -> AnalysisConfigBuilder {
        AnalysisConfigBuilder::new()
    }
}

/// Builder for AnalysisConfig
#[derive(Default)]
pub struct AnalysisConfigBuilder {
    config: AnalysisConfig,
}


impl AnalysisConfigBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set output format
    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.config.output_format = format;
        self
    }

    /// Set save results
    pub fn save_results(mut self, save: bool) -> Self {
        self.config.save_results = save;
        self
    }

    /// Set commit changes
    pub fn commit_changes(mut self, commit: bool) -> Self {
        self.config.commit_changes = commit;
        self
    }

    /// Set force refresh
    pub fn force_refresh(mut self, force: bool) -> Self {
        self.config.force_refresh = force;
        self
    }

    /// Set run metrics
    pub fn run_metrics(mut self, run: bool) -> Self {
        self.config.run_metrics = run;
        self
    }

    /// Set run context
    pub fn run_context(mut self, run: bool) -> Self {
        self.config.run_context = run;
        self
    }

    /// Set verbose
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    /// Set run coverage
    pub fn run_coverage(mut self, run: bool) -> Self {
        self.config.run_coverage = run;
        self
    }

    /// Build the configuration
    pub fn build(self) -> AnalysisConfig {
        self.config
    }
}

/// Results from unified analysis
#[derive(Debug)]
pub struct AnalysisResults {
    /// Metrics analysis results
    pub metrics: Option<ImprovementMetrics>,
    /// Context analysis results
    pub context: Option<AnalysisResult>,
    /// Improvement suggestions
    pub suggestions: Vec<ImprovementSuggestion>,
    /// Overall health score
    pub health_score: f64,
    /// Analysis timing information
    pub timing: AnalysisTiming,
}

/// Timing information for analysis
#[derive(Debug)]
pub struct AnalysisTiming {
    /// Total duration
    pub total: std::time::Duration,
    /// Metrics analysis duration
    pub metrics_duration: Option<std::time::Duration>,
    /// Context analysis duration
    pub context_duration: Option<std::time::Duration>,
}

/// Improvement suggestion
#[derive(Debug)]
pub struct ImprovementSuggestion {
    /// Title of the suggestion
    pub title: String,
    /// Description of the suggestion
    pub description: String,
    /// Priority level
    pub priority: Priority,
    /// Estimated impact
    pub estimated_impact: Impact,
}

/// Priority levels for suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Medium,
    Low,
}

/// Impact levels for suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impact {
    High,
    Medium,
    Low,
}

/// Trait for progress reporting
#[async_trait]
pub trait ProgressReporter: Send + Sync {
    /// Display progress message
    fn display_progress(&self, message: &str);
    /// Display info message
    fn display_info(&self, message: &str);
    /// Display warning message
    fn display_warning(&self, message: &str);
    /// Display success message
    fn display_success(&self, message: &str);
}

/// Default progress reporter that prints to stdout
pub struct DefaultProgressReporter;

impl ProgressReporter for DefaultProgressReporter {
    fn display_progress(&self, message: &str) {
        println!("🔄 {message}");
    }

    fn display_info(&self, message: &str) {
        println!("ℹ️  {message}");
    }

    fn display_warning(&self, message: &str) {
        println!("⚠️  {message}");
    }

    fn display_success(&self, message: &str) {
        println!("✅ {message}");
    }
}

/// Run unified analysis on a project
pub async fn run_analysis(
    project_path: &Path,
    config: AnalysisConfig,
    subprocess: SubprocessManager,
    progress: Arc<dyn ProgressReporter>,
) -> Result<AnalysisResults> {
    let start_time = std::time::Instant::now();
    let mut metrics_duration = None;
    let mut context_duration = None;

    // Run metrics analysis first if enabled (to collect test coverage data)
    let metrics = if config.run_metrics {
        if config.verbose {
            progress.display_progress("Running metrics analysis...");
        }
        let metrics_start = std::time::Instant::now();

        let collector = MetricsCollector::new(subprocess.clone());
        let iteration_id = format!("analysis-{}", chrono::Utc::now().timestamp());
        let mut metrics = collector
            .collect_metrics(project_path, iteration_id)
            .await?;

        // Calculate and add health score to metrics
        let health_score = ProjectHealthScore::from_metrics(&metrics);
        metrics.health_score = Some(health_score);

        metrics_duration = Some(metrics_start.elapsed());

        // Save metrics if requested
        if config.save_results {
            let storage = MetricsStorage::new(project_path);
            let commit_made = storage.save_current_with_commit(&metrics, config.commit_changes)?;
            if config.verbose {
                progress.display_info("Metrics saved to .mmm/metrics/");
                if commit_made {
                    progress.display_success("Metrics committed to git");
                }
            }
        }

        Some(metrics)
    } else {
        None
    };

    // Run context analysis if enabled
    let context = if config.run_context {
        if config.verbose {
            progress.display_progress("Running context analysis...");
        }
        let context_start = std::time::Instant::now();

        let analyzer = ProjectAnalyzer::new();
        let analysis_result = analyzer.analyze(project_path).await?;

        context_duration = Some(context_start.elapsed());

        // Save context if requested
        if config.save_results {
            let commit_made =
                save_analysis_with_options(project_path, &analysis_result, config.commit_changes)?;
            if config.verbose {
                progress.display_info("Analysis saved to .mmm/context/");
                if commit_made {
                    progress.display_success("Analysis committed to git");
                }
            }
        }

        Some(analysis_result)
    } else {
        None
    };

    // Commit all changes together if both analyses were run and saved
    if config.save_results && config.commit_changes && metrics.is_some() && context.is_some() {
        commit_all_analysis(project_path, metrics.as_ref(), context.as_ref())?;
    }

    // Calculate overall health score
    let health_score = if let Some(ref ctx) = context {
        let score = ProjectHealthScore::from_context(ctx);
        score.overall
    } else if let Some(ref m) = metrics {
        m.health_score.as_ref().map(|s| s.overall).unwrap_or(0.0)
    } else {
        0.0
    };

    // Collect suggestions
    let suggestions = collect_suggestions(&metrics, &context);

    // Display results based on format
    display_results(&config, &metrics, &context, &suggestions, progress.as_ref());

    Ok(AnalysisResults {
        metrics,
        context,
        suggestions,
        health_score,
        timing: AnalysisTiming {
            total: start_time.elapsed(),
            metrics_duration,
            context_duration,
        },
    })
}

/// Collect improvement suggestions from analysis results
fn collect_suggestions(
    metrics: &Option<ImprovementMetrics>,
    context: &Option<AnalysisResult>,
) -> Vec<ImprovementSuggestion> {
    let mut suggestions = Vec::new();

    // Add suggestions from metrics
    if let Some(m) = metrics {
        if m.test_coverage < 60.0 {
            suggestions.push(ImprovementSuggestion {
                title: "Increase test coverage".to_string(),
                description: format!(
                    "Current coverage is {:.1}%. Aim for at least 80%",
                    m.test_coverage
                ),
                priority: Priority::High,
                estimated_impact: Impact::High,
            });
        }

        if m.lint_warnings > 10 {
            suggestions.push(ImprovementSuggestion {
                title: "Fix lint warnings".to_string(),
                description: format!(
                    "Found {} lint warnings. Run 'cargo clippy --fix'",
                    m.lint_warnings
                ),
                priority: Priority::Medium,
                estimated_impact: Impact::Medium,
            });
        }
    }

    // Add suggestions from context
    if let Some(ctx) = context {
        let analyzer = ProjectAnalyzer::new();
        for suggestion in analyzer.get_improvement_suggestions() {
            suggestions.push(ImprovementSuggestion {
                title: suggestion.title,
                description: suggestion.description,
                priority: Priority::High,
                estimated_impact: Impact::Medium,
            });
        }

        if !ctx.dependency_graph.cycles.is_empty() {
            suggestions.push(ImprovementSuggestion {
                title: "Resolve circular dependencies".to_string(),
                description: format!(
                    "Found {} circular dependencies that should be resolved",
                    ctx.dependency_graph.cycles.len()
                ),
                priority: Priority::High,
                estimated_impact: Impact::High,
            });
        }
    }

    suggestions
}

/// Display analysis results based on configured format
fn display_results(
    config: &AnalysisConfig,
    metrics: &Option<ImprovementMetrics>,
    context: &Option<AnalysisResult>,
    suggestions: &[ImprovementSuggestion],
    progress: &dyn ProgressReporter,
) {
    match config.output_format {
        OutputFormat::Json => display_json_results(metrics, context),
        OutputFormat::Pretty => display_pretty_results(metrics, context, suggestions),
        OutputFormat::Summary => display_summary_results(metrics, context, suggestions, progress),
    }
}

/// Display results in JSON format
fn display_json_results(metrics: &Option<ImprovementMetrics>, context: &Option<AnalysisResult>) {
    let output = serde_json::json!({
        "metrics": metrics,
        "context": context,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

/// Display results in pretty format
fn display_pretty_results(
    metrics: &Option<ImprovementMetrics>,
    context: &Option<AnalysisResult>,
    suggestions: &[ImprovementSuggestion],
) {
    println!("\n=== Analysis Results ===\n");

    // Display metrics if available
    if let Some(m) = metrics {
        display_pretty_metrics_inline(m);
    }

    // Display context if available
    if let Some(ctx) = context {
        display_pretty_analysis_inline(ctx);
    }

    // Display suggestions
    if !suggestions.is_empty() {
        println!("\n💡 Improvement Suggestions:");
        for (i, suggestion) in suggestions.iter().take(5).enumerate() {
            println!(
                "   {}. {} - {}",
                i + 1,
                suggestion.title,
                suggestion.description
            );
            println!(
                "      Priority: {:?}, Impact: {:?}",
                suggestion.priority, suggestion.estimated_impact
            );
        }
    }
}

/// Display metrics in pretty format (inline version)
fn display_pretty_metrics_inline(metrics: &ImprovementMetrics) {
    println!("📊 Code Quality:");
    println!("   Test coverage: {:.1}%", metrics.test_coverage);
    println!("   Type coverage: {:.1}%", metrics.type_coverage);
    println!("   Lint warnings: {}", metrics.lint_warnings);
    println!("   Code duplication: {:.1}%", metrics.code_duplication);
    println!("   Documentation coverage: {:.1}%", metrics.doc_coverage);

    println!("\n⚡ Performance:");
    println!(
        "   Compile time: {:.2}s",
        metrics.compile_time.as_secs_f64()
    );
    println!(
        "   Binary size: {:.2} MB",
        metrics.binary_size as f64 / 1_048_576.0
    );

    println!("\n🧩 Complexity:");
    println!("   Total lines: {}", metrics.total_lines);
    println!("   Max nesting depth: {}", metrics.max_nesting_depth);

    if let Some(ref health_score) = metrics.health_score {
        println!("\n📊 Metrics Health Score: {:.1}/100", health_score.overall);
    }
}

/// Display context analysis in pretty format (inline version)
fn display_pretty_analysis_inline(analysis: &AnalysisResult) {
    println!("\n📦 Dependencies:");
    println!(
        "   Modules analyzed: {}",
        analysis.dependency_graph.nodes.len()
    );
    println!(
        "   Dependencies found: {}",
        analysis.dependency_graph.edges.len()
    );
    println!(
        "   Circular dependencies: {}",
        analysis.dependency_graph.cycles.len()
    );

    println!("\n🏗️  Architecture:");
    println!(
        "   Patterns detected: {}",
        analysis.architecture.patterns.join(", ")
    );
    println!("   Components: {}", analysis.architecture.components.len());
    println!("   Violations: {}", analysis.architecture.violations.len());

    println!("\n💸 Technical Debt:");
    println!(
        "   Debt items: {}",
        analysis.technical_debt.debt_items.len()
    );
    println!(
        "   Complexity hotspots: {}",
        analysis.technical_debt.hotspots.len()
    );
    println!(
        "   Code duplication areas: {}",
        analysis.technical_debt.duplication_map.len()
    );

    let health_score = ProjectHealthScore::from_context(analysis);
    println!("\n📊 Context Health Score: {:.1}/100", health_score.overall);
}

/// Display results in summary format
fn display_summary_results(
    metrics: &Option<ImprovementMetrics>,
    context: &Option<AnalysisResult>,
    suggestions: &[ImprovementSuggestion],
    progress: &dyn ProgressReporter,
) {
    progress.display_success("Analysis complete!");

    if let Some(m) = metrics {
        if let Some(ref health_score) = m.health_score {
            println!("📊 Metrics Health Score: {:.1}/100", health_score.overall);
        }
        println!("📊 Test coverage: {:.1}%", m.test_coverage);
        if m.lint_warnings > 0 {
            println!("⚠️  {} lint warnings found", m.lint_warnings);
        }
    }

    if let Some(ctx) = context {
        let health_score = ProjectHealthScore::from_context(ctx);
        println!("📊 Context Health Score: {:.1}/100", health_score.overall);
        println!("   - {} modules analyzed", ctx.dependency_graph.nodes.len());
        println!(
            "   - {} technical debt items",
            ctx.technical_debt.debt_items.len()
        );
    }

    if !suggestions.is_empty() {
        println!(
            "\n💡 {} improvement suggestions available",
            suggestions.len()
        );
        println!("   Use --output=pretty to see details");
    }
}

/// Commit all analysis files together
fn commit_all_analysis(
    project_path: &Path,
    metrics: Option<&ImprovementMetrics>,
    context: Option<&AnalysisResult>,
) -> Result<()> {
    use crate::scoring;

    // Check if we're in a git repository
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(project_path)
        .output()?;

    if !git_check.status.success() {
        return Ok(());
    }

    // Stage all analysis files
    let add_status = std::process::Command::new("git")
        .args(["add", ".mmm/"])
        .current_dir(project_path)
        .output()?;

    if !add_status.status.success() {
        eprintln!(
            "Failed to stage files: {}",
            String::from_utf8_lossy(&add_status.stderr)
        );
        return Ok(());
    }

    // Check if there are changes to commit
    let git_status = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(project_path)
        .output()?;

    if git_status.stdout.is_empty() {
        return Ok(());
    }

    // Calculate health score
    let health_score = if let Some(ctx) = context {
        scoring::ProjectHealthScore::from_context(ctx)
    } else if let Some(m) = metrics {
        m.health_score
            .clone()
            .unwrap_or_else(|| ProjectHealthScore {
                overall: 0.0,
                components: crate::scoring::ScoreComponents {
                    test_coverage: None,
                    code_quality: None,
                    maintainability: None,
                    documentation: None,
                    type_safety: None,
                },
                timestamp: chrono::Utc::now(),
            })
    } else {
        ProjectHealthScore {
            overall: 0.0,
            components: crate::scoring::ScoreComponents {
                test_coverage: None,
                code_quality: None,
                maintainability: None,
                documentation: None,
                type_safety: None,
            },
            timestamp: chrono::Utc::now(),
        }
    };

    // Create commit message
    let commit_msg = if let (Some(ctx), Some(m)) = (context, metrics) {
        format!(
            "analysis: update project context and metrics (health: {:.1}/100)\n\n\
            📊 Context Health Score: {:.1}/100\n\
            Test coverage: {:.1}%\n\
            Code quality: {:.1}%\n\
            Maintainability: {:.1}%\n\n\
            📈 Analysis Summary:\n\
            - {} modules analyzed\n\
            - {} dependencies mapped\n\
            - {} architectural violations\n\
            - {} technical debt items\n\
            - {} lint warnings\n\n\
            Generated by MMM v{}",
            health_score.overall,
            health_score.overall,
            m.test_coverage,
            health_score.components.code_quality.unwrap_or(0.0),
            health_score.components.maintainability.unwrap_or(0.0),
            ctx.dependency_graph.nodes.len(),
            ctx.dependency_graph.edges.len(),
            ctx.architecture.violations.len(),
            ctx.technical_debt.debt_items.len(),
            m.lint_warnings,
            env!("CARGO_PKG_VERSION")
        )
    } else if let Some(_ctx) = context {
        format!(
            "analysis: update project context (health: {:.1}/100)",
            health_score.overall
        )
    } else if let Some(m) = metrics {
        format!(
            "analysis: update project metrics (coverage: {:.1}%)",
            m.test_coverage
        )
    } else {
        "analysis: update project analysis".to_string()
    };

    let git_commit = std::process::Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(project_path)
        .output()?;

    if !git_commit.status.success() {
        eprintln!(
            "Git commit failed: {}",
            String::from_utf8_lossy(&git_commit.stderr)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct MockProgressReporter {
        messages: std::sync::Mutex<Vec<String>>,
    }

    impl MockProgressReporter {
        fn new() -> Self {
            Self {
                messages: std::sync::Mutex::new(Vec::new()),
            }
        }

        #[allow(dead_code)]
        fn get_messages(&self) -> Vec<String> {
            self.messages.lock().unwrap().clone()
        }
    }

    impl ProgressReporter for MockProgressReporter {
        fn display_progress(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("PROGRESS: {message}"));
        }

        fn display_info(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("INFO: {message}"));
        }

        fn display_warning(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("WARNING: {message}"));
        }

        fn display_success(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SUCCESS: {message}"));
        }
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
        assert_eq!(
            "unknown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Summary
        );
    }

    #[test]
    fn test_config_builder() {
        let config = AnalysisConfig::builder()
            .output_format(OutputFormat::Json)
            .save_results(true)
            .commit_changes(true)
            .force_refresh(true)
            .run_metrics(false)
            .run_context(true)
            .verbose(true)
            .build();

        assert_eq!(config.output_format, OutputFormat::Json);
        assert!(config.save_results);
        assert!(config.commit_changes);
        assert!(config.force_refresh);
        assert!(!config.run_metrics);
        assert!(config.run_context);
        assert!(config.verbose);
    }

    #[test]
    fn test_default_config() {
        let config = AnalysisConfig::default();
        assert_eq!(config.output_format, OutputFormat::Summary);
        assert!(!config.save_results);
        assert!(!config.commit_changes);
        assert!(!config.force_refresh);
        assert!(config.run_metrics);
        assert!(config.run_context);
        assert!(!config.verbose);
    }

    #[tokio::test]
    async fn test_run_analysis_basic() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create a basic Rust project
        std::fs::write(
            project_path.join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();
        std::fs::create_dir_all(project_path.join("src")).unwrap();
        std::fs::write(project_path.join("src/main.rs"), "fn main() {}").unwrap();

        let config = AnalysisConfig::builder()
            .output_format(OutputFormat::Summary)
            .save_results(false)
            .verbose(false)
            .build();

        let subprocess = SubprocessManager::production();
        let progress = Arc::new(MockProgressReporter::new());

        let result = run_analysis(project_path, config, subprocess, progress).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        assert!(results.metrics.is_some());
        assert!(results.context.is_some());
        assert!(results.health_score >= 0.0);
        assert!(results.timing.total > std::time::Duration::ZERO);
    }

    #[test]
    fn test_collect_suggestions() {
        // Test with low coverage metrics
        let metrics = Some(ImprovementMetrics {
            test_coverage: 30.0,
            lint_warnings: 15,
            ..Default::default()
        });

        let suggestions = collect_suggestions(&metrics, &None);
        assert!(!suggestions.is_empty());

        // Should have suggestions for low coverage and lint warnings
        let coverage_suggestion = suggestions
            .iter()
            .any(|s| s.title.contains("test coverage"));
        assert!(coverage_suggestion);

        let lint_suggestion = suggestions.iter().any(|s| s.title.contains("lint"));
        assert!(lint_suggestion);
    }
}
