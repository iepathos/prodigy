//! Language detection for projects

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use super::structure::ProjectStructure;

/// Supported programming languages
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Ruby,
    Swift,
    Kotlin,
    Other(String),
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
            Language::CSharp => write!(f, "C#"),
            Language::Ruby => write!(f, "Ruby"),
            Language::Swift => write!(f, "Swift"),
            Language::Kotlin => write!(f, "Kotlin"),
            Language::Other(name) => write!(f, "{}", name),
        }
    }
}

/// Language detector
pub struct LanguageDetector {
    build_file_patterns: HashMap<&'static str, Language>,
    extension_patterns: HashMap<&'static str, Language>,
}

impl LanguageDetector {
    pub fn new() -> Self {
        let mut build_file_patterns = HashMap::new();
        build_file_patterns.insert("Cargo.toml", Language::Rust);
        build_file_patterns.insert("package.json", Language::JavaScript);
        build_file_patterns.insert("tsconfig.json", Language::TypeScript);
        build_file_patterns.insert("requirements.txt", Language::Python);
        build_file_patterns.insert("setup.py", Language::Python);
        build_file_patterns.insert("pyproject.toml", Language::Python);
        build_file_patterns.insert("go.mod", Language::Go);
        build_file_patterns.insert("pom.xml", Language::Java);
        build_file_patterns.insert("build.gradle", Language::Java);
        build_file_patterns.insert(".csproj", Language::CSharp);
        build_file_patterns.insert("Gemfile", Language::Ruby);
        build_file_patterns.insert("Package.swift", Language::Swift);
        build_file_patterns.insert("build.gradle.kts", Language::Kotlin);

        let mut extension_patterns = HashMap::new();
        extension_patterns.insert("rs", Language::Rust);
        extension_patterns.insert("py", Language::Python);
        extension_patterns.insert("js", Language::JavaScript);
        extension_patterns.insert("jsx", Language::JavaScript);
        extension_patterns.insert("ts", Language::TypeScript);
        extension_patterns.insert("tsx", Language::TypeScript);
        extension_patterns.insert("go", Language::Go);
        extension_patterns.insert("java", Language::Java);
        extension_patterns.insert("cs", Language::CSharp);
        extension_patterns.insert("rb", Language::Ruby);
        extension_patterns.insert("swift", Language::Swift);
        extension_patterns.insert("kt", Language::Kotlin);
        extension_patterns.insert("kts", Language::Kotlin);

        Self {
            build_file_patterns,
            extension_patterns,
        }
    }

    pub fn detect(&self, structure: &ProjectStructure) -> Result<Language> {
        // Priority 1: Check build files
        if let Some(language) = self.detect_from_build_files(structure) {
            return Ok(language);
        }

        // Priority 2: Check file extensions frequency
        if let Some(language) = self.detect_from_extensions(structure) {
            return Ok(language);
        }

        // Priority 3: Check shebang lines (not implemented yet)

        // Priority 4: Content patterns (not implemented yet)

        // Default to Other
        Ok(Language::Other("unknown".to_string()))
    }

    fn detect_from_build_files(&self, structure: &ProjectStructure) -> Option<Language> {
        for config_file in &structure.config_files {
            let file_name = config_file.path.file_name()?.to_str()?;

            // Check exact matches
            if let Some(language) = self.build_file_patterns.get(file_name) {
                return Some(language.clone());
            }

            // Check patterns
            if file_name.ends_with(".csproj") {
                return Some(Language::CSharp);
            }
        }

        None
    }

    fn detect_from_extensions(&self, structure: &ProjectStructure) -> Option<Language> {
        let mut extension_counts: HashMap<String, usize> = HashMap::new();

        // Count extensions in source directories
        for src_dir in &structure.src_dirs {
            self.count_extensions_in_dir(src_dir, &mut extension_counts);
        }

        // Find the most common extension
        let (most_common_ext, _) = extension_counts.iter().max_by_key(|(_, count)| *count)?;

        self.extension_patterns.get(most_common_ext.as_str()).cloned()
    }

    fn count_extensions_in_dir(&self, dir: &Path, counts: &mut HashMap<String, usize>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if let Some(ext_str) = ext.to_str() {
                            if self.extension_patterns.contains_key(ext_str) {
                                *counts.entry(ext_str.to_string()).or_insert(0) += 1;
                            }
                        }
                    }
                } else if path.is_dir() {
                    // Recursively count in subdirectories
                    self.count_extensions_in_dir(&path, counts);
                }
            }
        }
    }
}

impl Default for LanguageDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::structure::ConfigFile;
    use std::path::PathBuf;

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Rust.to_string(), "Rust");
        assert_eq!(Language::CSharp.to_string(), "C#");
        assert_eq!(
            Language::Other("Haskell".to_string()).to_string(),
            "Haskell"
        );
    }

    #[test]
    fn test_detect_from_cargo_toml() {
        let detector = LanguageDetector::new();
        let structure = ProjectStructure {
            root: PathBuf::from("/test"),
            src_dirs: vec![],
            test_dirs: vec![],
            config_files: vec![ConfigFile {
                path: PathBuf::from("/test/Cargo.toml"),
                file_type: crate::analyzer::structure::ConfigFileType::Build,
            }],
            entry_points: vec![],
            important_files: vec![],
            ignored_patterns: vec![],
        };

        let language = detector.detect(&structure).unwrap();
        assert_eq!(language, Language::Rust);
    }

    #[test]
    fn test_detect_from_package_json() {
        let detector = LanguageDetector::new();
        let structure = ProjectStructure {
            root: PathBuf::from("/test"),
            src_dirs: vec![],
            test_dirs: vec![],
            config_files: vec![ConfigFile {
                path: PathBuf::from("/test/package.json"),
                file_type: crate::analyzer::structure::ConfigFileType::Build,
            }],
            entry_points: vec![],
            important_files: vec![],
            ignored_patterns: vec![],
        };

        let language = detector.detect(&structure).unwrap();
        assert_eq!(language, Language::JavaScript);
    }
}
