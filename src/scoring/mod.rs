//! Unified scoring system for project health assessment
//!
//! Provides a single, consistent scoring methodology across all analysis types.
//! All scores use 0-100 range where higher is better.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::metrics::ImprovementMetrics;

/// Unified project health score with component breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHealthScore {
    /// Overall health score (0-100, higher is better)
    pub overall: f64,
    /// Individual component scores
    pub components: ScoreComponents,
    /// When the score was calculated
    pub timestamp: DateTime<Utc>,
}

/// Individual scoring components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreComponents {
    /// Test coverage percentage (0-100)
    pub test_coverage: Option<f64>,
    /// Code quality based on lint warnings and duplication (0-100)
    pub code_quality: Option<f64>,
    /// Documentation coverage (0-100)
    pub documentation: Option<f64>,
    /// Maintainability based on complexity and debt (0-100)
    pub maintainability: Option<f64>,
    /// Type safety coverage (0-100)
    pub type_safety: Option<f64>,
}

impl ProjectHealthScore {
    /// Calculate unified health score from metrics
    pub fn from_metrics(metrics: &ImprovementMetrics) -> Self {
        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        let mut components = ScoreComponents {
            test_coverage: None,
            code_quality: None,
            documentation: None,
            maintainability: None,
            type_safety: None,
        };

        // Test coverage (35% weight)
        if metrics.test_coverage > 0.0 {
            components.test_coverage = Some(metrics.test_coverage as f64);
            total_score += metrics.test_coverage as f64 * 0.35;
            total_weight += 0.35;
        }

        // Code quality (25% weight) - based on lint warnings and duplication
        let quality_score =
            calculate_code_quality_score(metrics.lint_warnings, metrics.code_duplication);
        if let Some(score) = quality_score {
            components.code_quality = Some(score);
            total_score += score * 0.25;
            total_weight += 0.25;
        }

        // Maintainability (25% weight) - based on lint warnings and complexity
        let maintainability = calculate_maintainability_from_metrics(metrics);
        components.maintainability = Some(maintainability);
        total_score += maintainability * 0.25;
        total_weight += 0.25;

        // Documentation (10% weight)
        if metrics.doc_coverage > 0.0 {
            components.documentation = Some(metrics.doc_coverage as f64);
            total_score += metrics.doc_coverage as f64 * 0.10;
            total_weight += 0.10;
        }

        // Type safety (5% weight)
        if metrics.type_coverage > 0.0 {
            components.type_safety = Some(metrics.type_coverage as f64);
            total_score += metrics.type_coverage as f64 * 0.05;
            total_weight += 0.05;
        }

        let overall = if total_weight > 0.0 {
            total_score / total_weight
        } else {
            50.0 // Neutral score when no data
        };

        Self {
            overall,
            components,
            timestamp: Utc::now(),
        }
    }

    /* REMOVED: Analysis-dependent method
    /// Calculate unified health score from context analysis
    pub fn from_context(analysis: &AnalysisResult) -> Self {
        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        let mut components = ScoreComponents {
            test_coverage: None,
            code_quality: None,
            documentation: None,
            maintainability: None,
            type_safety: None,
        };

        // Test coverage (35% weight)
        if let Some(coverage_data) = &analysis.test_coverage {
            let coverage = coverage_data.overall_coverage * 100.0;
            components.test_coverage = Some(coverage);
            total_score += coverage * 0.35;
            total_weight += 0.35;
        }

        // Code quality (25% weight) - based on conventions and patterns
        // For now, estimate quality based on code patterns and project idioms
        // Higher number of patterns and idioms indicates better established conventions
        let pattern_count = analysis.conventions.code_patterns.len();
        let idiom_count = analysis.conventions.project_idioms.len();
        let quality_score = calculate_quality_from_patterns(pattern_count, idiom_count);
        components.code_quality = Some(quality_score);
        total_score += quality_score * 0.25;
        total_weight += 0.25;

        // Maintainability (25% weight) - based on debt items and complexity
        {
            let maintainability =
                calculate_maintainability_score(&analysis.technical_debt.debt_items);
            components.maintainability = Some(maintainability);
            total_score += maintainability * 0.25;
            total_weight += 0.25;
        }

        // Documentation (10% weight) - estimate from context
        // Since context analysis doesn't directly measure doc coverage,
        // we can estimate based on TODO/FIXME items mentioning docs
        {
            let doc_debt_count = analysis
                .technical_debt
                .debt_items
                .iter()
                .filter(|item| {
                    matches!(
                        item.debt_type,
                        crate::context::debt::DebtType::Todo
                            | crate::context::debt::DebtType::Fixme
                    ) && item.description.to_lowercase().contains("document")
                })
                .count();
            let estimated_doc_score = if doc_debt_count == 0 {
                80.0 // Good if no doc debt
            } else {
                f64::max(50.0 - (doc_debt_count as f64 * 5.0), 10.0)
            };
            components.documentation = Some(estimated_doc_score);
            total_score += estimated_doc_score * 0.10;
            total_weight += 0.10;
        }

        // Type safety (5% weight) - load from metrics if available
        if let Ok(type_coverage) = load_type_coverage_from_metrics(&analysis.metadata) {
            components.type_safety = Some(type_coverage);
            total_score += type_coverage * 0.05;
            total_weight += 0.05;
        }

        let overall = if total_weight > 0.0 {
            total_score / total_weight
        } else {
            50.0 // Neutral score when no data
        };

        Self {
            overall,
            components,
            timestamp: Utc::now(),
        }
    }
    */

    /// Get improvement suggestions based on component scores
    pub fn get_improvement_suggestions(&self) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Check test coverage
        if let Some(coverage) = self.components.test_coverage {
            if coverage < 60.0 {
                suggestions.push(format!("Increase test coverage (current: {coverage:.1}%)"));
            }
        } else {
            suggestions.push("Add test coverage tracking to the project".to_string());
        }

        // Check code quality
        if let Some(quality) = self.components.code_quality {
            if quality < 70.0 {
                suggestions.push("Address lint warnings and reduce code duplication".to_string());
            }
        }

        // Check documentation
        if let Some(docs) = self.components.documentation {
            if docs < 50.0 {
                suggestions.push(format!(
                    "Improve documentation coverage (current: {docs:.1}%)"
                ));
            }
        }

        // Check maintainability
        if let Some(maint) = self.components.maintainability {
            if maint < 70.0 {
                suggestions.push("Reduce technical debt and code complexity".to_string());
            }
        }

        // Check type safety
        if let Some(types) = self.components.type_safety {
            if types < 80.0 {
                suggestions.push(format!("Improve type annotations (current: {types:.1}%)"));
            }
        }

        // Limit to top 3 suggestions
        suggestions.truncate(3);
        suggestions
    }
}

/// Calculate code quality score based on lint warnings and duplication
fn calculate_code_quality_score(lint_warnings: u32, code_duplication: f32) -> Option<f64> {
    // Start with perfect score
    let mut score = 100.0;

    // Deduct for lint warnings (max 30 point deduction)
    let warning_penalty = (lint_warnings as f64 * 2.0).min(30.0);
    score -= warning_penalty;

    // Deduct for code duplication (max 20 point deduction)
    let duplication_penalty = (code_duplication as f64).min(20.0);
    score -= duplication_penalty;

    Some(score.max(0.0))
}

/// Calculate code quality from patterns and idioms
#[allow(dead_code)]
fn calculate_quality_from_patterns(pattern_count: usize, idiom_count: usize) -> f64 {
    // Start with base score
    let mut score = 50.0;

    // Add points for established patterns (up to 30 points)
    score += (pattern_count as f64 * 5.0).min(30.0);

    // Add points for project idioms (up to 20 points)
    score += (idiom_count as f64 * 4.0).min(20.0);

    score.min(100.0)
}

/// Calculate maintainability from metrics
fn calculate_maintainability_from_metrics(metrics: &ImprovementMetrics) -> f64 {
    let mut score = 100.0;

    // Deduct for lint warnings (max 30 point deduction)
    let warning_penalty = (metrics.lint_warnings as f64 * 2.0).min(30.0);
    score -= warning_penalty;

    // Calculate average complexity from either new or old format
    let avg_complexity = if let Some(ref summary) = metrics.complexity_summary {
        // New compressed format: calculate weighted average across files
        if summary.total_functions > 0 {
            let total_cyclomatic: f32 = summary
                .by_file
                .values()
                .map(|stats| stats.avg_cyclomatic * stats.functions_count as f32)
                .sum();
            (total_cyclomatic / summary.total_functions as f32) as f64
        } else {
            0.0
        }
    } else if !metrics.cyclomatic_complexity.is_empty() {
        // Old format: calculate average from raw data
        metrics.cyclomatic_complexity.values().sum::<u32>() as f64
            / metrics.cyclomatic_complexity.len() as f64
    } else {
        0.0
    };

    // Deduct for high complexity
    if avg_complexity > 0.0 {
        let complexity_penalty = (avg_complexity * 2.0).min(30.0);
        score -= complexity_penalty;
    }

    // Deduct for low test coverage
    let coverage_penalty = ((100.0 - metrics.test_coverage as f64) * 0.2).min(20.0);
    score -= coverage_penalty;

    score.max(0.0)
}

/* REMOVED: Analysis-dependent functions
/// Calculate maintainability score from technical debt items
fn calculate_maintainability_score(debt_items: &[TechnicalDebtItem]) -> f64 {
    // Count high-impact items
    let high_impact_count = debt_items.iter().filter(|item| item.impact >= 7).count();

    // Count medium-impact items
    let medium_impact_count = debt_items
        .iter()
        .filter(|item| item.impact >= 4 && item.impact < 7)
        .count();

    // Start with perfect score
    let mut score = 100.0;

    // Deduct more for high-impact items
    score -= (high_impact_count as f64 * 5.0).min(40.0);
    score -= (medium_impact_count as f64 * 2.0).min(30.0);

    // Consider total debt count with logarithmic scaling
    let total_debt = debt_items.len() as f64;
    if total_debt > 0.0 {
        let debt_penalty = (total_debt.ln() * 5.0).min(20.0);
        score -= debt_penalty;
    }

    score.max(0.0)
}

/// Calculate technical debt score from debt items (0-100, higher is better)
pub fn calculate_technical_debt_score(debt_items: &[TechnicalDebtItem]) -> f64 {
    // Count items by severity
    let critical_count = debt_items.iter().filter(|item| item.impact >= 9).count();
    let high_count = debt_items
        .iter()
        .filter(|item| item.impact >= 7 && item.impact < 9)
        .count();
    let medium_count = debt_items
        .iter()
        .filter(|item| item.impact >= 4 && item.impact < 7)
        .count();
    let low_count = debt_items.iter().filter(|item| item.impact < 4).count();

    // Start with perfect score
    let mut score = 100.0;

    // Apply penalties based on severity and count
    score -= (critical_count as f64 * 10.0).min(40.0); // Critical items have heavy impact
    score -= (high_count as f64 * 5.0).min(30.0); // High impact items
    score -= (medium_count as f64 * 2.0).min(20.0); // Medium impact items
    score -= (low_count as f64 * 0.5).min(10.0); // Low impact items have minimal effect

    // Additional penalty for overall debt volume (logarithmic)
    let total_debt = debt_items.len() as f64;
    if total_debt > 10.0 {
        let volume_penalty = ((total_debt - 10.0).ln() * 3.0).min(10.0);
        score -= volume_penalty;
    }

    // Consider debt type distribution
    let mut type_counts = std::collections::HashMap::new();
    for item in debt_items {
        *type_counts.entry(&item.debt_type).or_insert(0) += 1;
    }

    // Penalty for concentration of specific debt types
    for (debt_type, count) in type_counts {
        use crate::context::debt::DebtType;
        let type_penalty = match debt_type {
            DebtType::Security => (count as f64 * 3.0).min(15.0), // Security issues are critical
            DebtType::Performance => (count as f64 * 2.0).min(10.0),
            DebtType::Complexity => (count as f64 * 1.5).min(10.0),
            DebtType::Duplication => (count as f64 * 1.0).min(8.0),
            _ => (count as f64 * 0.5).min(5.0),
        };
        score -= type_penalty * 0.2; // Apply 20% of type penalty
    }

    score.max(0.0)
}
*/

/// Format score component for display
pub fn format_component(name: &str, value: Option<f64>, details: Option<&str>) -> String {
    match value {
        Some(v) if v >= 70.0 => {
            if let Some(d) = details {
                format!("  ✓ {name}: {v:.1}% {d}")
            } else {
                format!("  ✓ {name}: {v:.1}%")
            }
        }
        Some(v) if v >= 40.0 => {
            if let Some(d) = details {
                format!("  ⚠ {name}: {v:.1}% {d}")
            } else {
                format!("  ⚠ {name}: {v:.1}%")
            }
        }
        Some(v) => {
            if let Some(d) = details {
                format!("  ✗ {name}: {v:.1}% {d}")
            } else {
                format!("  ✗ {name}: {v:.1}%")
            }
        }
        None => format!("  - {name}: N/A"),
    }
}

/* REMOVED: Analysis-dependent function
/// Load type coverage from metrics file
fn load_type_coverage_from_metrics(_metadata: &AnalysisMetadata) -> Result<f64, ()> {
    // Get the project path from the current directory
    let project_path = std::env::current_dir().map_err(|_| ())?;
    let metrics_file = project_path
        .join(".mmm")
        .join("metrics")
        .join("current.json");

    if !metrics_file.exists() {
        return Err(());
    }

    // Read and parse metrics file
    let content = std::fs::read_to_string(&metrics_file).map_err(|_| ())?;
    let metrics: serde_json::Value = serde_json::from_str(&content).map_err(|_| ())?;

    // Extract type coverage
    metrics
        .get("type_coverage")
        .and_then(|v| v.as_f64())
        .ok_or(())
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_score_from_metrics() {
        let metrics = ImprovementMetrics {
            test_coverage: 75.0,
            lint_warnings: 5,
            code_duplication: 10.0,
            doc_coverage: 60.0,
            type_coverage: 90.0,
            ..Default::default()
        };

        let score = ProjectHealthScore::from_metrics(&metrics);

        assert!(score.overall > 70.0);
        assert_eq!(score.components.test_coverage, Some(75.0));
        assert!(score.components.code_quality.unwrap() > 70.0);
        assert!(score.components.maintainability.is_some());
        assert_eq!(score.components.documentation, Some(60.0));
        assert_eq!(score.components.type_safety, Some(90.0));
    }

    #[test]
    fn test_missing_data_handling() {
        let metrics = ImprovementMetrics::default();
        let score = ProjectHealthScore::from_metrics(&metrics);

        // Should handle missing data gracefully
        assert!(score.overall >= 0.0);
        assert!(score.overall <= 100.0);
    }

    #[test]
    fn test_improvement_suggestions() {
        let metrics = ImprovementMetrics {
            test_coverage: 30.0,
            doc_coverage: 20.0,
            ..Default::default()
        };

        let score = ProjectHealthScore::from_metrics(&metrics);
        let suggestions = score.get_improvement_suggestions();

        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.contains("test coverage")));
        assert!(suggestions.iter().any(|s| s.contains("documentation")));
    }
}
