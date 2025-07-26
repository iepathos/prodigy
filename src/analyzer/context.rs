//! Context file generation for Claude

use super::{AnalyzerResult, ImprovementArea};
use std::fmt::Write;

/// Context generator for creating analysis reports
pub struct ContextGenerator;

impl ContextGenerator {
    pub fn generate(result: &AnalyzerResult) -> String {
        let mut output = String::new();

        // Header
        writeln!(&mut output, "# Project Analysis\n").unwrap();

        // Overview section
        writeln!(&mut output, "## Overview").unwrap();
        writeln!(&mut output, "- Language: {}", result.language).unwrap();
        if let Some(framework) = &result.framework {
            writeln!(&mut output, "- Framework: {framework}").unwrap();
        }
        writeln!(
            &mut output,
            "- Size: {} files, {} lines",
            result.size.files, result.size.lines
        )
        .unwrap();
        writeln!(&mut output, "- Health Score: {:.1}/10", result.health_score).unwrap();
        writeln!(&mut output).unwrap();

        // Structure section
        writeln!(&mut output, "## Structure").unwrap();
        writeln!(
            &mut output,
            "- Source: {}",
            format_paths(&result.structure.src_dirs)
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Tests: {}",
            if result.structure.test_dirs.is_empty() {
                "None found".to_string()
            } else {
                format_paths(&result.structure.test_dirs)
            }
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Entry Points: {}",
            if result.structure.entry_points.is_empty() {
                "None found".to_string()
            } else {
                format_paths(&result.structure.entry_points)
            }
        )
        .unwrap();
        writeln!(&mut output).unwrap();

        // Build System
        if let Some(build) = &result.build {
            writeln!(&mut output, "## Build System").unwrap();
            writeln!(&mut output, "- Tool: {}", build.tool).unwrap();
            if !build.scripts.is_empty() {
                writeln!(&mut output, "- Available Scripts:").unwrap();
                for (name, _) in build.scripts.iter().take(5) {
                    writeln!(&mut output, "  - {name}").unwrap();
                }
            }
            writeln!(
                &mut output,
                "- Dependencies: {} production, {} development",
                build.dependencies.len(),
                build.dev_dependencies.len()
            )
            .unwrap();
            writeln!(&mut output).unwrap();
        }

        // Quality Indicators
        writeln!(&mut output, "## Quality Indicators").unwrap();
        writeln!(
            &mut output,
            "- Test Coverage: {}",
            if let Some(coverage) = result.health.test_coverage {
                format!("{coverage:.1}%")
            } else if result.health.has_tests {
                "Tests present (coverage unknown)".to_string()
            } else {
                "No tests found".to_string()
            }
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Documentation: {}",
            match result.health.documentation_level {
                super::DocLevel::None => "None",
                super::DocLevel::Minimal => "Minimal",
                super::DocLevel::Good => "Good",
                super::DocLevel::Comprehensive => "Comprehensive",
            }
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Code Complexity: {}",
            match result.health.code_complexity {
                super::ComplexityLevel::Simple => "Simple",
                super::ComplexityLevel::Moderate => "Moderate",
                super::ComplexityLevel::Complex => "Complex",
                super::ComplexityLevel::VeryComplex => "Very Complex",
            }
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Error Handling: {:.0}%",
            result.quality.error_handling_score * 100.0
        )
        .unwrap();
        writeln!(&mut output).unwrap();

        // Code Metrics
        writeln!(&mut output, "## Code Metrics").unwrap();
        writeln!(
            &mut output,
            "- Average Function Length: {:.0} lines",
            result.quality.avg_function_length
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Average File Length: {:.0} lines",
            result.quality.avg_file_length
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Comment Ratio: {:.1}%",
            result.quality.comment_ratio * 100.0
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Test File Ratio: {:.1}%",
            result.quality.test_ratio * 100.0
        )
        .unwrap();
        writeln!(&mut output).unwrap();

        // Development Practices
        writeln!(&mut output, "## Development Practices").unwrap();
        writeln!(
            &mut output,
            "- CI/CD: {}",
            if result.health.has_ci {
                "✓ Configured"
            } else {
                "✗ Not found"
            }
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Linting: {}",
            if result.health.has_linting {
                "✓ Configured"
            } else {
                "✗ Not found"
            }
        )
        .unwrap();
        writeln!(
            &mut output,
            "- Code Formatting: {}",
            if result.health.has_formatting {
                "✓ Configured"
            } else {
                "✗ Not found"
            }
        )
        .unwrap();
        if !result.health.open_todos.is_empty() {
            writeln!(
                &mut output,
                "- Open TODOs: {} found",
                result.health.open_todos.len()
            )
            .unwrap();
        }
        writeln!(&mut output).unwrap();

        // Suggested Improvements
        writeln!(&mut output, "## Suggested Improvements\n").unwrap();
        if !result.focus_areas.primary.is_empty() {
            writeln!(&mut output, "### Primary Focus Areas").unwrap();
            for area in &result.focus_areas.primary {
                writeln!(&mut output, "1. **{area}**").unwrap();
                writeln!(
                    &mut output,
                    "   {}",
                    get_improvement_description(area, result)
                )
                .unwrap();
            }
            writeln!(&mut output).unwrap();
        }

        if !result.focus_areas.secondary.is_empty() {
            writeln!(&mut output, "### Secondary Focus Areas").unwrap();
            for area in &result.focus_areas.secondary {
                writeln!(
                    &mut output,
                    "- **{}**: {}",
                    area,
                    get_brief_description(area)
                )
                .unwrap();
            }
            writeln!(&mut output).unwrap();
        }

        // Key Files
        writeln!(&mut output, "## Key Files").unwrap();

        // Entry points
        if !result.structure.entry_points.is_empty() {
            writeln!(&mut output, "### Entry Points").unwrap();
            for entry in result.structure.entry_points.iter().take(5) {
                if let Some(name) = entry.file_name() {
                    writeln!(&mut output, "- {}", name.to_string_lossy()).unwrap();
                }
            }
            writeln!(&mut output).unwrap();
        }

        // Important files
        if !result.structure.important_files.is_empty() {
            writeln!(&mut output, "### Important Files").unwrap();
            for file in result.structure.important_files.iter().take(5) {
                if let Some(name) = file.file_name() {
                    writeln!(&mut output, "- {}", name.to_string_lossy()).unwrap();
                }
            }
            writeln!(&mut output).unwrap();
        }

        // Config files
        writeln!(&mut output, "### Configuration").unwrap();
        for config in result.structure.config_files.iter().take(5) {
            if let Some(name) = config.path.file_name() {
                writeln!(&mut output, "- {}", name.to_string_lossy()).unwrap();
            }
        }

        output
    }
}

fn format_paths(paths: &[std::path::PathBuf]) -> String {
    if paths.is_empty() {
        "None".to_string()
    } else {
        paths
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn get_improvement_description(area: &ImprovementArea, result: &AnalyzerResult) -> &'static str {
    match area {
        ImprovementArea::TestCoverage => {
            if !result.health.has_tests {
                "No tests found. Consider adding unit tests to improve code reliability and catch bugs early."
            } else {
                "Test coverage is low. Consider adding more tests to uncovered code paths."
            }
        }
        ImprovementArea::ErrorHandling => {
            "Many potential error sites lack proper handling. Consider using Result types, try-catch blocks, or error boundaries."
        }
        ImprovementArea::Documentation => {
            "Code lacks sufficient documentation. Consider adding function docstrings, module documentation, and inline comments."
        }
        ImprovementArea::CodeOrganization => {
            "Code complexity is high. Consider refactoring long functions, splitting large files, and improving module organization."
        }
        ImprovementArea::Dependencies => {
            "Dependencies may be outdated. Consider updating to latest versions for security and performance improvements."
        }
        ImprovementArea::Configuration => {
            "Development tooling is incomplete. Consider adding linting, formatting, and CI/CD configuration."
        }
        ImprovementArea::Performance => {
            "Performance optimizations may be needed. Consider profiling and optimizing hot paths."
        }
        ImprovementArea::TypeSafety => {
            "Type safety could be improved. Consider adding type annotations or migrating to a typed variant."
        }
        ImprovementArea::Security => {
            "Security practices could be enhanced. Consider security scanning and dependency auditing."
        }
        ImprovementArea::Accessibility => {
            "Accessibility features may be missing. Consider adding ARIA labels and keyboard navigation."
        }
    }
}

fn get_brief_description(area: &ImprovementArea) -> &'static str {
    match area {
        ImprovementArea::TestCoverage => "Improve test coverage",
        ImprovementArea::ErrorHandling => "Enhance error handling",
        ImprovementArea::Documentation => "Add documentation",
        ImprovementArea::CodeOrganization => "Refactor complex code",
        ImprovementArea::Dependencies => "Update dependencies",
        ImprovementArea::Configuration => "Improve tooling setup",
        ImprovementArea::Performance => "Optimize performance",
        ImprovementArea::TypeSafety => "Improve type safety",
        ImprovementArea::Security => "Enhance security",
        ImprovementArea::Accessibility => "Improve accessibility",
    }
}
