//! Context summary structures for reduced file sizes

use super::{AnalysisMetadata, AnalysisResult};
use crate::scoring::ProjectHealthScore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Lightweight summary of analysis results with references to component files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    pub metadata: AnalysisMetadata,
    pub component_files: ComponentReferences,
    pub statistics: AnalysisStatistics,
    pub health_score: Option<ProjectHealthScore>,
    pub insights: Vec<String>,
}

/// References to individual component files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentReferences {
    pub dependency_graph: String,
    pub architecture: String,
    pub conventions: String,
    pub technical_debt: String,
    pub test_coverage: Option<String>,
}

/// Key statistics from the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStatistics {
    pub total_files: usize,
    pub total_modules: usize,
    pub dependency_edges: usize,
    pub circular_dependencies: usize,
    pub architectural_violations: usize,
    pub debt_items: usize,
    pub high_priority_debt: usize,
    pub overall_coverage: f64,
    pub untested_functions: usize,
    pub critical_untested: usize,
}

/// Summary of test coverage without listing all functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCoverageSummary {
    pub file_coverage: HashMap<PathBuf, FileCoverageSummary>,
    pub overall_coverage: f64,
    pub untested_summary: UntestedFunctionSummary,
    pub critical_gaps: Vec<CriticalGap>,
}

/// Simplified file coverage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCoverageSummary {
    pub coverage_percentage: f64,
    pub has_tests: bool,
    pub untested_count: usize,
}

/// Summary of untested functions without listing all
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UntestedFunctionSummary {
    pub total_count: usize,
    pub by_criticality: HashMap<String, usize>,
    pub by_file: Vec<FileUntestedSummary>,
}

/// Per-file untested summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUntestedSummary {
    pub file: PathBuf,
    pub count: usize,
    pub highest_criticality: String,
}

/// Critical coverage gap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalGap {
    pub file: PathBuf,
    pub critical_functions: Vec<String>,
    pub coverage_percentage: f64,
}

/// Optimized technical debt summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalDebtSummary {
    pub statistics: DebtStatistics,
    pub high_priority_items: Vec<super::debt::DebtItem>,
    pub hotspot_summary: Vec<HotspotSummary>,
    pub duplication_summary: DuplicationSummary,
}

/// Debt statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtStatistics {
    pub total_items: usize,
    pub by_type: HashMap<String, usize>,
    pub by_impact: HashMap<String, usize>,
    pub avg_impact: f64,
    pub avg_effort: f64,
}

/// Hotspot summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotSummary {
    pub file: PathBuf,
    pub total_issues: usize,
    pub max_complexity: u32,
    pub primary_issue_type: String,
}

/// Duplication summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicationSummary {
    pub total_duplicate_blocks: usize,
    pub total_duplicate_lines: usize,
    pub files_with_duplication: usize,
    pub largest_duplicate_lines: usize,
}

/// Optimized dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraphSummary {
    pub nodes: HashMap<String, NodeSummary>,
    pub edges: Vec<super::dependencies::DependencyEdge>,
    pub cycles: Vec<Vec<String>>,
    pub layers: Vec<super::dependencies::ArchitecturalLayer>,
    pub coupling_analysis: CouplingAnalysis,
}

/// Summarized node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub module_type: super::dependencies::ModuleType,
    pub import_count: usize,
    pub export_count: usize,
    pub external_dep_count: usize,
    pub coupling_score: usize,
}

/// Coupling analysis summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingAnalysis {
    pub high_coupling_modules: Vec<(String, usize)>,
    pub avg_coupling: f64,
    pub max_coupling: usize,
}

impl AnalysisSummary {
    /// Create a summary from full analysis results
    pub fn from_analysis(analysis: &AnalysisResult) -> Self {
        let statistics = AnalysisStatistics {
            total_files: analysis.metadata.files_analyzed,
            total_modules: analysis.dependency_graph.nodes.len(),
            dependency_edges: analysis.dependency_graph.edges.len(),
            circular_dependencies: analysis.dependency_graph.cycles.len(),
            architectural_violations: analysis.architecture.violations.len(),
            debt_items: analysis.technical_debt.debt_items.len(),
            high_priority_debt: analysis
                .technical_debt
                .debt_items
                .iter()
                .filter(|item| item.impact >= 7)
                .count(),
            overall_coverage: analysis
                .test_coverage
                .as_ref()
                .map(|tc| tc.overall_coverage)
                .unwrap_or(0.0),
            untested_functions: analysis
                .test_coverage
                .as_ref()
                .map(|tc| tc.untested_functions.len())
                .unwrap_or(0),
            critical_untested: analysis
                .test_coverage
                .as_ref()
                .map(|tc| {
                    tc.untested_functions
                        .iter()
                        .filter(|f| f.criticality == super::test_coverage::Criticality::High)
                        .count()
                })
                .unwrap_or(0),
        };

        let insights = Self::generate_insights(&statistics, analysis);

        // Calculate unified health score
        let health_score = Some(ProjectHealthScore::from_context(analysis));

        AnalysisSummary {
            metadata: analysis.metadata.clone(),
            component_files: ComponentReferences {
                dependency_graph: "dependency_graph.json".to_string(),
                architecture: "architecture.json".to_string(),
                conventions: "conventions.json".to_string(),
                technical_debt: "technical_debt.json".to_string(),
                test_coverage: analysis
                    .test_coverage
                    .as_ref()
                    .map(|_| "test_coverage.json".to_string()),
            },
            statistics,
            health_score,
            insights,
        }
    }

    /// Generate key insights from the analysis
    fn generate_insights(stats: &AnalysisStatistics, analysis: &AnalysisResult) -> Vec<String> {
        let mut insights = Vec::new();

        // Coverage insight
        if stats.overall_coverage < 0.5 {
            insights.push(format!(
                "Low test coverage ({:.1}%) with {} untested functions",
                stats.overall_coverage * 100.0,
                stats.untested_functions
            ));
        }

        // Circular dependency insight
        if stats.circular_dependencies > 0 {
            insights.push(format!(
                "Found {} circular dependencies that need refactoring",
                stats.circular_dependencies
            ));
        }

        // High priority debt
        if stats.high_priority_debt > 10 {
            insights.push(format!(
                "{} high-priority technical debt items require attention",
                stats.high_priority_debt
            ));
        }

        // Architecture violations
        if stats.architectural_violations > 0 {
            let high_severity = analysis
                .architecture
                .violations
                .iter()
                .filter(|v| v.severity == super::ViolationSeverity::High)
                .count();
            if high_severity > 0 {
                insights.push(format!(
                    "{high_severity} high-severity architecture violations detected"
                ));
            }
        }

        insights
    }
}

impl TestCoverageSummary {
    /// Create a summary from full test coverage
    pub fn from_coverage(coverage: &super::TestCoverageMap) -> Self {
        let mut by_criticality = HashMap::new();
        let mut by_file: HashMap<PathBuf, (usize, String)> = HashMap::new();

        // Count untested functions by criticality and file
        for func in &coverage.untested_functions {
            let crit_str = format!("{:?}", func.criticality);
            *by_criticality.entry(crit_str.clone()).or_insert(0) += 1;

            let entry = by_file
                .entry(func.file.clone())
                .or_insert((0, "Low".to_string()));
            entry.0 += 1;
            // Update highest criticality
            if crit_str == "High" || (crit_str == "Medium" && entry.1 == "Low") {
                entry.1 = crit_str;
            }
        }

        // Convert to summary format
        let mut file_summaries: Vec<_> = by_file
            .into_iter()
            .map(|(file, (count, crit))| FileUntestedSummary {
                file,
                count,
                highest_criticality: crit,
            })
            .collect();

        // Sort by count descending, take top 20
        file_summaries.sort_by(|a, b| b.count.cmp(&a.count));
        file_summaries.truncate(20);

        // Create file coverage summaries
        let file_coverage = coverage
            .file_coverage
            .iter()
            .map(|(path, cov)| {
                let summary = FileCoverageSummary {
                    coverage_percentage: cov.coverage_percentage,
                    has_tests: cov.has_tests,
                    untested_count: coverage
                        .untested_functions
                        .iter()
                        .filter(|f| &f.file == path)
                        .count(),
                };
                (path.clone(), summary)
            })
            .collect();

        // Critical gaps - only files with < 30% coverage and critical functions
        let critical_gaps = coverage
            .untested_functions
            .iter()
            .filter(|f| f.criticality == super::test_coverage::Criticality::High)
            .fold(HashMap::<PathBuf, Vec<String>>::new(), |mut acc, func| {
                acc.entry(func.file.clone())
                    .or_default()
                    .push(func.name.clone());
                acc
            })
            .into_iter()
            .filter_map(|(file, functions)| {
                coverage.file_coverage.get(&file).and_then(|cov| {
                    if cov.coverage_percentage < 30.0 {
                        Some(CriticalGap {
                            file,
                            critical_functions: functions,
                            coverage_percentage: cov.coverage_percentage,
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();

        TestCoverageSummary {
            file_coverage,
            overall_coverage: coverage.overall_coverage,
            untested_summary: UntestedFunctionSummary {
                total_count: coverage.untested_functions.len(),
                by_criticality,
                by_file: file_summaries,
            },
            critical_gaps,
        }
    }
}

impl TechnicalDebtSummary {
    /// Create a summary from full technical debt map
    pub fn from_debt_map(debt_map: &super::debt::TechnicalDebtMap) -> Self {
        let mut by_type = HashMap::new();
        let mut by_impact = HashMap::new();
        let mut total_impact = 0u32;
        let mut total_effort = 0u32;

        for item in &debt_map.debt_items {
            *by_type.entry(format!("{:?}", item.debt_type)).or_insert(0) += 1;

            let impact_level = if item.impact >= 8 {
                "High"
            } else if item.impact >= 5 {
                "Medium"
            } else {
                "Low"
            };
            *by_impact.entry(impact_level.to_string()).or_insert(0) += 1;

            total_impact += item.impact;
            total_effort += item.effort;
        }

        let count = debt_map.debt_items.len().max(1);
        let statistics = DebtStatistics {
            total_items: debt_map.debt_items.len(),
            by_type,
            by_impact,
            avg_impact: total_impact as f64 / count as f64,
            avg_effort: total_effort as f64 / count as f64,
        };

        // Get high priority items (impact >= 7 or effort <= 2)
        let mut high_priority: Vec<_> = debt_map
            .debt_items
            .iter()
            .filter(|item| item.impact >= 7 || item.effort <= 2)
            .cloned()
            .collect();
        high_priority.sort_by(|a, b| b.cmp(a));
        high_priority.truncate(20);

        // Summarize hotspots by file
        let mut hotspot_map: HashMap<PathBuf, (usize, u32, HashMap<String, usize>)> =
            HashMap::new();

        for item in &debt_map.debt_items {
            let entry = hotspot_map
                .entry(item.location.clone())
                .or_insert((0, 0, HashMap::new()));
            entry.0 += 1;
            *entry.2.entry(format!("{:?}", item.debt_type)).or_insert(0) += 1;
        }

        for hotspot in &debt_map.hotspots {
            if let Some(entry) = hotspot_map.get_mut(&hotspot.file) {
                entry.1 = entry.1.max(hotspot.complexity);
            }
        }

        let mut hotspot_summary: Vec<_> = hotspot_map
            .into_iter()
            .map(|(file, (issues, complexity, type_counts))| {
                let primary_type = type_counts
                    .into_iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(t, _)| t)
                    .unwrap_or_else(|| "Unknown".to_string());

                HotspotSummary {
                    file,
                    total_issues: issues,
                    max_complexity: complexity,
                    primary_issue_type: primary_type,
                }
            })
            .collect();

        hotspot_summary.sort_by(|a, b| b.total_issues.cmp(&a.total_issues));
        hotspot_summary.truncate(10);

        // Duplication summary
        let mut total_dup_lines = 0;
        let mut max_dup_lines = 0;
        let mut files_with_dup = std::collections::HashSet::new();

        for blocks in debt_map.duplication_map.values() {
            if let Some(first) = blocks.first() {
                let lines = (first.end_line - first.start_line + 1) as usize;
                total_dup_lines += lines * blocks.len();
                max_dup_lines = max_dup_lines.max(lines);

                for block in blocks {
                    files_with_dup.insert(block.file.clone());
                }
            }
        }

        let duplication_summary = DuplicationSummary {
            total_duplicate_blocks: debt_map.duplication_map.len(),
            total_duplicate_lines: total_dup_lines,
            files_with_duplication: files_with_dup.len(),
            largest_duplicate_lines: max_dup_lines,
        };

        TechnicalDebtSummary {
            statistics,
            high_priority_items: high_priority,
            hotspot_summary,
            duplication_summary,
        }
    }
}

impl DependencyGraphSummary {
    /// Create a summary from full dependency graph
    pub fn from_graph(graph: &super::dependencies::DependencyGraph) -> Self {
        // Calculate coupling scores
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut out_degree: HashMap<String, usize> = HashMap::new();

        for edge in &graph.edges {
            *out_degree.entry(edge.from.clone()).or_insert(0) += 1;
            *in_degree.entry(edge.to.clone()).or_insert(0) += 1;
        }

        // Create node summaries
        let nodes = graph
            .nodes
            .iter()
            .map(|(name, node)| {
                let coupling =
                    in_degree.get(name).unwrap_or(&0) + out_degree.get(name).unwrap_or(&0);
                let summary = NodeSummary {
                    module_type: node.module_type.clone(),
                    import_count: node.imports.len(),
                    export_count: node.exports.len(),
                    external_dep_count: node.external_deps.len(),
                    coupling_score: coupling,
                };
                (name.clone(), summary)
            })
            .collect();

        // Find high coupling modules
        let mut coupling_scores: Vec<_> = in_degree
            .iter()
            .map(|(name, &in_deg)| {
                let out_deg = out_degree.get(name).unwrap_or(&0);
                (name.clone(), in_deg + out_deg)
            })
            .collect();
        coupling_scores.sort_by(|a, b| b.1.cmp(&a.1));

        let total_coupling: usize = coupling_scores.iter().map(|(_, c)| c).sum();
        let avg_coupling = if !coupling_scores.is_empty() {
            total_coupling as f64 / coupling_scores.len() as f64
        } else {
            0.0
        };

        let coupling_analysis = CouplingAnalysis {
            high_coupling_modules: coupling_scores.into_iter().take(10).collect(),
            avg_coupling,
            max_coupling: in_degree
                .values()
                .chain(out_degree.values())
                .max()
                .copied()
                .unwrap_or(0),
        };

        DependencyGraphSummary {
            nodes,
            edges: graph.edges.clone(),
            cycles: graph.cycles.clone(),
            layers: graph.layers.clone(),
            coupling_analysis,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_summary_from_analysis() {
        // Test normal operation
        let analysis = create_test_analysis();
        let summary = AnalysisSummary::from_analysis(&analysis);

        assert!(summary.statistics.total_files > 0);
        assert!(summary.health_score.is_some());
        assert!(!summary.insights.is_empty());
    }

    #[test]
    fn test_analysis_summary_empty_analysis() {
        // Test with empty analysis
        let analysis = AnalysisResult {
            dependency_graph: super::super::dependencies::DependencyGraph {
                nodes: HashMap::new(),
                edges: vec![],
                cycles: vec![],
                layers: vec![],
            },
            architecture: super::super::ArchitectureInfo {
                patterns: vec![],
                layers: vec![],
                components: HashMap::new(),
                violations: vec![],
            },
            conventions: super::super::conventions::ProjectConventions {
                naming_patterns: super::super::conventions::NamingRules {
                    file_naming: super::super::conventions::NamingStyle::SnakeCase,
                    function_naming: super::super::conventions::NamingStyle::SnakeCase,
                    variable_naming: super::super::conventions::NamingStyle::SnakeCase,
                    type_naming: super::super::conventions::NamingStyle::PascalCase,
                    constant_naming: super::super::conventions::NamingStyle::ScreamingSnakeCase,
                },
                code_patterns: HashMap::new(),
                test_patterns: super::super::conventions::TestingConventions {
                    test_file_pattern: "test_".to_string(),
                    test_function_prefix: "test_".to_string(),
                    test_module_pattern: "tests".to_string(),
                    assertion_style: "assert".to_string(),
                },
                project_idioms: vec![],
            },
            technical_debt: super::super::debt::TechnicalDebtMap {
                debt_items: vec![],
                hotspots: vec![],
                duplication_map: HashMap::new(),
                priority_queue: std::collections::BinaryHeap::new(),
            },
            test_coverage: None,
            metadata: super::super::AnalysisMetadata {
                timestamp: chrono::Utc::now(),
                duration_ms: 0,
                files_analyzed: 0,
                incremental: false,
                version: "1.0.0".to_string(),
            },
        };
        let summary = AnalysisSummary::from_analysis(&analysis);

        assert_eq!(summary.statistics.total_files, 0);
        assert_eq!(summary.statistics.debt_items, 0);
    }

    fn create_test_analysis() -> AnalysisResult {
        use super::super::debt::{DebtItem, DebtType, TechnicalDebtMap};

        AnalysisResult {
            metadata: super::super::AnalysisMetadata {
                timestamp: chrono::Utc::now(),
                duration_ms: 100,
                files_analyzed: 10,
                incremental: false,
                version: "1.0.0".to_string(),
            },
            dependency_graph: super::super::dependencies::DependencyGraph {
                nodes: HashMap::new(),
                edges: vec![],
                cycles: vec![],
                layers: vec![],
            },
            architecture: super::super::ArchitectureInfo {
                patterns: vec![],
                layers: vec![],
                components: HashMap::new(),
                violations: vec![],
            },
            conventions: super::super::conventions::ProjectConventions {
                naming_patterns: super::super::conventions::NamingRules {
                    file_naming: super::super::conventions::NamingStyle::SnakeCase,
                    function_naming: super::super::conventions::NamingStyle::SnakeCase,
                    variable_naming: super::super::conventions::NamingStyle::SnakeCase,
                    type_naming: super::super::conventions::NamingStyle::PascalCase,
                    constant_naming: super::super::conventions::NamingStyle::ScreamingSnakeCase,
                },
                code_patterns: HashMap::new(),
                test_patterns: super::super::conventions::TestingConventions {
                    test_file_pattern: "test_".to_string(),
                    test_function_prefix: "test_".to_string(),
                    test_module_pattern: "tests".to_string(),
                    assertion_style: "assert".to_string(),
                },
                project_idioms: vec![],
            },
            technical_debt: TechnicalDebtMap {
                debt_items: vec![DebtItem {
                    id: "test-debt-1".to_string(),
                    title: "Test debt".to_string(),
                    description: "Test description".to_string(),
                    debt_type: DebtType::Complexity,
                    location: PathBuf::from("test.rs"),
                    line_number: Some(10),
                    impact: 8,
                    effort: 3,
                    tags: vec![],
                }],
                hotspots: vec![],
                duplication_map: HashMap::new(),
                priority_queue: std::collections::BinaryHeap::new(),
            },
            test_coverage: None,
        }
    }
}
