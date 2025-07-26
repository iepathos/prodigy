//! Project health indicators

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;
use std::pin::Pin;
use std::future::Future;

use super::build::BuildInfo;
use super::quality::QualitySignals;
use super::structure::ProjectStructure;

/// Documentation level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocLevel {
    None,
    Minimal,
    Good,
    Comprehensive,
}

/// Code complexity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityLevel {
    Simple,
    Moderate,
    Complex,
    VeryComplex,
}

/// TODO/FIXME item found in code
#[derive(Debug, Clone)]
pub struct TodoItem {
    pub file: String,
    pub line: usize,
    pub text: String,
    pub priority: TodoPriority,
}

/// Priority of TODO items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoPriority {
    Low,
    Medium,
    High,
}

/// Project health indicators
#[derive(Debug, Clone)]
pub struct HealthIndicators {
    pub has_tests: bool,
    pub test_coverage: Option<f32>,
    pub has_ci: bool,
    pub has_linting: bool,
    pub has_formatting: bool,
    pub dependencies_updated: bool,
    pub documentation_level: DocLevel,
    pub code_complexity: ComplexityLevel,
    pub last_commit: Option<DateTime<Utc>>,
    pub open_todos: Vec<TodoItem>,
}

/// Analyze project health
pub async fn analyze_health(
    structure: &ProjectStructure,
    quality: &QualitySignals,
    build: &Option<BuildInfo>,
) -> Result<HealthIndicators> {
    let has_tests = !structure.test_dirs.is_empty() || has_test_files(structure);
    let test_coverage = calculate_test_coverage(quality);
    let has_ci = detect_ci(structure);
    let (has_linting, has_formatting) = detect_linting_and_formatting(structure, build);
    let dependencies_updated = check_dependencies_updated(build);
    let documentation_level = assess_documentation_level(structure, quality);
    let code_complexity = assess_code_complexity(quality);
    let last_commit = get_last_commit_date(&structure.root).await;
    let open_todos = find_todos(structure).await?;

    Ok(HealthIndicators {
        has_tests,
        test_coverage,
        has_ci,
        has_linting,
        has_formatting,
        dependencies_updated,
        documentation_level,
        code_complexity,
        last_commit,
        open_todos,
    })
}

fn has_test_files(structure: &ProjectStructure) -> bool {
    // Check for test files in config
    structure.config_files.iter().any(|cf| {
        cf.path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n.contains("test")
                    || n.contains("spec")
                    || n == "jest.config.js"
                    || n == "pytest.ini"
                    || n == "phpunit.xml"
            })
            .unwrap_or(false)
    })
}

fn calculate_test_coverage(quality: &QualitySignals) -> Option<f32> {
    // If we have test ratio, estimate coverage
    if quality.test_ratio > 0.0 {
        // This is a rough estimate based on test file ratio
        Some((quality.test_ratio * 100.0).min(100.0))
    } else {
        None
    }
}

fn detect_ci(structure: &ProjectStructure) -> bool {
    structure.config_files.iter().any(|cf| {
        matches!(cf.file_type, super::structure::ConfigFileType::CI)
            || cf
                .path
                .components()
                .any(|c| c.as_os_str() == ".github" || c.as_os_str() == "workflows")
    })
}

fn detect_linting_and_formatting(
    structure: &ProjectStructure,
    build: &Option<BuildInfo>,
) -> (bool, bool) {
    let mut has_linting = false;
    let mut has_formatting = false;

    // Check config files
    for cf in &structure.config_files {
        if let Some(name) = cf.path.file_name().and_then(|n| n.to_str()) {
            // Linting configs
            if name.contains("eslint")
                || name.contains("pylint")
                || name.contains("clippy")
                || name.contains("rubocop")
            {
                has_linting = true;
            }

            // Formatting configs
            if name.contains("prettier")
                || name.contains("rustfmt")
                || name.contains("black")
                || name.contains("gofmt")
            {
                has_formatting = true;
            }
        }
    }

    // Check build scripts
    if let Some(build_info) = build {
        for script_name in build_info.scripts.keys() {
            if script_name.contains("lint") {
                has_linting = true;
            }
            if script_name.contains("format") || script_name.contains("fmt") {
                has_formatting = true;
            }
        }
    }

    (has_linting, has_formatting)
}

fn check_dependencies_updated(_build: &Option<BuildInfo>) -> bool {
    // TODO: Implement actual dependency freshness check
    // For now, assume dependencies are updated
    true
}

fn assess_documentation_level(structure: &ProjectStructure, quality: &QualitySignals) -> DocLevel {
    let has_readme = structure.important_files.iter().any(|f| {
        f.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_lowercase().starts_with("readme"))
            .unwrap_or(false)
    });

    let has_docs_dir = structure.src_dirs.iter().any(|d| {
        d.parent()
            .and_then(|p| p.join("docs").exists().then_some(()))
            .is_some()
    });

    match (has_readme, has_docs_dir, quality.comment_ratio) {
        (true, true, r) if r > 0.15 => DocLevel::Comprehensive,
        (true, _, r) if r > 0.1 => DocLevel::Good,
        (true, _, _) => DocLevel::Minimal,
        _ => DocLevel::None,
    }
}

fn assess_code_complexity(quality: &QualitySignals) -> ComplexityLevel {
    let avg_func_score = if quality.avg_function_length > 100.0 {
        3
    } else if quality.avg_function_length > 50.0 {
        2
    } else if quality.avg_function_length > 20.0 {
        1
    } else {
        0
    };

    let max_func_score = if quality.max_function_length > 300 {
        3
    } else if quality.max_function_length > 150 {
        2
    } else if quality.max_function_length > 75 {
        1
    } else {
        0
    };

    let file_score = if quality.avg_file_length > 500.0 {
        2
    } else if quality.avg_file_length > 250.0 {
        1
    } else {
        0
    };

    let total_score = avg_func_score + max_func_score + file_score;

    match total_score {
        0..=2 => ComplexityLevel::Simple,
        3..=4 => ComplexityLevel::Moderate,
        5..=6 => ComplexityLevel::Complex,
        _ => ComplexityLevel::VeryComplex,
    }
}

async fn get_last_commit_date(root: &Path) -> Option<DateTime<Utc>> {
    // Try to get last commit date from git
    if let Ok(output) = tokio::process::Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--format=%cI")
        .current_dir(root)
        .output()
        .await
    {
        if output.status.success() {
            let date_str = String::from_utf8_lossy(&output.stdout);
            date_str.trim().parse().ok()
        } else {
            None
        }
    } else {
        None
    }
}

async fn find_todos(structure: &ProjectStructure) -> Result<Vec<TodoItem>> {
    let mut todos = Vec::new();

    // Search for TODOs in source directories
    for src_dir in &structure.src_dirs {
        search_todos_in_dir(src_dir, &mut todos).await?;
    }

    // Limit to first 50 TODOs
    todos.truncate(50);

    Ok(todos)
}

fn search_todos_in_dir<'a>(
    dir: &'a Path,
    todos: &'a mut Vec<TodoItem>,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    // Only search in source files
                    if matches!(
                        ext.to_str().unwrap_or_default(),
                        "rs" | "py" | "js" | "ts" | "go" | "java" | "cs" | "rb" | "swift" | "kt"
                    ) {
                        search_todos_in_file(&path, todos).await?;
                    }
                }
            } else if path.is_dir()
                && !path
                    .file_name()
                    .map(|n| n == "node_modules")
                    .unwrap_or(false)
            {
                // Recurse into subdirectories
                search_todos_in_dir(&path, todos).await?;
            }
        }

        Ok(())
    })
}

async fn search_todos_in_file(file: &Path, todos: &mut Vec<TodoItem>) -> Result<()> {
    let content = tokio::fs::read_to_string(file).await?;

    for (line_num, line) in content.lines().enumerate() {
        if let Some(todo) = extract_todo(line) {
            todos.push(TodoItem {
                file: file.display().to_string(),
                line: line_num + 1,
                text: todo.0,
                priority: todo.1,
            });
        }
    }

    Ok(())
}

fn extract_todo(line: &str) -> Option<(String, TodoPriority)> {
    let line_upper = line.to_uppercase();

    if let Some(pos) = line_upper.find("TODO") {
        let text = line[pos..].trim().to_string();
        let priority = if line_upper.contains("TODO!") || line_upper.contains("IMPORTANT") {
            TodoPriority::High
        } else if line_upper.contains("FIXME") {
            TodoPriority::Medium
        } else {
            TodoPriority::Low
        };
        Some((text, priority))
    } else if let Some(pos) = line_upper.find("FIXME") {
        let text = line[pos..].trim().to_string();
        Some((text, TodoPriority::Medium))
    } else {
        None
    }
}
