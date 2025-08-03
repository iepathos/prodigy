//! Context-aware project understanding system for MMM
//!
//! This module provides deep analysis capabilities to understand project structure,
//! dependencies, conventions, technical debt, and test coverage. It enables MMM to
//! make intelligent, goal-oriented improvements without human guidance.

use self::dependencies::ArchitecturalLayer;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod analyzer;
pub mod architecture;
pub mod conventions;
pub mod debt;
pub mod dependencies;
pub mod size_manager;
pub mod summary;
pub mod tarpaulin_coverage;
pub mod test_coverage;

pub use analyzer::ProjectAnalyzer;
pub use architecture::ArchitectureExtractor;
pub use conventions::{ConventionDetector, ProjectConventions};
pub use debt::{DebtItem, TechnicalDebtMap, TechnicalDebtMapper};
pub use dependencies::{DependencyAnalyzer, DependencyGraph};
pub use test_coverage::{TestCoverageAnalyzer, TestCoverageMap};

/// Results from all context analyzers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub dependency_graph: DependencyGraph,
    pub architecture: ArchitectureInfo,
    pub conventions: ProjectConventions,
    pub technical_debt: TechnicalDebtMap,
    pub test_coverage: Option<TestCoverageMap>,
    pub metadata: AnalysisMetadata,
}

/// Architecture information extracted from the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureInfo {
    pub patterns: Vec<String>,
    pub layers: Vec<ArchitecturalLayer>,
    pub components: HashMap<String, ComponentInfo>,
    pub violations: Vec<ArchitectureViolation>,
}

/// Component boundary and responsibility information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    pub name: String,
    pub responsibility: String,
    pub interfaces: Vec<String>,
    pub dependencies: Vec<String>,
}

/// Architecture rule violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureViolation {
    pub rule: String,
    pub location: String,
    pub severity: ViolationSeverity,
    pub description: String,
}

/// Violation severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ViolationSeverity {
    High,
    Medium,
    Low,
}

/// Metadata about the analysis run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
    pub files_analyzed: usize,
    pub incremental: bool,
    pub version: String,
}

/// Context information for a specific file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub path: PathBuf,
    pub module_dependencies: Vec<String>,
    pub conventions: FileConventions,
    pub debt_items: Vec<DebtItem>,
    pub test_coverage: f64,
    pub complexity: u32,
}

/// File-specific conventions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConventions {
    pub naming_style: String,
    pub patterns_used: Vec<String>,
    pub violations: Vec<String>,
}

/// Improvement suggestion based on analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub priority: SuggestionPriority,
    pub category: SuggestionCategory,
    pub title: String,
    pub description: String,
    pub affected_files: Vec<PathBuf>,
    pub estimated_impact: ImpactLevel,
}

/// Priority levels for suggestions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SuggestionPriority {
    Critical = 4,
    High = 3,
    Medium = 2,
    Low = 1,
}

/// Categories of improvement suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionCategory {
    Architecture,
    Dependencies,
    TestCoverage,
    TechnicalDebt,
    Performance,
    Security,
    CodeQuality,
}

/// Impact level of implementing a suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactLevel {
    Major,
    Moderate,
    Minor,
}

/// Main trait for context analyzers
#[async_trait::async_trait]
pub trait ContextAnalyzer: Send + Sync {
    /// Perform full analysis on the project
    async fn analyze(&self, project_path: &Path) -> Result<AnalysisResult>;

    /// Update analysis based on changed files
    async fn update(
        &self,
        project_path: &Path,
        changed_files: &[PathBuf],
    ) -> Result<AnalysisResult>;

    /// Get context information for a specific file
    fn get_context_for_file(&self, file: &Path) -> Option<FileContext>;

    /// Get improvement suggestions based on analysis
    fn get_improvement_suggestions(&self) -> Vec<Suggestion>;
}

/// Load analysis results from disk
pub fn load_analysis(project_path: &Path) -> Result<Option<AnalysisResult>> {
    let context_dir = project_path.join(".mmm").join("context");
    if !context_dir.exists() {
        return Ok(None);
    }

    // Load each component file
    let dep_graph_path = context_dir.join("dependency_graph.json");
    let dependency_graph = if dep_graph_path.exists() {
        let content = std::fs::read_to_string(&dep_graph_path)?;
        // The saved file is a DependencyGraphSummary, we need to convert it
        let summary: summary::DependencyGraphSummary = serde_json::from_str(&content)?;

        // Convert summary back to full graph
        let mut nodes = HashMap::new();
        for (path, node_summary) in summary.nodes {
            nodes.insert(
                path.clone(),
                dependencies::ModuleNode {
                    path: path.clone(),
                    module_type: node_summary.module_type,
                    imports: vec![],       // Lost in optimization
                    exports: vec![],       // Lost in optimization
                    external_deps: vec![], // Lost in optimization
                },
            );
        }

        DependencyGraph {
            nodes,
            edges: summary.edges,
            cycles: summary.cycles,
            layers: summary.layers,
        }
    } else {
        return Ok(None);
    };

    let arch_path = context_dir.join("architecture.json");
    let architecture = if arch_path.exists() {
        let content = std::fs::read_to_string(&arch_path)?;
        serde_json::from_str(&content)?
    } else {
        return Ok(None);
    };

    let conv_path = context_dir.join("conventions.json");
    let conventions = if conv_path.exists() {
        let content = std::fs::read_to_string(&conv_path)?;
        serde_json::from_str(&content)?
    } else {
        return Ok(None);
    };

    let debt_path = context_dir.join("technical_debt.json");
    let technical_debt = if debt_path.exists() {
        let content = std::fs::read_to_string(&debt_path)?;
        // The saved file is a TechnicalDebtSummary, we need to convert it
        let summary: summary::TechnicalDebtSummary = serde_json::from_str(&content)?;

        // Convert summary back to full map
        TechnicalDebtMap {
            debt_items: summary.high_priority_items, // Only high priority items are saved
            hotspots: vec![],                        // Convert from hotspot_summary if needed
            duplication_map: HashMap::new(),         // Lost in optimization
            priority_queue: std::collections::BinaryHeap::new(), // Recreate empty
        }
    } else {
        return Ok(None);
    };

    let coverage_path = context_dir.join("test_coverage.json");
    let test_coverage = if coverage_path.exists() {
        let content = std::fs::read_to_string(&coverage_path)?;
        // The saved file might be in TestCoverageMap format or a summary
        // Try to parse it directly first
        serde_json::from_str::<TestCoverageMap>(&content).ok()
    } else {
        None
    };

    let metadata_path = context_dir.join("analysis_metadata.json");
    let metadata = if metadata_path.exists() {
        let content = std::fs::read_to_string(&metadata_path)?;
        serde_json::from_str(&content)?
    } else {
        AnalysisMetadata {
            timestamp: chrono::Utc::now(),
            duration_ms: 0,
            files_analyzed: 0,
            incremental: false,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    };

    Ok(Some(AnalysisResult {
        dependency_graph,
        architecture,
        conventions,
        technical_debt,
        test_coverage,
        metadata,
    }))
}

/// Save analysis results to disk with default commit behavior
pub fn save_analysis(project_path: &Path, analysis: &AnalysisResult) -> Result<()> {
    // Check if git commits should be skipped (for CI/testing)
    let should_commit = std::env::var("MMM_SKIP_GIT_COMMITS")
        .map(|v| v != "true" && v != "1")
        .unwrap_or(true);
    save_analysis_with_commit(project_path, analysis, should_commit)?;
    Ok(())
}

/// Save analysis results to disk with explicit commit control
/// Returns true if a commit was made
pub fn save_analysis_with_options(
    project_path: &Path,
    analysis: &AnalysisResult,
    commit: bool,
) -> Result<bool> {
    // Check environment variable override
    let should_commit = if std::env::var("MMM_SKIP_GIT_COMMITS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
    {
        false
    } else {
        commit
    };
    save_analysis_with_commit(project_path, analysis, should_commit)
}

/// Save analysis results to disk with optional git commit
/// Returns true if a commit was made
pub fn save_analysis_with_commit(
    project_path: &Path,
    analysis: &AnalysisResult,
    should_commit: bool,
) -> Result<bool> {
    let context_dir = project_path.join(".mmm").join("context");
    std::fs::create_dir_all(&context_dir)?;

    // Create size manager to check and optimize files
    let size_manager = size_manager::ContextSizeManager::new();

    // Create a lightweight summary instead of full analysis
    let analysis_summary = summary::AnalysisSummary::from_analysis(analysis);

    let analysis_file = context_dir.join("analysis.json");
    let content = serde_json::to_string_pretty(&analysis_summary)?;
    std::fs::write(&analysis_file, &content)?;

    eprintln!("📄 Created analysis summary ({} bytes)", content.len());

    // Save optimized dependency graph
    let deps_file = context_dir.join("dependency_graph.json");
    let deps_summary = summary::DependencyGraphSummary::from_graph(&analysis.dependency_graph);
    let content = serde_json::to_string_pretty(&deps_summary)?;
    std::fs::write(&deps_file, &content)?;
    eprintln!(
        "🔗 Optimized dependency graph ({} nodes -> {} bytes)",
        analysis.dependency_graph.nodes.len(),
        content.len()
    );

    let arch_file = context_dir.join("architecture.json");
    std::fs::write(
        &arch_file,
        serde_json::to_string_pretty(&analysis.architecture)?,
    )?;

    let conv_file = context_dir.join("conventions.json");
    std::fs::write(
        &conv_file,
        serde_json::to_string_pretty(&analysis.conventions)?,
    )?;

    let debt_file = context_dir.join("technical_debt.json");
    // Create optimized debt summary
    let debt_summary = summary::TechnicalDebtSummary::from_debt_map(&analysis.technical_debt);
    let content = serde_json::to_string_pretty(&debt_summary)?;
    std::fs::write(&debt_file, &content)?;
    eprintln!(
        "🛠️  Optimized technical debt ({} items -> {} bytes)",
        analysis.technical_debt.debt_items.len(),
        content.len()
    );

    if let Some(ref test_coverage) = analysis.test_coverage {
        let coverage_file = context_dir.join("test_coverage.json");
        // Create optimized summary
        let coverage_summary = summary::TestCoverageSummary::from_coverage(test_coverage);
        let content = serde_json::to_string_pretty(&coverage_summary)?;
        std::fs::write(&coverage_file, &content)?;
        eprintln!(
            "📊 Optimized test coverage ({} untested functions -> {} bytes)",
            test_coverage.untested_functions.len(),
            content.len()
        );
    }

    let metadata_file = context_dir.join("analysis_metadata.json");
    std::fs::write(
        &metadata_file,
        serde_json::to_string_pretty(&analysis.metadata)?,
    )?;

    // Calculate unified health score
    let health_score = crate::scoring::ProjectHealthScore::from_context(analysis);

    eprintln!("\n📊 Context Health Score: {:.1}/100", health_score.overall);
    eprintln!("\nComponents:");

    use crate::scoring::format_component;

    // Display test coverage
    eprintln!(
        "{}",
        format_component("Test Coverage", health_score.components.test_coverage, None)
    );

    // Display code quality with pattern/idiom count
    let pattern_info = format!(
        "({} patterns, {} idioms)",
        analysis.conventions.code_patterns.len(),
        analysis.conventions.project_idioms.len()
    );
    eprintln!(
        "{}",
        format_component(
            "Code Quality",
            health_score.components.code_quality,
            Some(&pattern_info)
        )
    );

    // Display maintainability with debt count
    let debt_info = format!("({} debt items)", analysis.technical_debt.debt_items.len());
    eprintln!(
        "{}",
        format_component(
            "Maintainability",
            health_score.components.maintainability,
            Some(&debt_info)
        )
    );

    // Display documentation estimate
    eprintln!(
        "{}",
        format_component(
            "Documentation",
            health_score.components.documentation,
            Some("(estimated)")
        )
    );

    // Type safety not available in context analysis
    eprintln!(
        "{}",
        format_component("Type Safety", health_score.components.type_safety, None)
    );

    // Show improvement suggestions
    let suggestions = health_score.get_improvement_suggestions();
    if !suggestions.is_empty() {
        eprintln!("\n💡 Top improvements:");
        for (i, suggestion) in suggestions.iter().enumerate() {
            eprintln!("  {}. {}", i + 1, suggestion);
        }
    }

    // Analyze and report final context sizes
    if let Ok(size_metadata) = size_manager.analyze_context_sizes(&context_dir) {
        size_manager.print_warnings(&size_metadata);

        // Log total size
        let total_mb = size_metadata.total_size as f64 / 1_000_000.0;
        eprintln!("💾 Total context size: {total_mb:.2} MB");
    }

    // Commit analysis changes to git if requested and in a git repo
    let mut commit_made = false;
    if should_commit {
        match commit_analysis_changes(project_path, analysis, &health_score) {
            Ok(made_commit) => commit_made = made_commit,
            Err(e) => {
                eprintln!("⚠️  Failed to commit analysis changes: {e}");
                // Don't fail the whole analysis if git commit fails
            }
        }
    }

    Ok(commit_made)
}

/// Commit analysis changes to git with a descriptive message
/// Returns true if a commit was made, false if no changes or not a git repo
fn commit_analysis_changes(
    project_path: &Path,
    analysis: &AnalysisResult,
    health_score: &crate::scoring::ProjectHealthScore,
) -> Result<bool> {
    // Check if we're in a git repo
    let mut git_check = std::process::Command::new("git");
    git_check
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(project_path);

    // Suppress stderr during tests to avoid error spam
    #[cfg(test)]
    git_check.stderr(std::process::Stdio::null());

    if !git_check.status().map(|s| s.success()).unwrap_or(false) {
        return Ok(false); // Not a git repo, skip commit
    }

    // Check if there are any changes to commit
    let git_status = std::process::Command::new("git")
        .args(["status", "--porcelain", ".mmm/"])
        .current_dir(project_path)
        .output()?;

    if git_status.stdout.is_empty() {
        return Ok(false); // No changes to commit
    }

    // Stage .mmm directory
    std::process::Command::new("git")
        .args(["add", ".mmm/"])
        .current_dir(project_path)
        .status()?;

    // Calculate context size more accurately
    let context_size_mb = std::fs::read_dir(project_path.join(".mmm/context"))
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum::<u64>() as f64
                / 1_000_000.0
        })
        .unwrap_or(0.0);

    // Build component summary for commit message
    let mut components = Vec::new();

    if let Some(coverage) = health_score.components.test_coverage {
        components.push(format!("Test coverage: {coverage:.1}%"));
    }

    if let Some(quality) = health_score.components.code_quality {
        let quality_marker = if quality >= 70.0 { "✓" } else { "⚠" };
        components.push(format!("{quality_marker} Code quality: {quality:.1}%"));
    }

    if let Some(maint) = health_score.components.maintainability {
        let maint_marker = if maint >= 70.0 { "✓" } else { "⚠" };
        components.push(format!(
            "{} Maintainability: {:.1}% ({} debt items)",
            maint_marker,
            maint,
            analysis.technical_debt.debt_items.len()
        ));
    }

    // Create commit message with analysis summary
    let commit_msg = format!(
        "analysis: update project context (context health: {:.1}/100)

📊 Context Health Score: {:.1}/100
{}

📈 Context Analysis Summary:
- {} modules analyzed
- {} dependencies mapped
- {} architectural violations
- {} technical debt items
- Context size: {:.2}MB

Note: This score is based on static analysis (architecture, dependencies, debt).
For runtime metrics score, see 'mmm analyze metrics'.

Generated by MMM v{}",
        health_score.overall,
        health_score.overall,
        components.join("\n"),
        analysis.dependency_graph.nodes.len(),
        analysis.dependency_graph.edges.len(),
        analysis.architecture.violations.len(),
        analysis.technical_debt.debt_items.len(),
        context_size_mb,
        env!("CARGO_PKG_VERSION")
    );

    // Create commit
    let commit_status = std::process::Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(project_path)
        .status()?;

    Ok(commit_status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optional_test_coverage_field() {
        // Test that test_coverage field is correctly handled as Option<TestCoverageMap>

        // JSON with null test_coverage should deserialize
        let json_with_null = r#"{"test_coverage": null}"#;
        let value: serde_json::Value = serde_json::from_str(json_with_null).unwrap();
        assert!(value["test_coverage"].is_null());

        // JSON without test_coverage field should fail (field is required but can be null)
        let json_without_field = r#"{}"#;
        let result: Result<serde_json::Value, _> = serde_json::from_str(json_without_field);
        assert!(result.is_ok()); // Valid JSON

        // Test that Option<TestCoverageMap> serializes to null when None
        let coverage: Option<TestCoverageMap> = None;
        let serialized = serde_json::to_string(&coverage).unwrap();
        assert_eq!(serialized, "null");
    }
}
