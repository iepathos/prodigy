//! Smart project analyzer for automatic language, framework, and structure detection

pub mod build;
pub mod context;
pub mod focus;
pub mod framework;
pub mod health;
pub mod language;
pub mod quality;
pub mod structure;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::path::Path;

pub use build::{BuildAnalyzer, BuildInfo, BuildTool};
pub use context::ContextGenerator;
pub use focus::{FocusAreas, FocusDetector, ImprovementArea};
pub use framework::{Framework, FrameworkDetector};
pub use health::{ComplexityLevel, DocLevel, HealthIndicators};
pub use language::{Language, LanguageDetector};
pub use quality::{QualityAnalyzer, QualitySignals};
pub use structure::{ProjectStructure, StructureAnalyzer};

/// Complete analysis result for a project
#[derive(Debug, Clone)]
pub struct AnalyzerResult {
    pub language: Language,
    pub framework: Option<Framework>,
    pub structure: ProjectStructure,
    pub health: HealthIndicators,
    pub build: Option<BuildInfo>,
    pub quality: QualitySignals,
    pub focus_areas: FocusAreas,
    pub size: ProjectSize,
    pub health_score: f32,
}

/// Project size metrics
#[derive(Debug, Clone)]
pub struct ProjectSize {
    pub files: usize,
    pub lines: usize,
    pub test_files: usize,
    pub test_lines: usize,
}

/// Main project analyzer
pub struct ProjectAnalyzer {
    language_detector: LanguageDetector,
    framework_detector: FrameworkDetector,
    structure_analyzer: StructureAnalyzer,
    build_analyzer: BuildAnalyzer,
    quality_analyzer: QualityAnalyzer,
    focus_detector: FocusDetector,
}

impl ProjectAnalyzer {
    pub fn new() -> Self {
        Self {
            language_detector: LanguageDetector::new(),
            framework_detector: FrameworkDetector::new(),
            structure_analyzer: StructureAnalyzer::new(),
            build_analyzer: BuildAnalyzer::new(),
            quality_analyzer: QualityAnalyzer::new(),
            focus_detector: FocusDetector::new(),
        }
    }

    pub async fn analyze(&self, path: &Path) -> Result<AnalyzerResult> {
        // Analyze project structure first
        let structure = self.structure_analyzer.analyze(path).await?;

        // Detect language
        let language = self.language_detector.detect(&structure)?;

        // Detect framework
        let framework = self.framework_detector.detect(&structure, &language)?;

        // Analyze build system
        let build = self.build_analyzer.analyze(&structure).await?;

        // Calculate project size
        let size = self.calculate_size(&structure).await?;

        // Analyze code quality
        let quality = self.quality_analyzer.analyze(&structure).await?;

        // Analyze health indicators
        let health = health::analyze_health(&structure, &quality, &build).await?;

        // Calculate health score
        let health_score = self.calculate_health_score(&health, &quality);

        // Detect focus areas
        let focus_areas = self.focus_detector.detect(&health, &quality, &language);

        Ok(AnalyzerResult {
            language,
            framework,
            structure,
            health,
            build,
            quality,
            focus_areas,
            size,
            health_score,
        })
    }

    async fn calculate_size(&self, structure: &ProjectStructure) -> Result<ProjectSize> {
        let mut files = 0;
        let mut lines = 0;
        let mut test_files = 0;
        let mut test_lines = 0;

        // Count files and lines in source directories
        for src_dir in &structure.src_dirs {
            let (f, l) = count_files_and_lines(src_dir).await?;
            files += f;
            lines += l;
        }

        // Count files and lines in test directories
        for test_dir in &structure.test_dirs {
            let (f, l) = count_files_and_lines(test_dir).await?;
            test_files += f;
            test_lines += l;
        }

        Ok(ProjectSize {
            files,
            lines,
            test_files,
            test_lines,
        })
    }

    fn calculate_health_score(&self, health: &HealthIndicators, quality: &QualitySignals) -> f32 {
        let mut score = 0.0;
        let mut weight = 0.0;

        // Tests contribute 25%
        if health.has_tests {
            score += 2.5;
            if let Some(coverage) = health.test_coverage {
                score += (coverage / 100.0) * 2.5;
                weight += 5.0;
            } else {
                weight += 2.5;
            }
        } else {
            weight += 5.0;
        }

        // CI/CD contributes 15%
        if health.has_ci {
            score += 1.5;
        }
        weight += 1.5;

        // Linting/Formatting contributes 10%
        if health.has_linting {
            score += 0.5;
        }
        if health.has_formatting {
            score += 0.5;
        }
        weight += 1.0;

        // Documentation contributes 20%
        score += match health.documentation_level {
            DocLevel::None => 0.0,
            DocLevel::Minimal => 0.5,
            DocLevel::Good => 1.5,
            DocLevel::Comprehensive => 2.0,
        };
        weight += 2.0;

        // Code quality contributes 20%
        let quality_score = calculate_quality_score(quality);
        score += quality_score * 2.0;
        weight += 2.0;

        // Complexity contributes 10%
        score += match health.code_complexity {
            ComplexityLevel::Simple => 1.0,
            ComplexityLevel::Moderate => 0.7,
            ComplexityLevel::Complex => 0.3,
            ComplexityLevel::VeryComplex => 0.0,
        };
        weight += 1.0;

        // Normalize to 0-10 scale
        (score / weight) * 10.0
    }
}

impl Default for ProjectAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

async fn count_files_and_lines(_path: &Path) -> Result<(usize, usize)> {
    // TODO: Implement actual file and line counting
    // For now, return placeholder values
    Ok((0, 0))
}

fn calculate_quality_score(quality: &QualitySignals) -> f32 {
    let mut score: f32 = 1.0;

    // Penalize long functions
    if quality.avg_function_length > 50.0 {
        score *= 0.8;
    }
    if quality.max_function_length > 200 {
        score *= 0.8;
    }

    // Penalize long files
    if quality.avg_file_length > 300.0 {
        score *= 0.9;
    }
    if quality.max_file_length > 1000 {
        score *= 0.9;
    }

    // Penalize duplicate code
    if quality.duplicate_code_ratio > 0.1 {
        score *= 0.8;
    }

    // Reward good comment ratio
    if quality.comment_ratio > 0.1 {
        score *= 1.1;
    }

    // Reward good test ratio
    if quality.test_ratio > 0.3 {
        score *= 1.1;
    }

    // Reward good error handling
    if quality.error_handling_score > 0.7 {
        score *= 1.1;
    }

    score.clamp(0.0, 1.0)
}
