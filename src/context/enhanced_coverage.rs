//! Enhanced test coverage analyzer with multi-factor criticality scoring

use super::architecture::ArchitectureExtractor;
use super::criticality::{AnalysisContext, BugHistory, EnhancedCriticalityScorer, GitHistory};
use super::debt::{TechnicalDebtMap, TechnicalDebtMapper};
use super::dependencies::{DependencyAnalyzer, DependencyGraph};
use super::test_coverage::{TestCoverageAnalyzer, TestCoverageMap};
use super::ArchitectureInfo;
use crate::metrics::complexity::ComplexityMetrics;
use crate::subprocess::SubprocessManager;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Enhanced coverage analyzer that wraps existing analyzers and adds criticality scoring
pub struct EnhancedCoverageAnalyzer {
    base_analyzer: Box<dyn TestCoverageAnalyzer>,
    criticality_scorer: EnhancedCriticalityScorer,
}

impl EnhancedCoverageAnalyzer {
    /// Create a new enhanced coverage analyzer
    pub fn new(subprocess: SubprocessManager) -> Self {
        Self {
            base_analyzer: Box::new(
                super::metrics_aware_coverage::MetricsAwareCoverageAnalyzer::new(subprocess),
            ),
            criticality_scorer: EnhancedCriticalityScorer::new(None),
        }
    }

    /// Enhance coverage map with better criticality scores
    async fn enhance_criticality(
        &self,
        mut coverage: TestCoverageMap,
        project_path: &Path,
    ) -> Result<TestCoverageMap> {
        // Load additional context for scoring
        let context = self.load_analysis_context(project_path).await?;

        // Rescore all untested functions
        for func in &mut coverage.untested_functions {
            let score = self.criticality_scorer.score_function(func, &context);
            func.criticality = score.criticality_level;
        }

        // Sort by criticality and score
        coverage.untested_functions.sort_by(|a, b| {
            // First by criticality level
            let crit_cmp = match (&b.criticality, &a.criticality) {
                (
                    super::test_coverage::Criticality::High,
                    super::test_coverage::Criticality::High,
                ) => std::cmp::Ordering::Equal,
                (super::test_coverage::Criticality::High, _) => std::cmp::Ordering::Less,
                (_, super::test_coverage::Criticality::High) => std::cmp::Ordering::Greater,
                (
                    super::test_coverage::Criticality::Medium,
                    super::test_coverage::Criticality::Medium,
                ) => std::cmp::Ordering::Equal,
                (super::test_coverage::Criticality::Medium, _) => std::cmp::Ordering::Less,
                (_, super::test_coverage::Criticality::Medium) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };

            // Then by file and name for stable ordering
            crit_cmp
                .then_with(|| a.file.cmp(&b.file))
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(coverage)
    }

    /// Load analysis context for criticality scoring
    async fn load_analysis_context(&self, project_path: &Path) -> Result<AnalysisContext> {
        // Load complexity metrics if available
        let complexity_metrics = self.load_complexity_metrics(project_path).await.ok();

        // Load dependency graph
        let dependency_graph = self.load_dependency_graph(project_path).await.ok();

        // Load architecture info
        let architecture = self.load_architecture_info(project_path).await.ok();

        // Load technical debt map
        let debt_map = self.load_technical_debt(project_path).await.ok();

        // Load git history (simplified for now)
        let git_history = self.load_git_history(project_path).await.ok();

        // Load bug history (simplified for now)
        let bug_history = self.load_bug_history(project_path).await.ok();

        // Calculate surrounding coverage
        let surrounding_coverage = self.calculate_surrounding_coverage(project_path).await?;

        Ok(AnalysisContext {
            complexity_metrics,
            dependency_graph,
            architecture,
            git_history,
            bug_history,
            surrounding_coverage,
            debt_map,
        })
    }

    /// Load complexity metrics from .mmm/metrics
    async fn load_complexity_metrics(&self, project_path: &Path) -> Result<ComplexityMetrics> {
        let metrics_file = project_path.join(".mmm/metrics/current.json");
        if !metrics_file.exists() {
            // Try to calculate on the fly
            let calculator = crate::metrics::complexity::ComplexityCalculator::new();
            return calculator.calculate(project_path);
        }

        let content = tokio::fs::read_to_string(&metrics_file).await?;
        let metrics_json: serde_json::Value = serde_json::from_str(&content)?;

        // Extract complexity data from the optimized format
        let mut cyclomatic_complexity = HashMap::new();
        let mut cognitive_complexity = HashMap::new();

        if let Some(summary) = metrics_json.get("complexity_summary") {
            if let Some(by_file) = summary.get("by_file").and_then(|v| v.as_object()) {
                for (file, data) in by_file {
                    if let Some(obj) = data.as_object() {
                        // Extract max complexities as representative values
                        if let Some(max_cyclo) = obj.get("max_cyclomatic").and_then(|v| v.as_u64())
                        {
                            cyclomatic_complexity.insert(file.clone(), max_cyclo as u32);
                        }
                        if let Some(max_cog) = obj.get("max_cognitive").and_then(|v| v.as_u64()) {
                            cognitive_complexity.insert(file.clone(), max_cog as u32);
                        }
                    }
                }
            }
        }

        Ok(ComplexityMetrics {
            cyclomatic_complexity,
            cognitive_complexity,
            max_nesting_depth: metrics_json
                .get("max_nesting_depth")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_lines: metrics_json
                .get("total_lines")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
        })
    }

    /// Load dependency graph from context
    async fn load_dependency_graph(&self, project_path: &Path) -> Result<DependencyGraph> {
        let deps_file = project_path.join(".mmm/context/dependency_graph.json");
        if !deps_file.exists() {
            // Run basic analysis
            let analyzer = super::dependencies::BasicDependencyAnalyzer::new();
            return analyzer.analyze_dependencies(project_path).await;
        }

        let content = tokio::fs::read_to_string(&deps_file).await?;
        let deps: DependencyGraph = serde_json::from_str(&content)?;
        Ok(deps)
    }

    /// Load architecture info from context
    async fn load_architecture_info(&self, project_path: &Path) -> Result<ArchitectureInfo> {
        let arch_file = project_path.join(".mmm/context/architecture.json");
        if !arch_file.exists() {
            // Run basic analysis
            let extractor = super::architecture::BasicArchitectureExtractor::new();
            return extractor.extract_architecture(project_path).await;
        }

        let content = tokio::fs::read_to_string(&arch_file).await?;
        let arch: ArchitectureInfo = serde_json::from_str(&content)?;
        Ok(arch)
    }

    /// Load technical debt from context
    async fn load_technical_debt(&self, project_path: &Path) -> Result<TechnicalDebtMap> {
        let debt_file = project_path.join(".mmm/context/technical_debt.json");
        if !debt_file.exists() {
            // Run basic analysis
            let mapper = super::debt::BasicTechnicalDebtMapper::new();
            return mapper.map_technical_debt(project_path).await;
        }

        let content = tokio::fs::read_to_string(&debt_file).await?;
        // The file contains TechnicalDebtSummary, convert back to full map
        let summary: super::summary::TechnicalDebtSummary = serde_json::from_str(&content)?;

        Ok(TechnicalDebtMap {
            debt_items: summary.high_priority_items,
            hotspots: vec![],
            duplication_map: HashMap::new(),
            priority_queue: std::collections::BinaryHeap::new(),
        })
    }

    /// Load git history (simplified implementation)
    async fn load_git_history(&self, project_path: &Path) -> Result<GitHistory> {
        use super::criticality::FileChangeHistory;

        // Simple git log parsing for file change counts
        let output = tokio::process::Command::new("git")
            .args([
                "log",
                "--pretty=format:",
                "--name-only",
                "--since=1 year ago",
            ])
            .current_dir(project_path)
            .output()
            .await?;

        let mut file_changes = HashMap::new();

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if !line.is_empty() && !line.starts_with(' ') {
                    let path = PathBuf::from(line);
                    let entry =
                        file_changes
                            .entry(path.clone())
                            .or_insert_with(|| FileChangeHistory {
                                change_count: 0,
                                last_modified: chrono::Utc::now(),
                                authors: vec![],
                            });
                    entry.change_count += 1;
                }
            }
        }

        Ok(GitHistory { file_changes })
    }

    /// Load bug history (simplified implementation)
    async fn load_bug_history(&self, project_path: &Path) -> Result<BugHistory> {
        // Look for bug-related commits
        let output = tokio::process::Command::new("git")
            .args([
                "log",
                "--grep=fix",
                "--grep=bug",
                "-i",
                "--pretty=format:",
                "--name-only",
            ])
            .current_dir(project_path)
            .output()
            .await?;

        let mut bugs_per_file = HashMap::new();

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if !line.is_empty() && !line.starts_with(' ') {
                    let path = PathBuf::from(line);
                    *bugs_per_file.entry(path).or_insert(0) += 1;
                }
            }
        }

        Ok(BugHistory {
            bugs_per_file,
            bug_patterns: vec!["fix".to_string(), "bug".to_string()],
        })
    }

    /// Calculate surrounding coverage for context
    async fn calculate_surrounding_coverage(
        &self,
        project_path: &Path,
    ) -> Result<HashMap<PathBuf, f64>> {
        // Load existing coverage data
        let coverage_file = project_path.join(".mmm/context/test_coverage.json");
        let mut surrounding_coverage = HashMap::new();

        if coverage_file.exists() {
            let content = tokio::fs::read_to_string(&coverage_file).await?;
            if let Ok(coverage) = serde_json::from_str::<TestCoverageMap>(&content) {
                for (path, file_cov) in coverage.file_coverage {
                    surrounding_coverage.insert(path, file_cov.coverage_percentage / 100.0);
                }
            }
        }

        Ok(surrounding_coverage)
    }
}

#[async_trait::async_trait]
impl TestCoverageAnalyzer for EnhancedCoverageAnalyzer {
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap> {
        // Get base coverage analysis
        let coverage = self.base_analyzer.analyze_coverage(project_path).await?;

        // Enhance with better criticality scoring
        self.enhance_criticality(coverage, project_path).await
    }

    async fn update_coverage(
        &self,
        project_path: &Path,
        current: &TestCoverageMap,
        changed_files: &[PathBuf],
    ) -> Result<TestCoverageMap> {
        // Get base coverage update
        let coverage = self
            .base_analyzer
            .update_coverage(project_path, current, changed_files)
            .await?;

        // Re-enhance criticality
        self.enhance_criticality(coverage, project_path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_enhanced_coverage_analyzer() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let analyzer = EnhancedCoverageAnalyzer::new(subprocess);

        // Should analyze without errors even on empty project
        let result = analyzer.analyze_coverage(temp_dir.path()).await;
        assert!(result.is_ok());
    }
}
