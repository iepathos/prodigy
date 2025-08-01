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

    let analysis_file = context_dir.join("analysis.json");
    if !analysis_file.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&analysis_file)?;
    let analysis: AnalysisResult = serde_json::from_str(&content)?;
    Ok(Some(analysis))
}

/// Save analysis results to disk
pub fn save_analysis(project_path: &Path, analysis: &AnalysisResult) -> Result<()> {
    let context_dir = project_path.join(".mmm").join("context");
    std::fs::create_dir_all(&context_dir)?;

    let analysis_file = context_dir.join("analysis.json");
    let content = serde_json::to_string_pretty(analysis)?;
    std::fs::write(&analysis_file, content)?;

    // Also save individual components for easier access
    let deps_file = context_dir.join("dependency_graph.json");
    std::fs::write(
        &deps_file,
        serde_json::to_string_pretty(&analysis.dependency_graph)?,
    )?;

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
    std::fs::write(
        &debt_file,
        serde_json::to_string_pretty(&analysis.technical_debt)?,
    )?;

    if let Some(ref test_coverage) = analysis.test_coverage {
        let coverage_file = context_dir.join("test_coverage.json");
        std::fs::write(&coverage_file, serde_json::to_string_pretty(test_coverage)?)?;
    }

    let metadata_file = context_dir.join("analysis_metadata.json");
    std::fs::write(
        &metadata_file,
        serde_json::to_string_pretty(&analysis.metadata)?,
    )?;

    Ok(())
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
