use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::analyzer::{Language, ProjectInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    sections: HashMap<String, String>,
    files: Vec<FileContext>,
    max_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub path: PathBuf,
    pub content: String,
    pub importance: f32,
}

impl Context {
    pub fn new() -> Self {
        Self {
            sections: HashMap::new(),
            files: Vec::new(),
            max_size: 100_000, // ~100KB context limit
        }
    }

    pub fn add_section(&mut self, name: impl Into<String>, content: impl Into<String>) {
        self.sections.insert(name.into(), content.into());
    }

    pub fn add_file(&mut self, file: FileContext) {
        self.files.push(file);
    }

    pub fn build_prompt(&self) -> String {
        let mut prompt = String::new();

        // Add sections
        for (name, content) in &self.sections {
            prompt.push_str(&format!("## {name}\n\n{content}\n\n"));
        }

        // Add files
        if !self.files.is_empty() {
            prompt.push_str("## Key Files\n\n");
            for file in &self.files {
                prompt.push_str(&format!(
                    "### {}\n\n```\n{}\n```\n\n",
                    file.path.display(),
                    file.content
                ));
            }
        }

        prompt
    }

    pub fn size(&self) -> usize {
        self.build_prompt().len()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ContextBuilder;

impl ContextBuilder {
    pub async fn build(project: &ProjectInfo, project_path: &Path) -> Result<Context> {
        let mut context = Context::new();

        // Add project summary
        context.add_section("Project Overview", Self::generate_project_summary(project));

        // Add focus areas
        if !project.focus_areas.is_empty() {
            context.add_section("Focus Areas", project.focus_areas.join(", "));
        }

        // Add improvement guidelines
        context.add_section(
            "Improvement Guidelines",
            Self::get_improvement_guidelines(project),
        );

        // Add key files
        let key_files = Self::select_key_files(project, project_path)?;
        for file_path in key_files {
            if let Ok(content) = fs::read_to_string(&file_path) {
                // Limit file size
                let content = if content.len() > 5000 {
                    format!("{}\n... (truncated)", &content[..5000])
                } else {
                    content
                };

                let relative_path = file_path.strip_prefix(project_path).unwrap_or(&file_path);
                context.add_file(FileContext {
                    path: relative_path.to_path_buf(),
                    content,
                    importance: Self::calculate_importance(&file_path, project),
                });
            }
        }

        // Sort files by importance and limit context size
        context
            .files
            .sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());

        // Trim files if context is too large
        while context.size() > context.max_size && !context.files.is_empty() {
            context.files.pop();
        }

        Ok(context)
    }

    fn generate_project_summary(project: &ProjectInfo) -> String {
        format!(
            r#"Language: {}
Framework: {}
Size: {} lines across {} files
Project Type: {:?}

Key Characteristics:
- Tests: {}
- Documentation: {}
- CI/CD: {}
- Linter: {}
- Formatter: {}

Main directories: {}
Configuration files: {}"#,
            project.language,
            project
                .framework
                .as_ref()
                .map(|f| format!("{f:?}"))
                .unwrap_or_else(|| "None".to_string()),
            project.line_count,
            project.file_count,
            project.size,
            if project.structure.has_tests {
                "✓"
            } else {
                "✗"
            },
            if project.structure.has_docs {
                "✓"
            } else {
                "✗"
            },
            if project.structure.has_ci {
                "✓"
            } else {
                "✗"
            },
            if project.health_indicators.uses_linter {
                "✓"
            } else {
                "✗"
            },
            if project.health_indicators.uses_formatter {
                "✓"
            } else {
                "✗"
            },
            project.structure.main_dirs.join(", "),
            project.structure.config_files.join(", ")
        )
    }

    fn get_improvement_guidelines(project: &ProjectInfo) -> &'static str {
        match project.language {
            Language::Rust => {
                r#"For Rust projects, focus on:
1. Proper error handling with Result types and context
2. Idiomatic use of ownership and borrowing
3. Comprehensive unit and integration tests
4. Clear module organization
5. Documentation with examples for public APIs
6. Use of appropriate derive macros
7. Efficient use of iterators over loops where applicable"#
            }
            Language::Python => {
                r#"For Python projects, focus on:
1. Type hints for all functions and classes
2. Proper exception handling with specific exceptions
3. Docstrings for all public functions and classes
4. Unit tests with good coverage
5. Following PEP 8 style guidelines
6. Using appropriate data structures
7. Avoiding global state"#
            }
            Language::JavaScript | Language::TypeScript => {
                r#"For JavaScript/TypeScript projects, focus on:
1. Proper async/await usage and error handling
2. TypeScript types for all functions and variables
3. Unit tests with Jest or similar
4. Consistent code style (ESLint/Prettier)
5. Avoiding callback hell
6. Proper module exports
7. Documentation with JSDoc comments"#
            }
            _ => {
                r#"General improvement guidelines:
1. Clear and consistent error handling
2. Comprehensive test coverage
3. Good documentation
4. Consistent code style
5. Proper separation of concerns
6. Avoiding code duplication
7. Performance optimizations where needed"#
            }
        }
    }

    fn select_key_files(project: &ProjectInfo, project_path: &Path) -> Result<Vec<PathBuf>> {
        let mut key_files = Vec::new();

        // Add main entry points
        match project.language {
            Language::Rust => {
                let main_rs = project_path.join("src/main.rs");
                let lib_rs = project_path.join("src/lib.rs");
                if main_rs.exists() {
                    key_files.push(main_rs);
                }
                if lib_rs.exists() {
                    key_files.push(lib_rs);
                }
            }
            Language::Python => {
                let main_py = project_path.join("main.py");
                let app_py = project_path.join("app.py");
                let init_py = project_path.join("__init__.py");
                for file in [main_py, app_py, init_py] {
                    if file.exists() {
                        key_files.push(file);
                        break;
                    }
                }
            }
            Language::JavaScript | Language::TypeScript => {
                let index_files = [
                    "index.js", "index.ts", "app.js", "app.ts", "main.js", "main.ts",
                ];
                for file_name in index_files {
                    let file = project_path.join(file_name);
                    if file.exists() {
                        key_files.push(file);
                        break;
                    }
                }
            }
            _ => {}
        }

        // Add configuration files
        key_files.push(project_path.join("Cargo.toml"));
        key_files.push(project_path.join("package.json"));
        key_files.push(project_path.join("pyproject.toml"));
        key_files.push(project_path.join("tsconfig.json"));

        // Add README if exists
        let readme_files = ["README.md", "readme.md", "README.rst", "README.txt"];
        for readme in readme_files {
            let file = project_path.join(readme);
            if file.exists() {
                key_files.push(file);
                break;
            }
        }

        // Find most recently modified source files
        let mut recent_files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for entry in WalkDir::new(project_path)
            .max_depth(4)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.')
                    && name != "target"
                    && name != "node_modules"
                    && name != "venv"
            })
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    let is_source = matches!(
                        (project.language.clone(), ext.to_str()),
                        (Language::Rust, Some("rs"))
                            | (Language::Python, Some("py"))
                            | (Language::JavaScript, Some("js"))
                            | (Language::JavaScript, Some("jsx"))
                            | (Language::TypeScript, Some("ts"))
                            | (Language::TypeScript, Some("tsx"))
                            | (Language::Go, Some("go"))
                            | (Language::Java, Some("java"))
                    );

                    if is_source {
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                recent_files.push((entry.path().to_path_buf(), modified));
                            }
                        }
                    }
                }
            }
        }

        // Sort by modification time and take top 5
        recent_files.sort_by(|a, b| b.1.cmp(&a.1));
        for (file, _) in recent_files.into_iter().take(5) {
            if !key_files.contains(&file) {
                key_files.push(file);
            }
        }

        // Filter out non-existent files
        Ok(key_files.into_iter().filter(|f| f.exists()).collect())
    }

    fn calculate_importance(file_path: &Path, project: &ProjectInfo) -> f32 {
        let mut importance: f32 = 0.5;

        // Entry points are most important
        if let Some(name) = file_path.file_name() {
            let name_str = name.to_string_lossy();
            if name_str == "main.rs"
                || name_str == "lib.rs"
                || name_str == "main.py"
                || name_str == "app.py"
                || name_str == "index.js"
                || name_str == "index.ts"
            {
                importance = 1.0;
            }
        }

        // Config files are important
        if let Some(ext) = file_path.extension() {
            if matches!(
                ext.to_str(),
                Some("toml") | Some("json") | Some("yaml") | Some("yml")
            ) {
                importance = 0.8;
            }
        }

        // Files in focus areas are more important
        let path_str = file_path.to_string_lossy().to_lowercase();
        for area in &project.focus_areas {
            if area.contains("test") && path_str.contains("test") {
                importance += 0.2;
            }
            if area.contains("error") && (path_str.contains("error") || path_str.contains("result"))
            {
                importance += 0.2;
            }
            if area.contains("doc") && path_str.contains("readme") {
                importance += 0.2;
            }
        }

        importance.min(1.0)
    }
}
