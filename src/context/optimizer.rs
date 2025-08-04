//! Context optimization for command-specific views

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Optimized context for Claude commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedContext {
    pub command: String,
    pub priority_items: Vec<ActionableItem>,
    pub relevant_analysis: RelevantAnalysis,
    pub recommendations: Vec<String>,
}

/// Actionable item with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionableItem {
    pub item_type: ItemType,
    pub priority: Priority,
    pub estimated_impact: ImpactScore,
    pub suggested_action: String,
    pub implementation_hints: Vec<String>,
    pub related_context: Vec<ContextReference>,
}

/// Types of actionable items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemType {
    TestGap,
    CodeQualityIssue,
    ArchitectureViolation,
    TechnicalDebt,
    SecurityIssue,
    PerformanceIssue,
}

/// Priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

/// Impact score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactScore {
    pub coverage_improvement: Option<f32>,
    pub quality_improvement: Option<f32>,
    pub risk_reduction: Option<f32>,
    pub effort_estimate: u32, // in minutes
}

/// Context reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextReference {
    pub ref_type: ReferenceType,
    pub location: String,
    pub details: String,
}

/// Reference types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReferenceType {
    Function,
    File,
    Module,
    Dependency,
    Pattern,
}

/// Relevant analysis subset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevantAnalysis {
    pub key_metrics: HashMap<String, serde_json::Value>,
    pub focus_areas: Vec<FocusArea>,
    pub context_summary: String,
}

/// Focus area for the command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusArea {
    pub area_type: String,
    pub files: Vec<PathBuf>,
    pub issues: Vec<String>,
    pub priority: Priority,
}

/// Context optimizer
pub struct ContextOptimizer;

impl ContextOptimizer {
    pub fn new() -> Self {
        Self
    }

    /// Optimize context for a specific command
    pub fn optimize_for_command(
        &self,
        command: &str,
        full_context: &AnalysisResult,
    ) -> OptimizedContext {
        match command {
            "/mmm-code-review" => self.optimize_for_code_review(full_context),
            "/mmm-implement-spec" => self.optimize_for_implementation(full_context),
            "/mmm-lint" => self.optimize_for_linting(full_context),
            "/mmm-coverage" => self.optimize_for_coverage(full_context),
            _ => self.create_general_context(command, full_context),
        }
    }

    /// Optimize for code review command
    fn optimize_for_code_review(&self, context: &AnalysisResult) -> OptimizedContext {
        let mut priority_items = Vec::new();
        let mut focus_areas = Vec::new();
        let mut key_metrics = HashMap::new();

        // High-risk code sections from debt analysis
        for debt_item in &context.technical_debt.debt_items {
            if debt_item.impact >= 7 {
                priority_items.push(ActionableItem {
                    item_type: ItemType::TechnicalDebt,
                    priority: Priority::High,
                    estimated_impact: ImpactScore {
                        coverage_improvement: None,
                        quality_improvement: Some(debt_item.impact as f32 * 2.0),
                        risk_reduction: Some(debt_item.impact as f32 * 3.0),
                        effort_estimate: debt_item.effort * 30,
                    },
                    suggested_action: format!("Review and fix: {}", debt_item.title),
                    implementation_hints: vec![
                        debt_item.description.clone(),
                        format!(
                            "Location: {}:{}",
                            debt_item.location.display(),
                            debt_item.line_number.unwrap_or(0)
                        ),
                    ],
                    related_context: vec![ContextReference {
                        ref_type: ReferenceType::File,
                        location: debt_item.location.to_string_lossy().to_string(),
                        details: format!("Type: {:?}", debt_item.debt_type),
                    }],
                });
            }
        }

        // Architecture violations
        for violation in &context.architecture.violations {
            if violation.severity == ViolationSeverity::High {
                focus_areas.push(FocusArea {
                    area_type: "Architecture Violation".to_string(),
                    files: vec![PathBuf::from(&violation.location)],
                    issues: vec![violation.description.clone()],
                    priority: Priority::High,
                });
            }
        }

        // Metrics summary
        key_metrics.insert(
            "total_debt_items".to_string(),
            serde_json::json!(context.technical_debt.debt_items.len()),
        );
        key_metrics.insert(
            "high_priority_debt".to_string(),
            serde_json::json!(context
                .technical_debt
                .debt_items
                .iter()
                .filter(|d| d.impact >= 7)
                .count()),
        );
        key_metrics.insert(
            "architecture_violations".to_string(),
            serde_json::json!(context.architecture.violations.len()),
        );

        OptimizedContext {
            command: "/mmm-code-review".to_string(),
            priority_items,
            relevant_analysis: RelevantAnalysis {
                key_metrics,
                focus_areas,
                context_summary: "Focus on high-impact technical debt and architecture violations"
                    .to_string(),
            },
            recommendations: vec![
                "Prioritize fixing high-impact debt items first".to_string(),
                "Address architecture violations to prevent future issues".to_string(),
                "Look for patterns in debt items to prevent recurrence".to_string(),
            ],
        }
    }

    /// Optimize for implementation command
    fn optimize_for_implementation(&self, context: &AnalysisResult) -> OptimizedContext {
        let mut key_metrics = HashMap::new();
        let mut focus_areas = Vec::new();

        // Convention patterns for implementation
        key_metrics.insert(
            "naming_style".to_string(),
            serde_json::json!(format!("{:?}", context.conventions.naming_patterns)),
        );
        key_metrics.insert(
            "code_patterns".to_string(),
            serde_json::json!(context.conventions.code_patterns.len()),
        );
        key_metrics.insert(
            "project_idioms".to_string(),
            serde_json::json!(context.conventions.project_idioms.len()),
        );

        // Architecture patterns to follow
        for pattern in &context.architecture.patterns {
            focus_areas.push(FocusArea {
                area_type: "Architecture Pattern".to_string(),
                files: vec![],
                issues: vec![format!("Follow {} pattern", pattern)],
                priority: Priority::Medium,
            });
        }

        OptimizedContext {
            command: "/mmm-implement-spec".to_string(),
            priority_items: vec![],
            relevant_analysis: RelevantAnalysis {
                key_metrics,
                focus_areas,
                context_summary: "Follow existing conventions and patterns for consistency"
                    .to_string(),
            },
            recommendations: vec![
                "Use existing code patterns and idioms".to_string(),
                "Follow the project's naming conventions".to_string(),
                "Maintain architectural boundaries".to_string(),
            ],
        }
    }

    /// Optimize for linting command
    fn optimize_for_linting(&self, context: &AnalysisResult) -> OptimizedContext {
        let mut priority_items = Vec::new();
        let mut key_metrics = HashMap::new();

        // Convention violations
        for (file, violations) in context.conventions.get_naming_violations() {
            if !violations.is_empty() {
                priority_items.push(ActionableItem {
                    item_type: ItemType::CodeQualityIssue,
                    priority: Priority::Low,
                    estimated_impact: ImpactScore {
                        coverage_improvement: None,
                        quality_improvement: Some(5.0),
                        risk_reduction: None,
                        effort_estimate: violations.len() as u32 * 5,
                    },
                    suggested_action: format!(
                        "Fix {} naming violations in {}",
                        violations.len(),
                        file
                    ),
                    implementation_hints: violations.clone(),
                    related_context: vec![ContextReference {
                        ref_type: ReferenceType::File,
                        location: file.to_string(),
                        details: "Naming violations".to_string(),
                    }],
                });
            }
        }

        // Complexity hotspots
        let hotspots: Vec<_> = context
            .technical_debt
            .hotspots
            .iter()
            .filter(|h| h.complexity > 15)
            .take(5)
            .collect();

        for hotspot in hotspots {
            priority_items.push(ActionableItem {
                item_type: ItemType::CodeQualityIssue,
                priority: Priority::Medium,
                estimated_impact: ImpactScore {
                    coverage_improvement: None,
                    quality_improvement: Some(15.0),
                    risk_reduction: Some(10.0),
                    effort_estimate: 60,
                },
                suggested_action: format!("Reduce complexity in {}", hotspot.file.display()),
                implementation_hints: vec![
                    format!("Current complexity: {}", hotspot.complexity),
                    "Consider extracting functions or simplifying logic".to_string(),
                ],
                related_context: vec![ContextReference {
                    ref_type: ReferenceType::File,
                    location: hotspot.file.to_string_lossy().to_string(),
                    details: format!("Complexity: {}", hotspot.complexity),
                }],
            });
        }

        key_metrics.insert(
            "total_violations".to_string(),
            serde_json::json!(context.conventions.get_naming_violations().len()),
        );

        OptimizedContext {
            command: "/mmm-lint".to_string(),
            priority_items,
            relevant_analysis: RelevantAnalysis {
                key_metrics,
                focus_areas: vec![],
                context_summary: "Focus on convention violations and complexity reduction"
                    .to_string(),
            },
            recommendations: vec![
                "Fix naming convention violations first".to_string(),
                "Address high-complexity functions".to_string(),
                "Run clippy for additional suggestions".to_string(),
            ],
        }
    }

    /// Optimize for coverage command
    fn optimize_for_coverage(&self, context: &AnalysisResult) -> OptimizedContext {
        let mut priority_items = Vec::new();
        let mut focus_areas = Vec::new();
        let mut key_metrics = HashMap::new();

        if let Some(ref coverage) = context.test_coverage {
            // Priority test gaps
            let high_crit_functions: Vec<_> = coverage
                .untested_functions
                .iter()
                .filter(|f| f.criticality == test_coverage::Criticality::High)
                .take(10)
                .collect();

            for func in high_crit_functions {
                priority_items.push(ActionableItem {
                    item_type: ItemType::TestGap,
                    priority: Priority::Critical,
                    estimated_impact: ImpactScore {
                        coverage_improvement: Some(5.0),
                        quality_improvement: Some(10.0),
                        risk_reduction: Some(20.0),
                        effort_estimate: 30,
                    },
                    suggested_action: format!("Add tests for {}", func.name),
                    implementation_hints: vec![
                        format!("Location: {}:{}", func.file.display(), func.line_number),
                        "This is a critical function that needs test coverage".to_string(),
                        self.generate_test_template(&func.name),
                    ],
                    related_context: vec![ContextReference {
                        ref_type: ReferenceType::Function,
                        location: format!("{}:{}", func.file.display(), func.line_number),
                        details: "Critical untested function".to_string(),
                    }],
                });
            }

            // Critical paths
            for path in &coverage.critical_paths {
                focus_areas.push(FocusArea {
                    area_type: "Critical Path".to_string(),
                    files: path.files.clone(),
                    issues: vec![format!(
                        "{} - Risk: {:?}",
                        path.description, path.risk_level
                    )],
                    priority: Priority::High,
                });
            }

            key_metrics.insert(
                "overall_coverage".to_string(),
                serde_json::json!(format!("{:.1}%", coverage.overall_coverage * 100.0)),
            );
            key_metrics.insert(
                "untested_functions".to_string(),
                serde_json::json!(coverage.untested_functions.len()),
            );
            key_metrics.insert(
                "critical_untested".to_string(),
                serde_json::json!(coverage
                    .untested_functions
                    .iter()
                    .filter(|f| f.criticality == test_coverage::Criticality::High)
                    .count()),
            );
        }

        OptimizedContext {
            command: "/mmm-coverage".to_string(),
            priority_items,
            relevant_analysis: RelevantAnalysis {
                key_metrics,
                focus_areas,
                context_summary: "Focus on critical untested functions and paths".to_string(),
            },
            recommendations: vec![
                "Test critical functions first".to_string(),
                "Ensure authentication and payment paths have coverage".to_string(),
                "Use property-based testing for complex logic".to_string(),
            ],
        }
    }

    /// Create general context
    fn create_general_context(&self, command: &str, context: &AnalysisResult) -> OptimizedContext {
        let mut key_metrics = HashMap::new();

        key_metrics.insert(
            "modules".to_string(),
            serde_json::json!(context.dependency_graph.nodes.len()),
        );
        key_metrics.insert(
            "debt_items".to_string(),
            serde_json::json!(context.technical_debt.debt_items.len()),
        );

        if let Some(ref coverage) = context.test_coverage {
            key_metrics.insert(
                "test_coverage".to_string(),
                serde_json::json!(format!("{:.1}%", coverage.overall_coverage * 100.0)),
            );
        }

        OptimizedContext {
            command: command.to_string(),
            priority_items: vec![],
            relevant_analysis: RelevantAnalysis {
                key_metrics,
                focus_areas: vec![],
                context_summary: "General project context".to_string(),
            },
            recommendations: vec!["Review the full analysis for details".to_string()],
        }
    }

    /// Generate test template for a function
    fn generate_test_template(&self, func_name: &str) -> String {
        format!(
            r#"
#[test]
fn test_{func_name}() {{
    // Arrange
    let input = // TODO: Set up test input
    
    // Act
    let result = {func_name}(input);
    
    // Assert
    assert_eq!(result, expected_value);
}}"#
        )
    }

    /// Get recommendations based on context
    pub fn get_recommendations(&self, context: &OptimizedContext) -> Vec<ActionableItem> {
        let mut items = context.priority_items.clone();

        // Sort by priority and impact
        items.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then_with(|| {
                let a_impact = a.estimated_impact.risk_reduction.unwrap_or(0.0)
                    + a.estimated_impact.quality_improvement.unwrap_or(0.0);
                let b_impact = b.estimated_impact.risk_reduction.unwrap_or(0.0)
                    + b.estimated_impact.quality_improvement.unwrap_or(0.0);
                b_impact
                    .partial_cmp(&a_impact)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        // Return top items
        items.into_iter().take(10).collect()
    }
}

impl Default for ContextOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimize_for_code_review() {
        let optimizer = ContextOptimizer::new();
        let context = create_test_context();

        let optimized = optimizer.optimize_for_command("/mmm-code-review", &context);

        assert_eq!(optimized.command, "/mmm-code-review");
        assert!(!optimized.recommendations.is_empty());
    }

    #[test]
    fn test_optimize_for_coverage() {
        let optimizer = ContextOptimizer::new();
        let context = create_test_context();

        let optimized = optimizer.optimize_for_command("/mmm-coverage", &context);

        assert_eq!(optimized.command, "/mmm-coverage");
        assert!(optimized
            .relevant_analysis
            .key_metrics
            .contains_key("overall_coverage"));
    }

    fn create_test_context() -> AnalysisResult {
        use crate::context::{
            debt::{DebtItem, DebtType},
            test_coverage::UntestedFunction,
        };

        AnalysisResult {
            dependency_graph: DependencyGraph {
                nodes: HashMap::new(),
                edges: vec![],
                cycles: vec![],
                layers: vec![],
            },
            architecture: ArchitectureInfo {
                patterns: vec!["MVC".to_string()],
                layers: vec![],
                components: HashMap::new(),
                violations: vec![],
            },
            conventions: ProjectConventions {
                naming_patterns: conventions::NamingRules {
                    file_naming: conventions::NamingStyle::SnakeCase,
                    function_naming: conventions::NamingStyle::SnakeCase,
                    variable_naming: conventions::NamingStyle::SnakeCase,
                    type_naming: conventions::NamingStyle::PascalCase,
                    constant_naming: conventions::NamingStyle::ScreamingSnakeCase,
                },
                code_patterns: HashMap::new(),
                test_patterns: conventions::TestingConventions {
                    test_file_pattern: "test_".to_string(),
                    test_function_prefix: "test_".to_string(),
                    test_module_pattern: "tests".to_string(),
                    assertion_style: "assert".to_string(),
                },
                project_idioms: vec![],
            },
            technical_debt: debt::TechnicalDebtMap {
                debt_items: vec![DebtItem {
                    id: "debt-1".to_string(),
                    title: "High complexity".to_string(),
                    description: "Complex function".to_string(),
                    debt_type: DebtType::Complexity,
                    location: PathBuf::from("src/main.rs"),
                    line_number: Some(10),
                    impact: 8,
                    effort: 3,
                    tags: vec![],
                }],
                hotspots: vec![],
                duplication_map: HashMap::new(),
                priority_queue: std::collections::BinaryHeap::new(),
            },
            test_coverage: Some(TestCoverageMap {
                file_coverage: HashMap::new(),
                untested_functions: vec![UntestedFunction {
                    file: PathBuf::from("src/auth.rs"),
                    name: "validate_token".to_string(),
                    line_number: 45,
                    criticality: test_coverage::Criticality::High,
                }],
                critical_paths: vec![],
                overall_coverage: 0.75,
            }),
            metadata: AnalysisMetadata {
                timestamp: chrono::Utc::now(),
                duration_ms: 100,
                files_analyzed: 10,
                incremental: false,
                version: "1.0.0".to_string(),
                scoring_algorithm: None,
                criticality_distribution: None,
            },
        }
    }
}
