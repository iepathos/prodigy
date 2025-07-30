//! Analyze command implementation

use crate::context::{save_analysis, ContextAnalyzer, ProjectAnalyzer};
use anyhow::Result;
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
    let project_path = cmd
        .path
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

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

/// Run metrics analysis (placeholder for spec 46)
async fn run_metrics_analysis(_project_path: &std::path::Path, cmd: &AnalyzeCommand) -> Result<()> {
    if cmd.verbose {
        println!("\n📈 Running metrics analysis...");
    }

    // TODO: Implement metrics analysis as per spec 46
    println!("⚠️  Metrics analysis not yet implemented (coming in spec 46)");

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
    if analysis.test_coverage.file_coverage.is_empty()
        && analysis.test_coverage.overall_coverage == 0.0
    {
        println!("   ⚠️  No coverage data available");
        println!("   Install cargo-tarpaulin for coverage metrics:");
        println!("   cargo install cargo-tarpaulin");
    } else {
        let tested_files = analysis
            .test_coverage
            .file_coverage
            .iter()
            .filter(|(_, cov)| cov.coverage_percentage > 0.0)
            .count();
        let untested_files = analysis.test_coverage.file_coverage.len() - tested_files;
        println!("   Files with tests: {tested_files}");
        println!("   Files without tests: {untested_files}");
        println!(
            "   Untested functions: {}",
            analysis.test_coverage.untested_functions.len()
        );
        println!(
            "   Overall coverage: {:.1}%",
            analysis.test_coverage.overall_coverage * 100.0
        );
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
    if analysis.test_coverage.file_coverage.is_empty()
        && analysis.test_coverage.overall_coverage == 0.0
    {
        println!("   - No test coverage data available");
    } else {
        println!(
            "   - {:.1}% test coverage",
            analysis.test_coverage.overall_coverage * 100.0
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
