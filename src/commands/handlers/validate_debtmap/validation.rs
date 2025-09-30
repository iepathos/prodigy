//! Debtmap improvement validation handler
//!
//! This module validates that technical debt improvements have been made by comparing
//! debtmap JSON output before and after changes. It implements the validation logic
//! specified in the /prodigy-validate-debtmap-improvement command.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Debtmap output structure (top-level)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebtmapOutput {
    pub items: Vec<DebtItem>,
    #[serde(default)]
    pub total_debt_score: f64,
    #[serde(default)]
    pub overall_coverage: Option<f64>,
    #[serde(default)]
    pub total_impact: f64,
}

/// Individual debt item from debtmap
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebtItem {
    pub location: ItemLocation,
    pub debt_type: DebtType,
    pub unified_score: UnifiedScore,
    pub function_role: String,
    pub recommendation: Recommendation,
    pub expected_impact: ExpectedImpact,
    #[serde(default)]
    pub upstream_dependencies: u32,
    #[serde(default)]
    pub downstream_dependencies: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ItemLocation {
    pub file: String,
    pub function: String,
    pub line: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DebtType {
    ComplexityHotspot {
        cyclomatic: u32,
        cognitive: u32,
    },
    CoverageGap {
        current_coverage: f64,
        desired_coverage: f64,
    },
    LongFunction {
        lines: u32,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UnifiedScore {
    pub complexity_factor: f64,
    pub coverage_factor: f64,
    pub dependency_factor: f64,
    pub role_multiplier: f64,
    pub final_score: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Recommendation {
    pub primary_action: String,
    pub rationale: String,
    pub implementation_steps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectedImpact {
    pub coverage_improvement: f64,
    pub lines_reduction: i32,
    pub complexity_reduction: f64,
    pub risk_reduction: f64,
}

/// Validation result output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub completion_percentage: f64,
    pub status: ValidationStatus,
    pub improvements: Vec<String>,
    pub remaining_issues: Vec<String>,
    pub gaps: HashMap<String, GapDetail>,
    pub before_summary: DebtSummary,
    pub after_summary: DebtSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    Complete,
    Incomplete,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapDetail {
    pub description: String,
    pub location: String,
    pub severity: String,
    pub suggested_fix: String,
    pub original_score: Option<f64>,
    pub current_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_complexity: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtSummary {
    pub total_items: usize,
    pub high_priority_items: usize,
    pub average_score: f64,
}

/// Priority category for debt items
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,      // Score < 4
    Medium,   // Score 4-6
    High,     // Score 6-8
    Critical, // Score >= 8
}

impl Priority {
    pub fn from_score(score: f64) -> Self {
        if score >= 8.0 {
            Priority::Critical
        } else if score >= 6.0 {
            Priority::High
        } else if score >= 4.0 {
            Priority::Medium
        } else {
            Priority::Low
        }
    }

    pub fn score_penalty(&self) -> f64 {
        match self {
            Priority::Critical => 0.20, // 20% penalty per unresolved item
            Priority::High => 0.10,     // 10% penalty
            Priority::Medium => 0.04,   // 4% penalty
            Priority::Low => 0.015,     // 1.5% penalty
        }
    }
}

/// Load debtmap output from JSON file
pub fn load_debtmap(path: &Path) -> Result<DebtmapOutput> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read debtmap file: {}", path.display()))?;

    serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse debtmap JSON: {}", path.display()))
}

/// Create a unique key for a debt item based on location
pub fn item_key(item: &DebtItem) -> String {
    format!(
        "{}:{}:{}",
        item.location.file, item.location.function, item.location.line
    )
}

/// Calculate summary statistics for a debtmap
pub fn calculate_summary(debtmap: &DebtmapOutput) -> DebtSummary {
    let total_items = debtmap.items.len();
    let high_priority_items = debtmap
        .items
        .iter()
        .filter(|item| item.unified_score.final_score >= 6.0)
        .count();

    let average_score = if total_items > 0 {
        debtmap
            .items
            .iter()
            .map(|item| item.unified_score.final_score)
            .sum::<f64>()
            / total_items as f64
    } else {
        0.0
    };

    DebtSummary {
        total_items,
        high_priority_items,
        average_score,
    }
}

/// Compare two debtmaps and calculate improvement metrics
pub fn compare_debtmaps(before: &DebtmapOutput, after: &DebtmapOutput) -> ValidationResult {
    let before_summary = calculate_summary(before);
    let after_summary = calculate_summary(after);

    // Build lookup maps by item key
    let before_map: HashMap<String, &DebtItem> = before
        .items
        .iter()
        .map(|item| (item_key(item), item))
        .collect();

    let after_map: HashMap<String, &DebtItem> = after
        .items
        .iter()
        .map(|item| (item_key(item), item))
        .collect();

    // Identify changes
    let resolved_items: Vec<&DebtItem> = before
        .items
        .iter()
        .filter(|item| !after_map.contains_key(&item_key(item)))
        .collect();

    let new_items: Vec<&DebtItem> = after
        .items
        .iter()
        .filter(|item| !before_map.contains_key(&item_key(item)))
        .collect();

    let improved_items: Vec<(&DebtItem, &DebtItem)> = before
        .items
        .iter()
        .filter_map(|before_item| {
            let key = item_key(before_item);
            after_map.get(&key).and_then(|after_item| {
                if after_item.unified_score.final_score < before_item.unified_score.final_score {
                    Some((before_item, *after_item))
                } else {
                    None
                }
            })
        })
        .collect();

    // Calculate component scores
    let resolved_high_priority_score = calculate_resolved_high_priority_score(&resolved_items);
    let overall_score_improvement =
        calculate_overall_score_improvement(&before_summary, &after_summary);
    let complexity_reduction_score = calculate_complexity_reduction_score(&improved_items);
    let regression_penalty = calculate_regression_penalty(&new_items);

    // Weighted improvement score
    let improvement_score = (resolved_high_priority_score * 0.4
        + overall_score_improvement * 0.3
        + complexity_reduction_score * 0.2
        + (1.0 - regression_penalty) * 0.1)
        * 100.0;

    let completion_percentage = improvement_score.clamp(0.0, 100.0);

    // Determine status
    let status = if completion_percentage >= 75.0 {
        ValidationStatus::Complete
    } else {
        ValidationStatus::Incomplete
    };

    // Build improvements and remaining issues lists
    let improvements = build_improvements_list(
        &resolved_items,
        &improved_items,
        &before_summary,
        &after_summary,
    );
    let remaining_issues = build_remaining_issues_list(after, &new_items);

    // Build gaps for incomplete improvements
    let gaps = if completion_percentage < 75.0 {
        build_gaps(&before_map, &after_map, &new_items)
    } else {
        HashMap::new()
    };

    ValidationResult {
        completion_percentage,
        status,
        improvements,
        remaining_issues,
        gaps,
        before_summary,
        after_summary,
    }
}

fn calculate_resolved_high_priority_score(resolved_items: &[&DebtItem]) -> f64 {
    if resolved_items.is_empty() {
        return 0.0;
    }

    let high_priority_resolved = resolved_items
        .iter()
        .filter(|item| item.unified_score.final_score >= 6.0)
        .count();

    // Normalize to 0-1 range (assume 2 high-priority items is excellent)
    // This is more realistic for individual fix iterations
    (high_priority_resolved as f64 / 2.0).min(1.0)
}

fn calculate_overall_score_improvement(before: &DebtSummary, after: &DebtSummary) -> f64 {
    if before.average_score == 0.0 {
        return 0.0;
    }

    let improvement = (before.average_score - after.average_score) / before.average_score;
    improvement.clamp(0.0, 1.0)
}

fn calculate_complexity_reduction_score(improved_items: &[(&DebtItem, &DebtItem)]) -> f64 {
    if improved_items.is_empty() {
        return 0.0;
    }

    let avg_reduction: f64 = improved_items
        .iter()
        .map(|(before, after)| {
            (before.unified_score.final_score - after.unified_score.final_score)
                / before.unified_score.final_score
        })
        .sum::<f64>()
        / improved_items.len() as f64;

    avg_reduction.clamp(0.0, 1.0)
}

fn calculate_regression_penalty(new_items: &[&DebtItem]) -> f64 {
    let critical_new = new_items
        .iter()
        .filter(|item| item.unified_score.final_score >= 8.0)
        .count();

    // Heavy penalty for new critical items
    (critical_new as f64 * 0.25).min(1.0)
}

fn build_improvements_list(
    resolved_items: &[&DebtItem],
    improved_items: &[(&DebtItem, &DebtItem)],
    before: &DebtSummary,
    after: &DebtSummary,
) -> Vec<String> {
    let mut improvements = Vec::new();

    let high_priority_resolved = resolved_items
        .iter()
        .filter(|item| item.unified_score.final_score >= 6.0)
        .count();

    if high_priority_resolved > 0 {
        improvements.push(format!(
            "Resolved {} high-priority debt items",
            high_priority_resolved
        ));
    }

    if before.average_score > after.average_score {
        let reduction_pct =
            ((before.average_score - after.average_score) / before.average_score * 100.0) as i32;
        improvements.push(format!("Reduced average debt score by {}%", reduction_pct));
    }

    if !improved_items.is_empty() {
        improvements.push(format!(
            "Improved {} existing debt items",
            improved_items.len()
        ));
    }

    if improvements.is_empty() {
        improvements.push("Some progress made on technical debt".to_string());
    }

    improvements
}

fn build_remaining_issues_list(after: &DebtmapOutput, new_items: &[&DebtItem]) -> Vec<String> {
    let mut issues = Vec::new();

    let critical_remaining = after
        .items
        .iter()
        .filter(|item| item.unified_score.final_score >= 8.0)
        .count();

    if critical_remaining > 0 {
        issues.push(format!(
            "{} critical debt items still present",
            critical_remaining
        ));
    }

    let critical_new = new_items
        .iter()
        .filter(|item| item.unified_score.final_score >= 8.0)
        .count();

    if critical_new > 0 {
        issues.push(format!(
            "Introduced {} new critical debt items",
            critical_new
        ));
    }

    issues
}

fn build_gaps(
    before_map: &HashMap<String, &DebtItem>,
    after_map: &HashMap<String, &DebtItem>,
    new_items: &[&DebtItem],
) -> HashMap<String, GapDetail> {
    let mut gaps = HashMap::new();

    // Find critical debt that wasn't resolved
    for (key, before_item) in before_map.iter() {
        if before_item.unified_score.final_score >= 8.0 {
            if let Some(after_item) = after_map.get(key) {
                if after_item.unified_score.final_score >= 8.0 {
                    gaps.insert(
                        format!("critical_debt_{}", gaps.len()),
                        GapDetail {
                            description: format!(
                                "High-priority debt item still present: {}",
                                before_item.location.function
                            ),
                            location: format!(
                                "{}:{}:{}",
                                before_item.location.file,
                                before_item.location.function,
                                before_item.location.line
                            ),
                            severity: "critical".to_string(),
                            suggested_fix: before_item.recommendation.primary_action.clone(),
                            original_score: Some(before_item.unified_score.final_score),
                            current_score: Some(after_item.unified_score.final_score),
                            original_complexity: None,
                            current_complexity: None,
                            target_complexity: None,
                        },
                    );
                }
            }
        }
    }

    // Find new critical items (regressions)
    for new_item in new_items.iter() {
        if new_item.unified_score.final_score >= 8.0 {
            gaps.insert(
                format!("regression_{}", gaps.len()),
                GapDetail {
                    description: format!(
                        "New critical debt introduced: {}",
                        new_item.location.function
                    ),
                    location: format!(
                        "{}:{}:{}",
                        new_item.location.file, new_item.location.function, new_item.location.line
                    ),
                    severity: "critical".to_string(),
                    suggested_fix: new_item.recommendation.primary_action.clone(),
                    original_score: None,
                    current_score: Some(new_item.unified_score.final_score),
                    original_complexity: None,
                    current_complexity: None,
                    target_complexity: None,
                },
            );
        }
    }

    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_item(
        file: &str,
        function: &str,
        line: u32,
        score: f64,
        cyclomatic: u32,
    ) -> DebtItem {
        DebtItem {
            location: ItemLocation {
                file: file.to_string(),
                function: function.to_string(),
                line,
            },
            debt_type: DebtType::ComplexityHotspot {
                cyclomatic,
                cognitive: cyclomatic * 4,
            },
            unified_score: UnifiedScore {
                complexity_factor: score * 0.3,
                coverage_factor: score * 0.3,
                dependency_factor: score * 0.3,
                role_multiplier: 1.0,
                final_score: score,
            },
            function_role: "EntryPoint".to_string(),
            recommendation: Recommendation {
                primary_action: "Refactor function".to_string(),
                rationale: "Too complex".to_string(),
                implementation_steps: vec!["Step 1".to_string()],
            },
            expected_impact: ExpectedImpact {
                coverage_improvement: 0.0,
                lines_reduction: 10,
                complexity_reduction: 5.0,
                risk_reduction: 10.0,
            },
            upstream_dependencies: 0,
            downstream_dependencies: 0,
        }
    }

    #[test]
    fn test_priority_from_score() {
        assert_eq!(Priority::from_score(9.5), Priority::Critical);
        assert_eq!(Priority::from_score(7.0), Priority::High);
        assert_eq!(Priority::from_score(5.0), Priority::Medium);
        assert_eq!(Priority::from_score(2.0), Priority::Low);
    }

    #[test]
    fn test_item_key() {
        let item = create_test_item("src/main.rs", "main", 10, 5.0, 10);
        assert_eq!(item_key(&item), "src/main.rs:main:10");
    }

    #[test]
    fn test_calculate_summary() {
        let debtmap = DebtmapOutput {
            items: vec![
                create_test_item("src/a.rs", "fn_a", 10, 8.0, 15),
                create_test_item("src/b.rs", "fn_b", 20, 4.0, 8),
                create_test_item("src/c.rs", "fn_c", 30, 2.0, 5),
            ],
            total_debt_score: 14.0,
            overall_coverage: Some(0.75),
            total_impact: 50.0,
        };

        let summary = calculate_summary(&debtmap);
        assert_eq!(summary.total_items, 3);
        assert_eq!(summary.high_priority_items, 1); // Only item with score >= 6.0
        assert!((summary.average_score - 4.666).abs() < 0.01);
    }

    #[test]
    fn test_compare_debtmaps_improvement() {
        let before = DebtmapOutput {
            items: vec![
                create_test_item("src/a.rs", "fn_a", 10, 9.0, 20),
                create_test_item("src/b.rs", "fn_b", 20, 7.0, 12),
                create_test_item("src/c.rs", "fn_c", 30, 3.0, 6),
            ],
            total_debt_score: 19.0,
            overall_coverage: Some(0.70),
            total_impact: 100.0,
        };

        let after = DebtmapOutput {
            items: vec![
                // fn_a resolved (removed)
                create_test_item("src/b.rs", "fn_b", 20, 5.0, 8), // improved
                create_test_item("src/c.rs", "fn_c", 30, 3.0, 6), // unchanged
            ],
            total_debt_score: 8.0,
            overall_coverage: Some(0.85),
            total_impact: 40.0,
        };

        let result = compare_debtmaps(&before, &after);

        // Should show meaningful improvement (resolving 1 critical item + score reduction)
        assert!(
            result.completion_percentage > 40.0,
            "Expected completion > 40%, got {}",
            result.completion_percentage
        );
        assert!(
            result.completion_percentage < 75.0,
            "Should be incomplete status, got {}%",
            result.completion_percentage
        );
        assert_eq!(result.status, ValidationStatus::Incomplete);
        assert!(!result.improvements.is_empty());
        assert_eq!(result.before_summary.total_items, 3);
        assert_eq!(result.after_summary.total_items, 2);
    }

    #[test]
    fn test_compare_debtmaps_regression() {
        let before = DebtmapOutput {
            items: vec![create_test_item("src/a.rs", "fn_a", 10, 5.0, 10)],
            total_debt_score: 5.0,
            overall_coverage: Some(0.80),
            total_impact: 20.0,
        };

        let after = DebtmapOutput {
            items: vec![
                create_test_item("src/a.rs", "fn_a", 10, 5.0, 10),
                create_test_item("src/b.rs", "fn_b", 20, 9.0, 25), // new critical item
            ],
            total_debt_score: 14.0,
            overall_coverage: Some(0.75),
            total_impact: 50.0,
        };

        let result = compare_debtmaps(&before, &after);

        // Should have low score due to regression
        assert!(result.completion_percentage < 50.0);
        assert!(!result.remaining_issues.is_empty());
    }
}
