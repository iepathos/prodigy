//! Analyze command implementation

use crate::context::{save_analysis, ContextAnalyzer, ProjectAnalyzer};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Command structure for analyze subcommand
#[derive(Debug, Clone)]
pub struct AnalyzeCommand {
    pub analysis_type: String,
    pub output: String,
    pub save: bool,
    pub verbose: bool,
    pub path: Option<PathBuf>,
    pub run_coverage: bool,
}

/// Execute the analyze command
pub async fn execute(cmd: AnalyzeCommand) -> Result<()> {
    let project_path = match cmd.path.clone() {
        Some(path) => path,
        None => std::env::current_dir().context("Failed to get current directory")?,
    };

    println!("🔍 Analyzing project at: {}", project_path.display());

    match cmd.analysis_type.as_str() {
        "context" => run_context_analysis(&project_path, &cmd).await?,
        "metrics" => run_metrics_analysis(&project_path, &cmd).await?,
        "all" => {
            run_context_analysis(&project_path, &cmd).await?;
            run_metrics_analysis(&project_path, &cmd).await?;
        }
        _ => {
            eprintln!("Unknown analysis type: {}", cmd.analysis_type);
            eprintln!("Valid types: context, metrics, all");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Run context analysis
async fn run_context_analysis(project_path: &std::path::Path, cmd: &AnalyzeCommand) -> Result<()> {
    if cmd.verbose {
        println!("\n📊 Running context analysis...");
    }

    // Create analyzer
    let analyzer = ProjectAnalyzer::new();

    // Run analysis
    let analysis_result = analyzer.analyze(project_path).await?;

    // Get improvement suggestions
    let suggestions = analyzer.get_improvement_suggestions();

    // Save if requested
    if cmd.save {
        save_analysis(project_path, &analysis_result)?;
        if cmd.verbose {
            println!("💾 Analysis saved to .mmm/context/");
        }
    }

    // Display results based on output format
    match cmd.output.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&analysis_result)?);
        }
        "pretty" => {
            display_pretty_analysis(&analysis_result, &suggestions);
        }
        "summary" => {
            display_summary_analysis(&analysis_result, &suggestions);
        }
        _ => {
            eprintln!("Unknown output format: {}", cmd.output);
            eprintln!("Valid formats: json, pretty, summary");
        }
    }

    Ok(())
}

/// Run metrics analysis
async fn run_metrics_analysis(project_path: &std::path::Path, cmd: &AnalyzeCommand) -> Result<()> {
    if cmd.verbose {
        println!("\n📈 Running metrics analysis...");
    }

    // Create metrics collector
    let collector =
        crate::metrics::MetricsCollector::new(crate::subprocess::SubprocessManager::production());

    // Generate iteration ID (timestamp-based for now)
    let iteration_id = format!("manual-{}", chrono::Utc::now().timestamp());

    // Collect metrics
    let metrics = collector
        .collect_metrics(project_path, iteration_id)
        .await?;

    // Display results based on output format
    match cmd.output.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        "pretty" => {
            display_pretty_metrics(&metrics);
        }
        "summary" => {
            display_summary_metrics(&metrics);
        }
        _ => {
            display_summary_metrics(&metrics);
        }
    }

    // Save metrics if requested
    if cmd.save {
        let storage = crate::metrics::MetricsStorage::new(project_path);
        storage.save_current(&metrics)?;
        if cmd.verbose {
            println!("💾 Metrics saved to .mmm/metrics/");
        }
    }

    Ok(())
}

/// Display analysis in pretty format
fn display_pretty_analysis(
    analysis: &crate::context::AnalysisResult,
    suggestions: &[crate::context::Suggestion],
) {
    println!("\n=== Project Analysis Results ===\n");

    // Dependencies
    println!("📦 Dependencies:");
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
    if !analysis.dependency_graph.cycles.is_empty() {
        println!("   ⚠️  Circular dependencies detected:");
        for cycle in &analysis.dependency_graph.cycles {
            println!("      - {}", cycle.join(" → "));
        }
    }

    // Architecture
    println!("\n🏗️  Architecture:");
    println!(
        "   Patterns detected: {}",
        analysis.architecture.patterns.join(", ")
    );
    println!("   Components: {}", analysis.architecture.components.len());
    println!("   Violations: {}", analysis.architecture.violations.len());
    if !analysis.architecture.violations.is_empty() {
        println!("   ⚠️  Architecture violations:");
        for violation in &analysis.architecture.violations {
            println!("      - {}: {}", violation.rule, violation.description);
        }
    }

    // Conventions
    println!("\n📝 Conventions:");
    println!(
        "   File naming: {:?}",
        analysis.conventions.naming_patterns.file_naming
    );
    println!(
        "   Function naming: {:?}",
        analysis.conventions.naming_patterns.function_naming
    );
    println!(
        "   Code patterns: {}",
        analysis.conventions.code_patterns.len()
    );
    println!(
        "   Project idioms: {}",
        analysis.conventions.project_idioms.len()
    );

    // Technical Debt
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
    if !analysis.technical_debt.debt_items.is_empty() {
        println!("   Top debt items:");
        for (i, item) in analysis
            .technical_debt
            .debt_items
            .iter()
            .take(3)
            .enumerate()
        {
            println!(
                "      {}. {} ({:?})",
                i + 1,
                item.description,
                item.debt_type
            );
        }
    }

    // Test Coverage
    println!("\n🧪 Test Coverage:");
    if let Some(ref test_coverage) = analysis.test_coverage {
        if test_coverage.file_coverage.is_empty() && test_coverage.overall_coverage == 0.0 {
            println!("   ⚠️  No coverage data available");
            println!("   Install cargo-tarpaulin for coverage metrics:");
            println!("   cargo install cargo-tarpaulin");
        } else {
            let tested_files = test_coverage
                .file_coverage
                .iter()
                .filter(|(_, cov)| cov.coverage_percentage > 0.0)
                .count();
            let untested_files = test_coverage.file_coverage.len() - tested_files;
            println!("   Files with tests: {tested_files}");
            println!("   Files without tests: {untested_files}");
            println!(
                "   Untested functions: {}",
                test_coverage.untested_functions.len()
            );
            println!(
                "   Overall coverage: {:.1}%",
                test_coverage.overall_coverage * 100.0
            );
        }
    } else {
        println!("   ⚠️  No coverage data available");
        println!("   Install cargo-tarpaulin for coverage metrics:");
        println!("   cargo install cargo-tarpaulin");
    }

    // Suggestions
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

    // Metadata
    println!("\n⏱️  Analysis Metadata:");
    println!("   Duration: {}ms", analysis.metadata.duration_ms);
    println!("   Files analyzed: {}", analysis.metadata.files_analyzed);
    println!("   Timestamp: {}", analysis.metadata.timestamp);
}

/// Display analysis in summary format
fn display_summary_analysis(
    analysis: &crate::context::AnalysisResult,
    suggestions: &[crate::context::Suggestion],
) {
    println!("\n✅ Analysis complete!");
    println!(
        "   - {} modules analyzed",
        analysis.dependency_graph.nodes.len()
    );
    println!(
        "   - {} architectural patterns detected",
        analysis.architecture.patterns.len()
    );
    println!(
        "   - {} technical debt items found",
        analysis.technical_debt.debt_items.len()
    );
    if let Some(ref test_coverage) = analysis.test_coverage {
        if test_coverage.file_coverage.is_empty() && test_coverage.overall_coverage == 0.0 {
            println!("   - No test coverage data available");
        } else {
            println!(
                "   - {:.1}% test coverage",
                test_coverage.overall_coverage * 100.0
            );
        }
    } else {
        println!("   - No test coverage data available");
    }

    if !suggestions.is_empty() {
        println!(
            "\n💡 {} improvement suggestions available",
            suggestions.len()
        );
        println!("   Use --output=pretty to see details");
    }
}

/// Display metrics in pretty format
fn display_pretty_metrics(metrics: &crate::metrics::ImprovementMetrics) {
    println!("\n=== Metrics Analysis Results ===\n");

    // Code Quality
    println!("📊 Code Quality:");
    println!("   Test coverage: {:.1}%", metrics.test_coverage);
    println!("   Type coverage: {:.1}%", metrics.type_coverage);
    println!("   Lint warnings: {}", metrics.lint_warnings);
    println!("   Code duplication: {:.1}%", metrics.code_duplication);
    println!("   Documentation coverage: {:.1}%", metrics.doc_coverage);

    // Performance
    println!("\n⚡ Performance:");
    println!(
        "   Compile time: {:.2}s",
        metrics.compile_time.as_secs_f64()
    );
    println!(
        "   Binary size: {:.2} MB",
        metrics.binary_size as f64 / 1_048_576.0 // Safe: usize to f64 always fits
    );
    if !metrics.benchmark_results.is_empty() {
        println!("   Benchmarks:");
        for (name, duration) in &metrics.benchmark_results {
            println!("      - {}: {:.3}ms", name, duration.as_secs_f64() * 1000.0);
        }
    }

    // Complexity
    println!("\n🧩 Complexity:");
    println!("   Total lines: {}", metrics.total_lines);
    println!("   Max nesting depth: {}", metrics.max_nesting_depth);
    if !metrics.cyclomatic_complexity.is_empty() {
        let avg_cyclomatic = metrics.cyclomatic_complexity.values().sum::<u32>() as f32
            / metrics.cyclomatic_complexity.len() as f32;
        println!("   Average cyclomatic complexity: {avg_cyclomatic:.1}");
    }

    // Progress
    println!("\n📈 Progress:");
    println!("   Technical debt score: {:.1}", metrics.tech_debt_score);
    println!(
        "   Improvement velocity: {:.1}",
        metrics.improvement_velocity
    );
    println!(
        "   Overall quality score: {:.1}/100",
        metrics.overall_score()
    );

    // Metadata
    println!("\n⏱️  Metadata:");
    println!("   Timestamp: {}", metrics.timestamp);
    println!("   Iteration ID: {}", metrics.iteration_id);
}

/// Display metrics in summary format
fn display_summary_metrics(metrics: &crate::metrics::ImprovementMetrics) {
    println!("\n✅ Metrics analysis complete!");
    println!("📊 Test coverage: {:.1}%", metrics.test_coverage);
    println!("🛠️  Technical debt score: {:.1}", metrics.tech_debt_score);
    println!(
        "🚀 Improvement velocity: {:.1}",
        metrics.improvement_velocity
    );
    println!(
        "🎯 Overall quality score: {:.1}/100",
        metrics.overall_score()
    );
    println!(
        "⏱️  Compile time: {:.2}s",
        metrics.compile_time.as_secs_f64()
    );

    if metrics.lint_warnings > 0 {
        println!("⚠️  {} lint warnings found", metrics.lint_warnings);
    }

    println!("\n💡 Use --output=pretty for detailed metrics");
}
