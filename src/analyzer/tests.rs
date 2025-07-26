//! Unit tests for the analyzer module

#[cfg(test)]
mod test {
    use super::super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_project(dir: &TempDir) -> PathBuf {
        let root = dir.path().to_path_buf();

        // Create source directory
        let src_dir = root.join("src");
        fs::create_dir(&src_dir).await.unwrap();

        // Create tests directory
        let tests_dir = root.join("tests");
        fs::create_dir(&tests_dir).await.unwrap();

        // Create main.rs
        fs::write(
            src_dir.join("main.rs"),
            r#"
fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#,
        )
        .await
        .unwrap();

        // Create integration test
        fs::write(
            tests_dir.join("integration_test.rs"),
            r#"
#[test]
fn test_integration() {
    assert_eq!(1 + 1, 2);
}
"#,
        )
        .await
        .unwrap();

        // Create Cargo.toml
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
"#,
        )
        .await
        .unwrap();

        // Create README.md
        fs::write(
            root.join("README.md"),
            "# Test Project\n\nThis is a test project.",
        )
        .await
        .unwrap();

        // Create .gitignore
        fs::write(root.join(".gitignore"), "/target\n*.log")
            .await
            .unwrap();

        root
    }

    #[tokio::test]
    async fn test_project_analyzer() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = create_test_project(&temp_dir).await;

        let analyzer = ProjectAnalyzer::new();
        let result = analyzer.analyze(&project_root).await.unwrap();

        // Test language detection
        assert_eq!(result.language, Language::Rust);

        // Test structure detection
        assert!(!result.structure.src_dirs.is_empty());
        assert!(result.structure.src_dirs[0].ends_with("src"));

        // Test build tool detection
        assert!(result.build.is_some());
        let build = result.build.unwrap();
        assert_eq!(build.tool, BuildTool::Cargo);
        assert_eq!(build.dependencies.len(), 1);
        assert_eq!(build.dev_dependencies.len(), 1);

        // Test health indicators
        assert!(result.health.has_tests);
        assert!(result.health.documentation_level != DocLevel::None);

        // Test important files
        assert!(result
            .structure
            .important_files
            .iter()
            .any(|f| f.ends_with("README.md")));
        assert!(result
            .structure
            .important_files
            .iter()
            .any(|f| f.ends_with(".gitignore")));
    }

    #[tokio::test]
    async fn test_language_detection_javascript() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create package.json
        fs::write(
            root.join("package.json"),
            r#"{"name": "test", "version": "1.0.0"}"#,
        )
        .await
        .unwrap();

        // Create index.js
        fs::create_dir(root.join("src")).await.unwrap();
        fs::write(root.join("src/index.js"), "console.log('Hello');")
            .await
            .unwrap();

        let analyzer = StructureAnalyzer::new();
        let structure = analyzer.analyze(root).await.unwrap();

        let detector = LanguageDetector::new();
        let language = detector.detect(&structure).unwrap();

        assert_eq!(language, Language::JavaScript);
    }

    #[tokio::test]
    async fn test_framework_detection_react() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create package.json with React
        fs::write(
            root.join("package.json"),
            r#"{
                "name": "test",
                "version": "1.0.0",
                "dependencies": {
                    "react": "^18.0.0",
                    "react-dom": "^18.0.0"
                }
            }"#,
        )
        .await
        .unwrap();

        let analyzer = StructureAnalyzer::new();
        let structure = analyzer.analyze(root).await.unwrap();

        let detector = FrameworkDetector::new();
        let framework = detector.detect(&structure, &Language::JavaScript).unwrap();

        assert_eq!(framework, Some(Framework::React));
    }

    #[tokio::test]
    async fn test_quality_signals() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let src_dir = root.join("src");
        fs::create_dir(&src_dir).await.unwrap();

        // Create a file with known metrics
        fs::write(
            src_dir.join("main.rs"),
            r#"
// This is a comment
fn short_function() {
    println!("Short");
}

fn long_function() {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
    let e = 5;
    let f = 6;
    let g = 7;
    let h = 8;
    let i = 9;
    let j = 10;
    println!("{} {} {} {} {} {} {} {} {} {}", a, b, c, d, e, f, g, h, i, j);
}

// Another comment
fn main() {
    short_function();
    long_function();
}
"#,
        )
        .await
        .unwrap();

        let analyzer = StructureAnalyzer::new();
        let structure = analyzer.analyze(root).await.unwrap();

        let quality_analyzer = QualityAnalyzer::new();
        let quality = quality_analyzer.analyze(&structure).await.unwrap();

        // Check that we detected functions
        assert!(quality.avg_function_length > 0.0);
        assert!(quality.max_function_length > 0);
        assert!(quality.comment_ratio > 0.0);
    }

    #[tokio::test]
    async fn test_focus_detection() {
        let health = HealthIndicators {
            has_tests: false,
            test_coverage: None,
            has_ci: false,
            has_linting: false,
            has_formatting: false,
            dependencies_updated: true,
            documentation_level: DocLevel::None,
            code_complexity: ComplexityLevel::Simple,
            last_commit: None,
            open_todos: vec![],
        };

        let quality = QualitySignals {
            avg_function_length: 20.0,
            max_function_length: 50,
            avg_file_length: 100.0,
            max_file_length: 200,
            duplicate_code_ratio: 0.05,
            comment_ratio: 0.05,
            test_ratio: 0.0,
            error_handling_score: 0.5,
        };

        let detector = FocusDetector::new();
        let focus = detector.detect(&health, &quality, &Language::Rust);

        // Should prioritize test coverage since there are no tests
        assert!(focus.primary.contains(&ImprovementArea::TestCoverage));

        // Should also flag documentation since it's none
        assert!(
            focus.primary.contains(&ImprovementArea::Documentation)
                || focus.secondary.contains(&ImprovementArea::Documentation)
        );
    }

    #[tokio::test]
    async fn test_context_generation() {
        let result = AnalyzerResult {
            language: Language::Rust,
            framework: Some(Framework::Axum),
            structure: ProjectStructure {
                root: PathBuf::from("/test"),
                src_dirs: vec![PathBuf::from("/test/src")],
                test_dirs: vec![PathBuf::from("/test/tests")],
                config_files: vec![],
                entry_points: vec![PathBuf::from("/test/src/main.rs")],
                important_files: vec![PathBuf::from("/test/README.md")],
                ignored_patterns: vec![],
            },
            health: HealthIndicators {
                has_tests: true,
                test_coverage: Some(75.0),
                has_ci: true,
                has_linting: true,
                has_formatting: true,
                dependencies_updated: true,
                documentation_level: DocLevel::Good,
                code_complexity: ComplexityLevel::Moderate,
                last_commit: None,
                open_todos: vec![],
            },
            build: Some(BuildInfo {
                tool: BuildTool::Cargo,
                scripts: std::collections::HashMap::new(),
                dependencies: vec![],
                dev_dependencies: vec![],
            }),
            quality: QualitySignals {
                avg_function_length: 25.0,
                max_function_length: 100,
                avg_file_length: 150.0,
                max_file_length: 500,
                duplicate_code_ratio: 0.05,
                comment_ratio: 0.15,
                test_ratio: 0.4,
                error_handling_score: 0.8,
            },
            focus_areas: FocusAreas {
                primary: vec![ImprovementArea::TestCoverage],
                secondary: vec![ImprovementArea::CodeOrganization],
                ignore: vec![],
            },
            size: ProjectSize {
                files: 50,
                lines: 5000,
                test_files: 20,
                test_lines: 2000,
            },
            health_score: 7.5,
        };

        let context = ContextGenerator::generate(&result);

        // Check that key sections are present
        assert!(context.contains("# Project Analysis"));
        assert!(context.contains("## Overview"));
        assert!(context.contains("Language: Rust"));
        assert!(context.contains("Framework: Axum"));
        assert!(context.contains("Health Score: 7.5/10"));
        assert!(context.contains("## Quality Indicators"));
        assert!(context.contains("Test Coverage: 75.0%"));
        assert!(context.contains("## Suggested Improvements"));
    }
}
