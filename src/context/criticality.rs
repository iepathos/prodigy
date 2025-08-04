//! Enhanced criticality assessment for functions and modules

use super::dependencies::DependencyGraph;
use super::test_coverage::{Criticality, UntestedFunction};
use super::ArchitectureInfo;
use crate::context::debt::TechnicalDebtMap;
use crate::metrics::complexity::ComplexityMetrics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Enhanced criticality scoring system with multi-factor analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalityScore {
    pub base_score: f32,        // From keyword/pattern matching (0-30)
    pub complexity_factor: f32, // From cyclomatic/cognitive complexity (1.0-3.0x)
    pub dependency_factor: f32, // From dependency graph position (1.0-2.0x)
    pub change_frequency: f32,  // From git history analysis (0-20)
    pub bug_correlation: f32,   // From historical bug density (0-15)
    pub architecture_role: f32, // From architecture boundaries (0-15)
    pub test_gap_impact: f32,   // From coverage analysis (0-20)
    pub total_score: f32,       // Computed total score
    pub criticality_level: Criticality,
}

/// Explanation for criticality score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalityExplanation {
    pub function_name: String,
    pub score: f32,
    pub level: Criticality,
    pub primary_factors: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Configuration for criticality scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalityConfig {
    pub high_priority_patterns: Vec<String>,
    pub critical_paths: Vec<String>,
    pub weights: ScoringWeights,
    pub thresholds: CriticalityThresholds,
}

/// Scoring weights for different factors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub complexity: f32,
    pub dependencies: f32,
    pub change_frequency: f32,
    pub bug_history: f32,
    pub architecture: f32,
}

/// Thresholds for criticality levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalityThresholds {
    pub high: f32,
    pub medium: f32,
}

/// Enhanced criticality scorer
pub struct EnhancedCriticalityScorer {
    config: CriticalityConfig,
}

impl Default for CriticalityConfig {
    fn default() -> Self {
        Self {
            high_priority_patterns: vec![
                "auth".to_string(),
                "security".to_string(),
                "payment".to_string(),
                "crypto".to_string(),
                "validate".to_string(),
                "encrypt".to_string(),
                "decrypt".to_string(),
                "token".to_string(),
                "permission".to_string(),
                "access".to_string(),
                "api".to_string(),
                "handler".to_string(),
                "process".to_string(),
                "save".to_string(),
                "delete".to_string(),
                "update".to_string(),
            ],
            critical_paths: vec![
                "src/auth".to_string(),
                "src/api".to_string(),
                "src/handlers".to_string(),
                "src/security".to_string(),
                "src/payment".to_string(),
            ],
            weights: ScoringWeights {
                complexity: 2.0,
                dependencies: 1.5,
                change_frequency: 1.2,
                bug_history: 1.8,
                architecture: 1.5,
            },
            thresholds: CriticalityThresholds {
                high: 70.0,
                medium: 40.0,
            },
        }
    }
}

impl EnhancedCriticalityScorer {
    pub fn new(config: Option<CriticalityConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
        }
    }

    /// Score a function based on multiple factors
    pub fn score_function(
        &self,
        func: &UntestedFunction,
        context: &AnalysisContext,
    ) -> CriticalityScore {
        // Base score from pattern matching
        let base_score = self.calculate_base_score(&func.name, &func.file);

        // Complexity factor
        let complexity_factor =
            self.calculate_complexity_factor(&func.name, &func.file, &context.complexity_metrics);

        // Dependency factor
        let dependency_factor =
            self.calculate_dependency_factor(&func.file, &context.dependency_graph);

        // Change frequency from git history
        let change_frequency = self.calculate_change_frequency(&func.file, &context.git_history);

        // Bug correlation
        let bug_correlation = self.calculate_bug_correlation(&func.file, &context.bug_history);

        // Architecture role
        let architecture_role =
            self.calculate_architecture_role(&func.file, &func.name, &context.architecture);

        // Test gap impact
        let test_gap_impact = self.calculate_test_gap_impact(
            &func.file,
            context.surrounding_coverage.get(&func.file).unwrap_or(&0.0),
        );

        // Calculate total score
        let total_score = self.calculate_total_score(
            base_score,
            complexity_factor,
            dependency_factor,
            change_frequency,
            bug_correlation,
            architecture_role,
            test_gap_impact,
        );

        // Determine criticality level
        let criticality_level = self.determine_criticality_level(total_score);

        CriticalityScore {
            base_score,
            complexity_factor,
            dependency_factor,
            change_frequency,
            bug_correlation,
            architecture_role,
            test_gap_impact,
            total_score,
            criticality_level,
        }
    }

    /// Calculate base score from pattern matching
    fn calculate_base_score(&self, func_name: &str, file_path: &Path) -> f32 {
        let mut score: f32 = 0.0;
        let func_lower = func_name.to_lowercase();
        let path_str = file_path.to_string_lossy().to_lowercase();

        // Check function name patterns
        for pattern in &self.config.high_priority_patterns {
            if func_lower.contains(pattern) {
                score += 10.0;
            }
        }

        // Check file path patterns
        for critical_path in &self.config.critical_paths {
            if path_str.contains(&critical_path.to_lowercase()) {
                score += 5.0;
            }
        }

        // Additional patterns
        if func_lower.starts_with("handle_") || func_lower.ends_with("_handler") {
            score += 5.0;
        }

        if func_lower.contains("validate") || func_lower.contains("verify") {
            score += 5.0;
        }

        score.min(30.0)
    }

    /// Calculate complexity factor
    fn calculate_complexity_factor(
        &self,
        func_name: &str,
        file_path: &Path,
        complexity_metrics: &Option<ComplexityMetrics>,
    ) -> f32 {
        if let Some(metrics) = complexity_metrics {
            let file_func_key = format!("{}::{}", file_path.display(), func_name);

            let cyclomatic = metrics
                .cyclomatic_complexity
                .get(&file_func_key)
                .copied()
                .unwrap_or(1);

            let cognitive = metrics
                .cognitive_complexity
                .get(&file_func_key)
                .copied()
                .unwrap_or(cyclomatic);

            // High complexity increases criticality
            let complexity_score = ((cyclomatic + cognitive) as f32 / 2.0).min(30.0);

            // Return multiplier between 1.0 and 3.0
            1.0 + (complexity_score / 15.0)
        } else {
            1.0
        }
    }

    /// Calculate dependency factor
    fn calculate_dependency_factor(
        &self,
        file_path: &Path,
        dependency_graph: &Option<DependencyGraph>,
    ) -> f32 {
        if let Some(graph) = dependency_graph {
            let path_str = file_path.to_string_lossy();

            // Count how many modules depend on this file
            let in_degree = graph
                .edges
                .iter()
                .filter(|edge| edge.to == path_str)
                .count();

            // Count how many modules this file depends on
            let out_degree = graph
                .edges
                .iter()
                .filter(|edge| edge.from == path_str)
                .count();

            // High fan-out indicates important module
            let dependency_score = (in_degree + out_degree / 2).min(20) as f32;

            // Return multiplier between 1.0 and 2.0
            1.0 + (dependency_score / 20.0)
        } else {
            1.0
        }
    }

    /// Calculate change frequency score
    fn calculate_change_frequency(
        &self,
        file_path: &Path,
        git_history: &Option<GitHistory>,
    ) -> f32 {
        if let Some(history) = git_history {
            if let Some(file_history) = history.file_changes.get(file_path) {
                // More changes indicate higher importance
                let change_count = file_history.change_count.min(50) as f32;
                (change_count / 2.5).min(20.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Calculate bug correlation score
    fn calculate_bug_correlation(&self, file_path: &Path, bug_history: &Option<BugHistory>) -> f32 {
        if let Some(history) = bug_history {
            if let Some(bug_count) = history.bugs_per_file.get(file_path) {
                // More bugs indicate higher criticality
                (*bug_count as f32 * 3.0).min(15.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Calculate architecture role score
    fn calculate_architecture_role(
        &self,
        file_path: &Path,
        func_name: &str,
        architecture: &Option<ArchitectureInfo>,
    ) -> f32 {
        if let Some(arch) = architecture {
            let mut score: f32 = 0.0;
            let path_str = file_path.to_string_lossy();

            // Check if function is at architecture boundary
            for layer in &arch.layers {
                if layer.modules.iter().any(|p| p.contains(path_str.as_ref())) {
                    score += 10.0;
                    break;
                }
            }

            // Check if it's part of a key component
            for component in arch.components.values() {
                // Check if this function is in a component's interface
                if component.interfaces.contains(&func_name.to_string()) {
                    score += 5.0;
                }
            }

            score.min(15.0)
        } else {
            0.0
        }
    }

    /// Calculate test gap impact
    fn calculate_test_gap_impact(&self, _file_path: &Path, surrounding_coverage: &f64) -> f32 {
        // Low surrounding coverage makes untested functions more critical
        if *surrounding_coverage < 0.3 {
            20.0
        } else if *surrounding_coverage < 0.5 {
            15.0
        } else if *surrounding_coverage < 0.7 {
            10.0
        } else {
            5.0
        }
    }

    /// Calculate total score with weights
    #[allow(clippy::too_many_arguments)]
    fn calculate_total_score(
        &self,
        base_score: f32,
        complexity_factor: f32,
        dependency_factor: f32,
        change_frequency: f32,
        bug_correlation: f32,
        architecture_role: f32,
        test_gap_impact: f32,
    ) -> f32 {
        let weighted_score = base_score
            + (base_score * (complexity_factor - 1.0) * self.config.weights.complexity)
            + (base_score * (dependency_factor - 1.0) * self.config.weights.dependencies)
            + (change_frequency * self.config.weights.change_frequency)
            + (bug_correlation * self.config.weights.bug_history)
            + (architecture_role * self.config.weights.architecture)
            + test_gap_impact;

        weighted_score.min(100.0)
    }

    /// Determine criticality level from score
    fn determine_criticality_level(&self, score: f32) -> Criticality {
        if score >= self.config.thresholds.high {
            Criticality::High
        } else if score >= self.config.thresholds.medium {
            Criticality::Medium
        } else {
            Criticality::Low
        }
    }

    /// Explain a criticality score
    pub fn explain_score(
        &self,
        score: &CriticalityScore,
        func_name: &str,
    ) -> CriticalityExplanation {
        let mut primary_factors = Vec::new();
        let mut recommendations = Vec::new();

        // Identify primary factors
        if score.base_score >= 15.0 {
            primary_factors.push("Contains critical patterns (auth/security/payment)".to_string());
        }

        if score.complexity_factor >= 2.0 {
            primary_factors.push("High code complexity".to_string());
            recommendations.push("Consider refactoring to reduce complexity".to_string());
        }

        if score.dependency_factor >= 1.5 {
            primary_factors.push("High coupling with other modules".to_string());
        }

        if score.change_frequency >= 10.0 {
            primary_factors.push("Frequently modified code".to_string());
            recommendations.push("Add comprehensive tests due to high change rate".to_string());
        }

        if score.bug_correlation >= 10.0 {
            primary_factors.push("History of bugs in this area".to_string());
            recommendations.push("Add tests to prevent regression".to_string());
        }

        if score.architecture_role >= 10.0 {
            primary_factors.push("Critical architectural boundary".to_string());
        }

        if score.test_gap_impact >= 15.0 {
            primary_factors.push("Low test coverage in surrounding code".to_string());
            recommendations.push("Improve overall file coverage".to_string());
        }

        if primary_factors.is_empty() {
            primary_factors.push("Standard code with no special risk factors".to_string());
        }

        CriticalityExplanation {
            function_name: func_name.to_string(),
            score: score.total_score,
            level: score.criticality_level.clone(),
            primary_factors,
            recommendations,
        }
    }
}

/// Analysis context for criticality scoring
#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub complexity_metrics: Option<ComplexityMetrics>,
    pub dependency_graph: Option<DependencyGraph>,
    pub architecture: Option<ArchitectureInfo>,
    pub git_history: Option<GitHistory>,
    pub bug_history: Option<BugHistory>,
    pub surrounding_coverage: HashMap<PathBuf, f64>,
    pub debt_map: Option<TechnicalDebtMap>,
}

/// Git history information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHistory {
    pub file_changes: HashMap<PathBuf, FileChangeHistory>,
}

/// File change history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeHistory {
    pub change_count: usize,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub authors: Vec<String>,
}

/// Bug history information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugHistory {
    pub bugs_per_file: HashMap<PathBuf, usize>,
    pub bug_patterns: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_score_calculation() {
        let scorer = EnhancedCriticalityScorer::new(None);

        // Test auth function
        let score = scorer.calculate_base_score("authenticate_user", Path::new("src/lib.rs"));
        assert!(score >= 10.0);

        // Test critical path
        let score = scorer.calculate_base_score("some_function", Path::new("src/auth/handler.rs"));
        assert!(score >= 5.0);

        // Test normal function
        let score = scorer.calculate_base_score("format_string", Path::new("src/utils.rs"));
        assert!(score < 10.0);
    }

    #[test]
    fn test_criticality_level_determination() {
        let scorer = EnhancedCriticalityScorer::new(None);

        assert_eq!(scorer.determine_criticality_level(75.0), Criticality::High);
        assert_eq!(
            scorer.determine_criticality_level(50.0),
            Criticality::Medium
        );
        assert_eq!(scorer.determine_criticality_level(30.0), Criticality::Low);
    }

    #[test]
    fn test_complexity_factor() {
        let scorer = EnhancedCriticalityScorer::new(None);
        let mut metrics = ComplexityMetrics {
            cyclomatic_complexity: HashMap::new(),
            cognitive_complexity: HashMap::new(),
            max_nesting_depth: 0,
            total_lines: 0,
        };

        // High complexity function
        metrics
            .cyclomatic_complexity
            .insert("src/lib.rs::complex_func".to_string(), 20);
        metrics
            .cognitive_complexity
            .insert("src/lib.rs::complex_func".to_string(), 25);

        let factor = scorer.calculate_complexity_factor(
            "complex_func",
            Path::new("src/lib.rs"),
            &Some(metrics),
        );

        assert!(factor > 2.0);
    }
}
