//! Focus area detection for targeted improvements

use super::health::{ComplexityLevel, DocLevel, HealthIndicators};
use super::language::Language;
use super::quality::QualitySignals;

/// Areas for improvement focus
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImprovementArea {
    ErrorHandling,
    TestCoverage,
    Documentation,
    Performance,
    Security,
    Accessibility,
    CodeOrganization,
    TypeSafety,
    Dependencies,
    Configuration,
}

impl std::fmt::Display for ImprovementArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImprovementArea::ErrorHandling => write!(f, "Error Handling"),
            ImprovementArea::TestCoverage => write!(f, "Test Coverage"),
            ImprovementArea::Documentation => write!(f, "Documentation"),
            ImprovementArea::Performance => write!(f, "Performance"),
            ImprovementArea::Security => write!(f, "Security"),
            ImprovementArea::Accessibility => write!(f, "Accessibility"),
            ImprovementArea::CodeOrganization => write!(f, "Code Organization"),
            ImprovementArea::TypeSafety => write!(f, "Type Safety"),
            ImprovementArea::Dependencies => write!(f, "Dependencies"),
            ImprovementArea::Configuration => write!(f, "Configuration"),
        }
    }
}

/// Focus areas for improvement
#[derive(Debug, Clone)]
pub struct FocusAreas {
    pub primary: Vec<ImprovementArea>,
    pub secondary: Vec<ImprovementArea>,
    pub ignore: Vec<String>,
}

/// Focus area detector
pub struct FocusDetector;

impl FocusDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect(
        &self,
        health: &HealthIndicators,
        quality: &QualitySignals,
        language: &Language,
    ) -> FocusAreas {
        let mut scores = Vec::new();

        // Score each improvement area
        scores.push((
            ImprovementArea::TestCoverage,
            self.score_test_coverage(health, quality),
        ));
        scores.push((
            ImprovementArea::ErrorHandling,
            self.score_error_handling(health, quality, language),
        ));
        scores.push((
            ImprovementArea::Documentation,
            self.score_documentation(health, quality),
        ));
        scores.push((
            ImprovementArea::CodeOrganization,
            self.score_code_organization(health, quality),
        ));
        scores.push((
            ImprovementArea::Dependencies,
            self.score_dependencies(health),
        ));
        scores.push((
            ImprovementArea::Configuration,
            self.score_configuration(health),
        ));
        scores.push((
            ImprovementArea::Performance,
            self.score_performance(quality, language),
        ));
        scores.push((
            ImprovementArea::TypeSafety,
            self.score_type_safety(language, quality),
        ));
        scores.push((
            ImprovementArea::Security,
            self.score_security(health, language),
        ));
        scores.push((
            ImprovementArea::Accessibility,
            self.score_accessibility(language),
        ));

        // Sort by score (higher score = higher priority)
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Select primary focus areas (top 3 with score > 0.5)
        let primary: Vec<ImprovementArea> = scores
            .iter()
            .filter(|(_, score)| *score > 0.5)
            .take(3)
            .map(|(area, _)| area.clone())
            .collect();

        // Select secondary focus areas (next 2 with score > 0.3)
        let secondary: Vec<ImprovementArea> = scores
            .iter()
            .skip(primary.len())
            .filter(|(_, score)| *score > 0.3)
            .take(2)
            .map(|(area, _)| area.clone())
            .collect();

        // Areas to ignore (very low scores)
        let ignore: Vec<String> = scores
            .iter()
            .filter(|(_, score)| *score < 0.1)
            .map(|(area, _)| area.to_string())
            .collect();

        FocusAreas {
            primary,
            secondary,
            ignore,
        }
    }

    fn score_test_coverage(&self, health: &HealthIndicators, quality: &QualitySignals) -> f32 {
        let mut score: f32 = 0.0;

        // No tests at all is critical
        if !health.has_tests {
            score = 1.0;
        } else {
            // Low test coverage
            if let Some(coverage) = health.test_coverage {
                if coverage < 30.0 {
                    score = 0.8;
                } else if coverage < 60.0 {
                    score = 0.5;
                } else if coverage < 80.0 {
                    score = 0.3;
                }
            }

            // Low test ratio
            if quality.test_ratio < 0.1 {
                score = score.max(0.7);
            } else if quality.test_ratio < 0.3 {
                score = score.max(0.4);
            }
        }

        score
    }

    fn score_error_handling(
        &self,
        _health: &HealthIndicators,
        quality: &QualitySignals,
        language: &Language,
    ) -> f32 {
        let mut score: f32 = 0.0;

        // Poor error handling score
        if quality.error_handling_score < 0.3 {
            score = 0.9;
        } else if quality.error_handling_score < 0.6 {
            score = 0.6;
        } else if quality.error_handling_score < 0.8 {
            score = 0.3;
        }

        // Language-specific considerations
        match language {
            Language::Rust => {
                // Rust has good built-in error handling
                score *= 0.8;
            }
            Language::JavaScript | Language::TypeScript => {
                // JS/TS often needs better error handling
                score *= 1.2;
            }
            Language::Python => {
                // Python error handling is important
                score *= 1.1;
            }
            _ => {}
        }

        score.min(1.0)
    }

    fn score_documentation(&self, health: &HealthIndicators, quality: &QualitySignals) -> f32 {
        let mut score: f32;

        // Documentation level
        match health.documentation_level {
            DocLevel::None => score = 0.9,
            DocLevel::Minimal => score = 0.6,
            DocLevel::Good => score = 0.2,
            DocLevel::Comprehensive => score = 0.0,
        }

        // Comment ratio
        if quality.comment_ratio < 0.05 {
            score = score.max(0.7);
        } else if quality.comment_ratio < 0.1 {
            score = score.max(0.4);
        }

        // Many TODOs indicate missing documentation
        if health.open_todos.len() > 20 {
            score = score.max(0.5);
        }

        score
    }

    fn score_code_organization(&self, health: &HealthIndicators, quality: &QualitySignals) -> f32 {
        let mut score: f32;

        // Complexity
        match health.code_complexity {
            ComplexityLevel::VeryComplex => score = 0.9,
            ComplexityLevel::Complex => score = 0.6,
            ComplexityLevel::Moderate => score = 0.3,
            ComplexityLevel::Simple => score = 0.0,
        }

        // Long functions
        if quality.avg_function_length > 50.0 {
            score = score.max(0.7);
        } else if quality.avg_function_length > 30.0 {
            score = score.max(0.4);
        }

        // Long files
        if quality.avg_file_length > 400.0 {
            score = score.max(0.6);
        } else if quality.avg_file_length > 250.0 {
            score = score.max(0.3);
        }

        // Duplicate code
        if quality.duplicate_code_ratio > 0.2 {
            score = score.max(0.8);
        } else if quality.duplicate_code_ratio > 0.1 {
            score = score.max(0.5);
        }

        score
    }

    fn score_dependencies(&self, health: &HealthIndicators) -> f32 {
        if !health.dependencies_updated {
            0.7
        } else {
            0.1
        }
    }

    fn score_configuration(&self, health: &HealthIndicators) -> f32 {
        let mut score: f32 = 0.0;

        // Missing linting
        if !health.has_linting {
            score = 0.5;
        }

        // Missing formatting
        if !health.has_formatting {
            score = score.max(0.4);
        }

        // Missing CI
        if !health.has_ci {
            score = score.max(0.6);
        }

        score
    }

    fn score_performance(&self, quality: &QualitySignals, language: &Language) -> f32 {
        let mut score: f32 = 0.0;

        // Very long functions might have performance issues
        if quality.max_function_length > 500 {
            score = 0.4;
        }

        // Language-specific performance concerns
        match language {
            Language::Python | Language::Ruby => {
                // Interpreted languages benefit more from performance focus
                score *= 1.3;
            }
            Language::Rust | Language::Go | Language::CSharp => {
                // Compiled languages usually have better performance
                score *= 0.7;
            }
            _ => {}
        }

        score.min(1.0)
    }

    fn score_type_safety(&self, language: &Language, quality: &QualitySignals) -> f32 {
        match language {
            Language::JavaScript => {
                // JavaScript could benefit from TypeScript
                0.7
            }
            Language::Python => {
                // Python could benefit from type hints
                if quality.comment_ratio < 0.1 {
                    0.6
                } else {
                    0.3
                }
            }
            Language::Ruby => {
                // Ruby could benefit from type annotations
                0.5
            }
            Language::TypeScript
            | Language::Rust
            | Language::Go
            | Language::Java
            | Language::CSharp
            | Language::Swift
            | Language::Kotlin => {
                // Already strongly typed
                0.0
            }
            _ => 0.2,
        }
    }

    fn score_security(&self, health: &HealthIndicators, language: &Language) -> f32 {
        let mut score: f32 = 0.2; // Base security importance

        // No security scanning
        if !health.has_linting {
            score += 0.2;
        }

        // Web frameworks need more security focus
        match language {
            Language::JavaScript | Language::TypeScript | Language::Python | Language::Ruby => {
                score += 0.2;
            }
            _ => {}
        }

        // Old dependencies are security risks
        if !health.dependencies_updated {
            score += 0.3;
        }

        score.min(1.0)
    }

    fn score_accessibility(&self, language: &Language) -> f32 {
        // Only relevant for frontend languages
        match language {
            Language::JavaScript | Language::TypeScript => 0.4,
            _ => 0.0,
        }
    }
}

impl Default for FocusDetector {
    fn default() -> Self {
        Self::new()
    }
}
