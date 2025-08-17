//! Test data builders for complex test scenarios

use crate::metrics::ImprovementMetrics;

/// Builder for creating test metrics
pub struct MetricsBuilder {
    metrics: ImprovementMetrics,
}

impl Default for MetricsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsBuilder {
    pub fn new() -> Self {
        Self {
            metrics: ImprovementMetrics::new("test-iteration".to_string()),
        }
    }

    pub fn with_test_coverage(mut self, coverage: f32) -> Self {
        self.metrics.test_coverage = coverage;
        self
    }

    pub fn with_type_coverage(mut self, coverage: f32) -> Self {
        self.metrics.type_coverage = coverage;
        self
    }

    pub fn with_lint_warnings(mut self, warnings: u32) -> Self {
        self.metrics.lint_warnings = warnings;
        self
    }

    pub fn with_code_duplication(mut self, percentage: f32) -> Self {
        self.metrics.code_duplication = percentage;
        self
    }

    pub fn with_improvement_velocity(mut self, velocity: f32) -> Self {
        self.metrics.improvement_velocity = velocity;
        self
    }

    pub fn build(self) -> ImprovementMetrics {
        self.metrics
    }
}

/* REMOVED: Analysis-dependent builders
/// Builder for creating test analysis results
pub struct AnalysisResultBuilder {
    result: crate::context::summary::AnalysisSummary,
}

impl Default for AnalysisResultBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisResultBuilder {
    pub fn new() -> Self {
        use crate::context::AnalysisMetadata;
        use chrono::Utc;

        Self {
            result: crate::context::summary::AnalysisSummary {
                metadata: AnalysisMetadata {
                    timestamp: Utc::now(),
                    duration_ms: 100,
                    files_analyzed: 0,
                    incremental: false,
                    version: "test".to_string(),
                    scoring_algorithm: None,
                    criticality_distribution: None,
                },
                component_files: crate::context::summary::ComponentReferences {
                    dependency_graph: "test_deps.json".to_string(),
                    architecture: "test_arch.json".to_string(),
                    conventions: "test_conv.json".to_string(),
                    technical_debt: "test_debt.json".to_string(),
                    test_coverage: None,
                },
                statistics: crate::context::summary::AnalysisStatistics {
                    total_files: 0,
                    total_modules: 0,
                    dependency_edges: 0,
                    circular_dependencies: 0,
                    architectural_violations: 0,
                    debt_items: 0,
                    high_priority_debt: 0,
                    overall_coverage: 0.0,
                    untested_functions: 0,
                    critical_untested: 0,
                },
                health_score: None,
                insights: Vec::new(),
            },
        }
    }

    pub fn with_statistics(mut self, stats: crate::context::summary::AnalysisStatistics) -> Self {
        self.result.statistics = stats;
        self
    }

    pub fn with_insights(mut self, insights: Vec<String>) -> Self {
        self.result.insights = insights;
        self
    }

    pub fn build(self) -> crate::context::summary::AnalysisSummary {
        self.result
    }
}

/// Builder for creating test coverage data
pub struct TestCoverageBuilder {
    overall_coverage: f64,
    file_coverage: HashMap<PathBuf, crate::context::summary::FileCoverageSummary>,
}

impl Default for TestCoverageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCoverageBuilder {
    pub fn new() -> Self {
        Self {
            overall_coverage: 0.0,
            file_coverage: HashMap::new(),
        }
    }

    pub fn with_overall_coverage(mut self, coverage: f64) -> Self {
        self.overall_coverage = coverage;
        self
    }

    pub fn with_file_coverage(mut self, file: &str, coverage: f64) -> Self {
        use crate::context::summary::FileCoverageSummary;
        self.file_coverage.insert(
            PathBuf::from(file),
            FileCoverageSummary {
                coverage_percentage: coverage,
                has_tests: coverage > 0.0,
                untested_count: if coverage < 100.0 { 1 } else { 0 },
            },
        );
        self
    }

    pub fn build(self) -> crate::context::summary::TestCoverageSummary {
        use crate::context::summary::{TestCoverageSummary, UntestedFunctionSummary};

        TestCoverageSummary {
            overall_coverage: self.overall_coverage,
            file_coverage: self.file_coverage,
            untested_summary: UntestedFunctionSummary {
                total_count: 0,
                by_criticality: HashMap::new(),
                by_file: Vec::new(),
            },
            critical_gaps: Vec::new(),
        }
    }
}

/// Builder for creating technical debt items
pub struct TechnicalDebtBuilder {
    debt: crate::context::debt::DebtItem,
}

impl TechnicalDebtBuilder {
    pub fn new(title: &str) -> Self {
        use crate::context::debt::DebtType;
        Self {
            debt: crate::context::debt::DebtItem {
                id: uuid::Uuid::new_v4().to_string(),
                title: title.to_string(),
                description: String::new(),
                debt_type: DebtType::CodeSmell,
                location: PathBuf::from("src/main.rs"),
                line_number: None,
                impact: 5,
                effort: 3,
                tags: Vec::new(),
            },
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.debt.description = description.to_string();
        self
    }

    pub fn with_type(mut self, debt_type: crate::context::debt::DebtType) -> Self {
        self.debt.debt_type = debt_type;
        self
    }

    pub fn with_location(mut self, location: &str) -> Self {
        self.debt.location = PathBuf::from(location);
        self
    }

    pub fn with_impact(mut self, impact: u32) -> Self {
        self.debt.impact = impact;
        self
    }

    pub fn with_effort(mut self, effort: u32) -> Self {
        self.debt.effort = effort;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.debt.tags = tags;
        self
    }

    pub fn build(self) -> crate::context::debt::DebtItem {
        self.debt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_builder() {
        let metrics = MetricsBuilder::new()
            .with_test_coverage(85.5)
            .with_lint_warnings(3)
            .with_improvement_velocity(7.2)
            .build();

        assert_eq!(metrics.test_coverage, 85.5);
        assert_eq!(metrics.lint_warnings, 3);
        assert_eq!(metrics.improvement_velocity, 7.2);
    }

    #[test]
    fn test_coverage_builder() {
        let coverage = TestCoverageBuilder::new()
            .with_overall_coverage(75.0)
            .with_file_coverage("src/main.rs", 90.0)
            .with_file_coverage("src/lib.rs", 60.0)
            .build();

        assert_eq!(coverage.overall_coverage, 75.0);
        assert_eq!(coverage.file_coverage.len(), 2);
    }

    #[test]
    fn test_debt_builder() {
        use crate::context::debt::DebtType;

        let debt = TechnicalDebtBuilder::new("High cyclomatic complexity")
            .with_description("Function has complexity of 15")
            .with_type(DebtType::Complexity)
            .with_location("src/parser.rs")
            .with_impact(8)
            .with_effort(4)
            .with_tags(vec!["refactoring".to_string(), "complexity".to_string()])
            .build();

        assert_eq!(debt.title, "High cyclomatic complexity");
        assert_eq!(debt.debt_type, DebtType::Complexity);
        assert_eq!(debt.impact, 8);
        assert_eq!(debt.tags.len(), 2);
    }
}
*/
