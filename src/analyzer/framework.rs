//! Framework detection for projects

use anyhow::Result;
use std::collections::HashMap;

use super::language::Language;
use super::structure::ProjectStructure;

/// Supported frameworks
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Framework {
    // Rust frameworks
    Actix,
    Axum,
    Rocket,
    Tauri,
    Yew,

    // JavaScript/TypeScript frameworks
    React,
    Vue,
    Angular,
    Next,
    Svelte,
    Express,
    Nest,

    // Python frameworks
    Django,
    Flask,
    FastAPI,
    Pytest,

    // Go frameworks
    Gin,
    Echo,
    Fiber,

    // Java frameworks
    Spring,
    SpringBoot,

    // C# frameworks
    AspNetCore,
    Blazor,

    // Ruby frameworks
    Rails,
    Sinatra,

    // Other
    Other(String),
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Framework::Actix => write!(f, "Actix Web"),
            Framework::Axum => write!(f, "Axum"),
            Framework::Rocket => write!(f, "Rocket"),
            Framework::Tauri => write!(f, "Tauri"),
            Framework::Yew => write!(f, "Yew"),
            Framework::React => write!(f, "React"),
            Framework::Vue => write!(f, "Vue"),
            Framework::Angular => write!(f, "Angular"),
            Framework::Next => write!(f, "Next.js"),
            Framework::Svelte => write!(f, "Svelte"),
            Framework::Express => write!(f, "Express"),
            Framework::Nest => write!(f, "NestJS"),
            Framework::Django => write!(f, "Django"),
            Framework::Flask => write!(f, "Flask"),
            Framework::FastAPI => write!(f, "FastAPI"),
            Framework::Pytest => write!(f, "Pytest"),
            Framework::Gin => write!(f, "Gin"),
            Framework::Echo => write!(f, "Echo"),
            Framework::Fiber => write!(f, "Fiber"),
            Framework::Spring => write!(f, "Spring"),
            Framework::SpringBoot => write!(f, "Spring Boot"),
            Framework::AspNetCore => write!(f, "ASP.NET Core"),
            Framework::Blazor => write!(f, "Blazor"),
            Framework::Rails => write!(f, "Ruby on Rails"),
            Framework::Sinatra => write!(f, "Sinatra"),
            Framework::Other(name) => write!(f, "{name}"),
        }
    }
}

/// Framework detector
pub struct FrameworkDetector {
    config_patterns: HashMap<&'static str, Framework>,
    dependency_patterns: HashMap<&'static str, Framework>,
    #[allow(dead_code)]
    file_patterns: HashMap<&'static str, Framework>,
}

impl FrameworkDetector {
    pub fn new() -> Self {
        let mut config_patterns = HashMap::new();
        // Config file indicators
        config_patterns.insert("next.config.js", Framework::Next);
        config_patterns.insert("next.config.ts", Framework::Next);
        config_patterns.insert("vue.config.js", Framework::Vue);
        config_patterns.insert(".angular.json", Framework::Angular);
        config_patterns.insert("angular.json", Framework::Angular);
        config_patterns.insert("svelte.config.js", Framework::Svelte);
        config_patterns.insert("nest-cli.json", Framework::Nest);
        config_patterns.insert("django.settings.py", Framework::Django);

        let mut dependency_patterns = HashMap::new();
        // Package/dependency indicators
        dependency_patterns.insert("actix-web", Framework::Actix);
        dependency_patterns.insert("axum", Framework::Axum);
        dependency_patterns.insert("rocket", Framework::Rocket);
        dependency_patterns.insert("tauri", Framework::Tauri);
        dependency_patterns.insert("yew", Framework::Yew);
        dependency_patterns.insert("react", Framework::React);
        dependency_patterns.insert("vue", Framework::Vue);
        dependency_patterns.insert("@angular/core", Framework::Angular);
        dependency_patterns.insert("next", Framework::Next);
        dependency_patterns.insert("svelte", Framework::Svelte);
        dependency_patterns.insert("express", Framework::Express);
        dependency_patterns.insert("@nestjs/core", Framework::Nest);
        dependency_patterns.insert("django", Framework::Django);
        dependency_patterns.insert("flask", Framework::Flask);
        dependency_patterns.insert("fastapi", Framework::FastAPI);
        dependency_patterns.insert("pytest", Framework::Pytest);
        dependency_patterns.insert("gin-gonic/gin", Framework::Gin);
        dependency_patterns.insert("labstack/echo", Framework::Echo);
        dependency_patterns.insert("gofiber/fiber", Framework::Fiber);
        dependency_patterns.insert("spring-boot", Framework::SpringBoot);
        dependency_patterns.insert("rails", Framework::Rails);

        let mut file_patterns = HashMap::new();
        // File structure indicators
        file_patterns.insert("app/controllers", Framework::Rails);
        file_patterns.insert("app/models", Framework::Rails);
        file_patterns.insert("app/views", Framework::Rails);
        file_patterns.insert("src/Controller", Framework::Spring);
        file_patterns.insert("src/main/java", Framework::Spring);

        Self {
            config_patterns,
            dependency_patterns,
            file_patterns,
        }
    }

    pub fn detect(
        &self,
        structure: &ProjectStructure,
        language: &Language,
    ) -> Result<Option<Framework>> {
        // Check config files first
        if let Some(framework) = self.detect_from_config_files(structure) {
            return Ok(Some(framework));
        }

        // Check dependency files based on language
        if let Some(framework) = self.detect_from_dependencies(structure, language) {
            return Ok(Some(framework));
        }

        // Check file structure patterns
        if let Some(framework) = self.detect_from_file_structure(structure) {
            return Ok(Some(framework));
        }

        Ok(None)
    }

    fn detect_from_config_files(&self, structure: &ProjectStructure) -> Option<Framework> {
        for config_file in &structure.config_files {
            let file_name = config_file.path.file_name()?.to_str()?;

            if let Some(framework) = self.config_patterns.get(file_name) {
                return Some(framework.clone());
            }
        }

        None
    }

    fn detect_from_dependencies(
        &self,
        structure: &ProjectStructure,
        language: &Language,
    ) -> Option<Framework> {
        match language {
            Language::Rust => self.detect_rust_framework(structure),
            Language::JavaScript | Language::TypeScript => self.detect_js_framework(structure),
            Language::Python => self.detect_python_framework(structure),
            Language::Go => self.detect_go_framework(structure),
            Language::Java | Language::Kotlin => self.detect_java_framework(structure),
            Language::Ruby => self.detect_ruby_framework(structure),
            _ => None,
        }
    }

    fn detect_rust_framework(&self, structure: &ProjectStructure) -> Option<Framework> {
        // Look for Cargo.toml
        for config_file in &structure.config_files {
            if config_file.path.file_name()?.to_str()? == "Cargo.toml" {
                if let Ok(content) = std::fs::read_to_string(&config_file.path) {
                    for (dep, framework) in &self.dependency_patterns {
                        if content.contains(dep)
                            && matches!(
                                framework,
                                Framework::Actix
                                    | Framework::Axum
                                    | Framework::Rocket
                                    | Framework::Tauri
                                    | Framework::Yew
                            )
                        {
                            return Some(framework.clone());
                        }
                    }
                }
            }
        }
        None
    }

    fn detect_js_framework(&self, structure: &ProjectStructure) -> Option<Framework> {
        // Look for package.json
        for config_file in &structure.config_files {
            if config_file.path.file_name()?.to_str()? == "package.json" {
                if let Ok(content) = std::fs::read_to_string(&config_file.path) {
                    for (dep, framework) in &self.dependency_patterns {
                        if content.contains(&format!("\"{dep}\""))
                            && matches!(
                                framework,
                                Framework::React
                                    | Framework::Vue
                                    | Framework::Angular
                                    | Framework::Next
                                    | Framework::Svelte
                                    | Framework::Express
                                    | Framework::Nest
                            )
                        {
                            return Some(framework.clone());
                        }
                    }
                }
            }
        }
        None
    }

    fn detect_python_framework(&self, structure: &ProjectStructure) -> Option<Framework> {
        // Look for requirements.txt or pyproject.toml
        for config_file in &structure.config_files {
            let file_name = config_file.path.file_name()?.to_str()?;
            if file_name == "requirements.txt" || file_name == "pyproject.toml" {
                if let Ok(content) = std::fs::read_to_string(&config_file.path) {
                    for (dep, framework) in &self.dependency_patterns {
                        if content.to_lowercase().contains(dep)
                            && matches!(
                                framework,
                                Framework::Django
                                    | Framework::Flask
                                    | Framework::FastAPI
                                    | Framework::Pytest
                            )
                        {
                            return Some(framework.clone());
                        }
                    }
                }
            }
        }
        None
    }

    fn detect_go_framework(&self, structure: &ProjectStructure) -> Option<Framework> {
        // Look for go.mod
        for config_file in &structure.config_files {
            if config_file.path.file_name()?.to_str()? == "go.mod" {
                if let Ok(content) = std::fs::read_to_string(&config_file.path) {
                    for (dep, framework) in &self.dependency_patterns {
                        if content.contains(dep)
                            && matches!(
                                framework,
                                Framework::Gin | Framework::Echo | Framework::Fiber
                            )
                        {
                            return Some(framework.clone());
                        }
                    }
                }
            }
        }
        None
    }

    fn detect_java_framework(&self, structure: &ProjectStructure) -> Option<Framework> {
        // Look for pom.xml or build.gradle
        for config_file in &structure.config_files {
            let file_name = config_file.path.file_name()?.to_str()?;
            if file_name == "pom.xml" || file_name.contains("build.gradle") {
                if let Ok(content) = std::fs::read_to_string(&config_file.path) {
                    if content.contains("spring-boot") {
                        return Some(Framework::SpringBoot);
                    } else if content.contains("spring") {
                        return Some(Framework::Spring);
                    }
                }
            }
        }
        None
    }

    fn detect_ruby_framework(&self, structure: &ProjectStructure) -> Option<Framework> {
        // Look for Gemfile
        for config_file in &structure.config_files {
            if config_file.path.file_name()?.to_str()? == "Gemfile" {
                if let Ok(content) = std::fs::read_to_string(&config_file.path) {
                    if content.contains("rails") {
                        return Some(Framework::Rails);
                    } else if content.contains("sinatra") {
                        return Some(Framework::Sinatra);
                    }
                }
            }
        }
        None
    }

    fn detect_from_file_structure(&self, _structure: &ProjectStructure) -> Option<Framework> {
        // TODO: Implement file structure pattern detection
        None
    }
}

impl Default for FrameworkDetector {
    fn default() -> Self {
        Self::new()
    }
}
