//! Analyze command implementation

use crate::context::{save_analysis_with_options, ContextAnalyzer, ProjectAnalyzer};
use crate::subprocess::SubprocessManager;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Command structure for analyze subcommand
#[derive(Debug, Clone)]
pub struct AnalyzeCommand {
    pub analysis_type: String,
    pub output: String,
    pub save: bool,
    pub verbose: bool,
    pub path: Option<PathBuf>,
    pub run_coverage: bool,
    pub no_commit: bool,
}

/// Execute the analyze command with production subprocess manager
pub async fn execute(cmd: AnalyzeCommand) -> Result<()> {
    execute_with_subprocess(cmd, SubprocessManager::production()).await
}

/// Execute the analyze command with injected subprocess manager
pub async fn execute_with_subprocess(
    cmd: AnalyzeCommand,
    subprocess: SubprocessManager,
) -> Result<()> {
    let project_path = match cmd.path.clone() {
        Some(path) => path,
        None => std::env::current_dir().context("Failed to get current directory")?,
    };

    println!("🔍 Analyzing project at: {}", project_path.display());

    match cmd.analysis_type.as_str() {
        "context" => {
            // Run metrics first (without committing) so context can use test coverage
            let mut cmd_no_commit = cmd.clone();
            cmd_no_commit.no_commit = true;
            cmd_no_commit.save = true; // Always save metrics for context to use
            
            if cmd.verbose {
                println!("📊 Collecting metrics first for complete context analysis...");
            }
            
            // Run metrics silently
            run_metrics_analysis_silent(&project_path, &cmd_no_commit, subprocess.clone()).await?;
            
            // Now run context analysis with normal commit settings
            run_context_analysis(&project_path, &cmd, subprocess).await?
        }
        "metrics" => run_metrics_analysis(&project_path, &cmd, subprocess).await?,
        "all" => {
            // For "all" mode, we want to commit everything together
            // So we temporarily disable auto-commit for individual analyses
            let mut cmd_no_commit = cmd.clone();
            cmd_no_commit.no_commit = true;

            // Run metrics first so context analysis can use test coverage data
            run_metrics_analysis(&project_path, &cmd_no_commit, subprocess.clone()).await?;
            run_context_analysis(&project_path, &cmd_no_commit, subprocess).await?;

            // Now commit everything together if save is enabled and no_commit is false
            if cmd.save && !cmd.no_commit {
                commit_all_analysis(&project_path)?;
            }
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
async fn run_context_analysis(
    project_path: &std::path::Path,
    cmd: &AnalyzeCommand,
    _subprocess: SubprocessManager,
) -> Result<()> {
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
        let commit_made =
            save_analysis_with_options(project_path, &analysis_result, !cmd.no_commit)?;
        if cmd.verbose {
            println!("💾 Analysis saved to .mmm/context/");
        }
        if commit_made {
            println!("✅ Analysis committed to git");
        } else if !cmd.no_commit && cmd.verbose {
            println!("ℹ️  No changes to commit or not a git repository");
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
async fn run_metrics_analysis(
    project_path: &std::path::Path,
    cmd: &AnalyzeCommand,
    subprocess: SubprocessManager,
) -> Result<()> {
    if cmd.verbose {
        println!("\n📈 Running metrics analysis...");
    }

    // Create metrics collector with injected subprocess
    let collector = crate::metrics::MetricsCollector::new(subprocess);

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
        let commit_made = storage.save_current_with_commit(&metrics, !cmd.no_commit)?;
        if cmd.verbose {
            println!("💾 Metrics saved to .mmm/metrics/");
            if commit_made {
                println!("📝 Changes committed to git");
            }
        }
    }

    Ok(())
}

/// Run metrics analysis silently (for use as prerequisite)
async fn run_metrics_analysis_silent(
    project_path: &std::path::Path,
    cmd: &AnalyzeCommand,
    subprocess: SubprocessManager,
) -> Result<()> {
    // Create metrics collector with injected subprocess
    let collector = crate::metrics::MetricsCollector::new(subprocess);

    // Generate iteration ID (timestamp-based for now)
    let iteration_id = format!("manual-{}", chrono::Utc::now().timestamp());

    // Collect metrics
    let metrics = collector
        .collect_metrics(project_path, iteration_id)
        .await?;

    // Save metrics without any output
    if cmd.save {
        let storage = crate::metrics::MetricsStorage::new(project_path);
        let _ = storage.save_current_with_commit(&metrics, false)?;
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

    // Calculate and display technical debt score
    let debt_score =
        crate::scoring::calculate_technical_debt_score(&analysis.technical_debt.debt_items);
    println!("\n   📊 Technical Debt Score: {debt_score:.1}/100");

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

    // Calculate and display unified health score
    let health_score = crate::scoring::ProjectHealthScore::from_context(analysis);
    println!("\n📊 Context Health Score: {:.1}/100", health_score.overall);
    println!("   (Based on static analysis: architecture, dependencies, technical debt)");

    println!("\nScore Components:");
    use crate::scoring::format_component;

    println!(
        "{}",
        format_component("Test Coverage", health_score.components.test_coverage, None)
    );
    println!(
        "{}",
        format_component("Code Quality", health_score.components.code_quality, None)
    );
    println!(
        "{}",
        format_component(
            "Maintainability",
            health_score.components.maintainability,
            None
        )
    );
    println!(
        "{}",
        format_component("Documentation", health_score.components.documentation, None)
    );

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

    // Display both scores in summary
    let health_score = crate::scoring::ProjectHealthScore::from_context(analysis);
    let debt_score =
        crate::scoring::calculate_technical_debt_score(&analysis.technical_debt.debt_items);
    println!("\n📊 Scores:");
    println!("   - Context Health: {:.1}/100", health_score.overall);
    println!("   - Technical Debt: {debt_score:.1}/100");

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
    println!(
        "   Improvement velocity: {:.1}",
        metrics.improvement_velocity
    );

    // Display unified health score
    if let Some(ref health_score) = metrics.health_score {
        println!("\n📊 Metrics Health Score: {:.1}/100", health_score.overall);
        println!("   (Based on runtime metrics: lint warnings, complexity, test coverage)");
        println!("\nComponents:");

        use crate::scoring::format_component;

        println!(
            "{}",
            format_component("Test Coverage", health_score.components.test_coverage, None)
        );
        println!(
            "{}",
            format_component(
                "Code Quality",
                health_score.components.code_quality,
                Some(&format!("({} warnings)", metrics.lint_warnings))
            )
        );
        println!(
            "{}",
            format_component(
                "Maintainability",
                health_score.components.maintainability,
                None
            )
        );
        println!(
            "{}",
            format_component("Documentation", health_score.components.documentation, None)
        );
        println!(
            "{}",
            format_component("Type Safety", health_score.components.type_safety, None)
        );

        let suggestions = health_score.get_improvement_suggestions();
        if !suggestions.is_empty() {
            println!("\n💡 Top improvements:");
            for (i, suggestion) in suggestions.iter().enumerate() {
                println!("  {}. {}", i + 1, suggestion);
            }
        }
    } else {
        // Fallback to old display
        println!(
            "   Overall quality score: {:.1}/100",
            metrics.overall_score()
        );
    }

    // Metadata
    println!("\n⏱️  Metadata:");
    println!("   Timestamp: {}", metrics.timestamp);
    println!("   Iteration ID: {}", metrics.iteration_id);
}

/// Display metrics in summary format
fn display_summary_metrics(metrics: &crate::metrics::ImprovementMetrics) {
    println!("\n✅ Metrics analysis complete!");

    // Display unified health score
    if let Some(ref health_score) = metrics.health_score {
        println!("📊 Metrics Health Score: {:.1}/100", health_score.overall);
        println!("📊 Test coverage: {:.1}%", metrics.test_coverage);
        println!(
            "🚀 Improvement velocity: {:.1}",
            metrics.improvement_velocity
        );
    } else {
        // Fallback display
        println!("📊 Test coverage: {:.1}%", metrics.test_coverage);
        println!(
            "🚀 Improvement velocity: {:.1}",
            metrics.improvement_velocity
        );
        println!(
            "🎯 Overall quality score: {:.1}/100",
            metrics.overall_score()
        );
    }
    println!(
        "⏱️  Compile time: {:.2}s",
        metrics.compile_time.as_secs_f64()
    );

    if metrics.lint_warnings > 0 {
        println!("⚠️  {} lint warnings found", metrics.lint_warnings);
    }

    println!("\n💡 Use --output=pretty for detailed metrics");
}

/// Commit all analysis files (context and metrics) together
fn commit_all_analysis(project_path: &Path) -> Result<()> {
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
    // First, let's try adding the entire .mmm directory to catch all changes
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
        println!("No staged changes to commit");
        return Ok(());
    }

    let staged_count = String::from_utf8_lossy(&git_status.stdout).lines().count();
    println!("Found {staged_count} staged files");

    // Calculate health score from the saved analysis
    let analysis = crate::context::load_analysis(project_path)?
        .ok_or_else(|| anyhow::anyhow!("No analysis data found"))?;
    let health_score = scoring::ProjectHealthScore::from_context(&analysis);

    // Load metrics for the commit message
    let storage = crate::metrics::MetricsStorage::new(project_path);
    let metrics = storage
        .load_current()?
        .unwrap_or_else(crate::metrics::ImprovementMetrics::default);

    // Create comprehensive commit message
    let commit_msg = format!(
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
        metrics.test_coverage,
        health_score.components.code_quality.unwrap_or(0.0),
        health_score.components.maintainability.unwrap_or(0.0),
        analysis.dependency_graph.nodes.len(),
        analysis.dependency_graph.edges.len(),
        analysis.architecture.violations.len(),
        analysis.technical_debt.debt_items.len(),
        metrics.lint_warnings,
        env!("CARGO_PKG_VERSION")
    );

    println!("Creating commit with message length: {}", commit_msg.len());

    let mut git_commit = std::process::Command::new("git");
    git_commit
        .args(["commit", "-m", &commit_msg])
        .current_dir(project_path);

    match git_commit.output() {
        Ok(output) => {
            if output.status.success() {
                println!("📝 Committed analysis update (context + metrics)");
            } else {
                eprintln!("⚠️  Git commit failed with status: {:?}", output.status);
                eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to run git commit: {e}");
        }
    }

    Ok(())
}
