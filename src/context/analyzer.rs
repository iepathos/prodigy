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
    /// Create a new project analyzer with default components
    pub fn new() -> Self {
        let subprocess = SubprocessManager::production();
        Self {
            dependency_analyzer: Box::new(dependencies::BasicDependencyAnalyzer::new()),
            architecture_extractor: Box::new(architecture::BasicArchitectureExtractor::new()),
            convention_detector: Box::new(conventions::BasicConventionDetector::new()),
            debt_mapper: Box::new(debt::BasicTechnicalDebtMapper::new()),
            coverage_analyzer: Box::new(super::tarpaulin_coverage::TarpaulinCoverageAnalyzer::new(
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

        let result = AnalysisResult {
            dependency_graph: deps,
            architecture: arch,
            conventions: conv,
            technical_debt: debt,
            test_coverage: Some(coverage),
            metadata: AnalysisMetadata {
                timestamp: chrono::Utc::now(),
                duration_ms,
                files_analyzed,
                incremental: false,
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        // Save analysis results
        save_analysis(project_path, &result)?;

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
            result.test_coverage = Some(
                self.coverage_analyzer
                    .update_coverage(project_path, test_coverage, changed_files)
                    .await?,
            );
        }

        // Update metadata
        result.metadata.timestamp = chrono::Utc::now();
        result.metadata.duration_ms = start.elapsed().as_millis() as u64;
        result.metadata.incremental = true;

        // Save updated analysis
        save_analysis(project_path, &result)?;

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
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "target"
                && name != "node_modules"
                && name != "dist"
                && name != "build"
        })
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
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
