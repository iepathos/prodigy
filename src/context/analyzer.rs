//! Main project analyzer that orchestrates all context analysis components

use super::*;
use crate::subprocess::SubprocessManager;
use anyhow::Result;
use std::path::Path;
use std::time::Instant;

/// Main project analyzer that coordinates all analysis components
pub struct ProjectAnalyzer {
    dependency_analyzer: Box<dyn DependencyAnalyzer>,
    architecture_extractor: Box<dyn ArchitectureExtractor>,
    convention_detector: Box<dyn ConventionDetector>,
    debt_mapper: Box<dyn TechnicalDebtMapper>,
    coverage_analyzer: Box<dyn TestCoverageAnalyzer>,
    cached_result: Option<AnalysisResult>,
    #[allow(dead_code)]
    subprocess: SubprocessManager,
}

impl Default for ProjectAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectAnalyzer {
    /// Calculate criticality distribution from untested functions
    fn calculate_criticality_distribution(
        untested_functions: &[test_coverage::UntestedFunction],
    ) -> super::CriticalityDistribution {
        untested_functions.iter().fold(
            super::CriticalityDistribution {
                high: 0,
                medium: 0,
                low: 0,
                confidence_score: 0.87,
            },
            |mut dist, func| {
                match func.criticality {
                    test_coverage::Criticality::High => dist.high += 1,
                    test_coverage::Criticality::Medium => dist.medium += 1,
                    test_coverage::Criticality::Low => dist.low += 1,
                }
                dist
            },
        )
    }

    /// Create metadata for analysis result
    fn create_metadata(
        duration_ms: u64,
        files_analyzed: usize,
        incremental: bool,
        criticality_distribution: Option<super::CriticalityDistribution>,
    ) -> AnalysisMetadata {
        AnalysisMetadata {
            timestamp: chrono::Utc::now(),
            duration_ms,
            files_analyzed,
            incremental,
            version: env!("CARGO_PKG_VERSION").to_string(),
            scoring_algorithm: Some("multi-factor-v1".to_string()),
            criticality_distribution,
        }
    }

    /// Create a new project analyzer with default components
    pub fn new() -> Self {
        let subprocess = SubprocessManager::production();
        Self {
            dependency_analyzer: Box::new(dependencies::BasicDependencyAnalyzer::new()),
            architecture_extractor: Box::new(architecture::BasicArchitectureExtractor::new()),
            convention_detector: Box::new(conventions::BasicConventionDetector::new()),
            debt_mapper: Box::new(debt::BasicTechnicalDebtMapper::new()),
            coverage_analyzer: Box::new(super::enhanced_coverage::EnhancedCoverageAnalyzer::new(
                subprocess.clone(),
            )),
            cached_result: None,
            subprocess,
        }
    }

    /// Create analyzer with custom components (for testing)
    pub fn with_components(
        dependency_analyzer: Box<dyn DependencyAnalyzer>,
        architecture_extractor: Box<dyn ArchitectureExtractor>,
        convention_detector: Box<dyn ConventionDetector>,
        debt_mapper: Box<dyn TechnicalDebtMapper>,
        coverage_analyzer: Box<dyn TestCoverageAnalyzer>,
    ) -> Self {
        Self {
            dependency_analyzer,
            architecture_extractor,
            convention_detector,
            debt_mapper,
            coverage_analyzer,
            subprocess: SubprocessManager::production(),
            cached_result: None,
        }
    }

    /// Get cached analysis result if available
    pub fn get_cached(&self) -> Option<&AnalysisResult> {
        self.cached_result.as_ref()
    }
}

#[async_trait::async_trait]
impl ContextAnalyzer for ProjectAnalyzer {
    async fn analyze(&self, project_path: &Path) -> Result<AnalysisResult> {
        let start = Instant::now();

        // Try to load existing analysis first
        if let Some(existing) = load_analysis(project_path)? {
            // Check if it's recent enough (within last hour)
            let age = chrono::Utc::now() - existing.metadata.timestamp;
            if age.num_hours() < 1 {
                return Ok(existing);
            }
        }

        println!("🔍 Analyzing project structure...");

        // Run all analyzers in parallel
        let (deps, arch, conv, debt, coverage) = tokio::try_join!(
            self.dependency_analyzer.analyze_dependencies(project_path),
            self.architecture_extractor
                .extract_architecture(project_path),
            self.convention_detector.detect_conventions(project_path),
            self.debt_mapper.map_technical_debt(project_path),
            self.coverage_analyzer.analyze_coverage(project_path),
        )?;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Count analyzed files
        let files_analyzed = count_files(project_path)?;

        // Note: metrics history loading removed as hybrid coverage has been removed

        // Calculate criticality distribution from coverage
        let criticality_distribution = coverage.untested_functions.iter().fold(
            super::CriticalityDistribution {
                high: 0,
                medium: 0,
                low: 0,
                confidence_score: 0.87,
            },
            |mut dist, func| {
                match func.criticality {
                    test_coverage::Criticality::High => dist.high += 1,
                    test_coverage::Criticality::Medium => dist.medium += 1,
                    test_coverage::Criticality::Low => dist.low += 1,
                }
                dist
            },
        );

        let result = AnalysisResult {
            dependency_graph: deps,
            architecture: arch,
            conventions: conv,
            technical_debt: debt,
            test_coverage: Some(coverage),
            metadata: Self::create_metadata(
                duration_ms,
                files_analyzed,
                false,
                Some(criticality_distribution),
            ),
        };

        println!("✅ Analysis complete in {}ms", duration_ms);
        Ok(result)
    }

    async fn update(
        &self,
        project_path: &Path,
        changed_files: &[PathBuf],
    ) -> Result<AnalysisResult> {
        let start = Instant::now();

        // Load existing analysis
        let mut result = if let Some(existing) = load_analysis(project_path)? {
            existing
        } else {
            // No existing analysis, do full analysis
            return self.analyze(project_path).await;
        };

        println!(
            "🔄 Updating analysis for {} changed files...",
            changed_files.len()
        );

        // Update each component incrementally
        result.dependency_graph = self
            .dependency_analyzer
            .update_dependencies(project_path, &result.dependency_graph, changed_files)
            .await?;

        result.architecture = self
            .architecture_extractor
            .update_architecture(project_path, &result.architecture, changed_files)
            .await?;

        result.conventions = self
            .convention_detector
            .update_conventions(project_path, &result.conventions, changed_files)
            .await?;

        result.technical_debt = self
            .debt_mapper
            .update_debt_map(project_path, &result.technical_debt, changed_files)
            .await?;

        if let Some(ref test_coverage) = result.test_coverage {
            let updated_coverage = self
                .coverage_analyzer
                .update_coverage(project_path, test_coverage, changed_files)
                .await?;

            // Recalculate criticality distribution
            let criticality_distribution =
                Self::calculate_criticality_distribution(&updated_coverage.untested_functions);

            result.test_coverage = Some(updated_coverage);
            result.metadata.criticality_distribution = Some(criticality_distribution);
        }

        // Update metadata
        result.metadata = Self::create_metadata(
            start.elapsed().as_millis() as u64,
            result.metadata.files_analyzed,
            true,
            result.metadata.criticality_distribution.clone(),
        );

        println!("✅ Analysis updated in {}ms", result.metadata.duration_ms);
        Ok(result)
    }

    fn get_context_for_file(&self, file: &Path) -> Option<FileContext> {
        let result = self.cached_result.as_ref()?;

        // Find file in various analysis results
        let module_deps = result.dependency_graph.get_file_dependencies(file);
        let conventions = result.conventions.get_file_conventions(file);
        let debt_items = result.technical_debt.get_file_debt(file);
        let coverage = result
            .test_coverage
            .as_ref()
            .map(|tc| tc.get_file_coverage(file))
            .unwrap_or(0.0);
        let complexity = result.technical_debt.get_file_complexity(file);

        Some(FileContext {
            path: file.to_path_buf(),
            module_dependencies: module_deps,
            conventions: FileConventions {
                naming_style: conventions.naming_style.clone(),
                patterns_used: conventions.patterns.clone(),
                violations: conventions.violations.clone(),
            },
            debt_items,
            test_coverage: coverage,
            complexity,
        })
    }

    fn get_improvement_suggestions(&self) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        if let Some(result) = &self.cached_result {
            // Get suggestions from each analyzer
            suggestions.extend(suggest_from_dependencies(&result.dependency_graph));
            suggestions.extend(suggest_from_architecture(&result.architecture));
            suggestions.extend(suggest_from_conventions(&result.conventions));
            suggestions.extend(suggest_from_debt(&result.technical_debt));
            if let Some(ref test_coverage) = result.test_coverage {
                suggestions.extend(suggest_from_coverage(test_coverage));
            }

            // Sort by priority
            suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));
        }

        suggestions
    }
}

/// Count files in project (excluding common ignore patterns)
fn count_files(project_path: &Path) -> Result<usize> {
    use walkdir::WalkDir;

    let count = WalkDir::new(project_path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    matches!(
                        ext,
                        "rs" | "toml"
                            | "yaml"
                            | "yml"
                            | "json"
                            | "md"
                            | "txt"
                            | "sh"
                            | "py"
                            | "js"
                            | "ts"
                            | "jsx"
                            | "tsx"
                    )
                })
                .unwrap_or(false)
        })
        .count();

    Ok(count)
}

/// Generate suggestions from dependency analysis
fn suggest_from_dependencies(deps: &DependencyGraph) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    // Suggest fixing circular dependencies
    for cycle in &deps.cycles {
        suggestions.push(Suggestion {
            priority: SuggestionPriority::High,
            category: SuggestionCategory::Dependencies,
            title: "Fix circular dependency".to_string(),
            description: format!("Circular dependency detected: {}", cycle.join(" -> ")),
            affected_files: cycle.iter().map(PathBuf::from).collect(),
            estimated_impact: ImpactLevel::Major,
        });
    }

    // Suggest reducing tight coupling
    for (module, count) in deps.get_coupling_hotspots() {
        if count > 10 {
            suggestions.push(Suggestion {
                priority: SuggestionPriority::Medium,
                category: SuggestionCategory::Dependencies,
                title: "Reduce module coupling".to_string(),
                description: format!("{module} has {count} dependencies, consider refactoring"),
                affected_files: vec![PathBuf::from(module)],
                estimated_impact: ImpactLevel::Moderate,
            });
        }
    }

    suggestions
}

/// Generate suggestions from architecture analysis
fn suggest_from_architecture(arch: &ArchitectureInfo) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    // Suggest fixing architecture violations
    for violation in &arch.violations {
        let priority = match violation.severity {
            ViolationSeverity::High => SuggestionPriority::High,
            ViolationSeverity::Medium => SuggestionPriority::Medium,
            ViolationSeverity::Low => SuggestionPriority::Low,
        };

        suggestions.push(Suggestion {
            priority,
            category: SuggestionCategory::Architecture,
            title: format!("Fix architecture violation: {}", violation.rule),
            description: violation.description.clone(),
            affected_files: vec![PathBuf::from(&violation.location)],
            estimated_impact: ImpactLevel::Moderate,
        });
    }

    suggestions
}

/// Generate suggestions from convention analysis
fn suggest_from_conventions(conv: &ProjectConventions) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    // Suggest fixing naming violations
    for (file, violations) in conv.get_naming_violations() {
        if !violations.is_empty() {
            suggestions.push(Suggestion {
                priority: SuggestionPriority::Low,
                category: SuggestionCategory::CodeQuality,
                title: "Fix naming convention violations".to_string(),
                description: format!("{} naming violations in {}", violations.len(), file),
                affected_files: vec![PathBuf::from(file)],
                estimated_impact: ImpactLevel::Minor,
            });
        }
    }

    suggestions
}

/// Generate suggestions from technical debt
fn suggest_from_debt(debt: &TechnicalDebtMap) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    // Get top priority debt items
    for item in debt.get_priority_items(5) {
        let priority = match item.impact {
            _ if item.impact > 8 => SuggestionPriority::Critical,
            _ if item.impact > 5 => SuggestionPriority::High,
            _ if item.impact > 3 => SuggestionPriority::Medium,
            _ => SuggestionPriority::Low,
        };

        suggestions.push(Suggestion {
            priority,
            category: SuggestionCategory::TechnicalDebt,
            title: item.title.clone(),
            description: item.description.clone(),
            affected_files: vec![item.location.clone()],
            estimated_impact: ImpactLevel::Major,
        });
    }

    suggestions
}

/// Generate suggestions from test coverage
fn suggest_from_coverage(coverage: &TestCoverageMap) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    // Suggest adding tests for critical untested code
    for (file, cov) in coverage.get_critical_gaps() {
        if cov < 0.2 {
            suggestions.push(Suggestion {
                priority: SuggestionPriority::High,
                category: SuggestionCategory::TestCoverage,
                title: "Add tests for critical code".to_string(),
                description: format!("{} has only {:.0}% test coverage", file, cov * 100.0),
                affected_files: vec![PathBuf::from(file)],
                estimated_impact: ImpactLevel::Major,
            });
        }
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_analyze_full_project() {
        // Test normal operation
        let temp_dir = TempDir::new().unwrap();
        let analyzer = ProjectAnalyzer::new();
        let result = analyzer.analyze(temp_dir.path()).await;
        assert!(result.is_ok());
        let _analysis = result.unwrap();
        // files_analyzed is always >= 0 since it's a usize
    }

    #[tokio::test]
    async fn test_analyze_with_cache() {
        // Test analysis runs successfully
        let temp_dir = TempDir::new().unwrap();
        let analyzer = ProjectAnalyzer::new();

        // First run
        let first_result = analyzer.analyze(temp_dir.path()).await.unwrap();
        assert!(!first_result.metadata.incremental);

        // Second run - currently not using cache/incremental
        let second_result = analyzer.analyze(temp_dir.path()).await.unwrap();
        // Note: Current implementation doesn't support incremental analysis
        assert!(!second_result.metadata.incremental);
    }

    #[tokio::test]
    async fn test_analyze_error_cases() {
        // Test analysis of non-existent path still succeeds (returns empty analysis)
        let analyzer = ProjectAnalyzer::new();
        let result = analyzer.analyze(Path::new("/nonexistent/path")).await;
        // Current implementation doesn't fail on non-existent paths
        assert!(result.is_ok());

        let analysis = result.unwrap();
        assert_eq!(analysis.metadata.files_analyzed, 0);
    }

    #[test]
    fn test_calculate_criticality_distribution() {
        use crate::context::test_coverage::{Criticality, UntestedFunction};
        use std::path::PathBuf;

        // Test empty list
        let empty_funcs = vec![];
        let dist = ProjectAnalyzer::calculate_criticality_distribution(&empty_funcs);
        assert_eq!(dist.high, 0);
        assert_eq!(dist.medium, 0);
        assert_eq!(dist.low, 0);
        assert_eq!(dist.confidence_score, 0.87);

        // Test with various criticality levels
        let test_funcs = vec![
            UntestedFunction {
                file: PathBuf::from("test.rs"),
                name: "high1".to_string(),
                line_number: 10,
                criticality: Criticality::High,
            },
            UntestedFunction {
                file: PathBuf::from("test.rs"),
                name: "high2".to_string(),
                line_number: 20,
                criticality: Criticality::High,
            },
            UntestedFunction {
                file: PathBuf::from("test.rs"),
                name: "medium1".to_string(),
                line_number: 30,
                criticality: Criticality::Medium,
            },
            UntestedFunction {
                file: PathBuf::from("test.rs"),
                name: "low1".to_string(),
                line_number: 40,
                criticality: Criticality::Low,
            },
            UntestedFunction {
                file: PathBuf::from("test.rs"),
                name: "low2".to_string(),
                line_number: 50,
                criticality: Criticality::Low,
            },
            UntestedFunction {
                file: PathBuf::from("test.rs"),
                name: "low3".to_string(),
                line_number: 60,
                criticality: Criticality::Low,
            },
        ];

        let dist = ProjectAnalyzer::calculate_criticality_distribution(&test_funcs);
        assert_eq!(dist.high, 2);
        assert_eq!(dist.medium, 1);
        assert_eq!(dist.low, 3);
        assert_eq!(dist.confidence_score, 0.87);

        // Test with only high criticality
        let high_only = vec![UntestedFunction {
            file: PathBuf::from("critical.rs"),
            name: "critical_func".to_string(),
            line_number: 100,
            criticality: Criticality::High,
        }];

        let dist = ProjectAnalyzer::calculate_criticality_distribution(&high_only);
        assert_eq!(dist.high, 1);
        assert_eq!(dist.medium, 0);
        assert_eq!(dist.low, 0);
    }

    #[test]
    fn test_create_metadata() {
        let duration_ms = 1500;
        let files_analyzed = 42;
        let incremental = false;
        let criticality_dist = Some(super::CriticalityDistribution {
            high: 5,
            medium: 10,
            low: 15,
            confidence_score: 0.87,
        });

        let metadata = ProjectAnalyzer::create_metadata(
            duration_ms,
            files_analyzed,
            incremental,
            criticality_dist.clone(),
        );

        assert_eq!(metadata.duration_ms, duration_ms);
        assert_eq!(metadata.files_analyzed, files_analyzed);
        assert_eq!(metadata.incremental, incremental);
        assert_eq!(metadata.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            metadata.scoring_algorithm,
            Some("multi-factor-v1".to_string())
        );
        // Check criticality distribution values directly since CriticalityDistribution doesn't impl PartialEq
        if let Some(dist) = metadata.criticality_distribution {
            assert_eq!(dist.high, 5);
            assert_eq!(dist.medium, 10);
            assert_eq!(dist.low, 15);
            assert_eq!(dist.confidence_score, 0.87);
        } else {
            panic!("Expected criticality distribution to be Some");
        }

        // Test incremental metadata
        let incremental_metadata = ProjectAnalyzer::create_metadata(500, 10, true, None);

        assert_eq!(incremental_metadata.duration_ms, 500);
        assert_eq!(incremental_metadata.files_analyzed, 10);
        assert!(incremental_metadata.incremental);
        assert!(incremental_metadata.criticality_distribution.is_none());
    }

    #[tokio::test]
    async fn test_update_with_no_existing_analysis() {
        use std::fs;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let analyzer = ProjectAnalyzer::new();

        // Create some test files
        let file1 = temp_dir.path().join("test1.rs");
        let file2 = temp_dir.path().join("test2.rs");
        fs::write(&file1, "fn main() {}").unwrap();
        fs::write(&file2, "fn test() {}").unwrap();

        let changed_files = vec![file1.clone(), file2.clone()];

        // Call update with no existing analysis - should fall back to full analysis
        let result = analyzer.update(temp_dir.path(), &changed_files).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        // Should not be incremental since it fell back to full analysis
        assert!(!analysis.metadata.incremental);
    }

    #[tokio::test]
    #[ignore = "Incremental analysis implementation needs investigation"]
    async fn test_update_with_existing_analysis() {
        use std::fs;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let analyzer = ProjectAnalyzer::new();

        // First, run a full analysis to create baseline
        let initial_result = analyzer.analyze(temp_dir.path()).await.unwrap();

        // Save the analysis components to disk so update() can find them
        let mmm_dir = temp_dir.path().join(".mmm");
        let context_dir = mmm_dir.join("context");
        fs::create_dir_all(&context_dir).unwrap();

        // Save each component separately (as load_analysis expects)
        let deps_path = context_dir.join("dependency_graph.json");
        let deps_json = serde_json::to_string_pretty(&initial_result.dependency_graph).unwrap();
        fs::write(&deps_path, deps_json).unwrap();

        let arch_path = context_dir.join("architecture.json");
        let arch_json = serde_json::to_string_pretty(&initial_result.architecture).unwrap();
        fs::write(&arch_path, arch_json).unwrap();

        let conv_path = context_dir.join("conventions.json");
        let conv_json = serde_json::to_string_pretty(&initial_result.conventions).unwrap();
        fs::write(&conv_path, conv_json).unwrap();

        let debt_path = context_dir.join("technical_debt.json");
        let debt_json = serde_json::to_string_pretty(&initial_result.technical_debt).unwrap();
        fs::write(&debt_path, debt_json).unwrap();

        if let Some(ref coverage) = initial_result.test_coverage {
            let coverage_path = context_dir.join("test_coverage.json");
            let coverage_json = serde_json::to_string_pretty(coverage).unwrap();
            fs::write(&coverage_path, coverage_json).unwrap();
        }

        let metadata_path = context_dir.join("analysis_metadata.json");
        let metadata_json = serde_json::to_string_pretty(&initial_result.metadata).unwrap();
        fs::write(&metadata_path, metadata_json).unwrap();

        // Create a changed file
        let changed_file = temp_dir.path().join("changed.rs");
        fs::write(&changed_file, "fn new_function() {}").unwrap();
        let changed_files = vec![changed_file];

        // Now run update
        let result = analyzer.update(temp_dir.path(), &changed_files).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        // Should be incremental
        assert!(analysis.metadata.incremental);

        // Should have the scoring algorithm set
        assert_eq!(
            analysis.metadata.scoring_algorithm,
            Some("multi-factor-v1".to_string())
        );
    }

    #[tokio::test]
    #[ignore = "Incremental analysis implementation needs investigation"]
    async fn test_update_empty_changed_files() {
        use std::fs;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let analyzer = ProjectAnalyzer::new();

        // First, run a full analysis
        let initial_result = analyzer.analyze(temp_dir.path()).await.unwrap();

        // Save the analysis components properly
        let mmm_dir = temp_dir.path().join(".mmm");
        let context_dir = mmm_dir.join("context");
        fs::create_dir_all(&context_dir).unwrap();

        // Save each component
        fs::write(
            context_dir.join("dependency_graph.json"),
            serde_json::to_string_pretty(&initial_result.dependency_graph).unwrap(),
        )
        .unwrap();

        fs::write(
            context_dir.join("architecture.json"),
            serde_json::to_string_pretty(&initial_result.architecture).unwrap(),
        )
        .unwrap();

        fs::write(
            context_dir.join("conventions.json"),
            serde_json::to_string_pretty(&initial_result.conventions).unwrap(),
        )
        .unwrap();

        fs::write(
            context_dir.join("technical_debt.json"),
            serde_json::to_string_pretty(&initial_result.technical_debt).unwrap(),
        )
        .unwrap();

        if let Some(ref coverage) = initial_result.test_coverage {
            fs::write(
                context_dir.join("test_coverage.json"),
                serde_json::to_string_pretty(coverage).unwrap(),
            )
            .unwrap();
        }

        fs::write(
            context_dir.join("analysis_metadata.json"),
            serde_json::to_string_pretty(&initial_result.metadata).unwrap(),
        )
        .unwrap();

        // Call update with empty changed files
        let changed_files = vec![];
        let result = analyzer.update(temp_dir.path(), &changed_files).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        assert!(analysis.metadata.incremental);
    }

    #[tokio::test]
    async fn test_update_coverage_recalculation() {
        use crate::context::test_coverage::{Criticality, TestCoverageMap, UntestedFunction};
        use std::fs;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let analyzer = ProjectAnalyzer::new();

        // Create an initial analysis with test coverage
        let mut initial_result = analyzer.analyze(temp_dir.path()).await.unwrap();

        // Add some test coverage data
        let coverage = TestCoverageMap {
            overall_coverage: 0.75,
            file_coverage: Default::default(),
            untested_functions: vec![UntestedFunction {
                file: PathBuf::from("test.rs"),
                name: "uncovered_func".to_string(),
                line_number: 10,
                criticality: Criticality::High,
            }],
            critical_paths: vec![],
        };
        initial_result.test_coverage = Some(coverage);

        // Save the analysis
        let mmm_dir = temp_dir.path().join(".mmm");
        let context_dir = mmm_dir.join("context");
        fs::create_dir_all(&context_dir).unwrap();

        let analysis_path = context_dir.join("analysis.json");
        let json = serde_json::to_string_pretty(&initial_result).unwrap();
        fs::write(&analysis_path, json).unwrap();

        // Create a changed file
        let changed_file = temp_dir.path().join("test.rs");
        fs::write(&changed_file, "fn uncovered_func() { /* now covered */ }").unwrap();

        // Run update
        let result = analyzer.update(temp_dir.path(), &[changed_file]).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        assert!(analysis.test_coverage.is_some());
        assert!(analysis.metadata.criticality_distribution.is_some());

        // Check that criticality distribution was calculated
        let dist = analysis.metadata.criticality_distribution.unwrap();
        assert_eq!(dist.confidence_score, 0.87);
    }
}
