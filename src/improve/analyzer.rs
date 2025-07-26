use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    Unknown,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "Rust"),
            Language::Python => write!(f, "Python"),
            Language::JavaScript => write!(f, "JavaScript"),
            Language::TypeScript => write!(f, "TypeScript"),
            Language::Go => write!(f, "Go"),
            Language::Java => write!(f, "Java"),
            Language::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Framework {
    React,
    Vue,
    Angular,
    Django,
    Flask,
    FastAPI,
    Spring,
    Express,
    Actix,
    Rocket,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectSize {
    Small,  // < 1k lines
    Medium, // 1k - 10k lines
    Large,  // 10k - 100k lines
    XLarge, // > 100k lines
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStructure {
    pub has_tests: bool,
    pub has_docs: bool,
    pub has_ci: bool,
    pub main_dirs: Vec<String>,
    pub config_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIndicators {
    pub has_readme: bool,
    pub has_license: bool,
    pub has_gitignore: bool,
    pub uses_linter: bool,
    pub uses_formatter: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub language: Language,
    pub framework: Option<Framework>,
    pub size: ProjectSize,
    pub test_coverage: Option<f32>,
    pub structure: ProjectStructure,
    pub health_indicators: HealthIndicators,
    pub line_count: usize,
    pub file_count: usize,
    pub focus_areas: Vec<String>,
}

impl ProjectInfo {
    pub fn summary(&self) -> String {
        format!(
            "Detected: {} project ({} lines, {} files)",
            self.language, self.line_count, self.file_count
        )
    }
}

pub struct ProjectAnalyzer;

impl ProjectAnalyzer {
    pub async fn analyze(path: impl AsRef<Path>) -> Result<ProjectInfo> {
        let path = path.as_ref();

        let language = Self::detect_language(path)?;
        let framework = Self::detect_framework(path, &language)?;
        let (line_count, file_count) = Self::calculate_size(path)?;
        let size = Self::categorize_size(line_count);
        let structure = Self::analyze_structure(path)?;
        let health_indicators = Self::analyze_health(path)?;
        let focus_areas = Self::determine_focus_areas(&structure, &health_indicators);

        Ok(ProjectInfo {
            language,
            framework,
            size,
            test_coverage: None, // TODO: Implement test coverage detection
            structure,
            health_indicators,
            line_count,
            file_count,
            focus_areas,
        })
    }

    fn detect_language(path: &Path) -> Result<Language> {
        // Check for language-specific files
        if path.join("Cargo.toml").exists() {
            return Ok(Language::Rust);
        }
        if path.join("package.json").exists() {
            let content = fs::read_to_string(path.join("package.json"))?;
            if content.contains("\"typescript\"") || path.join("tsconfig.json").exists() {
                return Ok(Language::TypeScript);
            }
            return Ok(Language::JavaScript);
        }
        if path.join("requirements.txt").exists()
            || path.join("setup.py").exists()
            || path.join("pyproject.toml").exists()
        {
            return Ok(Language::Python);
        }
        if path.join("go.mod").exists() {
            return Ok(Language::Go);
        }
        if path.join("pom.xml").exists() || path.join("build.gradle").exists() {
            return Ok(Language::Java);
        }

        // Fall back to checking file extensions
        let mut lang_counts = std::collections::HashMap::new();
        for entry in WalkDir::new(path).max_depth(3) {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    match ext.to_str() {
                        Some("rs") => *lang_counts.entry(Language::Rust).or_insert(0) += 1,
                        Some("py") => *lang_counts.entry(Language::Python).or_insert(0) += 1,
                        Some("js") | Some("jsx") => {
                            *lang_counts.entry(Language::JavaScript).or_insert(0) += 1
                        }
                        Some("ts") | Some("tsx") => {
                            *lang_counts.entry(Language::TypeScript).or_insert(0) += 1
                        }
                        Some("go") => *lang_counts.entry(Language::Go).or_insert(0) += 1,
                        Some("java") => *lang_counts.entry(Language::Java).or_insert(0) += 1,
                        _ => {}
                    }
                }
            }
        }

        Ok(lang_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(lang, _)| lang)
            .unwrap_or(Language::Unknown))
    }

    fn detect_framework(path: &Path, language: &Language) -> Result<Option<Framework>> {
        match language {
            Language::JavaScript | Language::TypeScript => {
                if path.join("package.json").exists() {
                    let content = fs::read_to_string(path.join("package.json"))?;
                    if content.contains("\"react\"") {
                        return Ok(Some(Framework::React));
                    }
                    if content.contains("\"vue\"") {
                        return Ok(Some(Framework::Vue));
                    }
                    if content.contains("\"@angular/core\"") {
                        return Ok(Some(Framework::Angular));
                    }
                    if content.contains("\"express\"") {
                        return Ok(Some(Framework::Express));
                    }
                }
            }
            Language::Python => {
                let files = ["requirements.txt", "pyproject.toml", "setup.py"];
                for file in files {
                    if path.join(file).exists() {
                        let content = fs::read_to_string(path.join(file))?;
                        if content.contains("django") {
                            return Ok(Some(Framework::Django));
                        }
                        if content.contains("flask") {
                            return Ok(Some(Framework::Flask));
                        }
                        if content.contains("fastapi") {
                            return Ok(Some(Framework::FastAPI));
                        }
                    }
                }
            }
            Language::Rust => {
                if path.join("Cargo.toml").exists() {
                    let content = fs::read_to_string(path.join("Cargo.toml"))?;
                    if content.contains("actix-web") {
                        return Ok(Some(Framework::Actix));
                    }
                    if content.contains("rocket") {
                        return Ok(Some(Framework::Rocket));
                    }
                }
            }
            Language::Java => {
                if path.join("pom.xml").exists() {
                    let content = fs::read_to_string(path.join("pom.xml"))?;
                    if content.contains("spring") {
                        return Ok(Some(Framework::Spring));
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn calculate_size(path: &Path) -> Result<(usize, usize)> {
        let mut line_count = 0;
        let mut file_count = 0;

        for entry in WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip common directories
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.')
                    && name != "target"
                    && name != "node_modules"
                    && name != "venv"
                    && name != "__pycache__"
                    && name != "dist"
                    && name != "build"
            })
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy();
                    // Count only source files
                    if matches!(
                        ext_str.as_ref(),
                        "rs" | "py"
                            | "js"
                            | "jsx"
                            | "ts"
                            | "tsx"
                            | "go"
                            | "java"
                            | "c"
                            | "cpp"
                            | "h"
                            | "hpp"
                    ) {
                        file_count += 1;
                        if let Ok(content) = fs::read_to_string(entry.path()) {
                            line_count += content.lines().count();
                        }
                    }
                }
            }
        }

        Ok((line_count, file_count))
    }

    fn categorize_size(line_count: usize) -> ProjectSize {
        match line_count {
            0..=999 => ProjectSize::Small,
            1000..=9999 => ProjectSize::Medium,
            10000..=99999 => ProjectSize::Large,
            _ => ProjectSize::XLarge,
        }
    }

    fn analyze_structure(path: &Path) -> Result<ProjectStructure> {
        let mut has_tests = false;
        let mut has_docs = false;
        let mut has_ci = false;
        let mut main_dirs = Vec::new();
        let mut config_files = Vec::new();

        // Check for common directories
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            if entry.file_type()?.is_dir() {
                match name.as_str() {
                    "tests" | "test" | "__tests__" | "spec" => has_tests = true,
                    "docs" | "documentation" => has_docs = true,
                    ".github" | ".gitlab" => {
                        has_ci = path.join(&name).join("workflows").exists()
                            || path.join(&name).join(".gitlab-ci.yml").exists();
                    }
                    "src" | "lib" | "app" | "pkg" => main_dirs.push(name),
                    _ => {}
                }
            } else if entry.file_type()?.is_file() {
                match name.as_str() {
                    ".gitlab-ci.yml" | ".travis.yml" | "Jenkinsfile" => has_ci = true,
                    _ => {}
                }

                // Collect config files
                if name.ends_with(".toml")
                    || name.ends_with(".json")
                    || name.ends_with(".yml")
                    || name.ends_with(".yaml")
                {
                    config_files.push(name);
                }
            }
        }

        Ok(ProjectStructure {
            has_tests,
            has_docs,
            has_ci,
            main_dirs,
            config_files,
        })
    }

    fn analyze_health(path: &Path) -> Result<HealthIndicators> {
        Ok(HealthIndicators {
            has_readme: path.join("README.md").exists() || path.join("readme.md").exists(),
            has_license: path.join("LICENSE").exists() || path.join("LICENSE.txt").exists(),
            has_gitignore: path.join(".gitignore").exists(),
            uses_linter: Self::detect_linter(path)?,
            uses_formatter: Self::detect_formatter(path)?,
        })
    }

    fn detect_linter(path: &Path) -> Result<bool> {
        // Check for linter configs
        let linter_files = [
            ".eslintrc",
            ".eslintrc.json",
            ".eslintrc.js",
            ".pylintrc",
            "pyproject.toml",
            ".golangci.yml",
            "rustfmt.toml",
            ".rustfmt.toml",
        ];

        for file in linter_files {
            if path.join(file).exists() {
                return Ok(true);
            }
        }

        // Check package.json for linter deps
        if path.join("package.json").exists() {
            let content = fs::read_to_string(path.join("package.json"))?;
            if content.contains("eslint") || content.contains("tslint") {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn detect_formatter(path: &Path) -> Result<bool> {
        // Check for formatter configs
        let formatter_files = [
            ".prettierrc",
            ".prettierrc.json",
            "prettier.config.js",
            ".black",
            "pyproject.toml",
            ".rustfmt.toml",
            "rustfmt.toml",
        ];

        for file in formatter_files {
            if path.join(file).exists() {
                return Ok(true);
            }
        }

        // Check package.json for formatter deps
        if path.join("package.json").exists() {
            let content = fs::read_to_string(path.join("package.json"))?;
            if content.contains("prettier") || content.contains("black") {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn determine_focus_areas(
        structure: &ProjectStructure,
        health: &HealthIndicators,
    ) -> Vec<String> {
        let mut areas = Vec::new();

        if !structure.has_tests {
            areas.push("test coverage".to_string());
        }
        if !structure.has_docs {
            areas.push("documentation".to_string());
        }
        if !health.uses_linter {
            areas.push("code quality".to_string());
        }
        if !health.has_readme {
            areas.push("project documentation".to_string());
        }

        // Always include these as potential areas
        areas.push("error handling".to_string());

        areas
    }
}
